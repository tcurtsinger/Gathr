//! The person's messages an AI app outside Gathr sent (`from_app`): the name the app gave as it connected. Such a
//! message is sent as the person's own, with one line of Gathr's before it for the agent; the app reads the answer
//! when it asks, through how a chat stands and what its agents said since.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::Error;

/// A chat that waits for the person: an approval, a question of an agent's, a sign-in.
pub const WAITING: &str = "waiting for the person";

/// Notes that `app` sent the saved `message`, and gives it back as the chat shows it now.
pub fn mark(conn: &Connection, message: &mut Value, app: &str) -> Result<(), Error> {
    let id = message["id"].as_str().unwrap_or_default().to_string();
    let sql = "INSERT INTO from_app (message_id, app) VALUES (?1, ?2)";
    conn.execute(sql, params![id, app])?;
    *message = crate::store::message(conn, &id)?;
    Ok(())
}

/// Gathr's line before such a message's words, for its agent: which app sent it, and that the app reads the answer.
pub fn line(conn: &Connection, message: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT app FROM from_app WHERE message_id = ?1";
    let app: Option<String> = (conn.prepare_cached(sql)?)
        .query_row([message], |r| r.get(0))
        .optional()?;
    Ok(app.map(|app| {
        format!("{app}, an AI app the person uses on this PC, sent this for the person. The app reads your answer.")
    }))
}

/// How the chat stands, in one word: working, done, stopped, failed, or waiting for the person. `asked` is an
/// approval waiting, which is in no table.
pub fn standing(conn: &Connection, chat: &str, asked: bool) -> Result<&'static str, Error> {
    let sql = "SELECT
        EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1 AND status = 'waiting_sign_in')
          OR EXISTS (SELECT 1 FROM questions WHERE chat_id = ?1 AND answered_ts IS NULL AND closed_ts IS NULL),
        EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1 AND (status = 'running'
          OR (status = 'queued' AND coalesce(held, '') NOT IN ('usage', 'pause')))),
        (SELECT status FROM turns WHERE chat_id = ?1 ORDER BY created_ts DESC, rowid DESC LIMIT 1)";
    let (needs, working, last): (bool, bool, Option<String>) =
        conn.query_row(sql, [chat], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(match last.as_deref() {
        _ if asked || needs => WAITING,
        _ if working => "working",
        Some("failed") => "failed",
        // A turn a pause or a usage limit holds waits for the person to start it again.
        Some("stopped" | "queued") => "stopped",
        _ => "done",
    })
}

/// What the chat waits for in the database, in one line: an agent's question and its words, or a sign-in.
pub fn waits(conn: &Connection, chat: &str) -> Result<Option<String>, Error> {
    let asked = "SELECT coalesce(a.name, 'An agent'), q.question FROM questions q LEFT JOIN agents a ON a.id = q.agent
                 WHERE q.chat_id = ?1 AND q.answered_ts IS NULL AND q.closed_ts IS NULL ORDER BY q.asked_ts LIMIT 1";
    let asked: Option<(String, String)> = (conn.prepare_cached(asked)?)
        .query_row([chat], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    if let Some((who, question)) = asked {
        return Ok(Some(format!("A question from {who}: {question}")));
    }
    let signing =
        "SELECT coalesce(a.name, 'An agent') FROM turns t LEFT JOIN agents a ON a.id = t.agent_id
                   WHERE t.chat_id = ?1 AND t.status = 'waiting_sign_in' LIMIT 1";
    let signing: Option<String> = (conn.prepare_cached(signing)?)
        .query_row([chat], |r| r.get(0))
        .optional()?;
    Ok(signing.map(|who| format!("A sign-in: {who}'s account needs signing in again.")))
}

/// The agents' words in the chat since `app` last sent a message there (since the person's last message, when it
/// sent none), oldest first: each as "Name: words", a long one cut as `read_chat` cuts it.
pub fn said_since(conn: &Connection, chat: &str, app: &str) -> Result<Vec<String>, Error> {
    let sql = "SELECT m.id, m.body, coalesce(a.name, 'An agent') FROM messages m
               LEFT JOIN turns t ON t.id = m.turn_id LEFT JOIN agents a ON a.id = t.agent_id
               WHERE m.chat_id = ?1 AND m.role = 'assistant' AND m.kind = 'text' AND m.ts > coalesce(
                   (SELECT max(u.ts) FROM messages u JOIN from_app f ON f.message_id = u.id
                    WHERE u.chat_id = ?1 AND f.app = ?2),
                   (SELECT max(u.ts) FROM messages u WHERE u.chat_id = ?1 AND u.role = 'user'), 0)
               ORDER BY m.ts, m.rowid";
    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map(params![chat, app], |r| {
        let (id, body, name): (String, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
        Ok(format!("{name}: {}", crate::text::kept(&body, &id)))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
