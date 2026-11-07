//! Chats, turns, messages, raw engine frames and settings in the database.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, ulid};
use crate::redact::redact;

/// The longest raw body kept for a frame that isn't JSON (0011).
const MALFORMED_CAP: usize = 64 * 1024;

pub fn create_chat(conn: &Connection, title: &str) -> Result<Value, Error> {
    let (id, ts) = (ulid(now_us()), now_us());
    conn.execute(
        "INSERT INTO chats (id, title, created_ts) VALUES (?1, ?2, ?3)",
        params![id, title, ts],
    )?;
    Ok(json!({ "id": id, "title": title, "created": crate::ids::utc(ts) }))
}

pub fn chats(conn: &Connection) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.id, c.title, c.created_ts, (SELECT count(*) FROM messages m WHERE m.chat_id = c.id)
         FROM chats c ORDER BY c.created_ts DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "title": r.get::<_, String>(1)?,
            "created": crate::ids::utc(r.get(2)?),
            "messages": r.get::<_, i64>(3)?,
        }))
    })?;
    Ok(Value::Array(rows.collect::<Result<_, _>>()?))
}

pub fn chat_exists(conn: &Connection, chat: &str) -> Result<bool, Error> {
    Ok(conn
        .query_row("SELECT 1 FROM chats WHERE id = ?1", [chat], |_| Ok(()))
        .optional()?
        .is_some())
}

/// A chat's messages in order, and its turns with how each ended.
pub fn history(conn: &Connection, chat: &str) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, turn_id, ts, role, kind, body FROM messages WHERE chat_id = ?1 ORDER BY ts, id",
    )?;
    let messages = stmt
        .query_map([chat], |r| {
            Ok(message_json(
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                &r.get::<_, String>(3)?,
                &r.get::<_, String>(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = conn.prepare_cached(
        "SELECT id, status, why, substr(trace_id, 1, 8), created_ts, ended_ts FROM turns
         WHERE chat_id = ?1 ORDER BY created_ts",
    )?;
    let turns = stmt
        .query_map([chat], |r| {
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                "status": r.get::<_, String>(1)?,
                "why": r.get::<_, Option<String>>(2)?,
                "ref": r.get::<_, String>(3)?,
                "created": crate::ids::utc(r.get(4)?),
                "ended": r.get::<_, Option<i64>>(5)?.map(crate::ids::utc),
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({ "chat": chat, "messages": messages, "turns": turns }))
}

/// A message as the API shows it; tool messages carry their JSON body as an object.
pub fn message_json(
    id: String,
    turn: Option<String>,
    ts: i64,
    role: &str,
    kind: &str,
    body: String,
) -> Value {
    let body = match kind {
        "tool" => serde_json::from_str(&body).unwrap_or(Value::String(body)),
        _ => Value::String(body),
    };
    json!({ "id": id, "turn": turn, "time": crate::ids::utc(ts), "role": role, "kind": kind, "body": body })
}

pub fn add_message(
    conn: &Connection,
    chat: &str,
    turn: &str,
    role: &str,
    kind: &str,
    body: &str,
) -> Result<Value, Error> {
    let ts = now_us();
    let id = ulid(ts);
    conn.prepare_cached(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?
    .execute(params![id, chat, turn, ts, role, kind, body])?;
    Ok(message_json(
        id,
        Some(turn.into()),
        ts,
        role,
        kind,
        body.into(),
    ))
}

pub fn set_message_body(conn: &Connection, id: &str, body: &str) -> Result<(), Error> {
    conn.prepare_cached("UPDATE messages SET body = ?2 WHERE id = ?1")?
        .execute(params![id, body])?;
    Ok(())
}

pub fn add_turn(conn: &Connection, chat: &str, id: &str, trace: &str) -> Result<(), Error> {
    conn.prepare_cached(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES (?1, ?2, ?3, 'queued', ?4)",
    )?
    .execute(params![id, chat, trace, now_us()])?;
    Ok(())
}

/// Moves a turn on; an ending status also stamps when it ended.
pub fn set_turn(conn: &Connection, id: &str, status: &str, why: Option<&str>) -> Result<(), Error> {
    let ended = matches!(status, "done" | "stopped" | "failed").then(now_us);
    conn.prepare_cached("UPDATE turns SET status = ?2, why = ?3, ended_ts = ?4 WHERE id = ?1")?
        .execute(params![id, status, why, ended])?;
    Ok(())
}

/// A turn left waiting from before: id, chat, trace, status and the person's message.
pub struct Waiting {
    pub turn: String,
    pub chat: String,
    pub trace: String,
    pub status: String,
    pub text: String,
}

/// Turns that were waiting when the service last stopped, oldest first. Turns that were
/// running then can't be continued: they're marked failed, with why.
pub fn take_over(conn: &Connection) -> Result<Vec<Waiting>, Error> {
    conn.execute(
        "UPDATE turns SET status = 'failed', why = 'the service stopped during this turn', ended_ts = ?1
         WHERE status = 'running'",
        [now_us()],
    )?;
    let mut stmt = conn.prepare(
        "SELECT t.id, t.chat_id, t.trace_id, t.status,
                (SELECT body FROM messages m WHERE m.turn_id = t.id AND m.role = 'user' ORDER BY ts LIMIT 1)
         FROM turns t WHERE t.status IN ('queued', 'waiting_sign_in') ORDER BY t.created_ts",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Waiting {
            turn: r.get(0)?,
            chat: r.get(1)?,
            trace: r.get(2)?,
            status: r.get(3)?,
            text: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn engine_session(conn: &Connection, chat: &str) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row(
            "SELECT engine_session FROM chats WHERE id = ?1",
            [chat],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

pub fn set_engine_session(conn: &Connection, chat: &str, session: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE chats SET engine_session = ?2 WHERE id = ?1",
        params![chat, session],
    )?;
    Ok(())
}

pub fn setting(conn: &Connection, key: &str) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
            r.get(0)
        })
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: Option<&str>) -> Result<(), Error> {
    match value {
        Some(value) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = ?2",
            params![key, value],
        )?,
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [key])?,
    };
    Ok(())
}

/// One engine frame, as it came, redacted before it's written (0011).
pub struct RawFrame<'a> {
    pub chat: &'a str,
    pub turn: Option<&'a str>,
    pub pid: u32,
    pub direction: &'static str,
    pub line: &'a str,
}

pub fn add_raw(conn: &Connection, frame: &RawFrame<'_>) -> Result<(), Error> {
    let parsed: Option<Value> = serde_json::from_str(frame.line).ok();
    let (kind, malformed, body) = match (&parsed, frame.direction) {
        (_, "err") => ("stderr".to_string(), false, redact(frame.line)),
        (Some(value), _) => (
            crate::claude::frames::kind(value),
            false,
            redact(frame.line),
        ),
        (None, _) => {
            let mut cut = frame.line.len().min(MALFORMED_CAP);
            while !frame.line.is_char_boundary(cut) {
                cut -= 1;
            }
            ("malformed".to_string(), true, redact(&frame.line[..cut]))
        }
    };
    conn.prepare_cached(
        "INSERT INTO engine_raw (ts, chat_id, turn_id, pid, direction, frame_type, size, malformed, body)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        now_us(),
        frame.chat,
        frame.turn,
        frame.pid,
        frame.direction,
        kind,
        frame.line.len() as i64,
        malformed,
        body
    ])?;
    Ok(())
}
