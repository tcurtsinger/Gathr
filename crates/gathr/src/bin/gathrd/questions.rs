//! The person's answer to a room chat's question (`gathr::questions`): a pick of its choices, their own
//! words or both. It starts the asking agent's turn with the answer, sends the chat's questions live as
//! `questions`, and the chat stops needing the person once none is open.

use std::sync::{Arc, Mutex};

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use rusqlite::OptionalExtension;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/questions/{id}/answer", post(answer))
}

#[derive(Deserialize)]
struct Given {
    pick: Option<String>,
    words: Option<String>,
    /// Files of the question's chat, by their saved names: a line each after the words.
    #[serde(default)]
    files: Vec<String>,
    /// For several questions asked together: an answer to each, in order, and no pick, words or files above.
    answers: Option<Vec<Part>>,
}

#[derive(Deserialize)]
struct Part {
    pick: Option<String>,
    words: Option<String>,
    #[serde(default)]
    files: Vec<String>,
}

async fn answer(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<i64>,
    Json(body): Json<Given>,
) -> Answer {
    let mut words = body.words.unwrap_or_default();
    let has_files =
        !body.files.is_empty() || (body.answers.iter().flatten()).any(|a| !a.files.is_empty());
    let mut answers: Option<Vec<gathr::questions::Given>> = None;
    if has_files {
        let Json(chat) = db(&s, &t, "Answer failed", move |c| {
            let chat: Option<String> = c
                .query_row("SELECT chat_id FROM questions WHERE id = ?1", [id], |r| {
                    r.get(0)
                })
                .optional()?;
            Ok(json!(chat))
        })
        .await?;
        let Some(chat) = chat.as_str().map(String::from) else {
            let why = "there's no such question.";
            return Err(fail(StatusCode::CONFLICT, "Answer failed", why, &t.0));
        };
        let lines = crate::files::lines(&s, &t, &chat, &body.files, "Answer failed").await?;
        words = gathr::files::with_lines(&words, &lines);
        if let Some(parts) = &body.answers {
            let mut given = Vec::new();
            for p in parts {
                let lines = crate::files::lines(&s, &t, &chat, &p.files, "Answer failed").await?;
                let words =
                    gathr::files::with_lines(p.words.as_deref().unwrap_or_default(), &lines);
                given.push(gathr::questions::Given {
                    pick: p.pick.clone(),
                    words: Some(words),
                });
            }
            answers = Some(given);
        }
    }
    if answers.is_none() {
        answers = body.answers.as_ref().map(|parts| {
            (parts.iter())
                .map(|p| gathr::questions::Given {
                    pick: p.pick.clone(),
                    words: p.words.clone(),
                })
                .collect()
        });
    }
    let after: Arc<Mutex<Option<(gathr::store::Start, Value)>>> = Arc::default();
    let kept = after.clone();
    let answered = db(&s, &t, "Answer failed", move |c| {
        let tx = c.transaction()?;
        let pick = body.pick.as_deref();
        let (question, start) = match &answers {
            Some(all) => gathr::questions::answer_all(&tx, id, all)?,
            None => gathr::questions::answer(&tx, id, pick, Some(&words))?,
        };
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
