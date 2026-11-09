//! Archiving and deleting a chat (F11, 0010). Archiving ends the chat's work in one transaction and
//! one event: its open tasks are cancelled with why, its waits end, its queued turns stop and its
//! pause goes. The service interrupts a turn still running. Deleting archives first, then removes
//! the chat's rows; its files are the service's to remove.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

/// When the chat was archived; none while it's in the list. Refused when there's no such chat.
pub fn archived(conn: &Connection, chat: &str) -> Result<Option<i64>, Error> {
    let found: Option<Option<i64>> = conn
        .query_row("SELECT archived_ts FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?;
    found.ok_or_else(|| Error::Refused("there's no such chat".into()))
}

/// Refuses a change in an archived chat.
pub fn refuse_archived(conn: &Connection, chat: &str) -> Result<(), Error> {
    match archived(conn, chat)? {
        Some(_) => Err(Error::Refused("the chat is archived".into())),
        None => Ok(()),
    }
}

/// What archiving ended.
#[derive(Debug, Default, Serialize)]
pub struct Ended {
    pub tasks: usize,
    pub waits: usize,
    /// Queued turns stopped, by id.
    pub turns: Vec<String>,
}

pub fn archive(conn: &mut Connection, chat: &str, trace: &str) -> Result<Ended, Error> {
    let tx = conn.transaction()?;
    if archived(&tx, chat)?.is_some() {
        return Err(Error::Refused("it's already archived".into()));
    }
    let now = now_us();
    tx.execute(
        "UPDATE chats SET archived_ts = ?2 WHERE id = ?1",
        params![chat, now],
    )?;
    let ended = end_work(&tx, chat, now, trace)?;
    Event::point("gathr.chat.archive")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("gathr.task.cancelled", ended.tasks)
        .attr("gathr.wait.ended", ended.waits)
        .attr("gathr.turn.stopped", ended.turns.len())
        .insert(&tx)?;
    tx.commit()?;
    Ok(ended)
}

/// Every open task cancelled (the person's too: they archived it), with no hear-backs, as nothing
/// starts in the chat any more; open waits ended; queued turns stopped; its pause gone.
fn end_work(conn: &Connection, chat: &str, now: i64, trace: &str) -> Result<Ended, Error> {
    const WHY: &str = "its chat was archived";
    let open: Vec<i64> = {
        let mut stmt = conn.prepare(
            "SELECT number FROM tasks WHERE chat_id = ?1 AND state NOT IN ('done', 'cancelled')",
        )?;
        let rows = stmt.query_map([chat], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for &task in &open {
        crate::tasks::cancelled_with(conn, task, WHY, trace)?;
    }
    let waits = conn.execute(
        "UPDATE waits SET state = 'cancelled', ended_ts = ?2, line = ?3
         WHERE chat_id = ?1 AND state IN ('open', 'met')",
        params![chat, now, WHY],
    )?;
    let turns: Vec<String> = {
        let mut stmt = conn.prepare(
            "UPDATE turns SET status = 'stopped', why = ?2, ended_ts = ?3, held = NULL
             WHERE chat_id = ?1 AND status IN ('queued', 'waiting_sign_in') RETURNING id",
        )?;
        let rows = stmt.query_map(params![chat, WHY, now], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    conn.execute(
        "DELETE FROM pauses WHERE scope = 'chat' AND target = ?1",
        [chat],
    )?;
    Ok(Ended {
        tasks: open.len(),
        waits,
        turns,
    })
}

pub fn unarchive(conn: &mut Connection, chat: &str, trace: &str) -> Result<(), Error> {
    let tx = conn.transaction()?;
    if archived(&tx, chat)?.is_none() {
        return Err(Error::Refused("it isn't archived".into()));
    }
    tx.execute("UPDATE chats SET archived_ts = NULL WHERE id = ?1", [chat])?;
    let event = Event::point("gathr.chat.unarchive").trace(trace).chat(chat);
    event.ok().insert(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Removes the chat's rows, archiving it first if it isn't: its messages, turns and engine frames,
/// tasks and waits, team, sessions and events. Answers the folders of its tasks' files.
pub fn delete(conn: &mut Connection, chat: &str, trace: &str) -> Result<Vec<PathBuf>, Error> {
    let tx = conn.transaction()?;
    let now = now_us();
    if archived(&tx, chat)?.is_none() {
        end_work(&tx, chat, now, trace)?;
    }
    let files = task_files(&tx, chat)?;
    let of_tasks = "(SELECT number FROM tasks WHERE chat_id = ?1)";
    let of_turns = "(SELECT id FROM turns WHERE chat_id = ?1)";
    for sql in [
        format!("DELETE FROM background WHERE turn_id IN {of_turns}"),
        "DELETE FROM engine_raw WHERE chat_id = ?1".into(),
        "DELETE FROM messages WHERE chat_id = ?1".into(),
        "DELETE FROM turns WHERE chat_id = ?1".into(),
        format!("DELETE FROM task_after WHERE task IN {of_tasks} OR after_task IN {of_tasks}"),
        // `finish_task` may name a task in another chat as the one it's blocked on.
        format!(
            "UPDATE tasks SET blocked_on = NULL WHERE blocked_on IN {of_tasks} AND chat_id != ?1"
        ),
        "DELETE FROM waits WHERE chat_id = ?1".into(),
        "DELETE FROM tasks WHERE chat_id = ?1".into(),
        "DELETE FROM chat_members WHERE chat_id = ?1".into(),
        "DELETE FROM engine_sessions WHERE chat_id = ?1".into(),
        "DELETE FROM pauses WHERE scope = 'chat' AND target = ?1".into(),
        "DELETE FROM events WHERE conversation_id = ?1".into(),
        "DELETE FROM chats WHERE id = ?1".into(),
    ] {
        tx.execute(&sql, [chat])?;
    }
    Event::point("gathr.chat.delete")
        .trace(trace)
        .ok()
        .attr("gathr.chat.id", chat)
        .attr("gathr.task.files", files.len())
        .insert(&tx)?;
    tx.commit()?;
    Ok(files)
}

/// The folders the chat's tasks' files were copied into.
fn task_files(conn: &Connection, chat: &str) -> Result<Vec<PathBuf>, Error> {
    let mut folders = Vec::new();
    for task in crate::tasks::list(conn, chat)? {
        for file in task.files {
            if let Some(folder) = std::path::Path::new(&file.path).parent() {
                folders.push(folder.to_path_buf());
            }
        }
    }
    folders.sort();
    folders.dedup();
    Ok(folders)
}
