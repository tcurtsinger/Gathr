//! Messages between agents, counted (plans/agent-messages.md, 0010's hand-off counter). Each
//! message is a record, never memory: its depth is the sender's chain plus one, and a chain starts
//! over after the person writes in the chat or a task there settles. Past the limit the chat
//! pauses.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// The limit until the person picks another (Settings > General).
pub const DEFAULT_LIMIT: i64 = 50;
const KEY: &str = "handoff_limit";
/// How a limit of none is saved: the settings table holds text.
const NO_LIMIT: &str = "none";

/// The hand-offs a chain may reach before the chat pauses, or `None` for no limit.
pub fn limit(conn: &Connection) -> Result<Option<i64>, Error> {
    let saved: Option<String> = conn
        .query_row("SELECT value FROM settings WHERE key = ?1", [KEY], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(match saved.as_deref() {
        None => Some(DEFAULT_LIMIT),
        Some(NO_LIMIT) => None,
        Some(n) => Some(n.parse().unwrap_or(DEFAULT_LIMIT)),
    })
}

/// Saves the limit: 1 to 1,000 hand-offs, or `None` for no limit.
pub fn set_limit(conn: &Connection, limit: Option<i64>) -> Result<(), Error> {
    if limit.is_some_and(|n| !(1..=1000).contains(&n)) {
        return Err(Error::Refused(
            "The hand-off limit is 1 to 1000, or none".into(),
        ));
    }
    let value = limit.map_or(NO_LIMIT.to_string(), |n| n.to_string());
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![KEY, value],
    )?;
    Ok(())
}

/// A hand-off as recorded: its depth, and the limit it went past, if it did.
pub struct Counted {
    pub depth: i64,
    pub over: Option<i64>,
}

/// Records a message from `from` to `to` in the chat as a hand-off.
pub fn record(conn: &Connection, chat: &str, from: &str, to: &str) -> Result<Counted, Error> {
    let settled = format!(
        "SELECT max(updated_ts) FROM tasks WHERE chat_id = ?1 AND state IN {}",
        crate::tasks::SETTLED
    );
    let person: Option<i64> = conn.query_row(
        "SELECT max(ts) FROM messages WHERE chat_id = ?1 AND role = 'user'",
        [chat],
        |r| r.get(0),
    )?;
    let task: Option<i64> = conn.query_row(&settled, [chat], |r| r.get(0))?;
    let since = person.max(task).unwrap_or(0);
    let chain: Option<i64> = conn.query_row(
        "SELECT max(depth) FROM handoffs WHERE chat_id = ?1 AND to_agent = ?2 AND created_ts > ?3",
        params![chat, from, since],
        |r| r.get(0),
    )?;
    let depth = chain.unwrap_or(0) + 1;
    conn.execute(
        "INSERT INTO handoffs (chat_id, from_agent, to_agent, depth, created_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat, from, to, depth, crate::ids::now_us()],
    )?;
    let over = limit(conn)?.filter(|&n| depth > n);
    Ok(Counted { depth, over })
}

/// What the chat says when a chain goes past the limit and pauses it.
pub fn paused_line(limit: i64) -> String {
    format!("Paused after {limit} hand-offs between agents. Resume to carry on.")
}
