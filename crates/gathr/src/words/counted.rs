//! Each kind's outcome: what `gathr diag words` shows.

use rusqlite::Connection;
use serde::Serialize;

use crate::Error;

/// The turns opened by one kind at one version, since a time.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Counted {
    /// The kind, or `unrecorded` for a turn made before turns recorded it.
    pub kind: String,
    pub version: Option<i64>,
    pub turns: i64,
    /// Ended done with no tool call.
    pub no_tool: i64,
}

// `messages` has no index on `turn_id`, so its tool rows are read in one pass, not per turn.
const SQL: &str = r#"
WITH tooled AS (
    SELECT DISTINCT turn_id FROM messages WHERE kind = 'tool' AND ts >= ?1
)
SELECT coalesce(u.opened_by, 'unrecorded'), u.opened_version,
       count(*),
       sum(u.status = 'done' AND t.turn_id IS NULL)
FROM turns u
LEFT JOIN tooled t ON t.turn_id = u.id
WHERE u.created_ts >= ?1
GROUP BY 1, 2 ORDER BY 1, 2"#;

/// The turns made since `since_us`, by kind and version.
pub fn counted(conn: &Connection, since_us: i64) -> Result<Vec<Counted>, Error> {
    let mut stmt = conn.prepare(SQL)?;
    let rows = stmt.query_map([since_us], |r| {
        Ok(Counted {
            kind: r.get(0)?,
            version: r.get(1)?,
            turns: r.get(2)?,
            no_tool: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
