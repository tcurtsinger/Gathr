//! Where the person left off (0077): per thread of a chat, the newest message they have seen there and
//! their own fold, kept by the service so a reload, a restart and their other windows read the same.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::Error;

/// One thread's mark as the window sends it: what it has seen up to, and the person's fold, if either
/// changed. A mark older than the one kept leaves it where it is.
#[derive(Deserialize)]
pub struct Mark {
    pub topic: String,
    #[serde(default)]
    pub seen_us: Option<i64>,
    /// `Some(None)`, sent as null: back to Gathr's rule. Left out: unchanged.
    #[serde(default, deserialize_with = "some")]
    pub folded: Option<Option<bool>>,
}

fn some<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Option<bool>>, D::Error> {
    Option::<bool>::deserialize(d).map(Some)
}

/// Keeps `marks` for `chat`, on the caller's transaction, all or none: a thread that isn't the chat's
/// refuses the lot. Answers each mark's row as kept.
pub fn mark(conn: &Connection, chat: &str, marks: &[Mark]) -> Result<Vec<Value>, Error> {
    for m in marks {
        if !is_thread(conn, chat, &m.topic)? {
            return Err(Error::Refused(format!(
                "No thread {} in this chat.",
                m.topic
            )));
        }
    }
    let mut kept = Vec::with_capacity(marks.len());
    for m in marks {
        conn.prepare_cached(
            "INSERT INTO reads (chat_id, topic, seen_ts, folded) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (chat_id, topic) DO UPDATE SET seen_ts = MAX(seen_ts, excluded.seen_ts),
                 folded = CASE WHEN ?5 THEN excluded.folded ELSE folded END",
        )?
        .execute(params![
            chat,
            m.topic,
            m.seen_us.unwrap_or(0).max(0),
            m.folded.flatten(),
            m.folded.is_some()
        ])?;
        kept.push(row(conn, chat, &m.topic)?);
    }
    Ok(kept)
}

/// The chat's marks on its history, by thread.
pub fn on_history(conn: &Connection, chat: &str, history: &mut Value) -> Result<(), Error> {
    let mut statement = conn.prepare_cached(
        "SELECT topic, seen_ts, folded FROM reads WHERE chat_id = ?1 ORDER BY topic",
    )?;
    let rows = statement.query_map([chat], read)?;
    history["reads"] = Value::Array(rows.collect::<rusqlite::Result<_>>()?);
    Ok(())
}

fn row(conn: &Connection, chat: &str, topic: &str) -> Result<Value, Error> {
    Ok(conn
        .prepare_cached(
            "SELECT topic, seen_ts, folded FROM reads WHERE chat_id = ?1 AND topic = ?2",
        )?
        .query_row([chat, topic], read)?)
}

fn read(r: &rusqlite::Row<'_>) -> rusqlite::Result<Value> {
    let (topic, seen, folded): (String, i64, Option<bool>) = (r.get(0)?, r.get(1)?, r.get(2)?);
    Ok(json!({ "topic": topic, "seen_us": seen, "folded": folded }))
}

/// A thread of the chat, by its key as history names it (0075), or '' for the main line.
fn is_thread(conn: &Connection, chat: &str, topic: &str) -> Result<bool, Error> {
    if topic.is_empty() {
        return Ok(true);
    }
    let sql = match topic.split_once(':') {
        Some(("m", _)) => "SELECT 1 FROM messages WHERE id = ?2 AND chat_id = ?1",
        Some(("r" | "b", _)) => "SELECT 1 FROM turns WHERE id = ?2 AND chat_id = ?1",
        _ => return Ok(false),
    };
    let key = topic.split_once(':').map_or("", |(_, k)| k);
    // The window's own thread for what a turn said after the person's message entered it (0078):
    // `b:<turn>:<message>`, kept as its turn's.
    let key = match topic.starts_with("b:") {
        true => key.split_once(':').map_or(key, |(turn, _)| turn),
        false => key,
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([chat, key], |_| Ok(()))
        .optional()?
        .is_some())
}
