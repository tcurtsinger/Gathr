//! Tools on this PC (`gathr::pc_tools`) for the Plugins screen and the relay. What pasted text would
//! run is said back before anything runs. Each tool has one running copy, shared by every agent and
//! the screen: started at its first need, and again at the next need once it has ended. One that
//! didn't start stays so until it's tried again, changed, or disabled and enabled. No answer, event
//! or log carries a variable's value: at most its name.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use gathr::pc_tools::{self, Printed, Runs};
use gathr::platform::env::Vars;
use gathr::web_tools::{self, Remote, Tool};
use rmcp::ErrorData;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::sync::watch;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, fail};
use crate::tools::relay;
use crate::web_tools::told;

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/tools/read", post(read))
        .route("/api/tools/{id}/start", post(try_again))
}

/// Where a copy is.
#[derive(Clone)]
pub enum Phase {
    Starting,
    Running(Arc<Remote>),
    NotStarted,
}

/// A tool's state in the API.
pub fn state(phase: &Phase) -> &'static str {
    match phase {
        Phase::Starting => "starting",
        Phase::Running(_) => "connected",
        Phase::NotStarted => "not_started",
    }
}

/// A tool's running copy.
pub struct Copy {
    phase: watch::Sender<Phase>,
    printed: Arc<Mutex<Printed>>,
    /// Someone was answered that it's starting: an open window is told when it's done.
    asked: AtomicBool,
}

impl Copy {
    /// Its phase once it's past starting, or `Starting` after `limit`.
    pub async fn phase(&self, limit: Duration) -> Phase {
        let mut phase = self.phase.subscribe();
        let done = phase.wait_for(|p| !matches!(p, Phase::Starting));
        if let Ok(Ok(done)) = tokio::time::timeout(limit, done).await {
            return done.clone();
        }
        self.asked.store(true, Ordering::Relaxed);
        Phase::Starting
    }

    fn ended(&self) -> bool {
        matches!(&*self.phase.borrow(), Phase::Running(r) if r.is_closed() || r.is_transport_closed())
    }
}

/// Each tool's copy, by id.
static COPIES: Mutex<Option<HashMap<String, Arc<Copy>>>> = Mutex::new(None);

fn copies() -> MutexGuard<'static, Option<HashMap<String, Arc<Copy>>>> {
    COPIES.lock().unwrap_or_else(|e| e.into_inner())
}

/// The tool's copy: the one there is, else one started now, as when the one there is has ended.
pub fn copy(s: &Arc<State>, id: &str) -> Arc<Copy> {
    let mut copies = copies();
    let copies = copies.get_or_insert_with(HashMap::new);
    if let Some(copy) = copies.get(id).filter(|c| !c.ended()) {
        return copy.clone();
    }
    let copy = Arc::new(Copy {
        phase: watch::Sender::new(Phase::Starting),
        printed: Default::default(),
        asked: AtomicBool::new(false),
    });
    copies.insert(id.to_string(), copy.clone());
    tokio::spawn(run(s.clone(), id.to_string(), copy.clone()));
    copy
}

/// Stops the tool's copy, if it has one: it ends once no call holds it.
pub fn stop(id: &str) {
    if let Some(copy) = copies().as_mut().and_then(|c| c.remove(id)) {
        copy.phase.send_replace(Phase::NotStarted);
    }
}

