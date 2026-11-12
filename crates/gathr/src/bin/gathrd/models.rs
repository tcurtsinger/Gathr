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
    let program = if account.engine == "codex" {
        gathr::codex::command::find(gathr::codex::copies::storage(&s.data).program())
    } else {
        command::find(s.chats.copy())
    };
    let login = s.chats.login_status(&folder).await.0;
    if let (Ok(program), Some((was, was_login, list))) = (&program, last.get(&account.id))
        && program == was
        && login == *was_login
    {
        return Ok(list.clone());
    }
    let started = Instant::now();
    let asked = match program.clone() {
        Ok(program) if account.engine == "codex" => ask_codex(s, &program, &folder).await,
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
    let chats = &s.chats;
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
    let args = command::args(
        Session::New(&gathr::ids::uuid()),
        command::Assigned::handshake(),
        None,
    );
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
    // Never killed for being slow: it runs on the account's login and may be renewing it (0001).
    // Only shutdown kills, after the grace every engine gets.
    close_later(s, engine);
    answer
}

fn close_later(s: &State, mut engine: Engine) {
    let (writer, pid) = (s.writer.clone(), engine.pid);
    let mut stopping = s.stopping.subscribe();
    let close = async move {
        engine.close_input();
        let closed = tokio::select! {
            code = engine.wait() => Closed::Exited(code),
            () = async { _ = stopping.wait_for(|stop| *stop).await } => engine.close(CLOSE_GRACE).await,
        };
        let event = match closed {
            Closed::Exited(code) => Event::point("gathr.engine.models.close")
                .ok()
                .attr("process.exit.code", code),
            Closed::Killed => Event::point("gathr.engine.models.close")
                .error("killed")
                .attr(
                    "killed_because",
                    "it hadn't exited 10 s after its input closed, at shutdown",
                ),
        };
        writer.record_now(event.attr("process.pid", pid)).await;
    };
    s.models
        .closing
        .lock()
        .expect("never poisoned")
        .spawn(close);
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
/// Claude Code lists its newest models by family only (`opus`, which it names "Opus 5.5" signed in):
/// each is followed by a row for the exact model it means today, so that model can be chosen and
/// kept, by the id the engine resolves the family to.
fn listed(init: &Value) -> Value {
    let rows: Vec<&Value> = init["models"].as_array().into_iter().flatten().collect();
    let mut models = Vec::new();
    for m in &rows {
        // A family's Latest: an alias the engine resolves to its newest model today (0025).
        let latest = m["value"] != "default"
            && m["resolvedModel"].is_string()
            && m["value"] != m["resolvedModel"];
        let row = |id: &Value, latest: bool, name: Value| {
            json!({
                "id": id,
                "resolved": m["resolvedModel"],
                "latest": latest,
                "name": name,
                "description": m["description"],
                "efforts": m["supportedEffortLevels"].as_array().cloned().unwrap_or_default(),
            })
        };
        models.push(row(&m["value"], latest, m["displayName"].clone()));
        let exact = &m["resolvedModel"];
        if latest && !rows.iter().any(|other| other["value"] == *exact) {
            models.push(row(exact, false, exact_name(m).into()));
        }
    }
    json!({ "version": init["claude_code_version"], "models": models })
}

/// The exact model a family row means, as the engine names it: its display name signed in ("Opus
/// 5.5"); signed out, where that's the family alone, the start of its description ("Opus 5.5 · …");
/// else its id.
fn exact_name(m: &Value) -> &str {
    let versioned = |name: &&str| name.bytes().any(|b| b.is_ascii_digit());
    let shown = m["displayName"].as_str().filter(versioned);
    let described = m["description"]
        .as_str()
        .and_then(|d| d.split(" · ").next());
    shown
        .or(described.filter(versioned))
        .or(m["resolvedModel"].as_str())
        .unwrap_or_default()
}

async fn ask_codex(
    s: &State,
    program: &std::path::Path,
    folder: &std::path::Path,
) -> Result<Value, String> {
    let mut gate = crate::sign_in::pass_gate(s, folder).await?;
    let mut client = gathr::codex::client::Client::start(program, folder).await?;
    let answer = tokio::select! {
        list = codex_list(&mut client) => list,
        () = s.stopped() => Err("the service is stopping".into()),
    };
    gate.release();
    close_later(s, client.engine);
    answer
}

async fn codex_list(client: &mut gathr::codex::client::Client) -> Result<Value, String> {
    let mut all = Vec::new();
    let mut cursor = Value::Null;
    let mut seen = std::collections::HashSet::new();
    loop {
        let page = client
            .request("model/list", json!({"cursor":cursor,"includeHidden":false}))
            .await?;
        let rows = page["data"]
            .as_array()
            .ok_or("Codex returned no model list")?;
        all.extend(rows.iter().filter(|m| m["hidden"] != true).cloned());
        cursor = page["nextCursor"].clone();
        if cursor.is_null() {
            break;
        }
        let value = cursor
            .as_str()
            .ok_or("Codex returned an invalid model cursor")?;
        if !seen.insert(value.to_string()) {
            return Err("Codex repeated a model-list cursor".into());
        }
    }
    let models = gathr::codex::models::listed(&all)?;
    Ok(json!({"version":client.version, "models":models}))
}
