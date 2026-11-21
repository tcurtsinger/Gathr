//! A PDF's bytes and a workbook's sheets for a file's tab in the window (`gathr::documents`).

use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use gathr::documents;
use gathr::files::Unshown;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::files::{safe, tree_of};
use crate::routes::{Fail, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/pdf", get(pdf))
        .route("/api/chats/{chat}/sheets", get(sheets))
}

#[derive(Deserialize)]
struct Shown {
    path: String,
}

/// Why a file isn't shown, as the window reads it: 404 when it isn't a file in this chat's folders,
/// 413 over the limit, 415 when it can't be read.
fn unshown(read: Option<Unshown>, error: &'static str, t: &Trace) -> Fail {
    let (status, why) = match read {
        Some(Unshown::Gone) => (
            StatusCode::NOT_FOUND,
            "it isn't a file in this chat's folders",
        ),
        Some(Unshown::Large) => (StatusCode::PAYLOAD_TOO_LARGE, "it's too large to show"),
        Some(Unshown::NotText) | None => (StatusCode::UNSUPPORTED_MEDIA_TYPE, "it can't be read"),
    };
    fail(status, error, why, &t.0)
}

/// A PDF's bytes, at most `documents::PDF`.
async fn pdf(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Response, Fail> {
    let tree = tree_of(&s, &t, &chat, "PDF failed").await?;
    let read = tokio::task::spawn_blocking(move || documents::pdf(&tree.roots(), &q.path)).await;
    match read {
        Ok(Ok(bytes)) => Ok((safe("application/pdf"), bytes).into_response()),
        Ok(Err(why)) => Err(unshown(Some(why), "PDF failed", &t)),
        Err(_) => Err(unshown(None, "PDF failed", &t)),
    }
}

/// A workbook's visible sheets, `{"sheets": [{"name", "rows", "total"}]}`, at most
/// `documents::ROWS` rows each. A reader that panics on a damaged file is a 415, as one that fails.
async fn sheets(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Query(q): Query<Shown>,
) -> Result<Json<serde_json::Value>, Fail> {
    let tree = tree_of(&s, &t, &chat, "Sheets failed").await?;
    let read = tokio::task::spawn_blocking(move || documents::sheets(&tree.roots(), &q.path)).await;
    match read {
        Ok(Ok(sheets)) => Ok(Json(json!({ "sheets": sheets }))),
        Ok(Err(why)) => Err(unshown(Some(why), "Sheets failed", &t)),
        Err(_) => Err(unshown(None, "Sheets failed", &t)),
    }
}
