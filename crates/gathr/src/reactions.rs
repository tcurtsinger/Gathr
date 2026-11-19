//! A thumbs-up (Stage 3, part C): an agent agrees with the person's message or another agent's, with no
//! message and no turn.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;

/// What a thumbs-up did: the message it is on, whether it is new, and when it was given.
pub struct Added {
    pub message: String,
    pub new: bool,
    pub time: String,
}

/// The person's message `turn` answers: the last of theirs to reach it, itself or Gathr's copy of it
/// (0028). Refused when the turn has none.
pub fn persons(conn: &Connection, turn: &str) -> Result<String, Error> {
    conn.query_row(
        "SELECT o.id FROM messages m JOIN messages o ON o.id = COALESCE(m.copy_of, m.id)
         WHERE m.turn_id = ?1 AND o.role = 'user' AND o.kind = 'text' ORDER BY m.ts DESC, m.rowid DESC LIMIT 1",
        [turn],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| {
        Error::Refused(
            "your turn answers no message of the person's: give the message's id.".into(),
        )
    })
}

/// Puts `agent`'s thumbs-up on `message` in `chat`, on the caller's transaction. Gathr's copy of a
/// message stands for the message it carries.
pub fn add(
    conn: &Connection,
    chat: &str,
    message: &str,
    agent: &str,
    trace: &str,
) -> Result<Added, Error> {
    let found: Option<(String, String, Option<String>)> = conn
        .query_row(
            "SELECT m.id, m.role, t.agent_id FROM messages c
             JOIN messages m ON m.id = COALESCE(c.copy_of, c.id)
             LEFT JOIN turns t ON t.id = m.turn_id
             WHERE c.id = ?1 AND c.chat_id = ?2 AND c.kind = 'text'",
            [message, chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((id, role, author)) = found else {
        return Err(Error::Refused(format!(
            "No message {message} in this chat: use an id that read_chat or a message to you gives."
        )));
    };
    let goes = "a thumbs-up goes on the person's message or another agent's.";
    if role != "assistant" && role != "user" {
        return Err(Error::Refused(format!("{message} is Gathr's line: {goes}")));
    }
    if role == "assistant" && author.as_deref() == Some(agent) {
        return Err(Error::Refused(format!(
            "{message} is your own message: {goes}"
        )));
    }
    let added = conn.execute(
        "INSERT OR IGNORE INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES (?1, ?2, ?3, ?4)",
        params![id, agent, chat, crate::ids::now_us()],
    )?;
    if added == 1 {
        Event::point("gathr.message.reaction")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("message", id.clone())
            .attr("agent", agent)
            .insert(conn)?;
    }
    let given: i64 = conn.query_row(
        "SELECT created_ts FROM reactions WHERE message_id = ?1 AND agent_id = ?2",
        [&id, agent],
        |r| r.get(0),
    )?;
    Ok(Added {
        message: id,
        new: added == 1,
        time: crate::ids::utc(given),
    })
}

/// The chat's thumbs-ups on its history, oldest first.
pub fn on_history(conn: &Connection, chat: &str, history: &mut Value) -> Result<(), Error> {
    let mut statement = conn.prepare_cached(
        "SELECT message_id, agent_id, created_ts FROM reactions WHERE chat_id = ?1
         ORDER BY created_ts, rowid",
    )?;
    let rows = statement.query_map([chat], |r| {
        let (message, agent, time): (String, String, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
        let time = crate::ids::utc(time);
        Ok(json!({ "message": message, "agent": agent, "time": time }))
    })?;
    history["reactions"] = Value::Array(rows.collect::<rusqlite::Result<_>>()?);
    Ok(())
}
