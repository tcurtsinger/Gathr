//! The person's like on a message (0057): given with PUT, taken back with DELETE. A refusal is a 409
//! with why and a reference.

use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::put;
use axum::{Json, Router};
use gathr::likes::{self, Delivery};
use gathr::tasks::Start;
use serde::Deserialize;
use serde_json::json;
use tokio::sync::oneshot;

use crate::api::{State, Trace};
use crate::chats::Cmd;
use crate::routes::{Answer, db, fail, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route(
        "/api/chats/{chat}/messages/{message}/like",
        put(give).delete(take_back),
    )
}

#[derive(Deserialize, Default)]
struct Give {
    /// The waiting questions the message's bubble holds: the like answers each with its pick.
    #[serde(default)]
    questions: Vec<i64>,
}

/// Gives the like: the reaction, what it did, and the turn it started or joined when it reached the
/// agent.
async fn give(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((chat, message)): Path<(String, String)>,
    body: Option<Json<Give>>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let questions = body.map(|Json(b)| b.questions).unwrap_or_default();
    let after: Arc<Mutex<(Vec<Start>, Option<Delivery>)>> = Arc::default();
    let (kept, id, trace) = (after.clone(), chat.clone(), t.0.clone());
    let Json(mut answer) = db(&s, &t, "Like failed", move |conn| {
        let given = likes::give(conn, &id, &message, &questions, &trace)?;
        *kept.lock().expect("never poisoned") = (given.starts, given.deliver);
        Ok(json!({ "reaction": given.reaction, "did": given.did }))
    })
    .await?;
    let (starts, deliver) = std::mem::take(&mut *after.lock().expect("never poisoned"));
    s.chats.start(starts);
    if answer["did"] != "standing" {
        let reaction = answer["reaction"].clone();
        s.chats
            .publish(json!({ "type": "reaction", "chat": chat, "reaction": reaction }));
    }
    let Some(d) = deliver else {
        return Ok(Json(answer));
    };
    let (reply, told) = oneshot::channel();
    let task = s.chats.task(&chat, Some(&d.agent));
    let trace = t.0.clone();
    let like = Cmd::Like {
        chat,
        liked: d.liked,
        line: d.line,
        trace,
        reply,
    };
    let _ = task.send(like);
    let error = StatusCode::INTERNAL_SERVER_ERROR;
    match told.await {
        Ok(Ok(turn)) => {
            answer["turn"] = turn;
            Ok(Json(answer))
        }
        Ok(Err(failed)) => Err(fail(error, "Like failed", &failed.why, &failed.trace)),
        Err(_) => Err(fail(error, "Like failed", "the agent's task ended", &t.0)),
    }
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
