//! Computer use (`gathr::computer`) for the Computer tab and the relay: Pause, the one thing that refuses an
//! agent's call; each agent's steps and latest picture; and how the driver's first fetch went. Kept in memory
//! while the service runs, nothing stored.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use axum::Json;
use axum::extract::{Path, State as Extract};
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use gathr::computer::ID;
use gathr::ids::{now_us, utc};
use gathr::web_tools::{self, Remote};
use rmcp::ErrorData;
use rmcp::model::{CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock};
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::api::State;

/// Steps kept for each agent.
const STEPS: usize = 50;
/// What an agent's call is told while the person has paused computer use.
const PAUSED: &str =
    "The person paused computer use in Gathr. Wait until they resume it, then try again.";

pub fn routes() -> axum::Router<Arc<State>> {
    axum::Router::new()
        .route("/api/computer", get(read))
        .route("/api/computer/pause", post(pause))
        .route("/api/computer/agents/{agent}/shot", get(shot))
}

static PAUSE: AtomicBool = AtomicBool::new(false);
static SHOTS: AtomicU64 = AtomicU64::new(0);

/// How the driver's copy is: `missing`, `fetching`, `ready` or `failed`, with why.
static DRIVER: Mutex<(&str, Option<String>)> = Mutex::new(("missing", None));

pub fn driver(state: &'static str, why: Option<String>) {
    *lock(&DRIVER) = (state, why);
}

struct Step {
    at: i64,
    tool: String,
    target: Option<String>,
    text: Option<String>,
    refused: Option<&'static str>,
    failed: Option<String>,
}

#[derive(Default)]
struct Agent {
    chat: String,
    at: i64,
    window: Option<String>,
    shot: Option<(u64, Vec<u8>, String)>,
    mark: Option<(f64, f64)>,
    steps: VecDeque<Step>,
    /// Its last read of a window, for the names of the elements it acts on by token.
    tree: String,
}

static AGENTS: Mutex<Option<HashMap<String, Agent>>> = Mutex::new(None);
/// Windows' titles by id, as the driver last named them.
static TITLES: Mutex<Option<HashMap<u64, String>>> = Mutex::new(None);

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// An agent's call to the driver: refused while paused, else relayed; either way recorded for the tab.
pub async fn call(
    s: &Arc<State>,
    caller: Option<&crate::tools::Caller>,
    remote: &Remote,
    request: CallToolRequestParams,
) -> Result<CallToolResponse, ErrorData> {
    let args = request.arguments.clone().unwrap_or_default();
    let mut step = Step {
        at: now_us(),
        tool: request.name.to_string(),
        target: None,
        text: text(&request.name, &args),
        refused: None,
        failed: None,
    };
    let (agent, chat) = match caller {
        Some(c) => (c.agent.clone().unwrap_or_default(), c.chat.clone()),
        None => (String::new(), String::new()),
    };
    if PAUSE.load(Ordering::Relaxed) {
        step.refused = Some("paused");
        record(s, &agent, &chat, &args, step, None);
        let refused = CallToolResult::error(vec![ContentBlock::text(PAUSED)]);
        return Ok(refused.into());
    }
    let called = web_tools::call(remote, request).await;
    let result = match &called {
        Ok(CallToolResponse::Complete(result)) => serde_json::to_value(result).ok(),
        _ => None,
    };
    match (&called, &result) {
        (Err(e), _) => step.failed = Some(e.message.to_string()),
        (_, Some(r)) if r["isError"] == true => {
            step.failed = Some(words(r).chars().take(200).collect())
        }
        _ => {}
    }
    record(s, &agent, &chat, &args, step, result.as_ref());
    called
}

/// What a step typed or pressed, cut to 80 characters.
fn text(tool: &str, args: &Map<String, Value>) -> Option<String> {
    let said = match tool {
        "type_text" => args.get("text")?.as_str()?.to_string(),
        "press_key" => args.get("key")?.as_str()?.to_string(),
        "hotkey" => {
            let keys = args.get("keys")?.as_array()?;
            keys.iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("+")
        }
        _ => return None,
    };
    Some(said.chars().take(80).collect())
}

/// A result's text blocks, joined.
fn words(result: &Value) -> String {
    let blocks = result["content"].as_array().into_iter().flatten();
    let texts = blocks
        .filter(|b| b["type"] == "text")
        .filter_map(|b| b["text"].as_str());
    texts.collect::<Vec<_>>().join("\n")
}

