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

/// Whether a turn of chat `c` runs (0040), or a command an agent left running in the background
/// there (0084). It names the index's own condition first, which is what lets SQLite read
/// `turns_live` instead of the chat's turns; `NEEDS` does the same.
const WORKING: &str = "(EXISTS (SELECT 1 FROM turns s WHERE s.chat_id = c.id
    AND s.status IN ('running', 'waiting_sign_in') AND s.status = 'running')
    OR EXISTS (SELECT 1 FROM background g JOIN turns s ON s.id = g.turn_id WHERE s.chat_id = c.id))";

/// Whether a turn of chat `c` will start by itself soon: queued behind its agent's work elsewhere, or
/// for an engine download or an update. One held for usage or by a pause isn't: that chat has stopped.
const QUEUED: &str = "EXISTS (SELECT 1 FROM turns s
    WHERE s.status IN ('queued', 'waiting_sign_in') AND s.status = 'queued' AND s.chat_id = c.id
      AND coalesce(s.held, '') NOT IN ('usage', 'pause'))";

/// Whether chat `c` waits on the person, as far as the database knows: a turn waits for a sign-in,
/// or a question to them is open. An approval waiting is the service's to add (it's in no table).
const NEEDS: &str = "(EXISTS (SELECT 1 FROM turns s WHERE s.chat_id = c.id
        AND s.status IN ('running', 'waiting_sign_in') AND s.status = 'waiting_sign_in')
    OR EXISTS (SELECT 1 FROM questions n WHERE n.chat_id = c.id AND n.answered_ts IS NULL AND n.closed_ts IS NULL))";

/// Whether chat `c` waits on the person for an answer of theirs: `NEEDS` but a sign-in.
const ASKS: &str = "EXISTS (SELECT 1 FROM questions n WHERE n.chat_id = c.id AND n.answered_ts IS NULL AND n.closed_ts IS NULL)";

/// Whether an agent of chat `c` can't run (0071): it waits for a sign-in, or its last turn there
/// failed and wasn't retried.
const TROUBLE: &str = "(EXISTS (SELECT 1 FROM turns s WHERE s.chat_id = c.id
        AND s.status IN ('running', 'waiting_sign_in') AND s.status = 'waiting_sign_in')
    OR EXISTS (SELECT 1 FROM chat_members cm WHERE cm.chat_id = c.id
        AND (SELECT l.status = 'failed' FROM turns l
             WHERE l.chat_id = c.id AND l.agent_id = cm.agent_id
             ORDER BY l.created_ts DESC, l.rowid DESC LIMIT 1)))";

/// One chat's state as its row in the list has it: whether a turn of it runs (0040), whether it
/// needs the person, for a notice, and the most pressing of three marks for its row (0071).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ChatState {
    pub working: bool,
    pub needs: bool,
    pub mark: Option<&'static str>,
}

impl ChatState {
    /// From the row's columns: working, needs, asks the person, and can't run.
    fn read(r: &rusqlite::Row<'_>, at: usize) -> rusqlite::Result<ChatState> {
        let (working, needs, asks, trouble): (bool, bool, bool, bool) =
            (r.get(at)?, r.get(at + 1)?, r.get(at + 2)?, r.get(at + 3)?);
        let mark = match () {
            _ if asks => Some("needs_you"),
            _ if trouble => Some("cant_run"),
            _ if working => Some("working"),
            _ => None,
        };
        Ok(ChatState {
            working,
            needs,
            mark,
        })
    }

    /// An approval waiting, which is in no table: the chat needs the person.
    pub fn asked(self) -> ChatState {
        ChatState {
            needs: true,
            mark: Some("needs_you"),
            ..self
        }
    }
}

/// The chats in the list, newest first, each with whether it's `working`, has a turn `queued`,
/// `needs` the person, its `mark` and whether it is the `assistant`'s chat; with `archived`, the
/// archived ones, last archived first, which carry none of them.
pub fn chats(conn: &Connection, archived: bool) -> Result<Value, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT c.id, c.title, c.created_ts, (SELECT count(*) FROM messages m WHERE m.chat_id = c.id),
                (SELECT agent_id FROM turns t WHERE t.chat_id = c.id ORDER BY t.created_ts DESC LIMIT 1),
                c.project_id, c.project, c.archived_ts, {WORKING}, {NEEDS}, {ASKS}, {TROUBLE}, {QUEUED},
                c.id IS (SELECT value FROM settings WHERE key = 'assistant_chat')
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
            let state = ChatState::read(r, 8)?;
            chat["working"] = state.working.into();
            chat["queued"] = r.get::<_, bool>(12)?.into();
            chat["needs"] = state.needs.into();
            chat["mark"] = state.mark.into();
            // The assistant's chat, which the sidebar shows as the assistant's row, in no list.
            chat["assistant"] = r.get::<_, bool>(13)?.into();
        }
        Ok(chat)
    })?;
    Ok(Value::Array(rows.collect::<Result<_, _>>()?))
}

