//! The person's name, which agents call them by, and their assistant: the agent a message that names
//! nobody goes to in a chat with no project and no agents yet. There is one whenever there is an
//! agent: the one picked on the Agents screen, else the first. The assistant has a chat of its own
//! that never ends, where it brings an agent in for a subject and sends it out after, each with a
//! quiet line (a stay); a new assistant takes that chat over.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::team::{self, Member, Role};

const NAME: &str = "person_name";
const ASSISTANT: &str = "assistant";
/// The assistant's chat, by id: cleared when it is archived or deleted.
const CHAT: &str = "assistant_chat";
/// The assistant's chat's stored title, which Search and Archived show.
pub const TITLE: &str = "Assistant";
/// The longest name the person can give.
pub const NAME_MOST: usize = 50;

fn get(conn: &Connection, key: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT value FROM settings WHERE key = ?1";
    Ok(conn.query_row(sql, [key], |r| r.get(0)).optional()?)
}

fn put(conn: &Connection, key: &str, value: Option<&str>) -> Result<(), Error> {
    match value {
        Some(value) => conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?,
        None => conn.execute("DELETE FROM settings WHERE key = ?1", [key])?,
    };
    Ok(())
}

/// What the person asked to be called, if they said.
pub fn person_name(conn: &Connection) -> Result<Option<String>, Error> {
    get(conn, NAME)
}

/// Saves the person's name, trimmed; blank or `None` clears it.
pub fn set_person_name(conn: &Connection, name: Option<&str>) -> Result<(), Error> {
    let name = name.map(str::trim).filter(|n| !n.is_empty());
    if name.is_some_and(|n| n.chars().count() > NAME_MOST || n.contains(['\n', '\r'])) {
        return Err(Error::Refused(format!(
            "One line, {NAME_MOST} characters at most"
        )));
    }
    put(conn, NAME, name)
}

/// The assistant's agent id: the one set, else the first agent. None only when there are no agents.
pub fn assistant(conn: &Connection) -> Result<Option<String>, Error> {
    let sql = "SELECT id FROM agents WHERE removed_ts IS NULL
               ORDER BY id IS (SELECT value FROM settings WHERE key = ?1) DESC, created_ts, id LIMIT 1";
    Ok(conn.query_row(sql, [ASSISTANT], |r| r.get(0)).optional()?)
}

/// Sets the assistant to an agent that exists.
pub fn set_assistant(conn: &Connection, agent: &str) -> Result<(), Error> {
    if crate::agents::get(conn, agent)?.is_none() {
        return Err(Error::Refused("no such agent".into()));
    }
    put(conn, ASSISTANT, Some(agent))
}

/// A removed agent is nobody's assistant: the first agent left is.
pub fn forget(conn: &Connection, agent: &str) -> Result<(), Error> {
    let sql = "DELETE FROM settings WHERE key = ?1 AND value = ?2";
    conn.execute(sql, params![ASSISTANT, agent])?;
    Ok(())
}

/// The assistant's chat, while it is there and not archived.
pub fn chat(conn: &Connection) -> Result<Option<String>, Error> {
    let sql = "SELECT c.id FROM chats c JOIN settings s ON s.key = ?1 AND s.value = c.id
               WHERE c.archived_ts IS NULL";
    Ok(conn
        .prepare_cached(sql)?
        .query_row([CHAT], |r| r.get(0))
        .optional()?)
}

/// Whether `chat` is the assistant's chat.
pub fn is_chat(conn: &Connection, chat: &str) -> Result<bool, Error> {
    Ok(self::chat(conn)?.as_deref() == Some(chat))
}

/// The assistant's chat as a new chat's answer shows it: made at the first message, with the
/// assistant as its lead; the one there is when another window made it first.
pub fn make_chat(conn: &mut Connection) -> Result<Value, Error> {
    let tx = conn.transaction()?;
    let id = match self::chat(&tx)? {
        Some(id) => id,
        None => {
            let agent = assistant(&tx)?.ok_or_else(|| Error::Refused("no agents yet".into()))?;
            let made = crate::store::create_chat(&tx, TITLE, (None, None))?;
            let id = made["id"].as_str().unwrap_or_default().to_string();
            team::replace(&tx, &id, &[(agent, Role::Supervisor)])?;
            put(&tx, CHAT, Some(&id))?;
            id
        }
    };
    let created: i64 = tx.query_row("SELECT created_ts FROM chats WHERE id = ?1", [&id], |r| {
        r.get(0)
    })?;
    tx.commit()?;
    Ok(
        json!({ "id": id, "title": TITLE, "created": crate::ids::utc(created), "project": null,
        "folder": null, "assistant": true }),
    )
}

/// An archived or deleted chat is the assistant's no more: the row opens a new one.
pub fn let_go(conn: &Connection, chat: &str) -> Result<(), Error> {
    let sql = "DELETE FROM settings WHERE key = ?1 AND value = ?2";
    conn.execute(sql, params![CHAT, chat])?;
    Ok(())
}

