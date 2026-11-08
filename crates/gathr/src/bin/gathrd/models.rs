//! An account's models and effort levels, as its Claude Code lists them in `initialize` (0025).
//! Signed out it lists the aliases only; signed in, what the account may use. So the list is
//! kept per account, engine copy and sign-in state, and asked again when either changes.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::accounts::Account;
use gathr::claude::command::{self, Session};
use gathr::claude::engine::{Closed, Engine, Line};
use gathr::claude::frames;
use gathr::claude::login::Login;
use gathr::event::Event;
use serde_json::{Value, json};
use tokio::sync::Mutex;
use tokio::task::JoinSet;

use crate::accounts::find;
use crate::api::{State, Trace};
use crate::routes::{Answer, fail};
use crate::turns::CLOSE_GRACE;

/// Claude Code 2.1.318 answered in 0.65 s signed in; past this it isn't coming.
const ANSWER_WAIT: Duration = Duration::from_secs(30);
const ASKED: &str = "gathr-models";

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/accounts/{account}/models", get(models))
}

#[derive(Default)]
pub struct Models {
    /// Each account's last list, and the engine copy and sign-in state it came from.
    last: Mutex<HashMap<String, (PathBuf, Login, Value)>>,
    /// Engines still closing after their answer, which shutdown waits for.
    closing: std::sync::Mutex<JoinSet<()>>,
}

impl Models {
    /// Waits for every engine an ask started to have exited.
    pub async fn closed(&self) {
        let closing = std::mem::take(&mut *self.closing.lock().expect("never poisoned"));
        closing.join_all().await;
    }
}

async fn models(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(account): Path<String>,
) -> Answer {
    let account = find(&s, &t, &account, "Models failed").await?;
    match list(&s, &t.0, &account).await {
        Ok(list) => Ok(Json(list)),
        Err(why) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Models failed",
            &why,
            &t.0,
        )),
    }
}

/// The account's models, asked of its engine once per engine copy and sign-in state.
pub async fn list(s: &State, trace: &str, account: &Account) -> Result<Value, String> {
    let folder = account.folder(&s.data);
    // One ask at a time: a second request waits for the first's list instead of starting another.
    let mut last = s.models.last.lock().await;
    let program = command::find(s.chats.copy());
    let login = s.chats.login_status(&folder).await.0;
    if let (Ok(program), Some((was, was_login, list))) = (&program, last.get(&account.id))
        && program == was
        && login == *was_login
    {
        return Ok(list.clone());
    }
    let started = Instant::now();
    let asked = match program.clone() {
        Ok(program) => ask(s, &program, &folder).await,
        Err(why) => Err(why),
    };
    let mut event = Event::point("gathr.engine.models")
        .trace(trace)
        .attr("account", account.id.clone())
        .attr("login", json!(login))
        .attr(
            "path",
            program.as_ref().map(|p| p.display().to_string()).ok(),
        );
    event.duration_us = Some(started.elapsed().as_micros() as i64);
    match asked {
        Ok(list) => {
            let count = list["models"].as_array().map_or(0, Vec::len);
            s.writer.record(
                event
                    .ok()
                    .attr("models", count)
                    .attr("version", list["version"].clone()),
            );
            let program = program.expect("asked only with a program");
            last.insert(account.id.clone(), (program, login, list.clone()));
            Ok(list)
        }
        Err(why) => {
            s.writer
                .record_now(event.error("engine_models").attr("why", why.clone()))
                .await;
            Err(why)
        }
    }
}

/// Whether the account's engine lists `model` (its default when `None`) with `effort`, if one is
/// given. Claude Code only warns on stderr about an effort it doesn't know, then ignores it.
pub fn offered(list: &Value, model: Option<&str>, effort: Option<&str>) -> Result<(), String> {
    let id = model.unwrap_or("default");
    let models = list["models"].as_array().into_iter().flatten();
    let Some(found) = models.into_iter().find(|m| m["id"] == id) else {
        return match model {
            Some(model) => Err(format!("the account has no model {model}")),
            None => Ok(()),
        };
    };
    match effort {
        Some(effort)
            if !found["efforts"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|e| e == effort) =>
        {
            Err(format!("{id} has no {effort} effort"))
        }
        _ => Ok(()),
    }
}