/// One chat's state as its row in the list has it; none for an archived chat, or one that's gone.
pub fn chat_state(conn: &Connection, chat: &str) -> Result<Option<ChatState>, Error> {
    let sql = format!(
        "SELECT {WORKING}, {NEEDS}, {ASKS}, {TROUBLE} FROM chats c WHERE c.id = ?1 AND c.archived_ts IS NULL"
    );
    let row = |r: &rusqlite::Row<'_>| ChatState::read(r, 0);
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
    let mut stmt = conn.prepare_cached(&history_order())?;
    let messages = stmt
        .query_map([chat], message_row)?
        .collect::<Result<Vec<_>, _>>()?;
    let mut stmt = conn.prepare_cached(
        "SELECT id, status, why, substr(trace_id, 1, 8), created_ts, ended_ts, agent_id, position,
                answers, held, opened_by
         FROM turns WHERE chat_id = ?1 ORDER BY created_ts",
    )?;
    let turns = stmt
        .query_map([chat], |r| {
            let (status, why): (String, Option<String>) = (r.get(1)?, r.get(2)?);
            Ok(json!({
                "id": r.get::<_, String>(0)?,
                // Who or what stopped it (0051): the person, a restart, or Gathr for a reason of its own.
                "end": crate::ends::word(&status, why.as_deref()),
                "status": status,
                "why": why,
                "ref": r.get::<_, String>(3)?,
                "created": crate::ids::utc(r.get(4)?),
                "ended": r.get::<_, Option<i64>>(5)?.map(crate::ids::utc),
                "agent": r.get::<_, Option<String>>(6)?,
                "position": r.get::<_, Option<i64>>(7)?,
                // The turn it answers: a check's, the lead's answer it checks.
                "answers": r.get::<_, Option<String>>(8)?,
                // What holds a queued turn back (F10): `pause` or `usage`.
                "held": r.get::<_, Option<String>>(9)?,
                // The words that opened it (0056): a member's check of an answer draws its Checked line.
                "opened": r.get::<_, Option<String>>(10)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let agent = turns.last().map_or(Value::Null, |t| t["agent"].clone());
    type Row = (Option<String>, Option<String>, Option<i64>);
    let (project, folder, archived): Row = conn
        .query_row(
            "SELECT project_id, project, archived_ts FROM chats WHERE id = ?1",
            [chat],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?
        .unwrap_or_default();
    let members = crate::team::members(conn, chat)?;
    let questions = crate::questions::of_chat(conn, chat)?;
    Ok(
        json!({ "chat": chat, "agent": agent, "project": project, "folder": folder,
        "archived": archived.map(crate::ids::utc), "members": members, "messages": messages, "turns": turns,
        "questions": questions }),
    )
}

/// A chat's messages in the order history reads them.
fn history_order() -> String {
    format!(
        "{} WHERE m.chat_id = ?1 ORDER BY {PLACE} IS NULL, {PLACE}, m.ts, m.id",
        *MESSAGE
    )
}

/// A message's columns, with the agent of its turn: who it was for, or who wrote it. A project's brief is the person's
/// message whose turn it opened, and the assistant that proposed the project wrote it (`proposals`).
static MESSAGE: std::sync::LazyLock<String> = std::sync::LazyLock::new(|| {
    format!(
        "SELECT m.id, m.turn_id, m.ts, m.role, m.kind, m.body, t.agent_id, m.to_agent,
         m.copy_of, m.picks, m.reply_to,
         CASE WHEN m.role = 'user' THEN (SELECT r.due_ts FROM schedule_runs r WHERE r.message_id = m.id) END,
         CASE WHEN m.role = 'user' AND t.opened_by = 'brief' THEN
           (SELECT bt.agent_id FROM messages b JOIN turns bt ON bt.id = b.turn_id WHERE {}) END,
         CASE WHEN m.role = 'user' THEN (SELECT f.app FROM from_app f WHERE f.message_id = m.id) END
         FROM messages m LEFT JOIN turns t ON t.id = m.turn_id",
        crate::proposals::BRIEF_OF
    )
});

/// Where a message reads among a chat's turns: with its turn, last while that hasn't started. One
/// said in no turn (0051) reads where the window put it live (0073): with the latest-started turn
/// that had a message by then, by time among that turn's, and first when no turn had. One window over the chat's
/// messages in time order, whose frame takes the rows of the same time too: a subquery for each such message read
/// every message before it.
const PLACE: &str = "CASE WHEN m.turn_id IS NOT NULL THEN t.position ELSE COALESCE(
     max(t.position) OVER (ORDER BY m.ts), 0) END";

/// A message as the API shows it; tool, thinking and block messages carry their JSON body as an object.
fn message_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let (kind, body): (String, String) = (r.get(4)?, r.get(5)?);
    let role: String = r.get(3)?;
    let compacted = role == "gathr" && kind == "text" && crate::context::is_compacted_line(&body);
    let mut body = match kind.as_str() {
        "tool" | "thinking" | "block" => serde_json::from_str(&body).unwrap_or(Value::String(body)),
        _ => Value::String(body),
    };
    crate::steps::unlined(&mut body);
    let mut row = json!({
        "id": r.get::<_, String>(0)?,
        "turn": r.get::<_, Option<String>>(1)?,
        "time": crate::ids::utc(r.get(2)?),
        "role": role,
        "kind": kind,
        "body": body,
        "agent": r.get::<_, Option<String>>(6)?,
        // A message an agent sent one member: whom it's for.
        "to": r.get::<_, Option<String>>(7)?,
        // Gathr's copy of such a message, told to the member in its own turn: the message it carries.
        "copy_of": r.get::<_, Option<String>>(8)?,
        // The person's message: the names they picked from the `@` list, to draw blue.
        "picks": r.get::<_, Option<String>>(9)?.and_then(|p| serde_json::from_str::<Value>(&p).ok()),
        // A reply: the message it sits under.
        "reply_to": r.get::<_, Option<String>>(10)?,
    });
    // The quiet line a compaction left, which the window shows: only on that line.
    if compacted {
        row["compacted"] = json!(true);
    }
    // A scheduled run's words: when they were due, which the window shows under them.
    if let Some(due) = r.get::<_, Option<i64>>(11)? {
        row["scheduled"] = json!(crate::ids::utc(due));
    }
    // A project's brief: the assistant that wrote it, which the window shows under it.
    if let Some(from) = r.get::<_, Option<String>>(12)? {
        row["from"] = json!(from);
    }
    // Words an AI app outside Gathr sent for the person: the app's name, which the window shows under them.
    if let Some(app) = r.get::<_, Option<String>>(13)? {
        row["app"] = json!(app);
    }
    Ok(row)
}

/// A saved message, as the API shows it.
pub fn message(conn: &Connection, id: &str) -> Result<Value, Error> {
    let sql = format!("{} WHERE m.id = ?1", *MESSAGE);
    Ok(conn.prepare_cached(&sql)?.query_row([id], message_row)?)
}

/// A new chat's name until its first message names it.
pub const UNTITLED: &str = "New chat";

/// A chat's name from its first message: the first line with text, up to 60 characters. The lines
/// naming its files don't count, nor the @Names it starts with, unless it is only those.
pub fn title_from(text: &str) -> Option<String> {
    let words = text.lines().map(str::trim).filter(|line| !line.is_empty());
    let mut words = words.filter(|line| !line.starts_with(crate::files::ATTACHED));
    let line = words.next()?;
    let mut rest = line;
    while let Some(named) = rest.strip_prefix('@') {
        let Some((_, after)) = named.split_once(char::is_whitespace) else {
            rest = line;
            break;
        };
        rest = after.trim_start();
    }
    Some(crate::text::cut(rest, 60, false))
}

pub fn add_message(
    conn: &Connection,
    chat: &str,
    turn: &str,
    role: &str,
    kind: &str,
    body: &str,
) -> Result<Value, Error> {
    add_message_at(conn, chat, turn, role, kind, body, now_us())
}

/// When a message that began at `began` may say it began (a thought known only once it's over,
/// 0042): the chat reads in time order, so never before the turn's last message.
pub fn after_last(conn: &Connection, chat: &str, turn: &str, began: i64) -> Result<i64, Error> {
    let last: Option<i64> = conn
        .prepare_cached(
            "SELECT ts FROM messages WHERE chat_id = ?1 AND turn_id = ?2 ORDER BY ts DESC LIMIT 1",
        )?
        .query_row([chat, turn], |r| r.get(0))
        .optional()?;
    Ok(began.max(last.map_or(0, |ts| ts + 1)))
}

/// `add_message`, at a time of its own.
pub fn add_message_at(
    conn: &Connection,
    chat: &str,
    turn: &str,
    role: &str,
    kind: &str,
    body: &str,
    ts: i64,
) -> Result<Value, Error> {
    insert_message(conn, chat, turn, (role, kind), body, ts, None)
}

/// The person's message, with the names they picked from the window's `@` list (`picks::kept`).
pub fn add_person_message(
    conn: &Connection,
    chat: &str,
    turn: &str,
    body: &str,
    picks: Option<&str>,
) -> Result<Value, Error> {
    insert_message(conn, chat, turn, ("user", "text"), body, now_us(), picks)
}

/// The person's reply under a message (0076), answering `reply` (checked by `replies::check`).
pub fn add_reply(
    conn: &Connection,
    chat: &str,
    turn: &str,
    body: &str,
    picks: Option<&str>,
    reply: &crate::replies::Target,
) -> Result<Value, Error> {
    let ts = now_us();
    let id = ulid(ts);
    conn.prepare_cached(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, picks, reply_to)
         VALUES (?1, ?2, ?3, ?4, 'user', 'text', ?5, ?6, ?7)",
    )?
    .execute(params![id, chat, turn, ts, body, picks, reply.reply_to])?;
    message(conn, &id)
}

/// Gathr's copy of the message `of` in `turn`, with what the agent reads in it: a teammate's message,
/// or the person's message to several agents (0028). Never drawn.
pub fn add_copy(
    conn: &Connection,
    chat: &str,
    turn: &str,
    body: &str,
    of: &str,
) -> Result<Value, Error> {
    let ts = now_us();
    let id = ulid(ts);
    conn.prepare_cached(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, copy_of)
         VALUES (?1, ?2, ?3, ?4, 'gathr', 'text', ?5, ?6)",
    )?
    .execute(params![id, chat, turn, ts, body, of])?;
    message(conn, &id)
}

fn insert_message(
    conn: &Connection,
    chat: &str,
    turn: &str,
    (role, kind): (&str, &str),
    body: &str,
    ts: i64,
    picks: Option<&str>,
) -> Result<Value, Error> {
    let id = ulid(ts);
    let (body, reply_to) = match (role, kind) {
        ("assistant", "text") => crate::room::words(conn, turn, body)?,
        _ => (body, None),
    };
    conn.prepare_cached(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, picks, reply_to)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        id, chat, turn, ts, role, kind, body, picks, reply_to
    ])?;
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

/// A new turn, run as `agent`, or on the oldest account when it's sent to none; answering the turn
/// `answers` when Gathr starts it; with the kind of words that opened it.
pub fn add_turn(
    conn: &Connection,
    chat: &str,
    id: &str,
    trace: &str,
    (agent, answers): (Option<&str>, Option<&str>),
    opened: crate::words::Opened,
) -> Result<(), Error> {
    conn.prepare_cached(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, answers,
                            opened_by, opened_version)
         VALUES (?1, ?2, ?3, 'queued', ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        id,
        chat,
        trace,
        now_us(),
        agent,
        answers,
        opened.as_str(),
        opened.version()
    ])?;
    Ok(())
}

