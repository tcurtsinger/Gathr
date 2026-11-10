//! A chat's agents, roles and project folder through the API (0013, 0028), until F8's screens.

use std::sync::Arc;

use axum::Json;
use axum::extract::{Extension, Path, State as Extract};
use axum::http::StatusCode;
use gathr::event::Event;
use gathr::platform::env;
use gathr::team::{self, Role};
use gathr::{agents, store, worktree};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, Fail, fail};

#[derive(Deserialize)]
pub struct Seat {
    agent: String,
    role: String,
}

#[derive(Deserialize)]
pub struct NewChat {
    title: Option<String>,
    /// The project it's made in: its folder and team are copied (F8b).
    project: Option<String>,
    members: Option<Vec<Seat>>,
}

pub async fn create(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Json(body): Json<NewChat>,
) -> Answer {
    const ERROR: &str = "New chat failed";
    let in_project = match body.project {
        Some(id) => Some(crate::projects::find(&s, &t, &id, ERROR).await?),
        None => None,
    };
    let seats = match (&in_project, body.members) {
        (Some(p), _) => Some(
            p.members
                .iter()
                .map(|m| (m.agent.clone(), m.role))
                .collect(),
        ),
        (None, Some(seats)) => Some(checked(&s, &t, seats, ERROR).await?),
        (None, None) => None,
    };
    let title = body.title.unwrap_or_else(|| store::UNTITLED.into());
    let made = s
        .writer
        .call(move |conn| {
            let at = in_project
                .as_ref()
                .map(|p| (p.id.as_str(), p.folder.as_str()));
            let at = (at.map(|a| a.0), at.map(|a| a.1));
            let chat = store::create_chat(conn, &title, at)?;
            if let Some(seats) = &seats {
                team::set_members(conn, chat["id"].as_str().unwrap_or_default(), seats)?;
            }
            Ok::<_, gathr::Error>((chat, seats))
        })
        .await;
    let (chat, seats) = written(&t, made, ERROR)?;
    if let Some(seats) = seats {
        record(&s, &t, chat["id"].as_str().unwrap_or_default(), &seats);
    }
    // Every window, so one open before it shows the chat in its sidebar.
    s.chats
        .publish(json!({ "type": "chat", "chat": chat["id"], "change": "created" }));
    Ok(Json(chat))
}

#[derive(Deserialize)]
pub struct Change {
    members: Option<Vec<Seat>>,
    project: Option<String>,
}

/// Replaces the chat's agents, or puts a chat with no messages yet in a project, copying its
/// folder and team. Once a chat has messages its folder stays as it was (0013).
pub async fn change(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
    Json(body): Json<Change>,
) -> Answer {
    const ERROR: &str = "Save failed";
    crate::routes::known(&s, &t, &chat).await?;
    let members = match (body.members, body.project) {
        (Some(members), None) => members,
        (None, Some(project)) => return put_in(&s, &t, &chat, &project).await,
        _ => return Err(bad(&t, ERROR, "send members or a project")),
    };
    let seats = checked(&s, &t, members, ERROR).await?;
    let (id, saved) = (chat.clone(), seats.clone());
    let changed = s
        .writer
        .call(move |conn| {
            team::set_members(conn, &id, &saved)?;
            team::members(conn, &id)
        })
        .await;
    let members = written(&t, changed, ERROR)?;
    changed_to(&s, &t, &chat, &seats, &members);
    Ok(Json(json!({ "chat": chat, "members": members })))
}

/// Not one transaction, but safe to send again: the chat has no turns until it's sent to.
async fn put_in(s: &State, t: &Trace, chat: &str, project: &str) -> Answer {
    const ERROR: &str = "Save failed";
    let p = crate::projects::find(s, t, project, ERROR).await?;
    let seats: Vec<(String, Role)> = p
        .members
        .iter()
        .map(|m| (m.agent.clone(), m.role))
        .collect();
    let (id, saved, at) = (
        chat.to_string(),
        seats.clone(),
        (p.id.clone(), p.folder.clone()),
    );
    let done = s
        .writer
        .call(move |conn| {
            store::put_in_project(conn, &id, (&at.0, &at.1))?;
            team::set_members(conn, &id, &saved)?;
            team::members(conn, &id)
        })
        .await;
    if let Some(Err(gathr::Error::Refused(why))) = &done {
        return Err(fail(StatusCode::CONFLICT, ERROR, why, &t.0));
    }
    let members = written(t, done, ERROR)?;
    changed_to(s, t, chat, &seats, &members);
    Ok(Json(
        json!({ "chat": chat, "project": p.id, "folder": p.folder, "members": members }),
    ))
}

