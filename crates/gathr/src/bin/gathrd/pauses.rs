//! Pause, resume and Stop now for the person (0010, F10): the whole app, one chat or one agent.
//! Restart is the resume of a stopped scope: its carry-ons start in the same sessions.

use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Extension, State as Extract};
use axum::routing::{get, post};
use gathr::pauses;
use serde::Deserialize;

use crate::api::{State, Trace};
use crate::routes::{Answer, db};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/pauses", get(list).post(pause))
        .route("/api/pauses/resume", post(resume))
        .route("/api/pauses/stop", post(stop_now))
}

/// A scope: `app`, or `chat` or `agent` with its id as `target`.
#[derive(Deserialize)]
pub struct Scope {
    scope: String,
    #[serde(default)]
    target: String,
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let checks = s.chats.checking();
    db(&s, &t, "Pauses failed", move |c| pauses::listed(c, &checks)).await
}

async fn pause(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Scope>,
) -> Answer {
    changed(&s, &t, "Pause failed", body, false).await
}

async fn resume(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Scope>,
) -> Answer {
    let (trace, checks) = (t.0.clone(), s.chats.checking());
    let answer = db(&s, &t, "Resume failed", move |c| {
        pauses::resume(c, &body.scope, &body.target, &trace)?;
        pauses::listed(c, &checks)
    })
    .await;
    s.chats.pauses_changed().await;
    answer
}

/// Stop now: the scope is paused as stopped first, so nothing starts while what runs is ended.
async fn stop_now(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Scope>,
) -> Answer {
    let (scope, target) = (body.scope.clone(), body.target.clone());
    let _paused = changed(&s, &t, "Stop failed", body, true).await?;
    s.chats.stop_now(&scope, &target).await;
    let checks = s.chats.checking();
    db(&s, &t, "Stop failed", move |c| pauses::listed(c, &checks)).await
}

async fn changed(
    s: &Arc<State>,
    t: &Trace,
    error: &'static str,
    body: Scope,
    stopped: bool,
) -> Answer {
    let (trace, checks) = (t.0.clone(), s.chats.checking());
    let answer = db(s, t, error, move |c| {
        pauses::pause(c, &body.scope, &body.target, stopped, &trace)?;
        pauses::settle(c, &checks, &trace)?;
        pauses::listed(c, &checks)
    })
    .await;
    s.chats.pauses_changed().await;
    answer
}