/// Records that `agent` `joined` or `left` the chat, when it is the assistant's: the line, as the
/// chat's history and the live event carry it.
pub fn stay_in(
    conn: &Connection,
    chat: &str,
    agent: &str,
    change: &str,
) -> Result<Option<Value>, Error> {
    if !is_chat(conn, chat)? {
        return Ok(None);
    }
    let ts = crate::ids::now_us();
    conn.execute(
        "INSERT INTO stays (chat_id, agent_id, change, ts) VALUES (?1, ?2, ?3, ?4)",
        params![chat, agent, change, ts],
    )?;
    Ok(Some(
        json!({ "agent": agent, "change": change, "time": crate::ids::utc(ts) }),
    ))
}

/// The chat's lines, oldest first: none but in the assistant's chat.
pub fn stays(conn: &Connection, chat: &str) -> Result<Vec<Value>, Error> {
    let sql = "SELECT agent_id, change, ts FROM stays WHERE chat_id = ?1 ORDER BY ts, rowid";
    let mut statement = conn.prepare_cached(sql)?;
    let rows = statement.query_map([chat], |r| {
        let (agent, change, ts): (String, String, i64) = (r.get(0)?, r.get(1)?, r.get(2)?);
        Ok(json!({ "agent": agent, "change": change, "time": crate::ids::utc(ts) }))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// What a change to the assistant's chat's agents did: its agents now and its new lines.
pub struct Changed {
    pub chat: String,
    pub members: Vec<Member>,
    pub stays: Vec<Value>,
}

/// The assistant takes its chat over (a new one picked, or the one before deleted): it leads it,
/// the agent that led it leaves, each with its line. Its turn still running ends as it would. The
/// new one starts with no conversation of its engine there. None when nothing changed.
pub fn take_over(conn: &mut Connection) -> Result<Option<Changed>, Error> {
    let tx = conn.transaction()?;
    let (Some(chat), Some(new)) = (self::chat(&tx)?, assistant(&tx)?) else {
        return Ok(None);
    };
    let team = team::members(&tx, &chat)?;
    let old = team.iter().find(|m| m.role == Role::Supervisor);
    if old.is_some_and(|m| m.agent == new) {
        return Ok(None);
    }
    let old = old.map(|m| m.agent.clone());
    let staying = team
        .iter()
        .filter(|m| m.agent != new && Some(&m.agent) != old.as_ref());
    let mut seats = vec![(new.clone(), Role::Supervisor)];
    seats.extend(staying.map(|m| (m.agent.clone(), Role::Member)));
    if team::refused(&seats).is_some() {
        return Ok(None);
    }
    let mut stays = Vec::new();
    if !team.iter().any(|m| m.agent == new) {
        stays.extend(stay_in(&tx, &chat, &new, "joined")?);
    }
    if let Some(old) = &old {
        stays.extend(stay_in(&tx, &chat, old, "left")?);
    }
    team::replace(&tx, &chat, &seats)?;
    tx.execute(
        "DELETE FROM engine_sessions WHERE chat_id = ?1 AND agent_key = ?2",
        params![chat, new],
    )?;
    let members = team::members(&tx, &chat)?;
    tx.commit()?;
    Ok(Some(Changed {
        chat,
        members,
        stays,
    }))
}

/// `send_out`: the assistant, in its own chat, sends the agent called `name` out of it. Its
/// conversation there is kept, for when it is brought in again.
pub fn send_out(
    conn: &mut Connection,
    chat: &str,
    caller: Option<&str>,
    name: &str,
) -> Result<Changed, Error> {
    let tx = conn.transaction()?;
    let refuse = |why: &str| Err(Error::Refused(why.to_string()));
    if !is_chat(&tx, chat)? || caller.is_none() || assistant(&tx)?.as_deref() != caller {
        return refuse("only the person's assistant sends an agent out, in its own chat");
    }
    let team = team::members(&tx, chat)?;
    let Some(out) = team::named(&team, name) else {
        return refuse(&format!("nobody called {name} is in this chat"));
    };
    if Some(out.agent.as_str()) == caller {
        return refuse("you can't send yourself out");
    }
    let busy: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1 AND agent_id = ?2
                        AND status IN ('queued', 'waiting_sign_in', 'running'))",
        params![chat, out.agent],
        |r| r.get(0),
    )?;
    if busy {
        return refuse(&format!(
            "{} has a turn running or waiting here: send them out once it ends",
            out.name
        ));
    }
    let seats: Vec<(String, Role)> = team
        .iter()
        .filter(|m| m.agent != out.agent)
        .map(|m| (m.agent.clone(), m.role))
        .collect();
    team::replace(&tx, chat, &seats)?;
    let stays = stay_in(&tx, chat, &out.agent, "left")?
        .into_iter()
        .collect();
    let members = team::members(&tx, chat)?;
    tx.commit()?;
    Ok(Changed {
        chat: chat.to_string(),
        members,
        stays,
    })
}
