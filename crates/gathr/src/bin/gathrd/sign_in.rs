//! Sign in and out from Settings > Engines, one account at a time. Sign in runs `claude auth login`
//! in the account's folder with no terminal: Claude Code prints its sign-in page, opens the browser
//! itself, and finishes on the browser's callback or a `code#state` pasted on its stdin (task 893).
//! Exit 0 is signed in. Sign out runs `claude auth logout`, which removes the folder's login.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use gathr::claude::engine::{Closed, Engine, Line};
use gathr::claude::{command, login};
use gathr::event::Event;
use gathr::ids;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::accounts::find;
use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, Fail, fail};
use crate::turns::CLOSE_GRACE;

/// How long Claude Code has to print its sign-in page (it took 0.2 s in task 893).
const PAGE_WAIT: Duration = Duration::from_secs(30);
/// `claude auth logout` took 0.4 s on 2.1.318 (task 947).
const SIGN_OUT_WAIT: Duration = Duration::from_secs(30);

/// Each account's sign-in, made when first asked for.
#[derive(Default)]
pub struct SignIns(Mutex<HashMap<String, Arc<SignIn>>>);

impl SignIns {
    pub fn get(&self, account: &str) -> Arc<SignIn> {
        let mut all = self.0.lock().expect("never poisoned");
        let made = || {
            Arc::new(SignIn {
                account: account.to_string(),
                ..SignIn::default()
            })
        };
        all.entry(account.to_string()).or_insert_with(made).clone()
    }

    /// Ends every sign-in still waiting, at shutdown.
    pub async fn cancel_all(&self) {
        let all: Vec<_> = self
            .0
            .lock()
            .expect("never poisoned")
            .values()
            .cloned()
            .collect();
        for sign_in in all {
            sign_in.cancel().await;
        }
    }
}

enum Ask {
    Code(String),
    Cancel(oneshot::Sender<()>),
}

#[derive(Default)]
pub struct SignIn {
    account: String,
    /// What Settings shows: null, `waiting` (with the page once there is one), or `failed` with
    /// why and a reference.
    shown: Mutex<Value>,
    running: Mutex<Running>,
    /// One start at a time, so a double click never leaves a process nobody can cancel.
    starting: tokio::sync::Mutex<()>,
}

#[derive(Default)]
struct Running {
    /// Counts Cancels, so a start queued behind another sees one that came while it waited.
    cancels: u64,
    asks: Option<mpsc::UnboundedSender<Ask>>,
}

impl SignIn {
    pub fn shown(&self) -> Value {
        self.shown.lock().expect("never poisoned").clone()
    }

    fn show(&self, chats: &Chats, value: Value) {
        *self.shown.lock().expect("never poisoned") = value;
        chats.publish(json!({ "type": "accounts" }));
    }

    /// Ends a sign-in still waiting, and any start queued. Killing it there leaves no login behind
    /// (task 893).
    pub async fn cancel(&self) {
        let running = {
            let mut running = self.running.lock().expect("never poisoned");
            running.cancels += 1;
            running.asks.take()
        };
        stop(running).await;
    }

    /// Takes `mine` out of `running`, unless a newer sign-in has replaced it.
    fn release(&self, mine: &mpsc::UnboundedSender<Ask>) -> bool {
        let mut running = self.running.lock().expect("never poisoned");
        let ours = running
            .asks
            .as_ref()
            .is_some_and(|tx| tx.same_channel(mine));
        if ours {
            running.asks.take();
        }
        ours
    }
}

async fn stop(running: Option<mpsc::UnboundedSender<Ask>>) {
    let (done, ended) = oneshot::channel();
    if running.is_some_and(|tx| tx.send(Ask::Cancel(done)).is_ok()) {
        let _ = ended.await;
    }
}

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(
            "/api/accounts/{account}/sign-in",
            post(start).delete(cancel),
        )
        .route("/api/accounts/{account}/sign-in/code", post(code))
        .route("/api/accounts/{account}/sign-out", post(sign_out))
}

