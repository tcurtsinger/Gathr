//! Skills for the Plugins screen (`gathr::skills`): read from the Skills folder each time they're
//! asked for, and changed there. Each change is sent live as `skills`.

use std::path::Path as FilePath;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, Extension, Path, Query, State as Extract};
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::{Error, files, skills};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db};

pub fn routes() -> Router<Arc<State>> {
    let most = DefaultBodyLimit::max(files::MOST);
    Router::new()
        .route("/api/skills", get(list).post(add).layer(most))
        .route("/api/skills/{name}", get(one).patch(change).delete(remove))
        .route("/api/skills/{name}/files", post(add_file).layer(most))
        .route("/api/skills/{name}/open", post(open))
}

/// The Skills folder and every skill in it.
fn listed(conn: &Connection, dir: &FilePath) -> Result<Value, Error> {
    Ok(json!({ "folder": dir, "skills": skills::list(dir, &skills::off(conn)?) }))
}

/// One skill, with its SKILL.md's text after the front matter.
fn shown(conn: &Connection, dir: &FilePath, name: &str) -> Result<Value, Error> {
    let path = skills::path(dir, name)?;
    let skill = skills::list(dir, &skills::off(conn)?)
        .into_iter()
        .find(|s| s.name == name);
    let mut shown = json!(skill);
    shown["text"] = json!(skills::text(&path));
    Ok(shown)
}

/// Runs `work` on the Skills folder: on success, an open window is told.
async fn on_skills(
    s: &Arc<State>,
    t: &Trace,
    error: &'static str,
    change: bool,
    work: impl FnOnce(&Connection, &FilePath) -> Result<Value, Error> + Send + 'static,
) -> Answer {
    let data = s.data.path().to_path_buf();
    let answer = db(s, t, error, move |conn| {
        work(conn, &skills::folder(conn, &data)?)
    })
    .await;
    if change && answer.is_ok() {
        let _ = s.chats.live.send(json!({ "type": "skills" }));
    }
    answer
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    on_skills(&s, &t, "Skills failed", false, listed).await
}

async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(name): Path<String>,
) -> Answer {
    on_skills(&s, &t, "Skill failed", false, move |c, dir| {
        shown(c, dir, &name)
    })
    .await
}

#[derive(Deserialize)]
struct FromFile {
    /// A single Markdown file's name, its text the body; without it, the body is a `New`.
    file: Option<String>,
}

#[derive(Deserialize)]
struct New {
    name: String,
    description: String,
    text: String,
}

/// A new skill: written from a name, a description and its text, or from one Markdown file.
async fn add(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Query(q): Query<FromFile>,
    body: Bytes,
) -> Answer {
    on_skills(&s, &t, "Add failed", true, move |c, dir| {
        let name = match q.file {
            Some(file) => skills::add_single(dir, &file, &body)?,
            None => {
                let new: New = serde_json::from_slice(&body)?;
                skills::create(dir, &new.name, &new.description, &new.text)?;
                new.name
            }
        };
        shown(c, dir, &name)
    })
    .await
}

#[derive(Deserialize)]
struct Change {
    enabled: bool,
}

async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(name): Path<String>,
    Json(body): Json<Change>,
) -> Answer {
    on_skills(&s, &t, "Save failed", true, move |c, dir| {
        skills::path(dir, &name)?;
        skills::set_enabled(c, &name, body.enabled)?;
        shown(c, dir, &name)
    })
    .await
}

/// Deletes the skill's folder; a skill of that name added later starts enabled.
async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(name): Path<String>,
) -> Answer {
    on_skills(&s, &t, "Remove failed", true, move |c, dir| {
        skills::remove(dir, &name)?;
        skills::set_enabled(c, &name, true)?;
        listed(c, dir)
    })
    .await
}

#[derive(Deserialize)]
struct File {
    /// Its path inside the skill.
    path: String,
    /// The first of a new skill's files: its folder is made, and refused if it's there.
    #[serde(default)]
    new: bool,
}

/// One file of a skill added from a folder, the window sending them one by one.
async fn add_file(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(name): Path<String>,
    Query(q): Query<File>,
    body: Bytes,
) -> Answer {
    on_skills(&s, &t, "Add failed", true, move |_, dir| {
        skills::add_file(dir, &name, &q.path, &body, q.new)?;
        Ok(json!({ "name": name }))
    })
    .await
}

/// Opens the skill's folder in the system's file manager, for the desktop window.
async fn open(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(name): Path<String>,
) -> Answer {
    on_skills(&s, &t, "Open failed", false, move |_, dir| {
        let path = skills::path(dir, &name)?;
        gathr::platform::open_folder(&path).map_err(Error::Other)?;
        Ok(json!({ "opened": path }))
    })
    .await
}