fn record(
    s: &Arc<State>,
    agent: &str,
    chat: &str,
    args: &Map<String, Value>,
    mut step: Step,
    result: Option<&Value>,
) {
    if let Some(result) = result {
        titles(&result["structuredContent"]);
    }
    let window = args
        .get("window_id")
        .and_then(Value::as_u64)
        .and_then(|id| lock(&TITLES).as_ref()?.get(&id).cloned());
    let mut agents = lock(&AGENTS);
    let one = agents
        .get_or_insert_with(HashMap::new)
        .entry(agent.to_string())
        .or_default();
    step.target = target(&step.tool, args, &one.tree);
    one.chat = chat.to_string();
    one.at = step.at;
    if window.is_some() {
        one.window = window;
    }
    if let (Some(x), Some(y)) = (num(args, "x"), num(args, "y")) {
        one.mark = Some((x, y));
    }
    if let Some(result) = result {
        let text = words(result);
        if text.contains("element_token for row") {
            one.tree = text;
        }
        let image = result["content"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["type"] == "image");
        if let Some(bytes) = image.and_then(|i| gathr::browser::decode(i["data"].as_str()?)) {
            let mime = image
                .and_then(|i| i["mimeType"].as_str())
                .unwrap_or("image/png");
            let n = SHOTS.fetch_add(1, Ordering::Relaxed) + 1;
            one.shot = Some((n, bytes, mime.to_string()));
        }
    }
    one.steps.push_back(step);
    while one.steps.len() > STEPS {
        one.steps.pop_front();
    }
    drop(agents);
    let _ = s.chats.live.send(json!({ "type": "computer" }));
}

fn num(args: &Map<String, Value>, key: &str) -> Option<f64> {
    args.get(key)?.as_f64()
}

/// Every window the driver named in a result, by its id: `list_windows`' and `launch_app`'s lists, and
/// a read's own window.
fn titles(found: &Value) {
    match found {
        Value::Object(o) => {
            if let (Some(id), Some(title)) = (
                o.get("window_id").and_then(Value::as_u64),
                o.get("title").and_then(Value::as_str),
            ) {
                lock(&TITLES)
                    .get_or_insert_with(HashMap::new)
                    .insert(id, title.to_string());
            }
            o.values().for_each(titles);
        }
        Value::Array(a) => a.iter().for_each(titles),
        _ => {}
    }
}

/// What a step acted on, by name: the element its token names in the agent's last read
/// (`[4] Button "Count"` for `s00000002:4`), or the app it launched.
fn target(tool: &str, args: &Map<String, Value>, tree: &str) -> Option<String> {
    if tool == "launch_app" {
        let named = ["name", "aumid", "bundle_id", "path"]
            .iter()
            .find_map(|k| args.get(*k)?.as_str());
        return named.map(|n| n.rsplit(['\\', '/']).next().unwrap_or(n).to_string());
    }
    let token = args.get("element_token")?.as_str()?;
    let (snapshot, row) = token.rsplit_once(':')?;
    if !tree.contains(&format!("snapshot_id={snapshot}")) {
        return None;
    }
    let line = tree
        .lines()
        .find(|l| l.trim_start().starts_with(&format!("- [{row}] ")))?;
    let name = line.split('"').nth(1)?;
    Some(name.to_string())
}

async fn on(s: &Arc<State>) -> bool {
    let found = s.writer.call(|conn| web_tools::get(conn, ID)).await;
    matches!(found, Some(Ok(tool)) if tool.enabled)
}

async fn state(s: &Arc<State>) -> Value {
    let on = on(s).await;
    let (mut driver, why) = lock(&DRIVER).clone();
    if driver == "missing" && gathr::computer::program(s.data.path()).is_file() {
        driver = "ready";
    }
    let agents = lock(&AGENTS);
    let mut all: Vec<(&String, &Agent)> = agents.iter().flatten().collect();
    all.sort_by_key(|(_, a)| std::cmp::Reverse(a.at));
    let agents: Vec<Value> = all
        .into_iter()
        .map(|(agent, a)| {
            let steps: Vec<Value> = a
                .steps
                .iter()
                .map(|step| {
                    json!({
                        "at": utc(step.at), "tool": step.tool, "target": step.target, "text": step.text,
                        "refused": step.refused, "failed": step.failed,
                    })
                })
                .collect();
            json!({
                "agent": agent, "chat": a.chat, "window": a.window,
                "shot": a.shot.as_ref().map(|(n, ..)| n),
                "mark": a.mark.map(|(x, y)| json!({ "x": x, "y": y })),
                "steps": steps,
            })
        })
        .collect();
    json!({
        "tool": ID, "on": on, "driver": driver, "why": why,
        "paused": PAUSE.load(Ordering::Relaxed), "agents": agents,
    })
}

async fn read(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    Json(state(&s).await)
}

#[derive(Deserialize)]
struct Pause {
    paused: bool,
}

async fn pause(Extract(s): Extract<Arc<State>>, Json(p): Json<Pause>) -> Json<Value> {
    PAUSE.store(p.paused, Ordering::Relaxed);
    let _ = s.chats.live.send(json!({ "type": "computer" }));
    Json(state(&s).await)
}

async fn shot(Path(agent): Path<String>) -> axum::response::Response {
    let agents = lock(&AGENTS);
    match agents.as_ref().and_then(|a| a.get(&agent)?.shot.as_ref()) {
        Some((_, bytes, mime)) => {
            ([(header::CONTENT_TYPE, mime.clone())], bytes.clone()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
