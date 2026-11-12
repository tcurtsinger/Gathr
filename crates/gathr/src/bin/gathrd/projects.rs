//! Projects through the API (F8b): saved with a name, a git folder and a team; a new chat made in
//! one copies its folder and team (`POST /api/chats {"project"}`). Each setting, its CI choice and
//! merge method too (0043), changes on its own (`PATCH /api/projects/<id>`); its checks are the
//! Supervisor's to set, never the person's (owner, task 1455).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Extension, Path as Id, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, patch};
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
        .route("/api/projects/{project}", patch(change))
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
    // A repo that runs GitHub Actions has CI to go by; one that doesn't has only Gathr's checks.
    let ci = if has_workflows(Path::new(&folder)) {
        "ci"
    } else {
        "gathr"
    };
    let made = db(&s, &t, ERROR, move |conn| {
        Ok(json!(projects::create(conn, &name, &folder, &seats, ci)?))
    })
    .await?;
    let event = Event::point("gathr.project.create").trace(&t.0).ok();
    s.writer.record(
        event
            .attr("gathr.project.id", made["id"].clone())
            .attr("ci", ci),
    );
    let created = json!({ "type": "project", "project": made["id"], "change": "created" });
    s.chats.publish(created);
    Ok(made)
}

/// Whether the checkout has a GitHub Actions workflow file.
fn has_workflows(folder: &Path) -> bool {
    let Ok(files) = std::fs::read_dir(folder.join(".github").join("workflows")) else {
        return false;
    };
    files.flatten().any(|file| {
        let path = file.path();
        let yaml = path.extension().is_some_and(|e| e == "yml" || e == "yaml");
        yaml && path.is_file()
    })
}

#[derive(Deserialize)]
struct Settings {
    name: Option<String>,
    folder: Option<PathBuf>,
    members: Option<Vec<Seat>>,
    /// Refused: only the Supervisor sets checks (owner, task 1455).
    checks: Option<serde_json::Value>,
    ci: Option<String>,
    merge: Option<String>,
}

/// Changes what the body names and leaves the rest. A new folder or team applies to chats made
/// from then on: a chat keeps the ones it copied (0028).
async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(id): Id<String>,
    Json(body): Json<Settings>,
) -> Answer {
    const ERROR: &str = "Project settings failed";
    find(&s, &t, &id, ERROR).await?;
    let one_of = |value: Option<String>, allowed: &[&str], what: &str| match value {
        Some(v) if !allowed.contains(&v.as_str()) => {
            let why = format!("{v} isn't a {what}: {}", allowed.join(", "));
            Err(bad(&t, ERROR, &why))
        }
        other => Ok(other),
    };
    let ci = one_of(body.ci, &projects::CI, "CI choice")?;
    let merge = one_of(body.merge, &projects::MERGE, "merge method")?;
    if body.checks.is_some() {
        let why = "The project's Supervisor sets its checks, with Gathr's checks tool";
        return Err(bad(&t, ERROR, why));
    }
    let name = match &body.name {
        Some(name) => Some(
            projects::name(name)
                .map_err(|why| bad(&t, ERROR, why))?
                .to_string(),
        ),
        None => None,
    };
    let team = match body.members {
        Some(seats) => Some(checked(&s, &t, seats, ERROR).await?),
        None => None,
    };
    let folder = match &body.folder {
        Some(folder) => Some(crate::team::project(&t, folder, ERROR).await?),
        None => None,
    };
    let changed: Vec<&str> = [
        ("name", name.is_some()),
        ("folder", folder.is_some()),
        ("members", team.is_some()),
        ("ci", ci.is_some()),
        ("merge", merge.is_some()),
    ]
    .into_iter()
    .filter_map(|(field, set)| set.then_some(field))
    .collect();
    let event = Event::point("gathr.project.change")
        .trace(&t.0)
        .ok()
        .attr("gathr.project.id", id.clone())
        .attr("changed", changed)
        .attr("ci", ci.clone())
        .attr("merge", merge.clone());
    let change = Change {
        name,
        folder,
        team,
        ci,
        merge,
        by: Some(projects::PERSON.to_string()),
    };
    let key = id.clone();
    let saved = db(&s, &t, ERROR, move |conn| {
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
