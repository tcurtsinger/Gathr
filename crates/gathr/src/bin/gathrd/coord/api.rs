//! Tasks and waits for the person (0010): a chat's tasks and waits, and the person's cancel and
//! restart. Screens come with F8b and F9.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use gathr::tasks;
use gathr::tasks::{Start, person};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/tasks", get(list))
        .route("/api/chats/{chat}/waiting", get(waiting))
        .route("/api/tasks/{task}/cancel", post(cancel))
        .route("/api/tasks/{task}/restart", post(restart))
        .route("/api/asks/{ask}/size", post(resize))
        .route("/api/person-tasks", get(person_open))
        .route("/api/person-tasks/answer", post(person_answer))
        .route("/api/tasks/{task}/files/{index}", get(file))
}

/// A file given with a task, as copied then. An agent wrote it, so a page never runs as Gathr's:
/// the sandbox gives it no scripts and an origin of its own.
async fn file(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path((task, index)): Path<(i64, usize)>,
) -> Result<Response, Fail> {
    let Json(found) = db(&s, &t, "File failed", move |c| {
        let task = tasks::get(c, task)?;
        let file = task.and_then(|t| t.files.get(index).cloned());
        Ok(json!(file.map(|f| f.path)))
    })
    .await?;
    let missing = || {
        fail(
            StatusCode::NOT_FOUND,
            "File failed",
            "there's no such file",
            &t.0,
        )
    };
    let path = std::path::PathBuf::from(found.as_str().ok_or_else(missing)?);
    let bytes = tokio::fs::read(&path).await.map_err(|e| {
        let why = format!("the copy can't be read: {e}");
        fail(StatusCode::NOT_FOUND, "File failed", &why, &t.0)
    })?;
    let headers = [
        (header::CONTENT_TYPE, crate::ui::content_type(&path)),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (header::CONTENT_SECURITY_POLICY, "sandbox"),
    ];
    Ok((headers, bytes).into_response())
}

#[derive(Deserialize)]
pub struct InChat {
    chat: Option<String>,
}

/// The person's open tasks across chats (or in `?chat=`), oldest first.
async fn person_open(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Query(q): Query<InChat>,
) -> Answer {
    db(&s, &t, "Tasks failed", move |c| {
        super::person_listed(c, q.chat.as_deref())
    })
    .await
}

#[derive(Deserialize)]
pub struct Answers {
    answers: Vec<person::Answer>,
}

/// The pager's Send: each answer is answered or refused on its own; a refused one stays open and
/// carries a reference that finds why.
async fn person_answer(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Answers>,
) -> Answer {
    let trace = t.0.clone();
    let picked = crate::copies::discussed(&s, &t, &body.answers).await?;
    let after = Arc::new(std::sync::Mutex::new((Vec::new(), false)));
    let kept = after.clone();
    let answer = db(&s, &t, "Answer failed", move |c| {
        let sent = person::answer(c, &body.answers, &trace)?;
        // Gathr's own question answered (0054): its PR may land now, not at the next look.
        let gathrs = sent.answered.iter().any(|t| t.giver.is_none());
        *kept.lock().expect("never poisoned") = (sent.starts, gathrs);
        let reference = gathr::ids::reference(&trace);
        let refused: Vec<_> = sent
            .refused
            .iter()
            .map(|(task, why)| json!({ "task": task, "why": why, "ref": reference }))
            .collect();
        Ok(json!({ "answered": sent.answered, "refused": refused }))
    })
    .await;
    let (starts, gathrs) = std::mem::take(&mut *after.lock().expect("never poisoned"));
    s.chats.start(starts);
    if gathrs {
        crate::landing::soon(&s.chats, None);
    }
    let Json(mut answer) = answer?;
    if !picked.is_empty() {
        let refused = answer["refused"].as_array().into_iter().flatten();
        let refused: Vec<i64> = refused.filter_map(|r| r["task"].as_i64()).collect();
        answer["turns"] = json!(crate::copies::give_discussed(&s, &t, picked, &refused).await);
    }
    Ok(Json(answer))
}

async fn list(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    db(&s, &t, "Tasks failed", move |c| super::listed(c, &chat)).await
}

/// What the chat waits on now (0050): every line with who can end it and since when, where each
/// agent stands, the same in words, and the service's clock for the ages.
async fn waiting(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let live = super::live_now(&s.chats, &chat);
    db(&s, &t, "Waiting failed", move |c| {
        let now = gathr::ids::now_us();
        let answer = gathr::waiting::answer(c, &chat, &live, now)?;
        Ok(gathr::waiting::shown(&answer, now, "service"))
    })
    .await
}

#[derive(Deserialize)]
pub struct Why {
    why: Option<String>,
}

async fn cancel(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(task): Path<i64>,
    Json(body): Json<Why>,
) -> Answer {
    let trace = t.0.clone();
    changed(&s, &t, "Cancel failed", task, move |c| {
        let why = body.why.as_deref().unwrap_or("cancelled by the person");
        tasks::cancel(c, None, task, why, &trace)
    })
    .await
}

async fn restart(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(task): Path<i64>,
) -> Answer {
    let trace = t.0.clone();
    changed(&s, &t, "Restart failed", task, move |c| {
        tasks::restart(c, None, task, &trace)
    })
    .await
}

#[derive(Deserialize)]
pub struct Size {
    size: String,
}

/// The person changes an ask's size (0061): the ask as the chat's tasks list has it. The turns the
/// change queues start: the agent told of it, and tasks back in line from waiting for a plan (0063).
async fn resize(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(ask): Path<String>,
    Json(body): Json<Size>,
) -> Answer {
    let trace = t.0.clone();
    let starts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = starts.clone();
    let answer = db(&s, &t, "Size failed", move |c| {
        let (ask, started) = gathr::asks::resize(c, &ask, &body.size, &trace)?;
        *kept.lock().expect("never poisoned") = started;
        Ok(json!(ask))
    })
    .await;
    s.chats
        .start(std::mem::take(&mut *starts.lock().expect("never poisoned")));
    answer
}

/// Makes the change, answers with the task as it is now, and queues the turns it starts. Turns
/// are queued here, off the database's thread, where they can be spawned.
async fn changed(
    s: &Arc<State>,
    t: &Trace,
    error: &'static str,
    task: i64,
    change: impl FnOnce(&mut rusqlite::Connection) -> Result<Vec<Start>, gathr::Error> + Send + 'static,
) -> Answer {
    let starts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = starts.clone();
    let answer = db(s, t, error, move |c| {
        *kept.lock().expect("never poisoned") = change(c)?;
        Ok(json!(tasks::get(c, task)?))
    })
    .await;
    s.chats
        .start(std::mem::take(&mut *starts.lock().expect("never poisoned")));
    answer
}
