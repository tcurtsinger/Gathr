//! The person's files for a message, and the tree, words and pictures the window shows from a
//! chat's folders (`gathr::files`).

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State as Extract};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::files;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(
            "/api/chats/{chat}/files",
            post(upload)
                .layer(DefaultBodyLimit::max(files::MOST))
                .get(tree),
        )
        .route("/api/chats/{chat}/file", get(text))
        .route("/api/chats/{chat}/picture", get(picture))
}

#[derive(Deserialize)]
struct Named {
    name: String,
}

/// Saves the body as the file `name` in the chat's files folder: its saved name and path.
async fn upload(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Named>,
    body: Bytes,
) -> Answer {
    known(&s, &t, &chat).await?;
    let dir = folder(&s, &t, &chat, "Upload failed").await?;
    let saved = tokio::task::spawn_blocking(move || {
        let name = files::save(&dir, &q.name, &body)?;
        Ok::<_, gathr::Error>((dir.join(&name), name))
    })
    .await;
    match saved {
        Ok(Ok((path, name))) => Ok(Json(json!({ "name": name, "path": path }))),
        Ok(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Upload failed",
            &e.to_string(),
            &t.0,
        )),
        Err(e) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Upload failed",
            &e.to_string(),
            &t.0,
        )),
    }
}

/// The chat's files folder, recorded.
pub async fn folder(
    s: &State,
    t: &Trace,
    chat: &str,
    error: &'static str,
) -> Result<std::path::PathBuf, Fail> {
    let (id, data) = (chat.to_string(), s.data.path().to_path_buf());
    let Json(dir) = db(s, t, error, move |conn| {
        Ok(json!(files::folder(conn, &data, &id)?))
    })
    .await?;
    Ok(std::path::PathBuf::from(dir.as_str().unwrap_or_default()))
}

/// The lines naming `names`, the chat's saved files, refused when one isn't there.
pub async fn lines(
    s: &State,
    t: &Trace,
    chat: &str,
    names: &[String],
    error: &'static str,
) -> Result<String, Fail> {
    if names.is_empty() {
        return Ok(String::new());
    }
    let dir = folder(s, t, chat, error).await?;
    files::lines(&dir, names)
        .map_err(|e| fail(StatusCode::BAD_REQUEST, error, &e.to_string(), &t.0))
}

#[derive(Deserialize)]
struct Shown {
    path: String,
}

/// A picture from one of the chat's folders, for the window's tiles and viewer.
async fn picture(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Response, Fail> {
    known(&s, &t, &chat).await?;
    let (id, data) = (chat.clone(), s.data.path().to_path_buf());
    let Json(roots) = db(&s, &t, "Picture failed", move |conn| {
        Ok(json!(files::roots(conn, &data, &id)?))
    })
    .await?;
    let roots: Vec<std::path::PathBuf> = serde_json::from_value(roots).unwrap_or_default();
    let read = tokio::task::spawn_blocking(move || {
        let (path, kind) = files::picture(&roots, &q.path)?;
        Some((std::fs::read(path).ok()?, kind))
    })
    .await;
    let Ok(Some((bytes, kind))) = read else {
        let why = "it isn't a picture in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Picture failed", why, &t.0));
    };
    Ok((safe(kind), bytes).into_response())
}

/// A file's bytes go out as only what they say they are, never run, never kept.
fn safe(kind: &'static str) -> [(header::HeaderName, &'static str); 4] {
    [
        (header::CONTENT_TYPE, kind),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::CONTENT_SECURITY_POLICY,
            "sandbox; default-src 'none'",
        ),
        (header::CACHE_CONTROL, "no-store"),
    ]
}

/// The chat's tree as the window shows it: the folders of this chat only.
async fn tree_of(
    s: &State,
    t: &Trace,
    chat: &str,
    error: &'static str,
) -> Result<files::Tree, Fail> {
    known(s, t, chat).await?;
    let (id, data) = (chat.to_string(), s.data.path().to_path_buf());
    let Json(tree) = db(s, t, error, move |conn| {
        Ok(json!(files::tree(conn, &data, &id)?))
    })
    .await?;
    serde_json::from_value(tree).map_err(|e| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            &e.to_string(),
            &t.0,
        )
    })
}

#[derive(Deserialize)]
struct Folder {
    path: Option<String>,
}

/// Without a path, the tree's top folders; with one, that folder's folders and files.
async fn tree(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Folder>,
) -> Answer {
    let tree = tree_of(&s, &t, &chat, "Files failed").await?;
    let Some(path) = q.path else {
        return Ok(Json(json!(tree)));
    };
    let listed = tokio::task::spawn_blocking(move || files::listing(&tree.roots(), &path)).await;
    let Ok(Some(entries)) = listed else {
        let why = "it isn't a folder in this chat's folders";
        return Err(fail(StatusCode::NOT_FOUND, "Files failed", why, &t.0));
    };
    Ok(Json(json!({ "entries": entries })))
}

/// A file's words, for its tab in the window: 404 when it isn't a file in this chat's folders,
/// 413 over `files::SHOWN`, 415 when it isn't text.
async fn text(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Response, Fail> {
    let tree = tree_of(&s, &t, &chat, "File failed").await?;
    let read = tokio::task::spawn_blocking(move || files::text(&tree.roots(), &q.path)).await;
    let (status, why) = match read {
        Ok(Ok(words)) => return Ok((safe("text/plain; charset=utf-8"), words).into_response()),
        Ok(Err(files::Unshown::Gone)) | Err(_) => (
            StatusCode::NOT_FOUND,
            "it isn't a file in this chat's folders",
        ),
        Ok(Err(files::Unshown::Large)) => (StatusCode::PAYLOAD_TOO_LARGE, "it's too large to show"),
        Ok(Err(files::Unshown::NotText)) => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "it isn't text"),
    };
    Err(fail(status, "File failed", why, &t.0))
}
