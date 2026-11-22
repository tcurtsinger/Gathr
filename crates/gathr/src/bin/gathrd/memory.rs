//! Memory for its screen (`gathr::memory`): read from its files each time it's asked for, and changed
//! there, by the person. Each change, here or by an agent's `note`, is sent live as `memory`.

use std::path::Path as FilePath;
use std::sync::Arc;

use axum::extract::{Extension, Path, State as Extract};
use axum::routing::get;
use axum::{Json, Router};
use gathr::memory::{self, Place};
use gathr::{Error, agents, projects};
use rusqlite::Connection;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db};

pub fn routes() -> Router<Arc<State>> {
    Router::new().route("/api/memory", get(list)).route(
        "/api/memory/{place}",
        get(one).post(add).patch(change).delete(remove),
    )
}

/// Every place, by its name, with how many notes it holds.
fn listed(conn: &Connection, root: &FilePath) -> Result<Value, Error> {
    let mut places = vec![(Place::You, "About you".to_string(), "you")];
    for p in projects::list(conn)? {
        places.push((Place::Project(p.id), p.name, "project"));
    }
    for a in agents::list(conn)? {
        places.push((Place::Agent(a.id), a.name, "agent"));
    }
    let mut shown = Vec::new();
    for (place, name, kind) in places {
        let notes = memory::notes(&memory::file(conn, root, &place)?)?;
        shown.push(json!({ "id": place.id(), "kind": kind, "name": name, "count": notes.len() }));
    }
    Ok(json!({ "folder": root.join(memory::FOLDER), "places": shown }))
}

/// One place: its name, its file and its notes.
fn shown(conn: &Connection, root: &FilePath, place: &Place) -> Result<Value, Error> {
    let path = memory::file(conn, root, place)?;
    let name = match place {
        Place::You => "About you".to_string(),
        Place::Project(id) => projects::get(conn, id)?.map(|p| p.name).unwrap_or_default(),
        Place::Agent(id) => agents::name_of(conn, id)?,
    };
    let notes = memory::notes(&path)?;
    Ok(json!({ "id": place.id(), "name": name, "path": path, "notes": notes }))
}

/// Runs `work` on the agent folder: on a change, an open window is told.
async fn on_memory(
    s: &Arc<State>,
    t: &Trace,
    (error, change): (&'static str, bool),
    work: impl FnOnce(&Connection, &FilePath) -> Result<Value, Error> + Send + 'static,
) -> Answer {
    let data = s.data.path().to_path_buf();
    let answer = db(s, t, error, move |conn| {
        work(conn, &memory::root(conn, &data)?)
    })
    .await;
    if change && answer.is_ok() {
        let _ = s.chats.live.send(json!({ "type": "memory" }));
    }
    answer
}

async fn list(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    on_memory(&s, &t, ("Memory failed", false), listed).await
}

async fn one(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
) -> Answer {
    on_memory(&s, &t, ("Memory failed", false), move |c, root| {
        shown(c, root, &Place::parse(&id)?)
    })
    .await
}

#[derive(Deserialize)]
struct Text {
    text: String,
}

/// The person's note, at the top.
async fn add(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Text>,
) -> Answer {
    on_memory(&s, &t, ("Add failed", true), move |c, root| {
        let place = Place::parse(&id)?;
        memory::add(&memory::file(c, root, &place)?, &body.text, memory::PERSON)?;
        shown(c, root, &place)
    })
    .await
}

#[derive(Deserialize)]
struct Changed {
    was: String,
    text: String,
}

/// The note that read `was`, now as the person wrote it.
async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Changed>,
) -> Answer {
    on_memory(&s, &t, ("Save failed", true), move |c, root| {
        let place = Place::parse(&id)?;
        let path = memory::file(c, root, &place)?;
        memory::replace(&path, &body.was, &body.text, memory::PERSON)?;
        shown(c, root, &place)
    })
    .await
}

async fn remove(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(id): Path<String>,
    Json(body): Json<Text>,
) -> Answer {
    on_memory(&s, &t, ("Remove failed", true), move |c, root| {
        let place = Place::parse(&id)?;
        memory::remove(&memory::file(c, root, &place)?, &body.text)?;
        shown(c, root, &place)
    })
    .await
}
