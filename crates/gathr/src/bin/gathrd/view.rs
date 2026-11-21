//! A page as it looks (`gathr::view`): the window asks for a view of a page in a chat's folders, or
//! in Designs for the Designs screen, and its sealed frame loads the page and the files beside it by
//! the view's key alone, outside `/api`; a design's tab in a chat asks with `notes`, and its page is
//! served with the script for notes. The Designs screen's list of every design is here too.

use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::designs;
use gathr::view::Views;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::files::{Shown, tree_of};
use crate::routes::{Answer, Fail, fail};

static VIEWS: Views = Views::new();
/// The Designs screen's views, kept apart: its many cards never let a page open in a chat's tab go.
static GALLERY: Views = Views::new();

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/view", post(open))
        .route("/api/designs", get(list))
        .route("/api/designs/view", post(open_design))
        .route("/view/{key}/{*rest}", get(serve))
}

#[derive(Deserialize)]
struct Opening {
    path: String,
    /// For a design's tab in the chat: the page is served with the script for notes.
    #[serde(default)]
    notes: bool,
}

/// A view of the file at `path` in the chat's folders: the address the frame opens it at.
async fn open(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Opening>,
) -> Answer {
    let tree = tree_of(&s, &t, &chat, "Page failed").await?;
    let roots = tree.roots();
    let designs = tree.designs.map(|d| d.path);
    let opened = tokio::task::spawn_blocking(move || match (q.notes, designs) {
        (true, Some(designs)) => VIEWS.note(&designs, &q.path),
        (true, None) => None,
        (false, _) => VIEWS.open(&roots, &q.path),
    })
    .await;
    let Ok(Some((key, name))) = opened else {
        let why = "it isn't a file in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Page failed", why, &t.0));
    };
    Ok(Json(json!({ "address": address(s.port, &key, &name) })))
}

/// A view of the page at `path` in Designs, one key per design while it is kept.
async fn open_design(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Query(q): Query<Shown>,
) -> Answer {
    let data = s.data.path().to_path_buf();
    let found = s.writer.call(move |conn| designs::folder(conn, &data));
    let root = answered(found.await, "Page failed", &t)?;
    let opened = tokio::task::spawn_blocking(move || GALLERY.reopen(&[root], &q.path)).await;
    let Ok(Some((key, name))) = opened else {
        let why = "it isn't a page in Designs";
        return Err(fail(StatusCode::NOT_FOUND, "Page failed", why, &t.0));
    };
    Ok(Json(json!({ "address": address(s.port, &key, &name) })))
}

/// Every design, by its part of Designs.
async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let data = s.data.path().to_path_buf();
    let found = s.writer.call(move |conn| {
        Ok::<_, gathr::Error>((designs::folder(conn, &data)?, designs::owners(conn)?))
    });
    let (root, owners) = answered(found.await, "Designs failed", &t)?;
    let parts = tokio::task::spawn_blocking(move || designs::list(&root, &owners))
        .await
        .unwrap_or_default();
    Ok(Json(json!({ "parts": parts })))
}

fn answered<T>(
    found: Option<Result<T, gathr::Error>>,
    error: &'static str,
    t: &Trace,
) -> Result<T, Fail> {
    let why = match found {
        Some(Ok(found)) => return Ok(found),
        Some(Err(e)) => e.to_string(),
        None => "the database didn't answer".into(),
    };
    Err(fail(StatusCode::INTERNAL_SERVER_ERROR, error, &why, &t.0))
}

/// Where the frame opens the page `name` of the view `key`.
fn address(port: u16, key: &str, name: &str) -> String {
    let name: String = name
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{b:02X}"),
        })
        .collect();
    format!("http://127.0.0.1:{port}/view/{key}/{name}")
}

/// A file of a view's folder, sealed: run only as a sandboxed page with no origin, loading only
/// from its own folder under this key and from `https:`, never kept.
async fn serve(
    Extension(t): Extension<Trace>,
    Path((key, rest)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<Response, Fail> {
    let k = key.clone();
    let found = tokio::task::spawn_blocking(move || {
        VIEWS.file(&k, &rest).or_else(|| GALLERY.file(&k, &rest))
    })
    .await;
    let read = match found {
        Ok(Some((path, noted))) => tokio::fs::read(&path).await.ok().map(|mut bytes| {
            // After the page's own end: the browser still runs it, after the page's own scripts.
            if noted {
                bytes.extend_from_slice(gathr::view::NOTES.as_bytes());
            }
            (path, bytes)
        }),
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