async fn run(s: Arc<State>, id: String, copy: Arc<Copy>) {
    let find = id.clone();
    let saved = s.writer.call(move |c| pc_tools::saved(c, &find)).await;
    let vars = gathr::run::fresh().await;
    let say = |why: &str| {
        copy.printed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(why)
    };
    let started = match saved {
        Some(Ok(runs)) => match pc_tools::find(&runs.program, &vars) {
            Some(program) => pc_tools::start(&runs, &program, vars, copy.printed.clone()).await,
            None => {
                say(&pc_tools::missing(&runs.program));
                None
            }
        },
        // What it runs wasn't read: its variables sealed on another PC or user, say.
        Some(Err(e)) => {
            say(&e.to_string());
            None
        }
        None => None,
    };
    let copies = copies();
    let current = copies.as_ref().and_then(|c| c.get(&id));
    // A copy stopped while it started is dropped here, which ends it.
    match (current.is_some_and(|c| Arc::ptr_eq(c, &copy)), started) {
        (true, Some(remote)) => copy.phase.send_replace(Phase::Running(Arc::new(remote))),
        _ => copy.phase.send_replace(Phase::NotStarted),
    };
    drop(copies);
    if copy.asked.load(Ordering::Relaxed) {
        let _ = s.chats.live.send(json!({ "type": "tools" }));
    }
}

/// The tool's copy, running, for the relay: an agent's call waits for its start.
pub async fn remote(s: &Arc<State>, id: &str) -> Result<Arc<Remote>, ErrorData> {
    match copy(s, id).phase(pc_tools::START).await {
        Phase::Running(remote) => Ok(remote),
        _ => Err(ErrorData::internal_error(pc_tools::NOT_STARTED, None)),
    }
}

/// How the tool's start went: its actions' names when it runs, else what it printed, if anything.
pub async fn outcome(s: &Arc<State>, id: &str) -> Result<Vec<String>, Option<String>> {
    let copy = copy(s, id);
    match copy.phase(web_tools::CONNECT).await {
        Phase::Running(remote) => {
            let actions = web_tools::actions(&remote, web_tools::CONNECT).await;
            Ok(actions
                .unwrap_or_default()
                .into_iter()
                .map(|a| a.name)
                .collect())
        }
        _ => Err(copy
            .printed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .text()),
    }
}

/// What a tool runs, as an answer says it: the program found, each variable by its name only.
fn said(runs: &Runs, vars: &Vars) -> Value {
    let found = pc_tools::find(&runs.program, vars);
    json!({
        "missing": found.is_none().then(|| pc_tools::missing(&runs.program)),
        "program": found.map(|p| p.display().to_string()),
        "typed": runs.program,
        "args": runs.args,
        "folder": runs.folder,
        "variables": runs.variables.keys().collect::<Vec<_>>(),
    })
}

#[derive(Deserialize)]
struct Pasted {
    text: String,
}

/// What Gathr would run for the pasted text, and where it comes from. Nothing runs.
async fn read(Extension(t): Extension<Trace>, Json(pasted): Json<Pasted>) -> Answer {
    let read = pc_tools::read(&pasted.text);
    let read = read.map_err(|e| fail(StatusCode::CONFLICT, "Read failed", &e.to_string(), &t.0))?;
    let mut shown = said(&read.runs, &gathr::run::fresh().await);
    shown["name"] = json!(read.name);
    shown["from"] = json!(pc_tools::from(&read.runs));
    Ok(Json(shown))
}

/// What a pasted text runs, refused when it can't be read or its program isn't on this PC.
async fn pasted(
    text: &str,
    t: &Trace,
    error: &'static str,
) -> Result<pc_tools::Pasted, crate::routes::Fail> {
    let refused = |why: &str| fail(StatusCode::CONFLICT, error, why, &t.0);
    let read = pc_tools::read(text).map_err(|e| refused(&e.to_string()))?;
    match pc_tools::find(&read.runs.program, &gathr::run::fresh().await) {
        Some(_) => Ok(read),
        None => Err(refused(&pc_tools::missing(&read.runs.program))),
    }
}

/// Saves the tool and starts it, waiting for its start: one that didn't start is saved too, for its
/// page to say what it printed.
pub async fn add(s: &Arc<State>, t: &Trace, text: &str, name: Option<String>) -> Answer {
    let read = pasted(text, t, "Add failed").await?;
    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or(read.name);
    let saved = db(s, t, "Add failed", move |conn| {
        Ok(json!({ "tool": pc_tools::add(conn, &name, &read.runs)? }))
    });
    let Json(saved) = saved.await?;
    let id = saved["tool"]["id"].as_str().unwrap_or_default();
    copy(s, id).phase(pc_tools::START).await;
    told(s, Ok(Json(saved)))
}

