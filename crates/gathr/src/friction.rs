//! Friction: what got in an agent's way, as it reported it before agents lost their friction tool
//! (docs/archive, decision 0048). The kept reports are still read: who made each, when, in which
//! chat and on which task.

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Error;

/// A kept report, as the window and `gathr diag friction` show it.
#[derive(Debug, Serialize)]
pub struct Friction {
    pub id: i64,
    pub time: String,
    pub chat: String,
    /// None once the chat is deleted: the report outlives it.
    pub chat_title: Option<String>,
    pub agent: Option<String>,
    pub agent_name: Option<String>,
    pub task: Option<i64>,
    pub what: String,
    #[serde(rename = "where")]
    pub place: Option<String>,
}

/// The reports made since `since_us`, newest first, at most `limit`.
pub fn list(conn: &Connection, since_us: i64, limit: i64) -> Result<Vec<Friction>, Error> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.ts, f.chat_id, c.title, f.agent_id, a.name, f.task, f.what, f.place
         FROM friction f LEFT JOIN chats c ON c.id = f.chat_id LEFT JOIN agents a ON a.id = f.agent_id
         WHERE f.ts >= ?1 ORDER BY f.id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![since_us, limit], |r| {
        Ok(Friction {
            id: r.get(0)?,
            time: crate::ids::utc(r.get(1)?),
            chat: r.get(2)?,
            chat_title: r.get(3)?,
            agent: r.get(4)?,
            agent_name: r.get(5)?,
            task: r.get(6)?,
            what: r.get(7)?,
            place: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
