//! Archive, unarchive and delete a chat (F11, 0010). Archiving ends the chat's work in one step:
//! the database part in one transaction and event, then every agent's queue stops what it ran there.
//! Deleting archives first, closes the chat's engines the gentle way, removes its rows, then its
//! worktrees and files.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::{Extension, Path, State as Extract};
use axum::routing::post;
use axum::{Json, routing::delete as remove};
use gathr::archive;
use gathr::event::Event;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use crate::api::{State, Trace};
use crate::routes::{Answer, db, known};

/// How long a delete waits for the chat's engines and wait checks to end before removing what it can.
const EXIT_WAIT: Duration = Duration::from_secs(35);

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/chats/{chat}/archive", post(archive_chat))
        .route("/api/chats/{chat}/unarchive", post(unarchive_chat))
        .route("/api/chats/{chat}", remove(delete_chat))
}

async fn archive_chat(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let Json(ended) = archive_now(&s, &t, &chat, "Archive failed").await?;
    announce(&s, &chat, "archived");
    Ok(Json(json!({ "archived": true, "ended": ended })))
}

/// Archives the chat: ends its work and starts what follows in other chats. Windows are told by the
/// caller: a delete tells them only "deleted", so an open chat never passes through Archived.
async fn archive_now(s: &Arc<State>, t: &Trace, chat: &str, error: &'static str) -> Answer {
    let (id, trace) = (chat.to_string(), t.0.clone());
    let follows = Arc::new(Mutex::new(Vec::new()));
    let found = follows.clone();
    let ended = db(s, t, error, move |c| {
        let mut ended = archive::archive(c, &id, &trace)?;
        *found.lock().expect("never poisoned") = std::mem::take(&mut ended.starts);
        Ok(json!(ended))
    })
    .await?;
    s.chats.start(std::mem::take(
        &mut *follows.lock().expect("never poisoned"),
    ));
    s.chats.end_chat(chat, false).await;
    s.chats.pauses_changed().await;
    Ok(ended)
}

async fn unarchive_chat(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let (id, trace) = (chat.clone(), t.0.clone());
    let _ = db(&s, &t, "Unarchive failed", move |c| {
        archive::unarchive(c, &id, &trace).map(|()| Value::Null)
    })
    .await?;
    announce(&s, &chat, "unarchived");
    Ok(Json(json!({ "archived": false })))
}

async fn delete_chat(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Path(chat): Path<String>,
) -> Answer {
    known(&s, &t, &chat).await?;
    let id = chat.clone();
    let Json(archived) = db(&s, &t, "Delete failed", move |c| {
        Ok(json!(archive::archived(c, &id)?.is_some()))
    })
    .await?;
    if archived != json!(true) {
        let _ = archive_now(&s, &t, &chat, "Delete failed").await?;
    }
    let id = chat.clone();
    let Json(project) = db(&s, &t, "Delete failed", move |c| {
        let project: Option<String> = c
            .query_row("SELECT project FROM chats WHERE id = ?1", [&id], |r| {
                r.get(0)
            })
            .optional()?
            .flatten();
        Ok(json!(project))
    })
    .await?;
    // Nothing may still run in its worktrees when they're removed.
    let ended = async {
        s.chats.end_chat(&chat, true).await;
        s.chats.checks_ended(&chat).await;
    };
    let exited = tokio::time::timeout(EXIT_WAIT, ended).await;
    s.chats.pauses_changed().await;
    let (id, trace) = (chat.clone(), t.0.clone());
    let Json(files) = db(&s, &t, "Delete failed", move |c| {
        Ok(json!(archive::delete(c, &id, &trace)?))
    })
    .await?;
    let files: Vec<PathBuf> = serde_json::from_value(files).unwrap_or_default();
    let project = project.as_str().map(PathBuf::from);
    let mut left = remove_files(&s, &chat, project, files).await;
    if exited.is_err() {
        left.insert(0, "An engine or wait check is still running".into());
    }
    let mut event = Event::point("gathr.chat.files")
        .trace(&t.0)
        .attr("gathr.chat.id", chat.clone());
    event = match left.is_empty() {
        true => event.ok(),
        false => event.error("left").attr("why", left.join("; ")),
    };
    s.writer.record(event);
    announce(&s, &chat, "deleted");
    Ok(Json(json!({ "deleted": true, "left": left })))
}

/// The chat's worktrees, hooks, folder and its tasks' files: what couldn't be removed, with why.
async fn remove_files(
    s: &State,
    chat: &str,
    project: Option<PathBuf>,
    task_files: Vec<PathBuf>,
) -> Vec<String> {
    let data = s.data.path();
    let fresh =
        tokio::task::spawn_blocking(|| gathr::platform::env::fresh(std::env::vars_os().collect()));
    let mut left = Vec::new();
    if let Ok(env) = fresh.await {
        let worktrees = data.join("worktrees").join(chat);
        if let Err(why) =
            gathr::worktree::remove_chat(project.as_deref(), &worktrees, &env.vars).await
        {
            left.push(why);
        }
    }
    let folders = [data.join("hooks").join(chat), data.join("chats").join(chat)];
    // Only copies under task-files: a path from anywhere else is never removed.
    let copies = data.join("task-files");
    let task_files = task_files
        .into_iter()
        .filter(|f| f.starts_with(&copies) && *f != copies);
    for folder in folders.into_iter().chain(task_files) {
        match std::fs::remove_dir_all(&folder) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                left.push(format!("{} wasn't deleted: {e}", folder.display()));
            }
            _ => {}
        }
    }
    left
}

/// Tells every window, so another one showing the chat drops or moves it.
fn announce(s: &State, chat: &str, change: &str) {
    s.chats
        .publish(json!({ "type": "chat", "chat": chat, "change": change }));
}
