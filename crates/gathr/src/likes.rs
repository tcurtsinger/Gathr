//! The person's like (0057): a thumbs-up on an agent's message. It is only a reaction: it starts no
//! turn and Gathr writes the agent nothing.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;

/// The person as a reactor: an agent's id is 16 hex characters, so it never clashes.
pub const PERSON: &str = "person";

/// What a like did: the reaction as history shows it, and `marked`, or `standing` when one already
/// stood.
pub struct Given {
    pub reaction: Value,
    pub did: &'static str,
}

/// The person likes `message` in `chat`.
pub fn give(conn: &Connection, chat: &str, message: &str, trace: &str) -> Result<Given, Error> {
    crate::archive::refuse_archived(conn, chat)?;
    let (id, kind) = target(conn, chat, message)?;
    if let Some(reaction) = standing(conn, &id)? {
        return Ok(Given {
            reaction,
            did: "standing",
        });
    }
    if kind != "text" {
        return Err(refused(format!(
            "{message} is a step, not a message: a like goes on an agent's words."
        )));
    }
    let ts = crate::ids::now_us();
    conn.execute(
        "INSERT INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES (?1, ?2, ?3, ?4)",
        params![id, PERSON, chat, ts],
    )?;
    Event::point("gathr.message.reaction")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("message", id.as_str())
        .attr("agent", PERSON)
        .attr("did", "marked")
        .insert(conn)?;
    Ok(Given {
        reaction: reaction(&id, ts),
        did: "marked",
    })
}

/// Takes the person's like off `message`. The message it was on, when one stood.
pub fn take_back(
    conn: &Connection,
    chat: &str,
    message: &str,
    trace: &str,
) -> Result<Option<String>, Error> {
    let (id, _) = target(conn, chat, message)?;
    let removed = conn.execute(
        "DELETE FROM reactions WHERE message_id = ?1 AND agent_id = ?2",
        params![id, PERSON],
    )?;
    if removed > 0 {
        Event::point("gathr.message.reaction.remove")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("message", id.as_str())
            .attr("agent", PERSON)
            .insert(conn)?;
    }
    Ok((removed > 0).then_some(id))
}

/// The agent's message in this chat a like goes on, and its kind: Gathr's copy of a teammate's
/// message stands for the message it carries. Or why it takes none.
fn target(conn: &Connection, chat: &str, message: &str) -> Result<(String, String), Error> {
    let found: Option<(String, String, String)> = conn
        .query_row(
            "SELECT m.id, m.role, m.kind FROM messages c
             JOIN messages m ON m.id = COALESCE(c.copy_of, c.id)
             WHERE c.id = ?1 AND c.chat_id = ?2",
            [message, chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let (id, role, kind) =
        found.ok_or_else(|| refused(format!("There's no message {message} in this chat.")))?;
    match role.as_str() {
        "assistant" => Ok((id, kind)),
        "user" => Err(refused(format!(
            "{message} is your own message: a like goes on an agent's words."
        ))),
        _ => Err(refused(format!(
            "{message} is Gathr's line: a like goes on an agent's words."
        ))),
    }
}

/// The person's like already on the message, as history shows it.
fn standing(conn: &Connection, message: &str) -> Result<Option<Value>, Error> {
    let sql = "SELECT created_ts FROM reactions WHERE message_id = ?1 AND agent_id = ?2";
    let given: Option<i64> = conn
        .query_row(sql, [message, PERSON], |r| r.get(0))
        .optional()?;
    Ok(given.map(|ts| reaction(message, ts)))
}

fn reaction(message: &str, ts: i64) -> Value {
    json!({ "message": message, "agent": PERSON, "time": crate::ids::utc(ts) })
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}
