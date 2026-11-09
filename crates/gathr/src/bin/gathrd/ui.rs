//! The built UI, served from the service's own port (0012), and the one-time link that lets a
//! browser in: the window asks for a code, opens `/?once=<code>`, and the service swaps it for
//! an HTTP-only cookie named after its port, since cookies aren't kept apart by port (0004).

use std::path::{Component, Path};
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Query, Request, State as Extract};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Extension, Json, Router};
use gathr::ids;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace, failure};

/// Long enough for a window or browser to open it, short enough that a leaked one is stale.
const ONCE_FOR: Duration = Duration::from_secs(60);

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/once", post(once))
}

/// The cookie that stands in for the token, named for this port.
pub fn cookie_name(port: u16) -> String {
    format!("gathr-{port}")
}

/// A new one-time code for `/?once=<code>`.
async fn once(Extract(s): Extract<Arc<State>>) -> Json<serde_json::Value> {
    let code = ids::token();
    let mut codes = s.once_codes.lock().unwrap_or_else(|e| e.into_inner());
    codes.retain(|(_, made)| made.elapsed() < ONCE_FOR);
    codes.push((code.clone(), Instant::now()));
    Json(json!({ "code": code }))
}

#[derive(Deserialize)]
pub struct Page {
    once: Option<String>,
}

/// Any path outside the API: a one-time link, else a file of the built UI.
pub async fn page(
    Extract(s): Extract<Arc<State>>,
    Extension(Trace(trace)): Extension<Trace>,
    Query(page): Query<Page>,
    req: Request,
) -> Response {
    if let Some(code) = page.once {
        return swap(&s, &code, &trace);
    }
    let path = match req.uri().path() {
        "/" => "index.html",
        path => path.trim_start_matches('/'),
    };
    let inside = Path::new(path)
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    let file = s.ui.join(path);
    let read = match inside {
        true => tokio::fs::read(&file).await.ok(),
        false => None,
    };
    match read {
        Some(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, content_type(&file).parse().unwrap());
            if path == "index.html" {
                headers.insert(header::CACHE_CONTROL, "no-cache".parse().unwrap());
            }
            (headers, bytes).into_response()
        }
        // A browser asks for one on any page, the error pages too; it isn't a failure.
        None if path == "favicon.ico" => StatusCode::NO_CONTENT.into_response(),
        _ => failure(
            StatusCode::NOT_FOUND,
            "Page failed",
            Some("no such page"),
            &trace,
        ),
    }
}

/// Swaps a fresh code for the cookie and sends the browser on to `/`, so the code leaves its history.
fn swap(s: &State, code: &str, trace: &str) -> Response {
    let mut codes = s.once_codes.lock().unwrap_or_else(|e| e.into_inner());
    let found = codes
        .iter()
        .position(|(c, made)| c == code && made.elapsed() < ONCE_FOR);
    let Some(at) = found else {
        let why = "the link was used or has expired";
        return failure(
            StatusCode::UNAUTHORIZED,
            "Sign-in link failed",
            Some(why),
            trace,
        );
    };
    codes.remove(at);
    let cookie = format!(
        "{}={}; Path=/; HttpOnly; SameSite=Strict",
        cookie_name(s.port),
        s.session
    );
    let headers = [
        (header::LOCATION, "/".to_string()),
        (header::SET_COOKIE, cookie),
    ];
    (StatusCode::SEE_OTHER, headers).into_response()
}

pub fn content_type(file: &Path) -> &'static str {
    match file.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "pdf" => "application/pdf",
        "js" => "text/javascript",
        "css" => "text/css",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}
