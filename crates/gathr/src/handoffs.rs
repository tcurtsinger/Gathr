//! Messages between agents, counted (0010's hand-off counter): `@Name` in an agent's words when its
//! turn ends (0081). Each is a record, never memory: its depth is the sender's chain plus one, and a
//! chain starts over after the person writes in the chat. Past the limit the chat pauses.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

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
    // Only the person's words start a chain over.
    let person: Option<i64> = conn.query_row(
        "SELECT max(ts) FROM messages WHERE chat_id = ?1 AND role = 'user'",
        [chat],
        |r| r.get(0),
    )?;
    let since = person.unwrap_or(0);
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

/// A member an agent's words named (0081): what it is told, `{from}: {words}`, with the words of the
/// sender's messages that name it, and the first of them, which Gathr's copy carries.
#[derive(Debug)]
pub struct Named {
    pub to: String,
    pub told: String,
    pub message: String,
    pub depth: i64,
}

/// The member a chain went past the limit at: nobody from it on is woken.
#[derive(Debug)]
pub struct Over {
    pub to: String,
    pub message: String,
    pub limit: i64,
    pub depth: i64,
    /// The pause line, as saved.
    pub line: Value,
}

/// Whom a turn's words woke.
#[derive(Debug, Default)]
pub struct Mentions {
    pub chat: String,
    pub from: String,
    pub named: Vec<Named>,
    pub over: Option<Over>,
}

/// The members `turn`'s words name with `@Name` (0081), but its own agent and, for a check, the lead
/// it checks, each recorded as a hand-off from the turn's agent. Its words are its text messages in
/// the chat. Past the limit the chat pauses, the line saying so goes in the turn, and nobody from
/// that member on is named. None in a chat of one agent, or an
/// archived one.
pub fn mentions(conn: &Connection, turn: &str, trace: &str) -> Result<Mentions, Error> {
    let found: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT chat_id, agent_id FROM turns WHERE id = ?1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((chat, Some(from))) = found else {
        return Ok(Mentions::default());
    };
    let mut woke = Mentions {
        chat: chat.clone(),
        from: from.clone(),
        ..Mentions::default()
    };
    match crate::archive::archived(conn, &chat) {
        Ok(None) => {}
        Ok(Some(_)) | Err(Error::Refused(_)) => return Ok(woke),
        Err(e) => return Err(e),
    }
    let team = crate::team::members(conn, &chat)?;
    let Some(sender) = team
        .iter()
        .find(|m| m.agent == from)
        .filter(|_| team.len() > 1)
    else {
        return Ok(woke);
    };
    let said: Vec<(String, String)> = conn
        .prepare_cached(
            "SELECT id, body FROM messages WHERE turn_id = ?1 AND role = 'assistant' AND kind = 'text'
             AND to_agent IS NULL ORDER BY ts, id",
        )?
        .query_map([turn], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    // A check's words reach the lead whose answer it checks in the lead's last word (0080): naming
    // the lead there wakes nobody.
    let lead: Option<String> = conn
        .prepare_cached(
            "SELECT a.agent_id FROM turns u JOIN turns a ON a.id = u.answers
             WHERE u.id = ?1 AND u.opened_by = 'check'",
        )?
        .query_row([turn], |r| r.get::<_, Option<String>>(0))
        .optional()?
        .flatten();
    let names = |body: &str, agent: &str| {
        let named = crate::team::mentioned(&team, body);
        named.iter().any(|m| m.agent == agent)
    };
    let all: Vec<&str> = said.iter().map(|(_, body)| body.as_str()).collect();
    for member in crate::team::mentioned(&team, &all.join("\n\n")) {
        if member.agent == from || lead.as_deref() == Some(member.agent.as_str()) {
            continue;
        }
        let naming: Vec<&(String, String)> = said
            .iter()
            .filter(|(_, b)| names(b, &member.agent))
            .collect();
        let Some((message, _)) = naming.first() else {
            continue;
        };
        let words: Vec<&str> = naming.iter().map(|(_, body)| body.as_str()).collect();
        let counted = record(conn, &chat, &from, &member.agent)?;
        if let Some(limit) = counted.over {
            crate::pauses::pause(conn, "chat", &chat, false, trace)?;
            let line =
                crate::store::add_message(conn, &chat, turn, "gathr", "text", &paused_line(limit))?;
            woke.over = Some(Over {
                to: member.agent.clone(),
                message: message.clone(),
                limit,
                depth: counted.depth,
                line,
            });
            break;
        }
        woke.named.push(Named {
            to: member.agent.clone(),
            told: format!("{}: {}", sender.name, words.join("\n\n")),
            message: message.clone(),
            depth: counted.depth,
        });
    }
    Ok(woke)
}
