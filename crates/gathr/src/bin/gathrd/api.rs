//! The HTTP API and who may call it (docs/decisions/0004-service-process.md).

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Request, State as Extract};
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::data::DataDir;
use gathr::event::Event;
use gathr::writer::Writer;
use gathr::{VERSION, db, ids};
use serde_json::json;

pub struct State {
    pub writer: Writer,
    pub token: String,
    pub port: u16,
    pub pid: u32,
    pub started_us: i64,
    pub data: DataDir,
    pub stop: tokio::sync::Notify,
    pub chats: Arc<crate::chats::Chats>,
    pub updates: Arc<crate::updates::Updates>,
    pub codex_updates: Arc<crate::updates::Updates>,
    pub app: Arc<crate::app_update::AppUpdates>,
    pub sign_ins: Arc<crate::sign_in::SignIns>,
    pub models: crate::models::Models,
    /// Set once shutdown starts, before the server waits on its requests: a request waiting on
    /// an engine ends at once instead of holding the stop.
    pub stopping: tokio::sync::watch::Sender<bool>,
    /// The built UI's folder.
    pub ui: std::path::PathBuf,
    /// The cookie's value: a browser's stand-in for the token, new at each start.
    pub session: String,
    /// One-time codes not yet swapped for the cookie, with when each was made.
    pub once_codes: std::sync::Mutex<Vec<(String, Instant)>>,
}

impl State {
    pub async fn stopped(&self) {
        let _ = self.stopping.subscribe().wait_for(|stop| *stop).await;
    }
}

/// The request's trace id, for handlers: a failure's reference and its events share it.
#[derive(Clone)]
pub struct Trace(pub String);