fn failed(status: StatusCode, why: &str, trace: &str) -> Fail {
    fail(status, "Sign in failed", why, trace)
}

/// A newer start replaces one still waiting. Registered before the process starts, so Cancel
/// reaches it while Claude Code is still getting its page.
async fn start(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(account): Path<String>,
) -> Answer {
    let account = find(&s, &t, &account, "Sign in failed").await?;
    let sign_in = s.sign_ins.get(&account.id);
    let cancels = sign_in.running.lock().expect("never poisoned").cancels;
    let _one = sign_in.starting.lock().await;
    let fail = |why: String| failed(StatusCode::INTERNAL_SERVER_ERROR, &why, &t.0);
    let program = command::find(s.chats.copy()).map_err(fail)?;
    let folder = account.folder(&s.data);
    std::fs::create_dir_all(&folder).map_err(|e| fail(e.to_string()))?;

    let (mine, mut asks) = mpsc::unbounded_channel();
    let previous = {
        let mut running = sign_in.running.lock().expect("never poisoned");
        if running.cancels != cancels {
            return Ok(Json(json!({ "cancelled": true })));
        }
        running.asks.replace(mine.clone())
    };
    stop(previous).await;
    sign_in.show(&s.chats, json!({ "state": "waiting" }));

    let env = command::env(std::env::vars_os(), &folder);
    let args = ["auth".to_string(), "login".to_string()];
    let started = Instant::now();
    let (engine, mut lines) = match Engine::start(&program, &args, env, &folder) {
        Ok(engine) => engine,
        Err(e) => {
            sign_in.release(&mine);
            sign_in.show(&s.chats, Value::Null);
            return Err(fail(e.to_string()));
        }
    };
    let mut said = Vec::new();
    let waited = tokio::time::timeout(PAGE_WAIT, async {
        tokio::select! {
            url = page(&mut lines, &mut said) => Ok(url),
            done = cancelled(&mut asks) => Err(done),
        }
    })
    .await;
    let url = match waited {
        Ok(Ok(Some(url))) => url,
        Ok(Err(done)) => {
            end_cancelled(&s, &sign_in, engine, &t.0, done).await;
            return Ok(Json(json!({ "cancelled": true })));
        }
        Ok(Ok(None)) | Err(_) => {
            let code = match engine.close(Duration::ZERO).await {
                Closed::Exited(code) => format!("exit {code:?}"),
                Closed::Killed => "no sign-in page within 30 s".into(),
            };
            let why = format!(
                "Claude Code gave no sign-in page ({code}): {}",
                said.join(" ")
            );
            record_as(&s, "sign_in", &account.id, &t.0, Some(&why), started).await;
            if sign_in.release(&mine) {
                sign_in.show(&s.chats, Value::Null);
            }
            return Err(fail(why));
        }
    };
    sign_in.show(&s.chats, json!({ "state": "waiting", "url": url }));
    let job = Job {
        trace: t.0.clone(),
        started,
        mine,
        sign_in: sign_in.clone(),
    };
    tokio::spawn(watch(s.clone(), engine, lines, asks, job));
    Ok(Json(json!({ "url": url })))
}

/// Waits for a Cancel; a code before the page has nowhere to go yet.
async fn cancelled(asks: &mut mpsc::UnboundedReceiver<Ask>) -> Option<oneshot::Sender<()>> {
    while let Some(ask) = asks.recv().await {
        if let Ask::Cancel(done) = ask {
            return Some(done);
        }
    }
    None
}

async fn end_cancelled(
    s: &State,
    sign_in: &SignIn,
    engine: Engine,
    trace: &str,
    done: Option<oneshot::Sender<()>>,
) {
    engine.close(Duration::ZERO).await;
    let ended = Event::point("gathr.engine.sign_in").trace(trace);
    let ended = ended.attr("account", sign_in.account.clone());
    s.writer.record(ended.attr("ended", "cancelled"));
    sign_in.show(&s.chats, Value::Null);
    if let Some(done) = done {
        let _ = done.send(());
    }
}

