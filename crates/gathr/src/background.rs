//! Commands an engine runs in the background (0010): each under the turn that started it, so a
//! pause stays `pausing`, and Stop now can end them by the engine's own id. They end with their
//! engine, so a start empties the table. One that ended moves to `background_ended` (0084), for
//! the chat's Background tab: how it ended and where its output went.

use rusqlite::{Connection, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::now_us;

/// An engine's turns: its chat, and its agent (none for turns sent to no agent).
const ENGINE: &str = "t.chat_id = ?1 AND t.agent_id IS ?2";

/// How many ended commands each chat keeps.
const KEEP: i64 = 100;

/// One command the engine lists as running: its id, what it is, and its kind (`local_bash`, ...)
/// when the engine names it.
pub type Listed = (String, String, Option<String>);

/// Saves what the engine in `chat` for `agent` lists as running now; a new one goes under `turn`,
/// or the engine's latest turn when none runs. Answers the turns whose commands ended, oldest first.
pub fn set(
    conn: &Connection,
    (chat, agent): (&str, Option<&str>),
    listed: &[Listed],
    turn: Option<&str>,
) -> Result<Vec<String>, Error> {
    let had = of(conn, chat, agent)?;
    let mut ended = Vec::new();
    for (id, turn_id) in &had {
        if listed.iter().all(|(l, _, _)| l != id) {
            end(conn, id, "done")?;
            ended.push(turn_id.clone());
        }
    }
    if !ended.is_empty() {
        trim(conn, chat)?;
    }
    let turn = match turn {
        Some(turn) => Some(turn.to_string()),
        None => latest(conn, chat, agent)?,
    };
    let Some(turn) = turn else { return Ok(ended) };
    for (id, description, kind) in listed {
        conn.execute(
            "INSERT OR IGNORE INTO background (id, turn_id, description, started_ts, kind)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, turn, description, now_us(), kind],
        )?;
    }
    Ok(ended)
}

/// The engine said how a command ended (`completed`, `failed`, `killed`, ...) and where its output
/// went: kept on its row until the engine no longer lists it.
pub fn told(conn: &Connection, id: &str, status: &str, output: Option<&str>) -> Result<(), Error> {
    let status = match status {
        "completed" => "done",
        "failed" => "failed",
        _ => "stopped",
    };
    conn.execute(
        "UPDATE background SET status = ?2, output = COALESCE(?3, output) WHERE id = ?1",
        params![id, status, output],
    )?;
    Ok(())
}

/// Moves a command to the ended ones: how the engine said it ended, else `how`.
fn end(conn: &Connection, id: &str, how: &str) -> Result<(), Error> {
    conn.execute(
        "INSERT OR REPLACE INTO background_ended
             (id, turn_id, description, kind, started_ts, ended_ts, status, output)
         SELECT id, turn_id, description, kind, started_ts, ?2, COALESCE(status, ?3), output
         FROM background WHERE id = ?1",
        params![id, now_us(), how],
    )?;
    conn.execute("DELETE FROM background WHERE id = ?1", [id])?;
    Ok(())
}

/// Keeps the chat's newest ended commands only.
fn trim(conn: &Connection, chat: &str) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM background_ended WHERE id IN (
             SELECT e.id FROM background_ended e JOIN turns t ON t.id = e.turn_id
             WHERE t.chat_id = ?1 ORDER BY e.ended_ts DESC LIMIT -1 OFFSET ?2)",
        params![chat, KEEP],
    )?;
    Ok(())
}

/// The engine in `chat` for `agent` exited: what it still listed ended with it, stopped unless it
/// said otherwise.
pub fn gone(conn: &Connection, chat: &str, agent: Option<&str>) -> Result<(), Error> {
    for (id, _) in of(conn, chat, agent)? {
        end(conn, &id, "stopped")?;
    }
    trim(conn, chat)
}

/// The person's Clear: the chat's ended commands go.
pub fn clear_ended(conn: &Connection, chat: &str) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM background_ended WHERE turn_id IN (SELECT id FROM turns WHERE chat_id = ?1)",
        [chat],
    )?;
    Ok(())
}

/// The agent whose engine runs the command in `chat` (none for turns sent to no agent), while it
/// runs.
pub fn running_in(
    conn: &Connection,
    chat: &str,
    id: &str,
) -> Result<Option<Option<String>>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.agent_id FROM background b JOIN turns t ON t.id = b.turn_id WHERE b.id = ?1 AND t.chat_id = ?2",
    )?;
    let mut rows = stmt.query(params![id, chat])?;
    Ok(match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

/// Where a command's output went, as the engine said when it ended: running or ended, in `chat`.
pub fn output_of(conn: &Connection, chat: &str, id: &str) -> Result<Option<String>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT b.output FROM background b JOIN turns t ON t.id = b.turn_id WHERE b.id = ?1 AND t.chat_id = ?2
         UNION ALL
         SELECT e.output FROM background_ended e JOIN turns t ON t.id = e.turn_id WHERE e.id = ?1 AND t.chat_id = ?2",
    )?;
    let mut rows = stmt.query(params![id, chat])?;
    Ok(match rows.next()? {
        Some(row) => row.get(0)?,
        None => None,
    })
}

