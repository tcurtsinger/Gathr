//! Tasks and waits for the person (0010): a chat's tasks and waits, and the person's cancel and
//! restart. Screens come with F8b and F9.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, Path, State as Extract};
use axum::routing::{get, post};
use gathr::tasks;
use gathr::tasks::Start;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db, known};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/tasks", get(list))
        .route("/api/tasks/{task}/cancel", post(cancel))
        .route("/api/tasks/{task}/restart", post(restart))
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