/// The page Claude Code prints for when its browser didn't open.
async fn page(lines: &mut mpsc::UnboundedReceiver<Line>, said: &mut Vec<String>) -> Option<String> {
    while let Some(line) = lines.recv().await {
        let (Line::Out(text) | Line::Err(text)) = line;
        if let Some((_, url)) = text.split_once("visit: ") {
            return Some(url.trim().to_string());
        }
        said.push(text);
    }
    None
}

struct Job {
    trace: String,
    started: Instant,
    /// This sign-in's own sender: it keeps `asks` open, and tells it from a newer sign-in.
    mine: mpsc::UnboundedSender<Ask>,
    sign_in: Arc<SignIn>,
}

/// Passes a pasted code on, and reports how the sign-in ended.
async fn watch(
    s: Arc<State>,
    mut engine: Engine,
    mut lines: mpsc::UnboundedReceiver<Line>,
    mut asks: mpsc::UnboundedReceiver<Ask>,
    job: Job,
) {
    let Job {
        trace,
        started,
        mine,
        sign_in,
    } = job;
    let mut last_error = None;
    loop {
        tokio::select! {
            line = lines.recv() => match line {
                Some(Line::Err(text)) if !text.trim().is_empty() => {
                    // Settings already says "Sign in failed".
                    let why = text.trim().trim_start_matches("Login failed:").trim_start();
                    last_error = Some(why.to_string());
                }
                Some(_) => {}
                None => break,
            },
            Some(ask) = asks.recv() => match ask {
                Ask::Code(code) => {
                    if let Err(e) = engine.write_line(&code).await {
                        last_error = Some(format!("the code didn't reach Claude Code: {e}"));
                    }
                }
                Ask::Cancel(done) => return end_cancelled(&s, &sign_in, engine, &trace, Some(done)).await,
            },
        }
    }
    let ours = sign_in.release(&mine);
    // Its output has ended, so it's exiting, but it may still be saving the login: never killed
    // for that (0001), only at shutdown after the grace every engine gets.
    let code = tokio::select! {
        code = engine.wait() => code,
        () = s.stopped() => match engine.close(CLOSE_GRACE).await {
            Closed::Exited(code) => code,
            Closed::Killed => None,
        },
    };
    let why = (code != Some(0))
        .then(|| last_error.unwrap_or_else(|| format!("Claude Code exited ({code:?})")));
    record_as(
        &s,
        "sign_in",
        &sign_in.account,
        &trace,
        why.as_deref(),
        started,
    )
    .await;
    if !ours {
        return;
    }
    let shown = match why {
        Some(why) => json!({ "state": "failed", "why": why, "ref": ids::reference(&trace) }),
        None => Value::Null,
    };
    sign_in.show(&s.chats, shown);
}

/// Recorded before the answer goes out, so its reference traces at once (0019).
pub async fn record_as(
    s: &State,
    what: &str,
    account: &str,
    trace: &str,
    why: Option<&str>,
    started: Instant,
) {
    let event = Event::point(&format!("gathr.engine.{what}")).trace(trace);
    let mut event = event.attr("account", account);
    event = match why {
        Some(why) => event.error(what).attr("why", why),
        None => event.ok(),
    };
    event.duration_us = Some(started.elapsed().as_micros() as i64);
    s.writer.record_now(event).await;
}

#[derive(Deserialize)]
struct Code {
    code: String,
}

