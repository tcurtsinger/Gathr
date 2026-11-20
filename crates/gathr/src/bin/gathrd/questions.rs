//! The person's answer to a room chat's question (`gathr::questions`): a pick of its choices, their own
//! words or both. It starts the asking agent's turn with the answer, sends the chat's questions live as
//! `questions`, and the chat stops needing the person once none is open.

use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Path, State as Extract};
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/questions/{id}/answer", post(answer))
}

#[derive(Deserialize)]
struct Given {
    pick: Option<String>,
    words: Option<String>,
}

async fn answer(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<i64>,
    Json(body): Json<Given>,
) -> Answer {
    let after: Arc<Mutex<Option<(gathr::store::Start, Value)>>> = Arc::default();
    let kept = after.clone();
    let answered = db(&s, &t, "Answer failed", move |c| {
        let tx = c.transaction()?;
        let pick = body.pick.as_deref();
        let (question, start) = gathr::questions::answer(&tx, id, pick, body.words.as_deref())?;
        let questions = gathr::questions::of_chat(&tx, &start.chat)?;
        tx.commit()?;
        let turn = start.turn.clone();
        *kept.lock().expect("never poisoned") = Some((start, questions));
        Ok(json!({ "question": question, "turn": turn }))
    })
    .await?;
    let taken = after.lock().expect("never poisoned").take();
    if let Some((start, questions)) = taken {
        let chat = start.chat.clone();
        s.chats
            .publish(json!({ "type": "questions", "chat": chat, "questions": questions }));
        s.chats.start(vec![start]);
        crate::chat_state::changed(&s.chats, &chat, &t.0).await;
    }
    Ok(answered)
}