/// A send added an agent: the change is recorded and goes out live.
pub fn changed(s: &State, t: &Trace, chat: &str, members: Vec<team::Member>) {
    let seats: Vec<(String, Role)> = members.iter().map(|m| (m.agent.clone(), m.role)).collect();
    changed_to(s, t, chat, &seats, &members);
}

fn changed_to(
    s: &State,
    t: &Trace,
    chat: &str,
    seats: &[(String, Role)],
    members: &[team::Member],
) {
    record(s, t, chat, seats);
    s.chats
        .publish(json!({ "type": "members", "chat": chat, "members": members }));
}

/// Each member's worktree folder, once it has one.
pub fn with_folders(s: &State, chat: &str, history: &mut Value) {
    let Some(members) = history["members"].as_array_mut() else {
        return;
    };
    for member in members {
        let agent = member["agent"].as_str().unwrap_or_default();
        let folder = worktree::folder(s.chats.data.path(), chat, agent);
        if folder.join(".git").exists() {
            member["folder"] = json!(folder.display().to_string());
        }
    }
}

/// The seats as roles, once the rules hold and every agent exists.
pub async fn checked(
    s: &State,
    t: &Trace,
    seats: Vec<Seat>,
    error: &'static str,
) -> Result<Vec<(String, Role)>, Fail> {
    let mut team = Vec::new();
    for seat in seats {
        let Some(role) = Role::named(&seat.role) else {
            let why = format!("{} isn't a role: supervisor, reviewer or member", seat.role);
            return Err(bad(t, error, &why));
        };
        team.push((seat.agent, role));
    }
    if let Some(why) = team::refused(&team) {
        return Err(bad(t, error, why));
    }
    let ids: Vec<String> = team.iter().map(|(a, _)| a.clone()).collect();
    let missing = s
        .writer
        .call(move |conn| {
            for id in ids {
                if agents::get(conn, &id)?.is_none() {
                    return Ok(Some(id));
                }
            }
            Ok::<_, gathr::Error>(None)
        })
        .await;
    match written(t, missing, error)? {
        Some(id) => Err(bad(t, error, &format!("no agent {id}"))),
        None => Ok(team),
    }
}

/// The project's top folder: it must be a git repository.
pub async fn project(
    t: &Trace,
    dir: &std::path::Path,
    error: &'static str,
) -> Result<String, Fail> {
    if !dir.is_absolute() {
        return Err(bad(t, error, "the project folder is a full path"));
    }
    let dir = dir.to_path_buf();
    let fresh = tokio::task::spawn_blocking(|| env::fresh(std::env::vars_os().collect())).await;
    let vars = fresh
        .map(|f| f.vars)
        .unwrap_or_else(|_| std::env::vars_os().collect());
    match worktree::top_level(&dir, &vars).await {
        Ok(top) => Ok(top.display().to_string()),
        Err(why) => Err(bad(t, error, &why)),
    }
}

/// Records the chat's new team (0013: a role change is an event).
fn record(s: &State, t: &Trace, chat: &str, seats: &[(String, Role)]) {
    let team: Vec<Value> = seats
        .iter()
        .map(|(agent, role)| json!({ "agent": agent, "role": role }))
        .collect();
    let event = Event::point("gathr.chat.members")
        .trace(&t.0)
        .chat(chat)
        .ok();
    s.writer.record(event.attr("members", Value::Array(team)));
}

pub fn bad(t: &Trace, error: &'static str, why: &str) -> Fail {
    fail(StatusCode::BAD_REQUEST, error, why, &t.0)
}

pub fn written<T>(
    t: &Trace,
    done: Option<Result<T, gathr::Error>>,
    error: &'static str,
) -> Result<T, Fail> {
    match done {
        Some(Ok(value)) => Ok(value),
        Some(Err(e)) => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            &e.to_string(),
            &t.0,
        )),
        None => Err(fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            error,
            "the database didn't answer",
            &t.0,
        )),
    }
}
