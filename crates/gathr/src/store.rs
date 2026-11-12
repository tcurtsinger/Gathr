//! Chats, turns, messages, raw engine frames and accounts in the database.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, ulid};
use crate::redact::{redact, redact_json};

/// The longest raw body kept for a frame that isn't JSON (0011).
const MALFORMED_CAP: usize = 64 * 1024;

/// A new chat, with the project it was made in and that project's folder, if any (0013: set only
/// here; the column `project` holds the folder).
pub fn create_chat(
    conn: &Connection,
    title: &str,
    (project, folder): (Option<&str>, Option<&str>),
) -> Result<Value, Error> {
    let (id, ts) = (ulid(now_us()), now_us());
    conn.execute(
        "INSERT INTO chats (id, title, created_ts, project_id, project) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, title, ts, project, folder],
    )?;
    Ok(
        json!({ "id": id, "title": title, "created": crate::ids::utc(ts), "project": project,
        "folder": folder }),
    )
}

/// Puts a chat with no turns yet in a project: "New" makes the chat before its project is picked.
pub fn put_in_project(
    conn: &Connection,
    chat: &str,
    (project, folder): (&str, &str),
) -> Result<(), Error> {
    let moved = conn.execute(
        "UPDATE chats SET project_id = ?2, project = ?3
         WHERE id = ?1 AND NOT EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1)",
        params![chat, project, folder],
    )?;
    match moved {
        0 => Err(Error::Refused("the chat has messages already".into())),
        _ => Ok(()),
    }
}

/// Whether a turn of chat `c` runs (0040). It names the index's own condition first, which is what
/// lets SQLite read `turns_live` instead of the chat's turns; `NEEDS` does the same.
const WORKING: &str = "EXISTS (SELECT 1 FROM turns s WHERE s.chat_id = c.id
    AND s.status IN ('running', 'waiting_sign_in') AND s.status = 'running')";

/// Whether chat `c` waits on the person, as far as the database knows: a turn waits for a sign-in,
/// or a question to them is open. An approval waiting is the service's to add (it's in no table).
const NEEDS: &str = "(EXISTS (SELECT 1 FROM turns s WHERE s.chat_id = c.id
        AND s.status IN ('running', 'waiting_sign_in') AND s.status = 'waiting_sign_in')
    OR EXISTS (SELECT 1 FROM tasks k WHERE k.chat_id = c.id AND k.kind = 'person' AND k.state = 'ready'))";

/// The chats in the list, newest first, each with whether it's `working` and `needs` the person;
/// with `archived`, the archived ones, last archived first, which carry neither.
pub fn chats(conn: &Connection, archived: bool) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT c.id, c.title, c.created_ts, (SELECT count(*) FROM messages m WHERE m.chat_id = c.id),
                (SELECT agent_id FROM turns t WHERE t.chat_id = c.id ORDER BY t.created_ts DESC LIMIT 1),
                c.project_id, c.project, c.archived_ts, {WORKING}, {NEEDS}
         FROM chats c WHERE (c.archived_ts IS NOT NULL) = ?1
         ORDER BY coalesce(c.archived_ts, c.created_ts) DESC"
    ))?;
    let rows = stmt.query_map([archived], |r| {
        let mut chat = json!({
            "id": r.get::<_, String>(0)?,
            "title": r.get::<_, String>(1)?,
            "created": crate::ids::utc(r.get(2)?),
            "messages": r.get::<_, i64>(3)?,
            // The last turn's agent, so the composer's picker reopens on it.
            "agent": r.get::<_, Option<String>>(4)?,
            "project": r.get::<_, Option<String>>(5)?,
            "folder": r.get::<_, Option<String>>(6)?,
            "archived": r.get::<_, Option<i64>>(7)?.map(crate::ids::utc),
        });
        if !archived {
            chat["working"] = r.get::<_, bool>(8)?.into();
            chat["needs"] = r.get::<_, bool>(9)?.into();
        }
        Ok(chat)
    })?;
    Ok(Value::Array(rows.collect::<Result<_, _>>()?))
}

/// One chat's `working` and `needs` as its row in the list has them; none for an archived chat, or
/// one that's gone.
pub fn chat_state(conn: &Connection, chat: &str) -> Result<Option<(bool, bool)>, Error> {
    let sql =
        format!("SELECT {WORKING}, {NEEDS} FROM chats c WHERE c.id = ?1 AND c.archived_ts IS NULL");
    let row = |r: &rusqlite::Row<'_>| Ok((r.get(0)?, r.get(1)?));
    Ok(conn
        .prepare_cached(&sql)?
        .query_row([chat], row)
        .optional()?)
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
        "SELECT id, status, why, substr(trace_id, 1, 8), created_ts, ended_ts, agent_id, position, task,
                answers, held
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
                // The task the turn works on: its work sits under the turn that gave it.
                "task": r.get::<_, Option<i64>>(8)?,
                // The turn it answers: the thread it belongs to (F8b).
                "answers": r.get::<_, Option<String>>(9)?,
                // What holds a queued turn back (F10): `pause` or `usage`.
                "held": r.get::<_, Option<String>>(10)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let agent = turns.last().map_or(Value::Null, |t| t["agent"].clone());
    let (project, folder, archived): (Option<String>, Option<String>, Option<i64>) = conn
        .query_row(
            "SELECT project_id, project, archived_ts FROM chats WHERE id = ?1",
            [chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get::<_, Option<i64>>(2)?)),
        )
        .optional()?
        .unwrap_or_default();
    let members = crate::team::members(conn, chat)?;
    Ok(
        json!({ "chat": chat, "agent": agent, "project": project, "folder": folder,
        "archived": archived.map(crate::ids::utc), "members": members, "messages": messages, "turns": turns }),
    )
}