/// Starts the account's Claude Code as a chat does, asks `initialize`, and closes it gently.
async fn ask(
    s: &State,
    program: &std::path::Path,
    folder: &std::path::Path,
) -> Result<Value, String> {
    const STOPPING: &str = "the service is stopping";
    let (chats, models) = (&s.chats, &s.models);
    let cwd = chats.data.path().join("chats");
    for dir in [folder, &cwd] {
        std::fs::create_dir_all(dir).map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
    }
    // It runs on the account's login, so it takes the renewal gate like any start (0002).
    let gate = chats.gate(folder);
    let mut gate = tokio::select! {
        gate = gate.pass() => gate,
        () = s.stopped() => return Err(STOPPING.into()),
    };
    let args = command::args(Session::New(&gathr::ids::uuid()), Default::default());
    let env = command::env(std::env::vars_os(), folder);
    let (mut engine, mut lines) = Engine::start(program, &args, env, &cwd)
        .map_err(|e| format!("Claude Code didn't start from {}: {e}", program.display()))?;
    let answer = match engine.write(&frames::initialize(ASKED)).await {
        Ok(()) => tokio::select! {
            answer = tokio::time::timeout(ANSWER_WAIT, answered(&mut lines)) => answer
                .unwrap_or_else(|_| Err(format!("Claude Code didn't answer in {} s", ANSWER_WAIT.as_secs()))),
            () = s.stopped() => Err(STOPPING.into()),
        },
        Err(e) => Err(format!("Claude Code didn't take the request: {e}")),
    };
    gate.release();
    // Its exit takes another 0.6 s, which the list needn't wait for, but shutdown does.
    let (writer, pid) = (s.writer.clone(), engine.pid);
    let close = async move {
        let event = match engine.close(CLOSE_GRACE).await {
            Closed::Exited(code) => Event::point("gathr.engine.models.close")
                .ok()
                .attr("process.exit.code", code),
            Closed::Killed => Event::point("gathr.engine.models.close")
                .error("killed")
                .attr(
                    "killed_because",
                    "it hadn't exited 10 s after its input closed",
                ),
        };
        writer.record_now(event.attr("process.pid", pid)).await;
    };
    models.closing.lock().expect("never poisoned").spawn(close);
    answer
}

async fn answered(lines: &mut tokio::sync::mpsc::UnboundedReceiver<Line>) -> Result<Value, String> {
    let mut last_err = None;
    while let Some(line) = lines.recv().await {
        let line = match line {
            Line::Out(line) => line,
            Line::Err(line) => {
                last_err = Some(line);
                continue;
            }
        };
        let frame: Value = serde_json::from_str(&line).unwrap_or_default();
        let response = &frame["response"];
        if frame["type"] != "control_response" || response["request_id"] != ASKED {
            continue;
        }
        return match response["subtype"].as_str() {
            Some("success") => Ok(listed(&response["response"])),
            _ => Err(format!("initialize failed: {}", response["error"])),
        };
    }
    Err(
        last_err.map_or("Claude Code exited without answering".into(), |e| {
            format!("Claude Code exited without answering: {e}")
        }),
    )
}

/// What a picker needs from each model: its id, name, line and effort levels, in the engine's order.
fn listed(init: &Value) -> Value {
    let models: Vec<Value> = init["models"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|m| {
            json!({
                "id": m["value"],
                // What a family alias such as `opus` runs today: it moves to each new release.
                "resolved": m["resolvedModel"],
                "name": m["displayName"],
                "description": m["description"],
                "efforts": m["supportedEffortLevels"].as_array().cloned().unwrap_or_default(),
            })
        })
        .collect();
    json!({ "version": init["claude_code_version"], "models": models })
}
