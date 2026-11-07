//! Sign in from Settings > Engines: the service runs `claude auth login` in the account's folder
//! with no terminal. Claude Code prints its sign-in page, opens the browser itself, and finishes
//! on the browser's callback or a `code#state` pasted on its stdin (task 893). Exit 0 is signed in.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use gathr::claude::command;
use gathr::claude::engine::{Closed, Engine, Line};
use gathr::event::Event;
use gathr::ids;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, Fail, fail};

/// How long Claude Code has to print its sign-in page (it took 0.2 s in task 893).
const PAGE_WAIT: Duration = Duration::from_secs(30);

enum Ask {
    Code(String),
    Cancel(oneshot::Sender<()>),
}

#[derive(Default)]
pub struct SignIn {
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
        chats.publish(json!({ "type": "engine" }));
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
        .route("/api/engines/claude/sign-in", post(start).delete(cancel))
        .route("/api/engines/claude/sign-in/code", post(code))
}

fn failed(status: StatusCode, why: &str, trace: &str) -> Fail {
    fail(status, "Sign in failed", why, trace)
}

/// A newer start replaces one still waiting. Registered before the process starts, so Cancel
/// reaches it while Claude Code is still getting its page.
async fn start(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let cancels = s.sign_in.running.lock().expect("never poisoned").cancels;
    let _one = s.sign_in.starting.lock().await;
    let fail = |why: String| failed(StatusCode::INTERNAL_SERVER_ERROR, &why, &t.0);
    let program = command::find(s.chats.path_setting().await.as_deref()).map_err(fail)?;
    let folder = s.chats.account_folder();
    std::fs::create_dir_all(&folder).map_err(|e| fail(e.to_string()))?;

    let (mine, mut asks) = mpsc::unbounded_channel();
    let previous = {
        let mut running = s.sign_in.running.lock().expect("never poisoned");
        if running.cancels != cancels {
            return Ok(Json(json!({ "cancelled": true })));
        }
        running.asks.replace(mine.clone())
    };
    stop(previous).await;
    s.sign_in.show(&s.chats, json!({ "state": "waiting" }));

    let env = command::env(std::env::vars_os(), &folder);
    let args = ["auth".to_string(), "login".to_string()];
    let started = Instant::now();
    let (engine, mut lines) = match Engine::start(&program, &args, env, &folder) {
        Ok(engine) => engine,
        Err(e) => {
            s.sign_in.release(&mine);
            s.sign_in.show(&s.chats, Value::Null);
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
            end_cancelled(&s, engine, &t.0, done).await;
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
            record(&s, &t.0, Some(&why), started).await;
            if s.sign_in.release(&mine) {
                s.sign_in.show(&s.chats, Value::Null);
            }
            return Err(fail(why));
        }
    };
    s.sign_in
        .show(&s.chats, json!({ "state": "waiting", "url": url }));
    let job = Job {
        trace: t.0.clone(),
        started,
        mine,
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

async fn end_cancelled(s: &State, engine: Engine, trace: &str, done: Option<oneshot::Sender<()>>) {
    engine.close(Duration::ZERO).await;
    let ended = Event::point("gathr.engine.sign_in").trace(trace);
    s.writer.record(ended.attr("ended", "cancelled"));
    s.sign_in.show(&s.chats, Value::Null);
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
                Ask::Cancel(done) => return end_cancelled(&s, engine, &trace, Some(done)).await,
            },
        }
    }
    let ours = s.sign_in.release(&mine);
    // Its output has ended, so it's exiting: the grace is for it to finish saving the login.
    let code = match engine.close(Duration::from_secs(10)).await {
        Closed::Exited(code) => code,
        Closed::Killed => None,
    };
    let why = (code != Some(0))
        .then(|| last_error.unwrap_or_else(|| format!("Claude Code exited ({code:?})")));
    record(&s, &trace, why.as_deref(), started).await;
    if !ours {
        return;
    }
    let shown = match why {
        Some(why) => json!({ "state": "failed", "why": why, "ref": ids::reference(&trace) }),
        None => Value::Null,
    };
    s.sign_in.show(&s.chats, shown);
}

/// Recorded before the answer goes out, so its reference traces at once (0019).
async fn record(s: &State, trace: &str, why: Option<&str>, started: Instant) {
    let mut event = Event::point("gathr.engine.sign_in").trace(trace);
    event = match why {
        Some(why) => event.error("sign_in").attr("why", why),
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
    Json(body): Json<Code>,
) -> Answer {
    let code = body.code.trim().to_string();
    let running = s
        .sign_in
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

async fn cancel(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    s.sign_in.cancel().await;
    Json(json!({ "cancelled": true }))
}