/// Moves a turn on; an ending status also stamps when it ended. A turn leaving the wait takes
/// the chat's next place (0028).
/// A turn that has ended never goes back to waiting: a start under way when its chat was archived
/// would otherwise put back a turn archiving stopped (F1).
pub fn set_turn(
    conn: &Connection,
    id: &str,
    status: &str,
    why: Option<&str>,
) -> Result<bool, Error> {
    let ended = matches!(status, "done" | "stopped" | "failed").then(now_us);
    let placed = !matches!(status, "queued" | "waiting_sign_in");
    conn.prepare_cached(
        "UPDATE turns SET status = ?2, why = ?3, ended_ts = ?4,
             held = CASE WHEN ?2 = 'queued' THEN held END,
             position = CASE WHEN ?5 AND position IS NULL
                 THEN (SELECT COALESCE(max(p.position), 0) + 1 FROM turns p WHERE p.chat_id = turns.chat_id)
                 ELSE position END
         WHERE id = ?1
           AND NOT (?2 IN ('queued', 'waiting_sign_in') AND status IN ('done', 'stopped', 'failed'))",
    )?
    .execute(params![id, status, why, ended, placed])
    .map(|n| n > 0)
    .map_err(Error::from)
}

/// A turn Gathr starts for an agent: a check, a last word, a carry-on. It's saved, queued with
/// Gathr's message as its ask, in the transaction of the change that calls for it: a stop right
/// after that commit loses nothing, as the next start takes it over.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    pub chat: String,
    pub agent: String,
    /// The turn it answers: a check's, the lead's answer it checks.
    pub answers: Option<String>,
    pub text: String,
    pub turn: String,
    pub trace: String,
    /// Gathr's message, as the live socket sends it.
    pub message: Value,
}

