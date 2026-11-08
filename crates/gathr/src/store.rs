//! Chats, turns, messages, raw engine frames and accounts in the database.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, ulid};
use crate::redact::redact;

/// The longest raw body kept for a frame that isn't JSON (0011).
const MALFORMED_CAP: usize = 64 * 1024;

/// A new chat, with the project folder it works in, if any (0013: set only here).
pub fn create_chat(conn: &Connection, title: &str, project: Option<&str>) -> Result<Value, Error> {
    let (id, ts) = (ulid(now_us()), now_us());
    conn.execute(
        "INSERT INTO chats (id, title, created_ts, project) VALUES (?1, ?2, ?3, ?4)",
        params![id, title, ts, project],
    )?;
    Ok(json!({ "id": id, "title": title, "created": crate::ids::utc(ts), "project": project }))
}

pub fn chats(conn: &Connection) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.id, c.title, c.created_ts, (SELECT count(*) FROM messages m WHERE m.chat_id = c.id),
                (SELECT agent_id FROM turns t WHERE t.chat_id = c.id ORDER BY t.created_ts DESC LIMIT 1),
                c.project
         FROM chats c ORDER BY c.created_ts DESC",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(json!({
            "id": r.get::<_, String>(0)?,
            "title": r.get::<_, String>(1)?,
            "created": crate::ids::utc(r.get(2)?),
            "messages": r.get::<_, i64>(3)?,
            // The last turn's agent, so the composer's picker reopens on it.
            "agent": r.get::<_, Option<String>>(4)?,
            "project": r.get::<_, Option<String>>(5)?,
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

/// A chat's messages in order, its turns with how each ended, its project and its agents.
///
/// Messages keep their turn's place, taken when the turn started (0028): replies from agents
/// working at once never interleave, and a turn not started yet shows last.
pub fn history(conn: &Connection, chat: &str) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "{MESSAGE} WHERE m.chat_id = ?1 ORDER BY t.position IS NULL, t.position, m.ts, m.id"
    ))?;
    let messages = stmt
        .query_map([chat], message_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = conn.prepare_cached(
        "SELECT id, status, why, substr(trace_id, 1, 8), created_ts, ended_ts, agent_id, position
         FROM turns WHERE chat_id = ?1 ORDER BY created_ts",
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
                "agent": r.get::<_, Option<String>>(6)?,
                "position": r.get::<_, Option<i64>>(7)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let agent = turns.last().map_or(Value::Null, |t| t["agent"].clone());
    let project: Option<String> = conn
        .query_row("SELECT project FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let members = crate::team::members(conn, chat)?;
    Ok(
        json!({ "chat": chat, "agent": agent, "project": project, "members": members,
        "messages": messages, "turns": turns }),
    )
}

/// A message's columns, with the agent of its turn: who it was for, or who wrote it.
const MESSAGE: &str = "SELECT m.id, m.turn_id, m.ts, m.role, m.kind, m.body, t.agent_id
     FROM messages m LEFT JOIN turns t ON t.id = m.turn_id";

/// A message as the API shows it; tool messages carry their JSON body as an object.
fn message_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let (kind, body): (String, String) = (r.get(4)?, r.get(5)?);
    let body = match kind.as_str() {
        "tool" => serde_json::from_str(&body).unwrap_or(Value::String(body)),
        _ => Value::String(body),
    };
    Ok(json!({
        "id": r.get::<_, String>(0)?,
        "turn": r.get::<_, Option<String>>(1)?,
        "time": crate::ids::utc(r.get(2)?),
        "role": r.get::<_, String>(3)?,
        "kind": kind,
        "body": body,
        "agent": r.get::<_, Option<String>>(6)?,
    }))
}

fn message(conn: &Connection, id: &str) -> Result<Value, Error> {
    let sql = format!("{MESSAGE} WHERE m.id = ?1");
    Ok(conn.prepare_cached(&sql)?.query_row([id], message_row)?)
}

/// A new chat's name until its first message names it.
pub const UNTITLED: &str = "New chat";

/// A chat's name from its first message: the first line with text, up to 60 characters.
fn title_from(text: &str) -> Option<String> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    if line.chars().count() <= 60 {
        return Some(line.to_string());
    }
    let cut: String = line.chars().take(59).collect();
    Some(format!("{}…", cut.trim_end()))
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
    if let Some(title) = title_from(body).filter(|_| role == "user") {
        conn.prepare_cached("UPDATE chats SET title = ?2 WHERE id = ?1 AND title = ?3")?
            .execute(params![chat, title, UNTITLED])?;
    }
    message(conn, &id)
}

