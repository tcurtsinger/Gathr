//! Where the person left off (0077): the window keeps what the person has seen in each thread, and
//! their folds, with PUT; the person's other windows hear it live. A refusal is a 409 with why and a
//! reference.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::routing::put;
use axum::{Json, Router};
use gathr::reads::{self, Mark};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/chats/{chat}/reads", put(mark))
}

#[derive(Deserialize)]
struct Marks {
    reads: Vec<Mark>,
}

/// Keeps the marks: each thread's row as kept, also sent live to every window.
async fn mark(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Json(body): Json<Marks>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let id = chat.clone();
    let Json(answer) = db(&s, &t, "Marks failed", move |conn| {
        let tx = conn.transaction()?;
        let kept = reads::mark(&tx, &id, &body.reads)?;
        tx.commit()?;
        Ok(json!({ "reads": kept }))
    })
    .await?;
    s.chats
        .publish(json!({ "type": "reads", "chat": chat, "reads": answer["reads"] }));
    Ok(Json(answer))
}
