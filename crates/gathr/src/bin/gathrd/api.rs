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
    pub sign_in: Arc<crate::sign_in::SignIn>,
}

/// The request's trace id, for handlers: a failure's reference and its events share it.
#[derive(Clone)]
pub struct Trace(pub String);

pub fn router(state: Arc<State>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/shutdown", post(shutdown))
        .merge(crate::routes::routes())
        .merge(crate::sign_in::routes())
        .layer(middleware::from_fn_with_state(state.clone(), guard))
        .with_state(state)
}

async fn health(Extract(s): Extract<Arc<State>>) -> Json<serde_json::Value> {
    let size = std::fs::metadata(s.data.db()).map(|m| m.len()).unwrap_or(0);
    Json(json!({
        "status": "ok",
        "version": VERSION,
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
    let refused = refusal(&s, req.headers(), req.uri());
    req.extensions_mut().insert(Trace(trace.clone()));
    let mut response = match refused {
        Some((status, why)) => failure(status, "Request failed", Some(why), &trace),
        None => next.run(req).await,
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
        if let Some((_, why)) = refused {
            event = event.attr("reason", why);
        }
        s.writer.record_now(event).await;
    }
    if let Ok(value) = ids::reference(&trace).parse() {
        response.headers_mut().insert("x-gathr-ref", value);
    }
    response
}

/// Why a request is refused, if it is: a foreign Host or Origin (403), or no valid token (401).
fn refusal(s: &State, headers: &HeaderMap, uri: &Uri) -> Option<(StatusCode, &'static str)> {
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
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| live_token(uri));
    match token {
        None => Some((StatusCode::UNAUTHORIZED, "token missing")),
        Some(t) if !same(t.as_bytes(), s.token.as_bytes()) => {
            Some((StatusCode::UNAUTHORIZED, "token wrong"))
        }
        Some(_) => None,
    }
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
    (status, Json(body)).into_response()
}
