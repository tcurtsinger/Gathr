//! How full each agent's context is in each chat (compaction plan, K1): the prompt of its last model
//! call and its model's window, as the engine reports them, kept across restarts.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::now_us;

/// The agent's last call filled this much: its context as now shown.
pub fn filled(conn: &Connection, chat: &str, agent: &str, used: i64) -> Result<Value, Error> {
    conn.execute(
        "INSERT INTO agent_context (chat_id, agent_id, used, updated_ts) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (chat_id, agent_id) DO UPDATE SET used = ?3, updated_ts = ?4",
        params![chat, agent, used, now_us()],
    )?;
    Ok(of(conn, chat, agent)?.unwrap_or(Value::Null))
}

/// The agent's model holds this much: its context as now shown.
pub fn window(conn: &Connection, chat: &str, agent: &str, window: i64) -> Result<Value, Error> {
    conn.execute(
        "INSERT INTO agent_context (chat_id, agent_id, window, updated_ts) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (chat_id, agent_id) DO UPDATE SET window = ?3, updated_ts = ?4",
        params![chat, agent, window, now_us()],
    )?;
    Ok(of(conn, chat, agent)?.unwrap_or(Value::Null))
}

/// `{used, window}`, either null until the engine has said; none before its first call.
pub fn of(conn: &Connection, chat: &str, agent: &str) -> Result<Option<Value>, Error> {
    Ok(conn
        .query_row(
            "SELECT used, window FROM agent_context WHERE chat_id = ?1 AND agent_id = ?2",
            params![chat, agent],
            |r| {
                let (used, window): (Option<i64>, Option<i64>) = (r.get(0)?, r.get(1)?);
                Ok(json!({ "used": used, "window": window }))
            },
        )
        .optional()?)
}

/// Each member of a chat's history with how full its context is (`context`, null before its first call).
pub fn on_members(conn: &Connection, chat: &str, history: &mut Value) -> Result<(), Error> {
    let Some(members) = history["members"].as_array_mut() else {
        return Ok(());
    };
    for member in members {
        let agent = member["agent"].as_str().unwrap_or_default().to_string();
        member["context"] = of(conn, chat, &agent)?.unwrap_or(Value::Null);
    }
    Ok(())
}