pub fn router(state: Arc<State>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/shutdown", post(shutdown))
        .merge(crate::routes::routes())
        .merge(crate::archive::routes())
        .merge(crate::coord::routes())
        .merge(crate::pauses::routes())
        .merge(crate::projects::routes())
        .merge(crate::accounts::routes())
        .merge(crate::agents::routes())
        .merge(crate::sign_in::routes())
        .merge(crate::models::routes())
        .merge(crate::startup::routes())
        .merge(crate::app_update::routes())
        .nest_service("/mcp", crate::tools::service(state.chats.clone()))
        .merge(crate::tools::hook_routes())
        .merge(crate::ui::routes())
        .fallback(crate::ui::page)
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

async fn health(Extract(s): Extract<Arc<State>>) -> Json<serde_json::Value> {
    let size = std::fs::metadata(s.data.db()).map(|m| m.len()).unwrap_or(0);
    Json(json!({
        "status": "ok",
        "version": s.app.running,
        "build": VERSION,
        "pid": s.pid,
        "port": s.port,
        "data_folder": s.data.path(),
        "database_bytes": size,
        "schema_version": db::SCHEMA_VERSION,
        "started": ids::utc(s.started_us),
    }))
}

async fn shutdown(Extract(s): Extract<Arc<State>>) -> impl IntoResponse {
    s.stop.notify_one();
    (StatusCode::ACCEPTED, Json(json!({ "stopping": true })))
}

/// Checks Host, Origin and the token on every request, and records every failed request
/// before answering, so its reference can be traced as soon as it's shown.
async fn guard(Extract(s): Extract<Arc<State>>, mut req: Request, next: Next) -> Response {
    let started = Instant::now();
    let trace = traceparent(req.headers()).unwrap_or_else(ids::trace_id);
    let (method, path) = (req.method().to_string(), req.uri().path().to_string());
    let admitted = admit(&s, req.headers(), req.uri());
    req.extensions_mut().insert(Trace(trace.clone()));
    let mut response = match admitted {
        Err((status, why)) => failure(status, "Request failed", Some(why), &trace),
        Ok(caller) => {
            if let Some(caller) = caller {
                req.extensions_mut().insert(caller);
            }
            next.run(req).await
        }
    };
    let status = response.status();
    if status.is_client_error() || status.is_server_error() {
        let mut event = Event::point("gathr.api.request")
            .trace(&trace)
            .error(&status.as_u16().to_string())
            .attr("http.request.method", method)
            .attr("url.path", path)
            .attr("http.response.status_code", status.as_u16());
        event.duration_us = Some(started.elapsed().as_micros() as i64);
        if let Some(Said(error, why)) = response.extensions().get::<Said>().cloned() {
            event = event.attr("error", error).attr("reason", why);
        }
        s.writer.record_now(event).await;
    }
    if let Ok(value) = ids::reference(&trace).parse() {
        response.headers_mut().insert("x-gathr-ref", value);
    }
    response
}

/// Whether a request may go on: refused for a foreign Host or Origin (403), or no valid token or
/// cookie (401). The UI's own files need neither: they hold nothing secret. Gathr's tools and its
/// hook take only a running engine's own token, never the service's or the cookie: the engine
/// they admit is the caller they act for (0005, 0029).
fn admit(
    s: &State,
    headers: &HeaderMap,
    uri: &Uri,
) -> Result<Option<crate::tools::Caller>, (StatusCode, &'static str)> {
    if let Some(refused) = refusal(s, headers) {
        return Err(refused);
    }
    let path = uri.path();
    let hook = [crate::tools::HOOK_PATH, crate::tools::PUSH_PATH].contains(&path);
    let tools = path == "/mcp" || path.starts_with("/mcp/") || hook;
    if !tools && !path.starts_with("/api/") && path != "/health" {
        return Ok(None);
    }
    if !tools
        && cookie(headers, &crate::ui::cookie_name(s.port))
            .is_some_and(|c| same(c.as_bytes(), s.session.as_bytes()))
    {
        return Ok(None);
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| live_token(uri));
    match (token, tools) {
        (None, _) => Err((StatusCode::UNAUTHORIZED, "token missing")),
        (Some(t), true) => match s.chats.passes.caller(t) {
            Some(caller) => Ok(Some(caller)),
            None => Err((StatusCode::UNAUTHORIZED, "not a running engine's token")),
        },
        (Some(t), false) if !same(t.as_bytes(), s.token.as_bytes()) => {
            Err((StatusCode::UNAUTHORIZED, "token wrong"))
        }
        (Some(_), false) => Ok(None),
    }
}

/// Why a request is refused before its token is looked at: a foreign Host or Origin.
fn refusal(s: &State, headers: &HeaderMap) -> Option<(StatusCode, &'static str)> {
    let port = s.port;
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if host != format!("127.0.0.1:{port}") && host != format!("localhost:{port}") {
        return Some((StatusCode::FORBIDDEN, "host not allowed"));
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let own = [
            format!("http://127.0.0.1:{port}"),
            format!("http://localhost:{port}"),
        ];
        let window = ["http://tauri.localhost", "tauri://localhost"];
        let origin = origin.to_str().unwrap_or("");
        if !own.iter().any(|o| o == origin) && !window.contains(&origin) {
            return Some((StatusCode::FORBIDDEN, "origin not allowed"));
        }
    }
    None
}

/// A cookie's value by name, from every `Cookie` header.
fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|pair| pair.trim().strip_prefix(name)?.strip_prefix('='))
}

/// A browser can't set headers on a WebSocket, so the live socket may carry the token in its query.
fn live_token(uri: &Uri) -> Option<&str> {
    if uri.path() != "/api/live" {
        return None;
    }
    uri.query()?
        .split('&')
        .find_map(|pair| pair.strip_prefix("token="))
}

/// Compares in time that doesn't depend on where the bytes differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// The trace id of a W3C `traceparent` header: `00-<32 hex>-<16 hex>-<2 hex>`.
fn traceparent(headers: &HeaderMap) -> Option<String> {
    let value = headers.get("traceparent")?.to_str().ok()?;
    let parts: Vec<&str> = value.split('-').collect();
    let hex =
        |s: &str, n: usize| s.len() == n && s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
    let valid = parts.len() == 4
        && hex(parts[0], 2)
        && hex(parts[1], 32)
        && hex(parts[2], 16)
        && hex(parts[3], 2);
    (valid && parts[1] != "0".repeat(32)).then(|| parts[1].to_string())
}

/// An error answer: "… failed", why, and the reference the person can trace.
pub fn failure(status: StatusCode, error: &str, why: Option<&str>, trace: &str) -> Response {
    let body = json!({ "error": error, "why": why, "ref": ids::reference(trace) });
    let mut response = (status, Json(body)).into_response();
    let said = Said(error.to_string(), why.map(String::from));
    response.extensions_mut().insert(said);
    response
}

/// What an error answer said, so the request's event records it and its reference finds why.
#[derive(Clone)]
struct Said(String, Option<String>);
