//! Which chats' tasks or waits a write changed, so the window's Summary follows them live (F8b).
//! Temporary triggers note each change's chat in the same transaction, so whichever code made the
//! change, it's noted, and a change rolled back notes nothing.

use rusqlite::Connection;
use serde_json::Value;

use crate::Error;

/// Sets the triggers up on the writer's connection; they last as long as it does.
pub fn watch(conn: &Connection) -> Result<(), Error> {
    let open = crate::quiet::WITH_OPEN_WORK;
    conn.execute_batch(&format!(
        "CREATE TEMP TABLE touched (chat_id TEXT PRIMARY KEY);
         CREATE TEMP TRIGGER task_added AFTER INSERT ON tasks
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER task_changed AFTER UPDATE ON tasks
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER wait_added AFTER INSERT ON waits
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         -- Not on every look: a look only stamps when it looked.
         CREATE TEMP TRIGGER wait_changed AFTER UPDATE OF state, line, then_do ON waits
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         -- What a chat waits on may have changed (0050), beyond its tasks and waits above: a turn
         -- queued, started, held or ended, a look that saw something new or began to fail, a
         -- background command that began or ended, a look at a PR to land that saw something new.
         CREATE TEMP TABLE touched_waiting (chat_id TEXT PRIMARY KEY);
         CREATE TEMP TRIGGER turn_added AFTER INSERT ON turns
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER turn_moved AFTER UPDATE OF status, held ON turns
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER wait_looked AFTER UPDATE OF seen, failures ON waits
             WHEN NEW.seen IS NOT OLD.seen OR NEW.failures IS NOT OLD.failures
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER command_began AFTER INSERT ON background
             BEGIN INSERT OR IGNORE INTO touched_waiting
                 SELECT chat_id FROM turns WHERE id = NEW.turn_id; END;
         CREATE TEMP TRIGGER command_ended AFTER DELETE ON background
             BEGIN INSERT OR IGNORE INTO touched_waiting
                 SELECT chat_id FROM turns WHERE id = OLD.turn_id; END;
         CREATE TEMP TRIGGER landing_added AFTER INSERT ON landings
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER landing_looked AFTER UPDATE OF saw, moving ON landings
             WHEN NEW.saw IS NOT OLD.saw OR NEW.moving IS NOT OLD.moving
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         -- A pause, a resume and the person's message into a running turn change no turn's row,
         -- and each can change whether a chat with work owed is stuck (0052).
         CREATE TEMP TRIGGER pause_added AFTER INSERT ON pauses
             BEGIN INSERT OR IGNORE INTO touched_waiting {open}; END;
         CREATE TEMP TRIGGER pause_ended AFTER DELETE ON pauses
             BEGIN INSERT OR IGNORE INTO touched_waiting {open}; END;
         CREATE TEMP TRIGGER person_wrote AFTER INSERT ON messages WHEN NEW.role = 'user'
             BEGIN INSERT OR IGNORE INTO touched_waiting VALUES (NEW.chat_id); END;
         -- What Gathr said in a chat in no turn (0051): no agent's turn is there to send it.
         CREATE TEMP TABLE said (id TEXT PRIMARY KEY, chat_id TEXT NOT NULL);
         CREATE TEMP TRIGGER gathr_said AFTER INSERT ON messages
             WHEN NEW.turn_id IS NULL AND NEW.role = 'gathr'
             BEGIN INSERT OR IGNORE INTO said VALUES (NEW.id, NEW.chat_id); END;"
    ))?;
    Ok(())
}

/// The chats changed since the last call, committed; noted no more.
pub fn take(conn: &Connection) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached("SELECT chat_id FROM temp.touched")?;
    let chats: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if !chats.is_empty() {
        conn.execute("DELETE FROM temp.touched", [])?;
    }
    Ok(chats)
}

/// The chats whose turns, looks or background commands changed since the last call, and those a
/// pause, a resume or the person's message may have moved, committed; noted no more.
pub fn take_waiting(conn: &Connection) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached("SELECT chat_id FROM temp.touched_waiting")?;
    let chats: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if !chats.is_empty() {
        conn.execute("DELETE FROM temp.touched_waiting", [])?;
    }
    Ok(chats)
}

/// What Gathr said in no turn since the last call, committed, oldest first: each chat and the
/// message as the API shows it; noted no more.
pub fn take_said(conn: &Connection) -> Result<Vec<(String, Value)>, Error> {
    let noted: Vec<(String, String)> = {
        let mut stmt = conn.prepare_cached("SELECT id, chat_id FROM temp.said ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    if noted.is_empty() {
        return Ok(Vec::new());
    }
    conn.execute("DELETE FROM temp.said", [])?;
    let mut said = Vec::new();
    for (id, chat) in noted {
        said.push((chat, crate::store::message(conn, &id)?));
    }
    Ok(said)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_rolled_back_notes_nothing_and_one_committed_notes_its_chat() {
        let dir = std::env::temp_dir().join(format!("gathr-touched-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        watch(&conn).unwrap();
        conn.execute(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c1', 'One', 0), ('c2', 'Two', 0)",
            [],
        )
        .unwrap();
        let add = "INSERT INTO tasks (chat_id, owner, kind, what, state, created_ts, updated_ts)
                   VALUES (?1, 'a', 'work', 'x', 'ready', 0, 0)";
        let tx = conn.transaction().unwrap();
        tx.execute(add, ["c1"]).unwrap();
        drop(tx);
        assert_eq!(take(&conn).unwrap(), Vec::<String>::new());
        conn.execute(add, ["c2"]).unwrap();
        assert_eq!(take(&conn).unwrap(), ["c2"]);
        assert_eq!(take(&conn).unwrap(), Vec::<String>::new());
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_pause_a_resume_and_the_persons_message_note_the_chats_they_may_move() {
        let dir = std::env::temp_dir().join(format!("gathr-touched-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        watch(&conn).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('owes', 'One', 0), ('rests', 'Two', 0);
             INSERT INTO tasks (chat_id, owner, kind, what, state, created_ts, updated_ts)
             VALUES ('owes', 'a', 'work', 'x', 'ready', 0, 0), ('rests', 'a', 'work', 'x', 'done', 0, 0);",
        )
        .unwrap();
        // Only a chat with work owed can be stuck, so a pause asks only it again.
        crate::pauses::pause(&conn, "app", "", false, "tr").unwrap();
        assert_eq!(take_waiting(&conn).unwrap(), ["owes"]);
        crate::pauses::resume(&conn, "app", "", "tr").unwrap();
        assert_eq!(take_waiting(&conn).unwrap(), ["owes"]);

        let add = "INSERT INTO messages (id, chat_id, ts, role, kind, body) VALUES (?1, 'rests', 1, ?1, 'text', 'x')";
        conn.execute(add, ["gathr"]).unwrap();
        conn.execute(add, ["assistant"]).unwrap();
        assert_eq!(take_waiting(&conn).unwrap(), Vec::<String>::new());
        conn.execute(add, ["user"]).unwrap();
        assert_eq!(take_waiting(&conn).unwrap(), ["rests"]);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
