//! The person's like on a message (0057): given with PUT, taken back with DELETE. A refusal is a 409
//! with why and a reference.

use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::routing::put;
use axum::{Json, Router};
use gathr::likes;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route(
        "/api/chats/{chat}/messages/{message}/like",
        put(give).delete(take_back),
    )
}

/// Gives the like: the reaction, and what it did.
async fn give(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, message)): Path<(String, String)>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (id, trace) = (chat.clone(), t.0.clone());
    let Json(answer) = db(&s, &t, "Like failed", move |conn| {
        let given = likes::give(conn, &id, &message, &trace)?;
        Ok(json!({ "reaction": given.reaction, "did": given.did }))
    })
    .await?;
    if answer["did"] != "standing" {
        let reaction = answer["reaction"].clone();
        s.chats
            .publish(json!({ "type": "reaction", "chat": chat, "reaction": reaction }));
    }
    Ok(Json(answer))
}

/// Takes the like back: the mark goes, and nothing it did is undone.
async fn take_back(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, message)): Path<(String, String)>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (id, trace) = (chat.clone(), t.0.clone());
    let Json(answer) = db(&s, &t, "Like failed", move |conn| {
        let removed = likes::take_back(conn, &id, &message, &trace)?;
        Ok(json!({ "removed": removed.is_some(), "message": removed }))
    })
    .await?;
    if answer["removed"] == true {
        let reaction = json!({ "message": answer["message"], "agent": likes::PERSON });
        s.chats.publish(
            json!({ "type": "reaction", "chat": chat, "reaction": reaction, "removed": true }),
        );
    }
    Ok(Json(answer))
}