/// The engine's background commands: id, and the turn that started it.
pub fn of(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
) -> Result<Vec<(String, String)>, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT b.id, b.turn_id FROM background b JOIN turns t ON t.id = b.turn_id
         WHERE {ENGINE} ORDER BY b.started_ts"
    ))?;
    let rows = stmt.query_map(params![chat, agent], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The chat's background commands, oldest first, as the window lists them under their agents:
/// `{id, agent, turn, description, kind, started}`, with `ended` and `status` once one ended (the
/// running ones first, then the ended ones).
pub fn in_chat(conn: &Connection, chat: &str) -> Result<Vec<Value>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT * FROM (
             SELECT b.id, t.agent_id, b.turn_id, b.description, b.kind, b.started_ts, NULL AS ended,
                    NULL AS status, 0 AS done, b.output IS NOT NULL
             FROM background b JOIN turns t ON t.id = b.turn_id WHERE t.chat_id = ?1
             UNION ALL
             SELECT e.id, t.agent_id, e.turn_id, e.description, e.kind, e.started_ts, e.ended_ts,
                    e.status, 1, e.output IS NOT NULL
             FROM background_ended e JOIN turns t ON t.id = e.turn_id WHERE t.chat_id = ?1)
         ORDER BY done, COALESCE(ended, started_ts)",
    )?;
    let rows = stmt.query_map([chat], |r| {
        let (id, agent, turn): (String, Option<String>, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
        let (description, kind): (String, Option<String>) = (r.get(3)?, r.get(4)?);
        let started = crate::ids::utc(r.get(5)?);
        let ended = r.get::<_, Option<i64>>(6)?.map(crate::ids::utc);
        let status: Option<String> = r.get(7)?;
        let mut one = json!({ "id": id, "agent": agent, "turn": turn, "description": description,
            "kind": kind, "started": started, "output": r.get::<_, bool>(9)? });
        if let Some(ended) = ended {
            one["ended"] = ended.into();
            one["status"] = status.into();
        }
        Ok(one)
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn latest(conn: &Connection, chat: &str, agent: Option<&str>) -> Result<Option<String>, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT t.id FROM turns t WHERE {ENGINE} ORDER BY t.created_ts DESC LIMIT 1"
    ))?;
    let mut rows = stmt.query(params![chat, agent])?;
    Ok(match rows.next()? {
        Some(row) => Some(row.get(0)?),
        None => None,
    })
}

/// The service started: no engine runs, so neither does any command of one. Each ended with its
/// engine.
pub fn clear(conn: &Connection) -> Result<(), Error> {
    conn.execute(
        "INSERT OR REPLACE INTO background_ended
             (id, turn_id, description, kind, started_ts, ended_ts, status, output)
         SELECT id, turn_id, description, kind, started_ts, ?1, COALESCE(status, 'stopped'), output
         FROM background",
        [now_us()],
    )?;
    conn.execute("DELETE FROM background", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_is_kept_under_its_turn_until_the_engine_no_longer_lists_it() {
        let dir = std::env::temp_dir().join(format!("gathr-bg-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c1', 'x', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id)
                 VALUES ('t1', 'c1', 'x', 'running', 1, 'ann'), ('t0', 'c1', 'x', 'done', 0, 'bob');",
        )
        .unwrap();
        let ping = [(
            "b1".to_string(),
            "ping".to_string(),
            Some("local_bash".to_string()),
        )];
        let engine = ("c1", Some("ann"));
        assert!(set(&conn, engine, &ping, Some("t1")).unwrap().is_empty());
        assert_eq!(
            of(&conn, "c1", Some("bob")).unwrap().len(),
            0,
            "another agent's engine"
        );
        assert!(
            set(&conn, engine, &ping, None).unwrap().is_empty(),
            "listed again"
        );
        told(&conn, "b1", "failed", Some("/tmp/b1.output")).unwrap();
        assert_eq!(set(&conn, engine, &[], None).unwrap(), ["t1".to_string()]);
        assert!(of(&conn, "c1", Some("ann")).unwrap().is_empty());
        let listed = in_chat(&conn, "c1").unwrap();
        assert_eq!(listed.len(), 1, "kept as ended");
        assert_eq!(listed[0]["status"], "failed");
        assert_eq!(listed[0]["kind"], "local_bash");
        assert_eq!(listed[0]["output"], true);
        assert_eq!(
            output_of(&conn, "c1", "b1").unwrap().as_deref(),
            Some("/tmp/b1.output")
        );
        assert_eq!(
            output_of(&conn, "c2", "b1").unwrap(),
            None,
            "another chat's"
        );
        clear_ended(&conn, "c1").unwrap();
        assert!(in_chat(&conn, "c1").unwrap().is_empty());
    }

    #[test]
    fn an_engine_gone_or_a_start_ends_its_commands_as_stopped() {
        let dir = std::env::temp_dir().join(format!("gathr-bg-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c1', 'x', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id)
                 VALUES ('t1', 'c1', 'x', 'done', 1, 'ann');",
        )
        .unwrap();
        let two = [
            ("b1".to_string(), "ping".to_string(), None),
            ("b2".to_string(), "pong".to_string(), None),
        ];
        set(&conn, ("c1", Some("ann")), &two, Some("t1")).unwrap();
        gone(&conn, "c1", Some("ann")).unwrap();
        set(&conn, ("c1", Some("ann")), &two[..1], Some("t1")).unwrap();
        clear(&conn).unwrap();
        let listed = in_chat(&conn, "c1").unwrap();
        assert_eq!(listed.len(), 2);
        assert!(listed.iter().all(|b| b["status"] == "stopped"));
    }
}
