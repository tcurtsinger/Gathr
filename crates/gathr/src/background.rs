//! Commands an engine runs in the background (0010): each under the turn that started it, so a
//! task stays held while one runs, a pause stays `pausing`, and Stop now can end them by the
//! engine's own id. They end with their engine, so a start empties the table.

use rusqlite::{Connection, params};

use crate::Error;
use crate::ids::now_us;

/// An engine's turns: its chat, and its agent (none for turns sent to no agent).
const ENGINE: &str = "t.chat_id = ?1 AND t.agent_id IS ?2";

/// Saves what the engine in `chat` for `agent` lists as running now; a new one goes under `turn`,
/// or the engine's latest turn when none runs. Answers the turns whose commands ended, with each
/// one's task, oldest first.
pub fn set(
    conn: &Connection,
    (chat, agent): (&str, Option<&str>),
    listed: &[(String, String)],
    turn: Option<&str>,
) -> Result<Vec<(String, Option<i64>)>, Error> {
    let had = of(conn, chat, agent)?;
    let mut ended = Vec::new();
    for (id, turn_id, task) in &had {
        if listed.iter().all(|(l, _)| l != id) {
            conn.execute("DELETE FROM background WHERE id = ?1", [id])?;
            ended.push((turn_id.clone(), *task));
        }
    }
    let turn = match turn {
        Some(turn) => Some(turn.to_string()),
        None => latest(conn, chat, agent)?,
    };
    let Some(turn) = turn else { return Ok(ended) };
    for (id, description) in listed {
        conn.execute(
            "INSERT OR IGNORE INTO background (id, turn_id, description, started_ts)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, turn, description, now_us()],
        )?;
    }
    Ok(ended)
}

/// The engine's background commands: id, the turn that started it, and that turn's task.
pub fn of(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
) -> Result<Vec<(String, String, Option<i64>)>, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT b.id, b.turn_id, t.task FROM background b JOIN turns t ON t.id = b.turn_id
         WHERE {ENGINE} ORDER BY b.started_ts"
    ))?;
    let rows = stmt.query_map(params![chat, agent], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?))
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

/// The service started: no engine runs, so neither does any command of one.
pub fn clear(conn: &Connection) -> Result<(), Error> {
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
        let ping = [("b1".to_string(), "ping".to_string())];
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
        assert_eq!(
            set(&conn, engine, &[], None).unwrap(),
            [("t1".to_string(), None)]
        );
        assert!(of(&conn, "c1", Some("ann")).unwrap().is_empty());
    }
}