/// A message's columns, with the agent of its turn: who it was for, or who wrote it.
const MESSAGE: &str =
    "SELECT m.id, m.turn_id, m.ts, m.role, m.kind, m.body, t.agent_id, m.to_agent,
     m.copy_of FROM messages m LEFT JOIN turns t ON t.id = m.turn_id";

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
        // A message an agent sent one member: whom it's for.
        "to": r.get::<_, Option<String>>(7)?,
        // Gathr's copy of such a message, told to the member in its own turn: the message it carries.
        "copy_of": r.get::<_, Option<String>>(8)?,
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

/// How many of the turn's own messages are saved: its reply's blocks and tools.
pub fn replies(conn: &Connection, chat: &str, turn: &str) -> Result<i64, Error> {
    Ok(conn
        .prepare_cached("SELECT count(*) FROM messages WHERE chat_id = ?1 AND turn_id = ?2 AND role = 'assistant'",
        )?
        .query_row([chat, turn], |r| r.get(0))?)
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

/// A new turn, run as `agent`, or on the oldest account when it's sent to none; on `task` when it
/// works on one (0010); answering the turn `answers` when Gathr starts it (F8b).
pub fn add_turn(
    conn: &Connection,
    chat: &str,
    id: &str,
    trace: &str,
    (agent, task, answers): (Option<&str>, Option<i64>, Option<&str>),
) -> Result<(), Error> {
    conn.prepare_cached(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, task, answers)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?7)",
    )?
    .execute(params![id, chat, trace, now_us(), agent, task, answers])?;
    Ok(())
}

/// Moves a turn on; an ending status also stamps when it ended. A turn leaving the wait takes
/// the chat's next place (0028).
pub fn set_turn(conn: &Connection, id: &str, status: &str, why: Option<&str>) -> Result<(), Error> {
    let ended = matches!(status, "done" | "stopped" | "failed").then(now_us);
    let placed = !matches!(status, "queued" | "waiting_sign_in");
    conn.prepare_cached(
        "UPDATE turns SET status = ?2, why = ?3, ended_ts = ?4,
             held = CASE WHEN ?2 = 'queued' THEN held END,
             position = CASE WHEN ?5 AND position IS NULL
                 THEN (SELECT COALESCE(max(p.position), 0) + 1 FROM turns p WHERE p.chat_id = turns.chat_id)
                 ELSE position END
         WHERE id = ?1",
    )?
    .execute(params![id, status, why, ended, placed])?;
    Ok(())
}

/// A turn's place in its chat (none until it starts) and when it was made, as history shows them.
pub fn turn_place(conn: &Connection, id: &str) -> Result<(Option<i64>, String), Error> {
    let sql = "SELECT position, created_ts FROM turns WHERE id = ?1";
    let row = |r: &rusqlite::Row<'_>| Ok((r.get(0)?, crate::ids::utc(r.get(1)?)));
    Ok(conn.prepare_cached(sql)?.query_row([id], row)?)
}

/// What holds a queued turn back (`pause`, `usage`, `claude`, `codex`, or none), and why in words.
pub fn set_held(
    conn: &Connection,
    id: &str,
    held: Option<&str>,
    why: Option<&str>,
) -> Result<(), Error> {
    conn.prepare_cached(
        "UPDATE turns SET held = ?2, why = ?3 WHERE id = ?1 AND status = 'queued'",
    )?
    .execute(params![id, held, why])?;
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
    pub task: Option<i64>,
    pub answers: Option<String>,
    /// The person wrote it, rather than Gathr: a pause never holds it (0010).
    pub person: bool,
}

/// Turns that were waiting when the service last stopped, oldest first, each with its whole ask:
/// a message another agent sent before it started joined it. Turns that were running then can't
/// be continued: they're marked failed, with why.
pub fn take_over(conn: &Connection) -> Result<Vec<Waiting>, Error> {
    conn.execute(
        "UPDATE turns SET status = 'failed', why = 'the service stopped during this turn', ended_ts = ?1
         WHERE status = 'running'",
        [now_us()],
    )?;
    // An update held these until the service before stopped: nothing holds them now (0032).
    conn.execute(
        "UPDATE turns SET held = NULL, why = NULL WHERE status = 'queued' AND held = 'update'",
        [],
    )?;
    let mut stmt = conn.prepare(
        "SELECT t.id, t.chat_id, t.trace_id, t.status,
                (SELECT group_concat(body, char(10) || char(10) ORDER BY ts) FROM messages m
                 WHERE m.turn_id = t.id AND m.role IN ('user', 'gathr')),
                t.agent_id, t.task, t.answers,
                (SELECT role FROM messages m WHERE m.turn_id = t.id AND m.role IN ('user', 'gathr')
                 ORDER BY ts LIMIT 1) = 'user'
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
            task: r.get(6)?,
            answers: r.get(7)?,
            person: r.get::<_, Option<bool>>(8)?.unwrap_or(false),
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
            redact_json(value).to_string(),
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
