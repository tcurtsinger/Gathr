//! A page in a chat's folders as it looks (`gathr::view`): the window asks for a view of it, and its
//! sealed frame loads the page and the files beside it by the view's key alone, outside `/api`.

use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::view::Views;
use serde_json::json;

use crate::api::{State, Trace};
use crate::files::{Shown, tree_of};
use crate::routes::{Answer, Fail, fail};

static VIEWS: Views = Views::new();

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/view", post(open))
        .route("/view/{key}/{*rest}", get(serve))
}

/// A view of the file at `path` in the chat's folders: the address the frame opens it at.
async fn open(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Answer {
    let roots = tree_of(&s, &t, &chat, "Page failed").await?.roots();
    let opened = tokio::task::spawn_blocking(move || VIEWS.open(&roots, &q.path)).await;
    let Ok(Some((key, name))) = opened else {
        let why = "it isn't a file in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Page failed", why, &t.0));
    };
    let name: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{b:02X}"),
        })
        .collect();
    let address = format!("http://127.0.0.1:{}/view/{key}/{name}", s.port);
    Ok(Json(json!({ "address": address })))
}

/// A file of a view's folder, sealed: run only as a sandboxed page with no origin, loading only
/// from its own folder under this key and from `https:`, never kept.
async fn serve(
    Extension(t): Extension<Trace>,
    Path((key, rest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Fail> {
    let k = key.clone();
    let found = tokio::task::spawn_blocking(move || VIEWS.file(&k, &rest)).await;
    let read = match found {
        Ok(Some(path)) => tokio::fs::read(&path).await.ok().map(|bytes| (path, bytes)),
        _ => None,
    };
    // Only the kinds of file a page loads (pages, styles, scripts, pictures, fonts, media, data):
    // whatever else sits beside it stays unread.
    let known =
        |path: &std::path::Path| crate::ui::content_type(path) != "application/octet-stream";
    let Some((path, bytes)) = read.filter(|(path, _)| known(path)) else {
        let why = "it isn't a file of this page";
        return Err(fail(StatusCode::NOT_FOUND, "Page failed", why, &t.0));
    };
    // The guard let in only this service's own Host. A sandboxed page's origin is opaque, so its
    // own folder is named in full rather than as 'self'.
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let policy = format!(
        "sandbox allow-scripts; default-src http://{host}/view/{key}/ https: data: blob: \
         'unsafe-inline' 'unsafe-eval'"
    );
    let sealed = [
        (
            header::CONTENT_TYPE,
            crate::ui::content_type(&path).to_string(),
        ),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
        (header::CACHE_CONTROL, "no-store".into()),
        (header::CONTENT_SECURITY_POLICY, policy),
        // Its fonts and fetches go with `Origin: null`; the key, not the origin, is the permission.
        (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".into()),
    ];
    Ok((sealed, bytes).into_response())
}
