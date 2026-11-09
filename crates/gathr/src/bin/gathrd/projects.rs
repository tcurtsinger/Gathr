//! Projects through the API (F8b): saved with a name, a git folder and a team; a new chat made in
//! one copies its folder and team (`POST /api/chats {"project"}`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::event::Event;
use gathr::projects::{self, Project};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail};
use crate::team::{Seat, bad, checked, written};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/projects", get(list).post(create))
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    db(&s, &t, "Projects failed", |conn| {
        Ok(json!({ "projects": projects::list(conn)? }))
    })
    .await
}

#[derive(Deserialize)]
struct NewProject {
    name: String,
    folder: PathBuf,
    members: Vec<Seat>,
}

async fn create(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<NewProject>,
) -> Answer {
    const ERROR: &str = "New project failed";
    let name = projects::name(&body.name)
        .map_err(|why| bad(&t, ERROR, why))?
        .to_string();
    let seats = checked(&s, &t, body.members, ERROR).await?;
    let folder = crate::team::project(&t, &body.folder, ERROR).await?;
    let made = db(&s, &t, ERROR, move |conn| {
        Ok(json!(projects::create(conn, &name, &folder, &seats)?))
    })
    .await?;
    let event = Event::point("gathr.project.create").trace(&t.0).ok();
    s.writer
        .record(event.attr("gathr.project.id", made["id"].clone()));
    Ok(made)
}

/// The project, or a 404 that says so.
pub async fn find(s: &State, t: &Trace, id: &str, error: &'static str) -> Result<Project, Fail> {
    let id = id.to_string();
    let found = s.writer.call(move |conn| projects::get(conn, &id)).await;
    match written(t, found, error)? {
        Some(project) => Ok(project),
        None => Err(fail(StatusCode::NOT_FOUND, error, "no such project", &t.0)),
    }
}
