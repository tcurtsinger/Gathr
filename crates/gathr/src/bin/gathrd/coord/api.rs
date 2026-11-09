//! Tasks and waits for the person (0010): a chat's tasks and waits, and the person's cancel and
//! restart. Screens come with F8b and F9.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, Path, Query, State as Extract};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use gathr::event::Event;
use gathr::tasks;
use gathr::tasks::{Start, person, stale};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/tasks", get(list))
        .route("/api/tasks/{task}/cancel", post(cancel))
        .route("/api/tasks/{task}/restart", post(restart))
        .route("/api/person-tasks", get(person_open))
        .route("/api/person-tasks/answer", post(person_answer))
        .route("/api/tasks/{task}/files/{index}", get(file))
        .route("/api/settings", get(settings).patch(change_settings))
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

/// Settings > General's stale days (F11): after how many days without a move a task is stale, and
/// cancelled.
async fn settings(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    db(&s, &t, "Settings failed", |c| shown(c)).await
}

#[derive(Deserialize)]
pub struct Change {
    stale_days: Option<i64>,
    cancel_days: Option<i64>,
}

async fn change_settings(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Change>,
) -> Answer {
    let trace = t.0.clone();
    db(&s, &t, "Save failed", move |c| {
        let was = stale::days(c)?;
        let days = stale::Days {
            stale: body.stale_days.unwrap_or(was.stale),
            cancel: body.cancel_days.unwrap_or(was.cancel),
        };
        let saved = stale::set_days(c, days);
        let event = Event::point("gathr.settings.change").trace(&trace);
        let event = match &saved {
            Ok(()) => event.ok(),
            Err(e) => event.error("refused").attr("why", e.to_string()),
        };
        event
            .attr("stale_days", days.stale)
            .attr("cancel_days", days.cancel)
            .insert(c)?;
        saved?;
        shown(c)
    })
    .await
}

fn shown(c: &rusqlite::Connection) -> Result<serde_json::Value, gathr::Error> {
    let days = stale::days(c)?;
    Ok(json!({ "stale_days": days.stale, "cancel_days": days.cancel }))
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
    let starts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = starts.clone();
    let answer = db(&s, &t, "Answer failed", move |c| {
        let sent = person::answer(c, &body.answers, &trace)?;
        *kept.lock().expect("never poisoned") = sent.starts;
        let reference = gathr::ids::reference(&trace);
        let refused: Vec<_> = sent
            .refused
            .iter()
            .map(|(task, why)| json!({ "task": task, "why": why, "ref": reference }))
            .collect();
        Ok(json!({ "answered": sent.answered, "refused": refused }))
    })
    .await;
    s.chats
        .start(std::mem::take(&mut *starts.lock().expect("never poisoned")));
    answer
}

async fn list(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    db(&s, &t, "Tasks failed", move |c| super::listed(c, &chat)).await
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
