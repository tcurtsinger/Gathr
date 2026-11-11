//! The records of Gathr's checks runs (0036 C3): one per project and commit, with each command's
//! exit, duration and the end of its log. The service runs them; this keeps what they found.

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, utc};

/// The most of a command's output a step keeps: its end, where a failure says why.
pub const LOG_MOST: usize = 64 * 1024;

/// A run starts: its id.
pub fn start(conn: &Connection, project: &str, head: &str, trace: &str) -> Result<String, Error> {
    let ts = now_us();
    let id = crate::ids::ulid(ts);
    conn.execute(
        "INSERT INTO check_runs (id, project_id, head, state, trace_id, started_ts) VALUES (?1, ?2, ?3, 'running', ?4, ?5)",
        params![id, project, head, trace, ts],
    )?;
    Ok(id)
}

/// One command ran: its exit (`None` when it couldn't run or finish), how long, what it printed.
pub fn step(
    conn: &Connection,
    run: &str,
    (position, command): (usize, &str),
    exit: Option<i32>,
    duration_ms: i64,
    log: &str,
) -> Result<(), Error> {
    let start = log.len().saturating_sub(LOG_MOST);
    let start = (start..=log.len())
        .find(|&i| log.is_char_boundary(i))
        .unwrap_or(log.len());
    conn.execute(
        "INSERT INTO check_steps (run_id, position, command, exit_code, duration_ms, log) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![run, position as i64, command, exit, duration_ms, &log[start..]],
    )?;
    Ok(())
}

/// The run ended: passed, or failed with why.
pub fn end(conn: &Connection, run: &str, why: Option<&str>) -> Result<(), Error> {
    let state = if why.is_some() { "failed" } else { "passed" };
    conn.execute(
        "UPDATE check_runs SET state = ?2, why = ?3, ended_ts = ?4 WHERE id = ?1",
        params![run, state, why, now_us()],
    )?;
    Ok(())
}

/// Whether the project has a run at this commit already, of any state.
pub fn ran_at(conn: &Connection, project: &str, head: &str) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM check_runs WHERE project_id = ?1 AND head = ?2)",
        params![project, head],
        |r| r.get(0),
    )?)
}

/// The project's runs, newest first, each with its steps and, once it failed, its reference.
pub fn list(conn: &Connection, project: &str) -> Result<Vec<Value>, Error> {
    let mut runs = conn.prepare_cached(
        "SELECT id, head, state, why, trace_id, started_ts, ended_ts FROM check_runs
         WHERE project_id = ?1 ORDER BY started_ts DESC LIMIT 50",
    )?;
    let mut steps = conn.prepare_cached(
        "SELECT command, exit_code, duration_ms, log FROM check_steps WHERE run_id = ?1 ORDER BY position",
    )?;
    let rows = runs.query_map([project], |r| {
        let (id, trace): (String, String) = (r.get(0)?, r.get(4)?);
        let state: String = r.get(2)?;
        Ok(json!({
            "run": id, "head": r.get::<_, String>(1)?, "state": state, "why": r.get::<_, Option<String>>(3)?,
            "ref": (state == "failed").then(|| crate::ids::reference(&trace).to_string()),
            "started": utc(r.get(5)?), "ended": r.get::<_, Option<i64>>(6)?.map(utc),
        }))
    })?;
    let mut listed: Vec<Value> = rows.collect::<Result<_, _>>()?;
    for run in &mut listed {
        let id = run["run"].as_str().unwrap_or_default().to_string();
        let found = steps.query_map([id], |r| {
            Ok(
                json!({ "command": r.get::<_, String>(0)?, "exit": r.get::<_, Option<i32>>(1)?,
                "duration_ms": r.get::<_, i64>(2)?, "log": r.get::<_, String>(3)? }),
            )
        })?;
        run["steps"] = Value::Array(found.collect::<Result<_, _>>()?);
    }
    Ok(listed)
}

/// The project's caches were used now.
pub fn used(conn: &Connection, project: &str) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO check_caches (project_id, used_ts) VALUES (?1, ?2)
         ON CONFLICT (project_id) DO UPDATE SET used_ts = excluded.used_ts",
        params![project, now_us()],
    )?;
    Ok(())
}

/// The projects whose caches nothing has used since `before`: theirs go.
pub fn unused_since(conn: &Connection, before: i64) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare("SELECT project_id FROM check_caches WHERE used_ts < ?1")?;
    let ids = stmt.query_map([before], |r| r.get(0))?;
    Ok(ids.collect::<Result<_, _>>()?)
}

/// The project's caches are gone: nothing to remove until it runs again.
pub fn forget(conn: &Connection, project: &str) -> Result<(), Error> {
    conn.execute("DELETE FROM check_caches WHERE project_id = ?1", [project])?;
    Ok(())
}
