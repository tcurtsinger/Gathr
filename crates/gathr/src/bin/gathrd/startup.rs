//! Settings > General's Open at login: whether this computer starts Gathr's window, in the tray,
//! when the person signs in. Read from the system each time, so a change made outside Gathr shows.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::app::Copies;
use gathr::event::Event;
use gathr::launch;
use gathr::platform::login::{self, Item};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, fail};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/startup", get(read).put(change))
}

/// The window on this data folder: beside this service when it runs where it was built, else beside
/// the installed one the window last reached it from (the service runs from the folder's copy).
fn item(s: &State) -> Result<Item, String> {
    let beside = launch::beside_me("gathr-app").map_err(|e| e.to_string())?;
    let installed = Copies::new(&s.data)
        .installed()
        .map(|gathrd| gathrd.with_file_name(beside.file_name().unwrap_or_default()));
    let app = std::iter::once(&beside)
        .chain(&installed)
        .find(|p| p.is_file());
    let Some(app) = app else {
        let missing = installed.as_ref().unwrap_or(&beside);
        return Err(format!("{} isn't there", missing.display()));
    };
    Ok(Item::new(app, s.data.path()))
}

async fn read(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    let state = s.clone();
    let on = tokio::task::spawn_blocking(move || login::is_on(&item(&state)?))
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match on {
        Ok(on) => Ok(Json(json!({ "on": on }))),
        Err(why) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Open at login failed",
            &why,
            &t.0,
        )),
    }
}

#[derive(Deserialize)]
struct Change {
    on: bool,
}

/// Sets it, then answers what the system now says.
async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<Change>,
) -> Answer {
    let began = Instant::now();
    let state = s.clone();
    let done = tokio::task::spawn_blocking(move || {
        let item = item(&state)?;
        login::set(&item, body.on)?;
        login::is_on(&item)
    })
    .await
    .unwrap_or_else(|e| Err(e.to_string()));
    let mut event = Event::point("gathr.startup.set")
        .trace(&t.0)
        .attr("on", body.on);
    event.duration_us = Some(began.elapsed().as_micros() as i64);
    let answer = match done {
        Ok(on) if on == body.on => {
            s.writer.record(event.ok());
            return Ok(Json(json!({ "on": on })));
        }
        Ok(_) => "The system kept the previous setting".to_string(),
        Err(why) => why,
    };
    s.writer
        .record(event.error("startup_failed").attr("why", answer.clone()));
    Err(fail(
        StatusCode::INTERNAL_SERVER_ERROR,
        "Change failed",
        &answer,
        &t.0,
    ))
}