impl Start {
    pub fn queue(
        conn: &Connection,
        (chat, agent): (&str, &str),
        answers: Option<&str>,
        opened: crate::words::Opened,
        text: String,
    ) -> Result<Start, Error> {
        let saved = Self::save(conn, (chat, Some(agent)), answers, opened, &text)?;
        let (turn, trace, message) = saved;
        Ok(Start {
            chat: chat.to_string(),
            agent: agent.to_string(),
            answers: answers.map(String::from),
            text,
            turn,
            trace,
            message,
        })
    }

    /// Saves the turn and its ask, for an agent or (in a chat sent to no agent) none: its id, trace
    /// and message.
    pub fn save(
        conn: &Connection,
        (chat, agent): (&str, Option<&str>),
        answers: Option<&str>,
        opened: crate::words::Opened,
        text: &str,
    ) -> Result<(String, String, Value), Error> {
        let (turn, trace) = (ulid(now_us()), crate::ids::trace_id());
        add_turn(conn, chat, &turn, &trace, (agent, answers), opened)?;
        let message = add_message(conn, chat, &turn, "gathr", "text", text)?;
        Ok((turn, trace, message))
    }
}

/// A turn's place in its chat (none until it starts), when it was made, the words that opened it
/// and when it ended, as history shows them.
pub type TurnPlace = (Option<i64>, String, Option<String>, Option<String>);

