//! A chat's turns with the events recorded for each, for the window's Diagnostics (0023).

use std::collections::HashMap;

use rusqlite::Connection;
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{reference, utc};

/// Every turn of a chat, oldest first, each with what was asked and its events in order.
pub fn turns(conn: &Connection, chat: &str) -> Result<Value, Error> {
    let mut events: HashMap<String, Vec<Value>> = HashMap::new();
    let mut stmt = conn.prepare_cached(
        "SELECT t.id, e.ts - t.created_ts, e.name, e.phase, e.status, e.error_type, e.duration_us, json(e.attrs)
         FROM turns t JOIN events e ON e.trace_id = t.trace_id
         WHERE t.chat_id = ?1 ORDER BY e.ts, e.id",
    )?;
    let rows = stmt.query_map([chat], |r| {
        let attrs: Option<String> = r.get(7)?;
        let attrs = attrs.and_then(|a| serde_json::from_str::<Value>(&a).ok());
        Ok((
            r.get::<_, String>(0)?,
            json!({
                "at_ms": r.get::<_, i64>(1)? as f64 / 1000.0,
                "name": r.get::<_, String>(2)?,
                "phase": r.get::<_, String>(3)?,
                "status": r.get::<_, Option<String>>(4)?,
                "error": r.get::<_, Option<String>>(5)?,
                "duration_ms": r.get::<_, Option<i64>>(6)?.map(|us| us as f64 / 1000.0),
                "attrs": attrs,
            }),
        ))
    })?;
    for row in rows {
        let (turn, event) = row?;
        events.entry(turn).or_default().push(event);
    }

    let mut stmt = conn.prepare_cached(
        "SELECT t.id, t.status, t.why, t.trace_id, t.created_ts, t.ended_ts,
                (SELECT substr(m.body, 1, 120) FROM messages m
                 WHERE m.turn_id = t.id AND m.role = 'user' ORDER BY m.ts LIMIT 1)
         FROM turns t WHERE t.chat_id = ?1 ORDER BY t.created_ts",
    )?;
    let turns = stmt.query_map([chat], |r| {
        let (id, trace): (String, String) = (r.get(0)?, r.get(3)?);
        Ok(json!({
            "id": id,
            "status": r.get::<_, String>(1)?,
            "why": r.get::<_, Option<String>>(2)?,
            "ref": reference(&trace),
            "created": utc(r.get(4)?),
            "ended": r.get::<_, Option<i64>>(5)?.map(utc),
            "asked": r.get::<_, Option<String>>(6)?,
            "events": events.remove(&id).unwrap_or_default(),
        }))
    })?;
    let numbered = turns
        .enumerate()
        .map(|(i, turn)| {
            let mut turn = turn?;
            turn["n"] = json!(i + 1);
            Ok(turn)
        })
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;
    Ok(Value::Array(numbered))
}
