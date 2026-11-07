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
    /// What Settings shows: null, `waiting` with the page, or `failed` with why and a reference.
    shown: Mutex<Value>,
    running: Mutex<Option<mpsc::UnboundedSender<Ask>>>,
}

impl SignIn {
    pub fn shown(&self) -> Value {
        self.shown.lock().expect("never poisoned").clone()
    }

    fn show(&self, chats: &Chats, value: Value) {
        *self.shown.lock().expect("never poisoned") = value;
        chats.publish(json!({ "type": "engine" }));
    }

    /// Ends a sign-in still waiting. Killing it there leaves no login behind (task 893).
    pub async fn cancel(&self) {
        let running = self.running.lock().expect("never poisoned").take();
        let (done, ended) = oneshot::channel();
        if running.is_some_and(|tx| tx.send(Ask::Cancel(done)).is_ok()) {
            let _ = ended.await;
        }
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

async fn start(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    s.sign_in.cancel().await;
    let fail = |why: String| failed(StatusCode::INTERNAL_SERVER_ERROR, &why, &t.0);
    let program = command::find(s.chats.path_setting().await.as_deref()).map_err(fail)?;
    let folder = s.chats.account_folder();
    std::fs::create_dir_all(&folder).map_err(|e| fail(e.to_string()))?;
    let env = command::env(std::env::vars_os(), &folder);
    let args = ["auth".to_string(), "login".to_string()];
    let (engine, mut lines) =
        Engine::start(&program, &args, env, &folder).map_err(|e| fail(e.to_string()))?;
    let started = Instant::now();
    let mut said = Vec::new();
    let url = tokio::time::timeout(PAGE_WAIT, page(&mut lines, &mut said)).await;
    let Ok(Some(url)) = url else {
        let code = match engine.close(Duration::ZERO).await {
            Closed::Exited(code) => format!("exit {code:?}"),
            Closed::Killed => "no sign-in page within 30 s".into(),
        };
        let why = format!(
            "Claude Code gave no sign-in page ({code}): {}",
            said.join(" ")
        );
        record(&s, &t.0, Some(&why), started).await;
        return Err(fail(why));
    };
    let (tx, asks) = mpsc::unbounded_channel();
    *s.sign_in.running.lock().expect("never poisoned") = Some(tx.clone());
    s.sign_in
        .show(&s.chats, json!({ "state": "waiting", "url": url }));
    let job = Job {
        trace: t.0.clone(),
        started,
        mine: tx,
    };
    tokio::spawn(watch(s.clone(), engine, lines, asks, job));
    Ok(Json(json!({ "url": url })))
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
                Ask::Cancel(done) => {
                    engine.close(Duration::ZERO).await;
                    s.writer.record(Event::point("gathr.engine.sign_in").trace(&trace).attr("ended", "cancelled"));
                    s.sign_in.show(&s.chats, Value::Null);
                    let _ = done.send(());
                    return;
                }
            },
        }
    }
    let ours = {
        let mut running = s.sign_in.running.lock().expect("never poisoned");
        let ours = running.as_ref().is_some_and(|tx| tx.same_channel(&mine));
        if ours {
            running.take();
        }
        ours
    };
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
    let running = s.sign_in.running.lock().expect("never poisoned").clone();
    match running {
        Some(tx) if tx.send(Ask::Code(code)).is_ok() => Ok(Json(json!({ "sent": true }))),
        _ => Err(failed(StatusCode::CONFLICT, "no sign-in is waiting", &t.0)),
    }
}

async fn cancel(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    s.sign_in.cancel().await;
    Json(json!({ "cancelled": true }))
}
