//! The person's name, which agents call them by, and their assistant: the agent a message that names
//! nobody goes to in a chat with no project and no agents yet. Both are in Settings.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

const NAME: &str = "person_name";
const ASSISTANT: &str = "assistant";
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
            "Your name is one line of up to {NAME_MOST} characters"
        )));
    }
    put(conn, NAME, name)
}

/// The assistant's agent id, if one is set.
pub fn assistant(conn: &Connection) -> Result<Option<String>, Error> {
    get(conn, ASSISTANT)
}

/// Sets the assistant to an agent that exists, or none.
pub fn set_assistant(conn: &Connection, agent: Option<&str>) -> Result<(), Error> {
    if let Some(agent) = agent
        && crate::agents::get(conn, agent)?.is_none()
    {
        return Err(Error::Refused("no such agent".into()));
    }
    put(conn, ASSISTANT, agent)
}

/// A removed agent is nobody's assistant.
pub fn forget(conn: &Connection, agent: &str) -> Result<(), Error> {
    let sql = "DELETE FROM settings WHERE key = ?1 AND value = ?2";
    conn.execute(sql, params![ASSISTANT, agent])?;
    Ok(())
}