async fn code(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(account): Path<String>,
    Json(body): Json<Code>,
) -> Answer {
    let account = find(&s, &t, &account, "Sign in failed").await?;
    let code = body.code.trim().to_string();
    let running = s
        .sign_ins
        .get(&account.id)
        .running
        .lock()
        .expect("never poisoned")
        .asks
        .clone();
    match running {
        Some(tx) if tx.send(Ask::Code(code)).is_ok() => Ok(Json(json!({ "sent": true }))),
        _ => Err(failed(StatusCode::CONFLICT, "no sign-in is waiting", &t.0)),
    }
}

async fn cancel(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(account): Path<String>,
) -> Answer {
    let account = find(&s, &t, &account, "Cancel failed").await?;
    s.sign_ins.get(&account.id).cancel().await;
    Ok(Json(json!({ "cancelled": true })))
}

/// Ends a sign-in still waiting, then runs `claude auth logout` in the account's folder. Engines
/// on the account keep running; their next turn finds it signed out and waits (0002).
async fn sign_out(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(account): Path<String>,
) -> Answer {
    let account = find(&s, &t, &account, "Sign out failed").await?;
    s.sign_ins.get(&account.id).cancel().await;
    let started = Instant::now();
    let done = log_out(&s, &account.folder(&s.data)).await;
    let why = done.as_ref().err().map(String::as_str);
    record_as(&s, "sign_out", &account.id, &t.0, why, started).await;
    s.chats.publish(json!({ "type": "accounts" }));
    match done {
        Ok(()) => Ok(Json(json!({ "signed_out": true }))),
        Err(why) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Sign out failed",
            &why,
            &t.0,
        )),
    }
}

const STOPPING: &str = "service stopped during sign out";

/// The account folder's renewal gate, or why it wasn't passed. Signing out never runs while an
/// engine on it renews the login: that renewal would save over the sign-out.
pub async fn pass_gate(s: &State, folder: &std::path::Path) -> Result<login::Hold, String> {
    let gate = s.chats.gate(folder);
    tokio::select! {
        gate = gate.pass() => Ok(gate),
        () = s.stopped() => Err(STOPPING.into()),
    }
}

async fn log_out(s: &State, folder: &std::path::Path) -> Result<(), String> {
    let program = command::find(s.chats.copy())?;
    let _gate = pass_gate(s, folder).await?;
    log_out_held(s, &program, folder).await
}

/// `claude auth logout` in the folder, with its renewal gate already held.
pub async fn log_out_held(
    s: &State,
    program: &std::path::Path,
    folder: &std::path::Path,
) -> Result<(), String> {
    std::fs::create_dir_all(folder)
        .map_err(|e| format!("{} wasn't made: {e}", folder.display()))?;
    let env = command::env(std::env::vars_os(), folder);
    let args = ["auth".to_string(), "logout".to_string()];
    let (mut engine, mut lines) = Engine::start(program, &args, env, folder)
        .map_err(|e| format!("Claude Code didn't start from {}: {e}", program.display()))?;
    // `None` when shutdown came first: it then gets the grace every engine gets to finish.
    let waited = tokio::select! {
        exited = tokio::time::timeout(SIGN_OUT_WAIT, engine.wait()) => Some(exited),
        () = s.stopped() => None,
    };
    let closed = match waited {
        Some(Ok(code)) => Closed::Exited(code),
        Some(Err(_)) => engine.close(Duration::ZERO).await,
        None => match engine.close(CLOSE_GRACE).await {
            Closed::Killed => return Err(STOPPING.into()),
            exited => exited,
        },
    };
    let mut said = Vec::new();
    let next = Duration::from_secs(2);
    while let Ok(Some(Line::Out(text) | Line::Err(text))) =
        tokio::time::timeout(next, lines.recv()).await
    {
        said.push(text);
    }
    match closed {
        Closed::Exited(Some(0)) => Ok(()),
        Closed::Exited(code) => Err(format!("Claude Code exited ({code:?}): {}", said.join(" "))),
        Closed::Killed => Err(format!(
            "Claude Code didn't finish signing out in {} s",
            SIGN_OUT_WAIT.as_secs()
        )),
    }
}