/// Replaces what it runs and its name, or only its name; a variable given empty keeps its value. Its
/// copy is stopped, and started again when it's enabled.
pub async fn change(
    s: &Arc<State>,
    t: &Trace,
    id: String,
    text: Option<String>,
    name: Option<String>,
) -> Answer {
    let read = match &text {
        Some(text) => Some(pasted(text, t, "Save failed").await?),
        None => None,
    };
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    let changing = id.clone();
    let saved = db(s, t, "Save failed", move |conn| {
        let was = pc_tools::saved(conn, &changing)?;
        let runs = match read {
            Some(read) => pc_tools::kept(read.runs, &was),
            None => was,
        };
        Ok(json!(pc_tools::change(
            conn,
            &changing,
            name.as_deref(),
            &runs
        )?))
    });
    let Json(tool) = saved.await?;
    relay::forget(&id);
    if tool["enabled"] == json!(true) {
        copy(s, &id).phase(pc_tools::START).await;
    }
    told(s, Ok(Json(tool)))
}

/// The tool's page: its state, what it runs, and its actions when it runs, or what it printed when
/// it didn't start. A disabled one isn't started.
pub async fn one(s: &Arc<State>, t: &Trace, tool: Tool) -> Answer {
    let id = tool.id.clone();
    // Only the variables' names leave the writer, and which of them have no value yet.
    let read = s.writer.call(move |conn| {
        pc_tools::saved(conn, &id).map(|runs| {
            let empty = runs.variables.iter().filter(|(_, value)| value.is_empty());
            let empty: Vec<String> = empty.map(|(name, _)| name.clone()).collect();
            (nameless(runs), empty)
        })
    });
    let (runs, empty) = match read.await {
        Some(Ok(read)) => read,
        Some(Err(e)) => {
            return Err(fail(
                StatusCode::CONFLICT,
                "Tool failed",
                &e.to_string(),
                &t.0,
            ));
        }
        None => {
            let why = "the database didn't answer";
            return Err(fail(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Tool failed",
                why,
                &t.0,
            ));
        }
    };
    let (mut state, mut actions, mut printed) = (None, None, None);
    if tool.enabled {
        let copy = copy(s, &tool.id);
        let phase = copy.phase(web_tools::CONNECT).await;
        if let Phase::Running(remote) = &phase {
            actions = web_tools::actions(remote, web_tools::CONNECT).await.ok();
        }
        if let Phase::NotStarted = phase {
            printed = copy
                .printed
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .text();
        }
        state = Some(self::state(&phase));
    }
    let mut shown = said(&runs, &gathr::run::fresh().await);
    for (key, value) in json!(tool).as_object().into_iter().flatten() {
        shown[key] = value.clone();
    }
    shown["state"] = json!(state);
    shown["actions"] = json!(actions);
    shown["printed"] = json!(printed);
    shown["empty"] = json!(empty);
    Ok(Json(shown))
}

/// What a tool runs with its variables' values left out.
fn nameless(mut runs: Runs) -> Runs {
    runs.variables.values_mut().for_each(String::clear);
    runs
}
/// Try again: stops what's left of the tool's copy and starts it again.
async fn try_again(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let find = id.clone();
    let _checked = db(&s, &t, "Start failed", move |conn| {
        let tool = web_tools::get(conn, &find)?;
        match (tool.kind, tool.enabled) {
            ("pc", true) => Ok(Value::Null),
            ("pc", false) => Err(gathr::Error::Refused("It's disabled.".into())),
            _ => Err(gathr::Error::Refused(format!("{find} isn't on this PC"))),
        }
    })
    .await?;
    relay::forget(&id);
    let phase = copy(&s, &id).phase(web_tools::CONNECT).await;
    told(&s, Ok(Json(json!({ "state": state(&phase) }))))
}
