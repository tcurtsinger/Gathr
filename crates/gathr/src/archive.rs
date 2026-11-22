//! Archiving and deleting a chat (F11, 0010). Archiving ends the chat's work in one transaction and
//! one event: its queued turns stop, its pause goes, its open questions close unanswered and its schedules are
//! disabled. The service interrupts a turn still running.
//! Deleting takes an archived chat and removes its rows; its files are the service's to remove.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
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
    let ended = end_work(&tx, chat, now)?;
    let folders = crate::folders::due(&tx, chat, false)?;
    Event::point("gathr.chat.archive")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("gathr.turn.stopped", ended.turns.len())
        .attr("gathr.folder.due", folders)
        .insert(&tx)?;
    tx.commit()?;
    Ok(ended)
}

/// Queued turns stopped; its pause gone; its open questions closed, so Restore brings none back.
fn end_work(conn: &Transaction, chat: &str, now: i64) -> Result<Ended, Error> {
    const WHY: &str = "its chat was archived";
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
    conn.execute(
        "UPDATE questions SET closed_ts = ?2 WHERE chat_id = ?1 AND answered_ts IS NULL AND closed_ts IS NULL",
        params![chat, now],
    )?;
    // Its schedules are disabled; Restore doesn't enable them.
    conn.execute(
        "UPDATE schedules SET next_ts = NULL WHERE chat_id = ?1",
        [chat],
    )?;
    Ok(Ended { turns })
}

pub fn unarchive(conn: &mut Connection, chat: &str, trace: &str) -> Result<(), Error> {
    let tx = conn.transaction()?;
    if archived(&tx, chat)?.is_none() {
        return Err(Error::Refused("it isn't archived".into()));
    }
    tx.execute("UPDATE chats SET archived_ts = NULL WHERE id = ?1", [chat])?;
    crate::folders::kept(&tx, chat)?;
    let event = Event::point("gathr.chat.unarchive").trace(trace).chat(chat);
    event.ok().insert(&tx)?;
    tx.commit()?;
    Ok(())
}

/// Removes an archived chat's rows: its messages, turns and engine frames, team, sessions and
/// events.
pub fn delete(conn: &mut Connection, chat: &str, trace: &str) -> Result<(), Error> {
    let tx = conn.transaction()?;
    if archived(&tx, chat)?.is_none() {
        return Err(Error::Refused("archive it first".into()));
    }
    crate::folders::due(&tx, chat, true)?;
    let of_turns = "(SELECT id FROM turns WHERE chat_id = ?1)";
    for sql in [
        format!("DELETE FROM background WHERE turn_id IN {of_turns}"),
        "DELETE FROM engine_raw WHERE chat_id = ?1".into(),
        "DELETE FROM handoffs WHERE chat_id = ?1".into(),
        "DELETE FROM agent_context WHERE chat_id = ?1".into(),
        "DELETE FROM reactions WHERE chat_id = ?1".into(),
        "DELETE FROM reads WHERE chat_id = ?1".into(),
        "DELETE FROM board_lines WHERE chat_id = ?1".into(),
        "DELETE FROM questions WHERE chat_id = ?1".into(),
        "DELETE FROM schedule_runs WHERE chat_id = ?1".into(),
        "DELETE FROM schedules WHERE chat_id = ?1".into(),
        "DELETE FROM messages WHERE chat_id = ?1".into(),
        "DELETE FROM turns WHERE chat_id = ?1".into(),
        "DELETE FROM open_checks WHERE chat_id = ?1".into(),
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
        .insert(&tx)?;
    tx.commit()?;
    Ok(())
}
