//! Each kind's outcome: what `gathr diag words` shows, and what part K's trial reads.

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
    /// `finish_task` calls turned away on the turn.
    pub finish_refused: i64,
    /// Turns on a task that ended done with no report and nothing open: Gathr asked for one, or
    /// gave up asking.
    pub no_report: i64,
}

// `messages` has no index on `turn_id`, so its tool rows are read in one pass, not per turn.
const SQL: &str = r#"
WITH tooled AS (
    SELECT DISTINCT turn_id FROM messages WHERE kind = 'tool' AND ts >= ?1
), turned_away AS (
    SELECT trace_id, count(*) AS n FROM events
    WHERE name = 'gathr.tool.call' AND ts >= ?1 AND status = 'error' AND error_type = 'finish_task'
    GROUP BY trace_id
), unreported AS (
    SELECT DISTINCT trace_id, task_id FROM events
    WHERE name = 'gathr.task.state' AND ts >= ?1
      AND (json_extract(attrs, '$."gathr.task.state"') = 'ready'
           OR json_extract(attrs, '$."gathr.task.reason"') = ?2)
)
SELECT coalesce(u.opened_by, 'unrecorded'), u.opened_version,
       count(*),
       sum(u.status = 'done' AND t.turn_id IS NULL),
       coalesce(sum(a.n), 0),
       sum(u.task IS NOT NULL AND u.status = 'done' AND r.trace_id IS NOT NULL)
FROM turns u
LEFT JOIN tooled t ON t.turn_id = u.id
LEFT JOIN turned_away a ON a.trace_id = u.trace_id
LEFT JOIN unreported r ON r.trace_id = u.trace_id AND r.task_id = CAST(u.task AS TEXT)
WHERE u.created_ts >= ?1
GROUP BY 1, 2 ORDER BY 1, 2"#;

/// The turns made since `since_us`, by kind and version. A turn's own task set back to ready on
/// its trace is the one ask for a report: every other ready is set on a fresh trace, or is a new
/// task of the person's, which isn't the turn's own.
pub fn counted(conn: &Connection, since_us: i64) -> Result<Vec<Counted>, Error> {
    let mut stmt = conn.prepare(SQL)?;
    let rows = stmt.query_map(
        rusqlite::params![since_us, crate::tasks::held::NO_REPORT],
        |r| {
            Ok(Counted {
                kind: r.get(0)?,
                version: r.get(1)?,
                turns: r.get(2)?,
                no_tool: r.get(3)?,
                finish_refused: r.get(4)?,
                no_report: r.get(5)?,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}