pub fn set_message_body(conn: &Connection, id: &str, body: &str) -> Result<(), Error> {
    conn.prepare_cached("UPDATE messages SET body = ?2 WHERE id = ?1")?
        .execute(params![id, body])?;
    Ok(())
}

/// Moves a message to another turn: one sent into a turn that the engine answered after it.
/// The message as it is now, for the live socket.
pub fn move_message(conn: &Connection, id: &str, turn: &str) -> Result<Value, Error> {
    conn.prepare_cached("UPDATE messages SET turn_id = ?2 WHERE id = ?1")?
        .execute(params![id, turn])?;
    message(conn, id)
}

/// A new turn, run as `agent`, or on the oldest account when it's sent to none.
pub fn add_turn(
    conn: &Connection,
    chat: &str,
    id: &str,
    trace: &str,
    agent: Option<&str>,
) -> Result<(), Error> {
    conn.prepare_cached(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5)",
    )?
    .execute(params![id, chat, trace, now_us(), agent])?;
    Ok(())
}

/// Moves a turn on; an ending status also stamps when it ended. A turn leaving the wait takes
/// the chat's next place (0028).
pub fn set_turn(conn: &Connection, id: &str, status: &str, why: Option<&str>) -> Result<(), Error> {
    let ended = matches!(status, "done" | "stopped" | "failed").then(now_us);
    let placed = !matches!(status, "queued" | "waiting_sign_in");
    conn.prepare_cached(
        "UPDATE turns SET status = ?2, why = ?3, ended_ts = ?4,
             position = CASE WHEN ?5 AND position IS NULL
                 THEN (SELECT COALESCE(max(p.position), 0) + 1 FROM turns p WHERE p.chat_id = turns.chat_id)
                 ELSE position END
         WHERE id = ?1",
    )?
    .execute(params![id, status, why, ended, placed])?;
    Ok(())
}

/// A turn left waiting from before: id, chat, trace, status and the person's message.
pub struct Waiting {
    pub turn: String,
    pub chat: String,
    pub trace: String,
    pub status: String,
    pub text: String,
    pub agent: Option<String>,
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
                (SELECT body FROM messages m WHERE m.turn_id = t.id AND m.role = 'user' ORDER BY ts LIMIT 1),
                t.agent_id
         FROM turns t WHERE t.status IN ('queued', 'waiting_sign_in') ORDER BY t.created_ts",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Waiting {
            turn: r.get(0)?,
            chat: r.get(1)?,
            trace: r.get(2)?,
            status: r.get(3)?,
            text: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            agent: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The agent's engine session in the chat (`None`: turns sent to no agent), if it lives in
/// `account`'s login folder: only there can it resume.
pub fn engine_session(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    account: &str,
) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row(
            "SELECT session_id FROM engine_sessions
             WHERE chat_id = ?1 AND agent_key = ?2 AND account_id = ?3",
            [chat, agent.unwrap_or_default(), account],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_engine_session(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    session: &str,
    account: &str,
) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO engine_sessions (chat_id, agent_key, account_id, session_id) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (chat_id, agent_key) DO UPDATE SET account_id = ?3, session_id = ?4",
        params![chat, agent.unwrap_or_default(), account, session],
    )?;
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
