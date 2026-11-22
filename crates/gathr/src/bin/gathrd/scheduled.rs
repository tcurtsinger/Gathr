//! Scheduled work (`gathr::schedules`): the Scheduled screen's routes, and the look every 30 seconds, and once as the
//! service starts, for what has come due. A run's turn queues as a turn Gathr starts does (`Chats::start`). Every
//! change is sent live as `schedules`.

use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::event::Event;
use gathr::schedules;
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, db, fail};

/// How often the service looks for what has come due.
const EVERY: Duration = Duration::from_secs(30);

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/schedules", get(list))
        .route("/api/schedules/{id}", get(one).patch(change).delete(remove))
        .route("/api/schedules/{id}/run", post(run_now))
}

/// Looks now, then every 30 seconds, while the service runs.
pub async fn every(chats: Arc<Chats>) {
    let mut tick = tokio::time::interval(EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        look(&chats).await;
    }
}

/// Queues the runs that have come due, each saved with its turn already.
async fn look(chats: &Arc<Chats>) {
    let due = chats
        .writer
        .call(|c| schedules::due(c, gathr::ids::now_us()))
        .await;
    match due {
        Some(Ok((starts, handled))) => {
            chats.start(starts);
            // A skip changes the list too.
            if handled {
                chats.publish(json!({ "type": "schedules" }));
            }
        }
        Some(Err(e)) => {
            let event = Event::point("gathr.schedules.due")
                .trace(&gathr::ids::trace_id())
                .error("sqlite")
                .attr("why", e.to_string());
            chats.writer.record(event);
        }
        None => {}
    }
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    db(&s, &t, "Scheduled failed", |c| schedules::list(c)).await
}

async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    db(&s, &t, "Scheduled failed", move |c| schedules::get(c, &id)).await
}

#[derive(Deserialize)]
struct Change {
    enabled: bool,
}

async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Change>,
) -> Answer {
    let changed = db(&s, &t, "Change failed", move |c| {
        schedules::set_enabled(c, &id, body.enabled, gathr::ids::now_us())
    })
    .await?;
    s.chats.publish(json!({ "type": "schedules" }));
    Ok(changed)
}

async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let removed = db(&s, &t, "Delete failed", move |c| {
        schedules::delete(c, &id, None).map(|()| json!({ "deleted": true }))
    })
    .await?;
    s.chats.publish(json!({ "type": "schedules" }));
    Ok(removed)
}

/// Run now: a run at once, queued as a due one is; the next time stays.
async fn run_now(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    let started = s.writer.call(move |c| schedules::run_now(c, &id)).await;
    let failed = |status, why: &str| Err(fail(status, "Run failed", why, &t.0));
    let start = match started {
        Some(Ok(start)) => start,
        Some(Err(gathr::Error::Refused(why))) => return failed(StatusCode::CONFLICT, &why),
        Some(Err(e)) => return failed(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
        None => {
            return failed(
                StatusCode::INTERNAL_SERVER_ERROR,
                "the database didn't answer",
            );
        }
    };
    let answer = json!({ "chat": start.chat, "turn": start.turn, "message": start.message["id"] });
    s.chats.start(vec![start]);
    s.chats.publish(json!({ "type": "schedules" }));
    Ok(Json(answer))
}
