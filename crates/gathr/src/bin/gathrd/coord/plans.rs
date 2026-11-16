//! A plan for the person (0062): read whole, approved, sent back or dropped. A refusal is a 409
//! with its error and a reference.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, Path, State as Extract};
use axum::routing::{get, post};
use gathr::Error;
use gathr::plans::{self, Decided};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, db};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/plans/{plan}", get(read))
        .route("/api/plans/{plan}/approve", post(approve))
        .route("/api/plans/{plan}/send-back", post(send_back))
        .route("/api/plans/{plan}/drop", post(drop))
}

#[derive(Deserialize)]
pub struct Version {
    version: i64,
}

#[derive(Deserialize)]
pub struct SendBack {
    version: i64,
    words: String,
}

async fn read(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(plan): Path<i64>,
) -> Answer {
    db(&s, &t, "Plan failed", move |c| {
        plans::get(c, plan)?.ok_or_else(|| Error::Refused(format!("there's no plan #{plan}")))
    })
    .await
}

async fn approve(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(plan): Path<i64>,
    Json(body): Json<Version>,
) -> Answer {
    let trace = t.0.clone();
    decided(&s, &t, "Approve failed", move |c| {
        plans::approve(c, plan, body.version, &trace)
    })
    .await
}

async fn send_back(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(plan): Path<i64>,
    Json(body): Json<SendBack>,
) -> Answer {
    let trace = t.0.clone();
    decided(&s, &t, "Send back failed", move |c| {
        plans::send_back(c, plan, body.version, &body.words, &trace)
    })
    .await
}

async fn drop(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(plan): Path<i64>,
    Json(body): Json<Version>,
) -> Answer {
    let trace = t.0.clone();
    decided(&s, &t, "Drop failed", move |c| {
        plans::drop(c, plan, body.version, &trace)
    })
    .await
}

/// The person's word: the plan's summary, and the turns it starts queued off the database's thread.
async fn decided(
    s: &Arc<State>,
    t: &Trace,
    error: &'static str,
    decide: impl FnOnce(&mut rusqlite::Connection) -> Result<Decided, Error> + Send + 'static,
) -> Answer {
    let starts = Arc::new(std::sync::Mutex::new(Vec::new()));
    let kept = starts.clone();
    let answer = db(s, t, error, move |c| {
        let decided = decide(c)?;
        *kept.lock().expect("never poisoned") = decided.starts;
        Ok(json!(decided.summary))
    })
    .await;
    s.chats
        .start(std::mem::take(&mut *starts.lock().expect("never poisoned")));
    answer
}
