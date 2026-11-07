//! `gathr diag`: read-only answers from the event record (docs/decisions/0019-troubleshooting.md).
//! Every answer is JSON; the command line can print it as text.

use rusqlite::Connection;
use rusqlite::types::ValueRef;
use serde_json::{Map, Value, json};

use crate::crash_log::CrashLog;
use crate::data::{DataDir, ServiceInfo};
use crate::{Error, db, http};

/// The most rows `query` returns.
pub const QUERY_CAP: usize = 1000;
const CRASH_LINES: usize = 20;

/// The service now, the database, the last start-up health check and the crash log's last lines.
/// Answers even when the service is down or the database won't open.
pub fn health(data: &DataDir) -> Value {
    let database = match db::open_read_only(&data.db()) {
        Ok((conn, version)) => json!({
            "opened": true,
            "path": data.db(),
            "schema_version": version,
            "size_bytes": std::fs::metadata(data.db()).map(|m| m.len()).unwrap_or(0),
            "last_start": last_start_health(&conn).unwrap_or(Value::Null),
        }),
        Err(e) => json!({ "opened": false, "path": data.db(), "why": e.to_string() }),
    };
    let crash_log = CrashLog::new(data.crash_log());
    json!({
        "service": service_now(data),
        "database": database,
        "crash_log": { "path": crash_log.path(), "last_lines": crash_log.last_lines(CRASH_LINES) },
    })
}

fn service_now(data: &DataDir) -> Value {
    let Ok(info) = ServiceInfo::read(data) else {
        return json!({ "running": false, "why": "no service.json: never started on this data folder" });
    };
    match http::call(info.port, &info.token, "GET", "/health") {
        Ok(r) if r.status == 200 => json!({ "running": true, "health": r.json() }),
        Ok(r) => {
            json!({ "running": false, "why": format!("port {} answered {}", info.port, r.status) })
        }
        Err(e) => {
            json!({ "running": false, "why": format!("nothing answers on port {}: {e}", info.port) })
        }
    }
}

fn last_start_health(conn: &Connection) -> Result<Value, Error> {
    let rows = query(
        conn,
        "SELECT time, attrs FROM event_log WHERE name = 'gathr.health' ORDER BY ts DESC LIMIT 1",
    )?;
    Ok(rows["rows"].get(0).cloned().unwrap_or(Value::Null))
}

/// Everything in one trace, in order, with each row's offset from the first.
pub fn trace(conn: &Connection, reference: &str) -> Result<Value, Error> {
    let reference = reference.trim().to_ascii_lowercase();
    if reference.len() < 8 || !reference.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(Error::Other(format!(
            "{reference:?} isn't a reference: 8 or more hex characters"
        )));
    }
    // Trace ids are lowercase hex, so every id starting with the reference sorts in [ref, ref + "g").
    let mut stmt = conn.prepare(
        "SELECT time, ts, name, phase, status, error_type, duration_us, trace_id, span_id, parent_span_id, conversation_id, attrs
         FROM event_log WHERE trace_id >= ?1 AND trace_id < ?1 || 'g' ORDER BY ts, id LIMIT ?2",
    )?;
    let mut rows = rows_as_json(&mut stmt, rusqlite::params![reference, QUERY_CAP as i64])?;
    if rows.is_empty() {
        return Err(Error::Other(format!("no events for reference {reference}")));
    }
    let first_ts = rows[0]["ts"].as_i64().unwrap_or(0);
    for row in &mut rows {
        let offset_ms = (row["ts"].as_i64().unwrap_or(0) - first_ts) as f64 / 1000.0;
        row.as_object_mut()
            .map(|r| r.insert("offset_ms".into(), offset_ms.into()));
    }
    Ok(json!({ "reference": reference, "events": rows }))
}

/// Failures since `since_us`, grouped by cause (event name and error type), newest group first,
/// each with its five newest references. Grouped in SQL, so no cap hides a count or a cause.
pub fn failures(conn: &Connection, since_us: i64) -> Result<Value, Error> {
    let mut stmt = conn.prepare(
        "SELECT name, coalesce(error_type, '') AS error_type, count(*) AS count, max(time) AS latest
         FROM event_log WHERE status = 'error' AND ts >= ?1
         GROUP BY name, coalesce(error_type, '') ORDER BY max(ts) DESC",
    )?;
    let mut groups = rows_as_json(&mut stmt, [since_us])?;
    let mut refs = conn.prepare(
        "SELECT ref FROM event_log
         WHERE status = 'error' AND ts >= ?1 AND name = ?2 AND coalesce(error_type, '') = ?3 AND ref IS NOT NULL
         GROUP BY ref ORDER BY max(ts) DESC LIMIT 5",
    )?;
    for group in &mut groups {
        let (name, error_type) = (group["name"].as_str(), group["error_type"].as_str());
        let references: Vec<Value> = refs
            .query_map(rusqlite::params![since_us, name, error_type], |r| {
                r.get::<_, String>(0)
            })?
            .map(|r| r.map(Value::from))
            .collect::<Result<_, _>>()?;
        group["references"] = references.into();
    }
    Ok(json!({ "since": crate::ids::utc(since_us), "groups": groups }))
}

/// Read-only SQL, capped at `QUERY_CAP` rows. The connection is read-only, so writes fail.
pub fn query(conn: &Connection, sql: &str) -> Result<Value, Error> {
    let mut stmt = conn.prepare(sql)?;
    if !stmt.readonly() {
        return Err(Error::Other(
            "diag query only reads: that statement would write".into(),
        ));
    }
    let mut rows = rows_as_json(&mut stmt, [])?;
    let truncated = rows.len() > QUERY_CAP;
    rows.truncate(QUERY_CAP);
    Ok(json!({ "rows": rows, "truncated": truncated }))
}

/// Up to `QUERY_CAP + 1` rows as JSON objects keyed by column name.
fn rows_as_json(
    stmt: &mut rusqlite::Statement<'_>,
    params: impl rusqlite::Params,
) -> Result<Vec<Value>, Error> {
    let names: Vec<String> = stmt.column_names().map(String::from).collect();
    let mut rows = stmt.query(params)?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        let mut object = Map::new();
        for (i, name) in names.iter().enumerate() {
            object.insert(name.clone(), cell(row.get_ref(i)?, name));
        }
        out.push(Value::Object(object));
        if out.len() > QUERY_CAP {
            break;
        }
    }
    Ok(out)
}

fn cell(value: ValueRef<'_>, column: &str) -> Value {
    match value {
        ValueRef::Null => Value::Null,
        ValueRef::Integer(i) => i.into(),
        ValueRef::Real(f) => f.into(),
        ValueRef::Text(t) => {
            let text = String::from_utf8_lossy(t);
            // The view's attrs column is JSON text: show it as JSON, not as a string of JSON.
            match column {
                "attrs" => serde_json::from_str(&text).unwrap_or_else(|_| text.into()),
                _ => text.into(),
            }
        }
        ValueRef::Blob(b) => format!(
            "<{} bytes: select json(attrs), or use the event_log view>",
            b.len()
        )
        .into(),
    }
}