/// A turn's `TurnPlace`.
pub fn turn_place(conn: &Connection, id: &str) -> Result<TurnPlace, Error> {
    let sql = "SELECT position, created_ts, opened_by, ended_ts FROM turns WHERE id = ?1";
    let row = |r: &rusqlite::Row<'_>| {
        let ended: Option<i64> = r.get(3)?;
        let created = crate::ids::utc(r.get(1)?);
        Ok((r.get(0)?, created, r.get(2)?, ended.map(crate::ids::utc)))
    };
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
    pub answers: Option<String>,
    /// The person wrote it, rather than Gathr: a pause never holds it (0010).
    pub person: bool,
}

/// A turn's whole ask, for a turn `t`: every message the person or Gathr put in it, in order. A
/// message another agent sent before it started joined it.
pub const ASK: &str = "(SELECT group_concat(body, char(10) || char(10) ORDER BY ts) FROM messages m
                        WHERE m.turn_id = t.id AND m.role IN ('user', 'gathr'))";

/// A turn that was running when the service stopped: a restart cut it. It stops, never fails: its message reached
/// the agent, which can carry on. Its reference leads to the cut.
fn cut(conn: &Connection, turn: &str) -> Result<(), Error> {
    let (chat, trace): (String, String) = conn.query_row(
        "SELECT chat_id, trace_id FROM turns WHERE id = ?1",
        [turn],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    set_turn(conn, turn, "stopped", Some(crate::ends::CUT_BY_RESTART))?;
    let event = crate::event::Event::point("gathr.turn.cut")
        .trace(&trace)
        .chat(&chat);
    event.attr("gathr.turn.id", turn).ok().insert(conn)?;
    Ok(())
}

/// At a start: every turn still running was cut off by the stop (`cut`), and the steps it left
/// running are stopped.
pub fn cut_running(conn: &Connection) -> Result<(), Error> {
    // By chat first: `messages` has no index on its turn. A thought's time isn't known, so it
    // gets none. `json_valid` first: a body that isn't JSON would fail the statement, and with
    // it every turn's recovery.
    conn.execute(
        "UPDATE messages SET body = json_set(body, '$.status', 'stopped')
         WHERE chat_id IN (SELECT chat_id FROM turns WHERE status = 'running')
           AND turn_id IN (SELECT id FROM turns WHERE status = 'running')
           AND kind IN ('tool', 'thinking')
           AND CASE WHEN json_valid(body) THEN json_extract(body, '$.status') END = 'running'",
        [],
    )?;
    let running: Vec<String> = conn
        .prepare("SELECT id FROM turns WHERE status = 'running'")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for turn in running {
        cut(conn, &turn)?;
    }
    Ok(())
}

/// Turns that were waiting when the service last stopped, oldest first, each with its whole ask:
/// a message another agent sent before it started joined it. The turns that were running then were
/// cut first, in `carry::recover`. When that failed, a turn answering one that still reads running
/// waits for the next start: a carry-on run now would end before its turn was cut, and the next
/// start would save that turn a second one.
pub fn take_over(conn: &Connection) -> Result<Vec<Waiting>, Error> {
    // An update held these until the service before stopped: nothing holds them now (0032).
    conn.execute(
        "UPDATE turns SET held = NULL, why = NULL WHERE status = 'queued' AND held = 'update'",
        [],
    )?;
    // A copy of the person's message and their answer are the person's too (0028): a pause never
    // holds the turn they opened.
    let mut stmt = conn.prepare(&format!(
        "SELECT t.id, t.chat_id, t.trace_id, t.status, {ASK}, t.agent_id, t.answers,
                (SELECT role FROM messages m WHERE m.turn_id = t.id AND m.role IN ('user', 'gathr')
                 ORDER BY ts LIMIT 1) = 'user' OR t.opened_by IN ('picked', 'reply')
         FROM turns t WHERE t.status IN ('queued', 'waiting_sign_in')
           AND NOT EXISTS (SELECT 1 FROM turns was WHERE was.id = t.answers AND was.status = 'running')
         ORDER BY t.created_ts",
    ))?;
    let rows = stmt.query_map([], |r| {
        Ok(Waiting {
            turn: r.get(0)?,
            chat: r.get(1)?,
            trace: r.get(2)?,
            status: r.get(3)?,
            text: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
            agent: r.get(5)?,
            answers: r.get(6)?,
            person: r.get::<_, Option<bool>>(7)?.unwrap_or(false),
        })
    })?;
    let mut waiting: Vec<Waiting> = rows.collect::<Result<_, _>>()?;
    // A reply's agent reads what it answers first, as it would have before the restart (0076).
    for w in &mut waiting {
        if let Some(line) = crate::replies::of_turn(conn, &w.turn)? {
            w.text = format!("{line}\n\n{}", w.text);
        }
    }
    Ok(waiting)
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
         ON CONFLICT (chat_id, agent_key) DO UPDATE SET account_id = ?3, session_id = ?4,
             summed = CASE WHEN account_id = ?3 AND session_id = ?4 THEN summed ELSE 0 END",
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

#[cfg(test)]
mod tests {
    use rusqlite::{StatementStatus, params};

    /// The steps SQLite takes to read the history of a chat of `turns` turns of 9 messages, each followed by a line
    /// said in no turn.
    fn steps(turns: i64) -> i32 {
        let path = std::env::temp_dir().join(format!("gathr-order-{}.db", crate::ids::uuid()));
        let conn = crate::db::open(&path).unwrap();
        conn.execute(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 0)",
            [],
        )
        .unwrap();
        let mut ts = 0;
        for turn in 0..turns {
            let id = format!("t{turn}");
            conn.execute(
                "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, position)
                 VALUES (?1, 'c', 'x', 'done', ?2, ?3)",
                params![id, ts, turn + 1],
            )
            .unwrap();
            for i in 0..10 {
                ts += 1;
                let (turn_id, role) = match i < 9 {
                    true => (Some(id.as_str()), "assistant"),
                    false => (None, "gathr"),
                };
                conn.execute(
                    "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body)
                     VALUES (?1, 'c', ?2, ?3, ?4, 'text', '')",
                    params![format!("m{ts}"), turn_id, ts, role],
                )
                .unwrap();
            }
        }
        let mut stmt = conn.prepare(&super::history_order()).unwrap();
        let read = stmt.query_map(["c"], |_| Ok(())).unwrap().count();
        assert_eq!(read as i64, turns * 10);
        let steps = stmt.get_status(StatementStatus::VmStep);
        drop(stmt);
        drop(conn);
        for end in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{end}", path.display()));
        }
        steps
    }

    /// Found in review: the order once read every earlier message for each line said in no turn, so a long chat
    /// with many such lines took seconds to open. Twice the chat is about twice the steps, not four times.
    #[test]
    fn the_history_order_grows_with_the_chat_not_with_its_lines_said_in_no_turn() {
        let (small, large) = (steps(100), steps(200));
        assert!(
            large < small * 3,
            "{small} steps for 1,000 messages, {large} for 2,000"
        );
    }
}
