//! Projects through the API (F8b): saved with a name, a git folder and a team; a new chat made in
//! one copies its folder and team (`POST /api/chats {"project"}`). Each setting changes on its own
//! (`PATCH /api/projects/<id>`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::{Extension, Path as Id, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, patch, post};
use axum::{Json, Router};
use gathr::event::Event;
use gathr::projects::{self, Change, Project};
use serde::Deserialize;
use serde_json::json;

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, db, fail};
use crate::team::{Seat, bad, checked, written};

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/projects", get(list).post(create))
        .route("/api/projects/check", post(check))
        .route(
            "/api/projects/{project}",
            patch(change).delete(crate::deletes::project),
        )
        .route("/api/proposals/{message}/make", post(make))
}

#[derive(Deserialize, Default)]
struct Changed {
    name: Option<String>,
    folder: Option<PathBuf>,
    members: Option<Vec<Seat>>,
    #[serde(default)]
    init: bool,
}

#[derive(Deserialize)]
struct Folder {
    folder: PathBuf,
}

/// New project's Folder row: whether the folder is there and in a git repository. One that isn't in one is
/// made one when the project is (`"init": true`).
async fn check(Json(body): Json<Folder>) -> Answer {
    let (folder, repo) = crate::team::checked_folder(&body.folder).await;
    Ok(Json(json!({ "folder": folder, "repo": repo })))
}

/// Make project, from the card of a project the assistant proposed (`propose_project`): as proposed, or with what
/// the New project form changed. The project as `create` makes it, its first chat and the brief there, which starts
/// the lead's turn; once only.
async fn make(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(message): Id<String>,
    body: Option<Json<Changed>>,
) -> Answer {
    const ERROR: &str = "Make project failed";
    let body = body.map(|Json(b)| b).unwrap_or_default();
    let id = message.clone();
    let found = s.writer.call(move |c| gathr::proposals::find(c, &id)).await;
    let Some(p) = written(&t, found, ERROR)? else {
        return Err(fail(StatusCode::NOT_FOUND, ERROR, "no such proposal", &t.0));
    };
    if p.made.is_some() {
        let why = "This project was made already.";
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    let name = body.name.unwrap_or(p.name.clone());
    let name = projects::name(&name)
        .map_err(|why| bad(&t, ERROR, why))?
        .to_string();
    let seats = match body.members {
        Some(seats) => seats,
        None => p
            .seats()
            .into_iter()
            .map(|(agent, role)| Seat {
                agent,
                role: role.as_str().to_string(),
            })
            .collect(),
    };
    let seats = checked(&s, &t, seats, ERROR).await?;
    let folder = body.folder.unwrap_or_else(|| PathBuf::from(&p.folder));
    let folder = crate::team::project_made(&t, &folder, body.init, ERROR).await?;
    let made = s
        .writer
        .call(move |c| gathr::proposals::make(c, &message, (&name, &folder), &seats))
        .await;
    if let Some(Err(gathr::Error::Refused(why))) = &made {
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    let made = written(&t, made, ERROR)?;
    let event = Event::point("gathr.project.create").trace(&t.0).ok();
    s.writer
        .record(event.attr("gathr.project.id", made.project.clone()));
    let chat = made.chat["id"].clone();
    s.chats
        .publish(json!({ "type": "project", "project": made.project, "change": "created" }));
    s.chats
        .publish(json!({ "type": "chat", "chat": chat, "change": "created" }));
    s.chats
        .publish(json!({ "type": "message", "chat": made.proposed_in, "message": made.block }));
    let answer = json!({ "project": made.project, "chat": chat, "turn": made.start.turn, "message": made.block });
    s.chats.start(vec![made.start]);
    Ok(Json(answer))
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
    #[serde(default)]
    init: bool,
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
    let folder = crate::team::project_made(&t, &body.folder, body.init, ERROR).await?;
    let made = db(&s, &t, ERROR, move |conn| {
        Ok(json!(projects::create(conn, &name, &folder, &seats)?))
    })
    .await?;
    let event = Event::point("gathr.project.create").trace(&t.0).ok();
    s.writer
        .record(event.attr("gathr.project.id", made["id"].clone()));
    let created = json!({ "type": "project", "project": made["id"], "change": "created" });
    s.chats.publish(created);
    Ok(made)
}

#[derive(Deserialize)]
pub struct Settings {
    pub name: Option<String>,
    pub folder: Option<PathBuf>,
    pub members: Option<Vec<Seat>>,
}

async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(id): Id<String>,
    Json(body): Json<Settings>,
) -> Answer {
    change_to(&s, &t, id, body).await
}

/// Changes what `body` names and leaves the rest. A new folder or team applies to chats made
/// from then on: a chat keeps the ones it copied (0028).
pub async fn change_to(s: &Arc<State>, t: &Trace, id: String, body: Settings) -> Answer {
    const ERROR: &str = "Save failed";
    find(s, t, &id, ERROR).await?;
    let name = match &body.name {
        Some(name) => Some(
            projects::name(name)
                .map_err(|why| bad(t, ERROR, why))?
                .to_string(),
        ),
        None => None,
    };
    let team = match body.members {
        Some(seats) => Some(checked(s, t, seats, ERROR).await?),
        None => None,
    };
    let folder = match &body.folder {
        Some(folder) => Some(crate::team::project(t, folder, ERROR).await?),
        None => None,
    };
    let changed: Vec<&str> = [
        ("name", name.is_some()),
        ("folder", folder.is_some()),
        ("members", team.is_some()),
    ]
    .into_iter()
    .filter_map(|(field, set)| set.then_some(field))
    .collect();
    let event = Event::point("gathr.project.change")
        .trace(&t.0)
        .ok()
        .attr("gathr.project.id", id.clone())
        .attr("changed", changed);
    let change = Change { name, folder, team };
    let key = id.clone();
    let saved = db(s, t, ERROR, move |conn| {
        Ok(json!(projects::update(conn, &key, change)?))
    })
    .await?;
    if saved.is_null() {
        return Err(fail(StatusCode::NOT_FOUND, ERROR, "no such project", &t.0));
    }
    s.writer.record(event);
    s.chats
        .publish(json!({ "type": "project", "project": id, "change": "changed" }));
    Ok(saved)
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
