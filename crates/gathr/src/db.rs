//! The database: one SQLite file, its pragmas and numbered migrations (docs/decisions/0003-storage.md).

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::Error;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_turns.sql"),
    include_str!("../migrations/0003_accounts.sql"),
    include_str!("../migrations/0004_agents.sql"),
    include_str!("../migrations/0005_engine_sessions.sql"),
    include_str!("../migrations/0006_teams.sql"),
    include_str!("../migrations/0007_approvals.sql"),
    include_str!("../migrations/0008_tasks.sql"),
    include_str!("../migrations/0009_projects.sql"),
    include_str!("../migrations/0010_person_tasks.sql"),
    include_str!("../migrations/0011_pauses.sql"),
    include_str!("../migrations/0012_archive_stale.sql"),
    include_str!("../migrations/0013_engine_waits.sql"),
    include_str!("../migrations/0014_agent_folder.sql"),
    include_str!("../migrations/0015_search.sql"),
    include_str!("../migrations/0016_project_settings.sql"),
    include_str!("../migrations/0017_task_heard_in.sql"),
    include_str!("../migrations/0018_check_approvals.sql"),
    include_str!("../migrations/0019_up_to_date_wait.sql"),
    include_str!("../migrations/0020_check_runs.sql"),
    include_str!("../migrations/0021_agent_messages.sql"),
    include_str!("../migrations/0022_task_pr_head.sql"),
    include_str!("../migrations/0023_setting_sources.sql"),
    include_str!("../migrations/0024_pr_cleanup.sql"),
    include_str!("../migrations/0025_agent_context.sql"),
    include_str!("../migrations/0026_completed_reviews.sql"),
    include_str!("../migrations/0027_chat_state.sql"),
    include_str!("../migrations/0028_thinking.sql"),
    include_str!("../migrations/0029_review_only.sql"),
    include_str!("../migrations/0030_compact_told.sql"),
    include_str!("../migrations/0031_context_ran.sql"),
    include_str!("../migrations/0032_reports.sql"),
    include_str!("../migrations/0033_hear_backs.sql"),
];

/// The schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Opens the database for writing, creating it if needed, and brings its schema up to date.
pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Other(format!(
            "the database stayed in journal mode {mode}, not WAL"
        )));
    }
    conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Opens an existing database read-only, for diagnostics. Fails if the file isn't a Gathr
/// database this build can read.
pub fn open_read_only(path: &Path) -> Result<(Connection, i64), Error> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let version = schema_version(&conn)?;
    if version > SCHEMA_VERSION {
        return Err(Error::NewerDatabase {
            found: version,
            known: SCHEMA_VERSION,
        });
    }
    Ok((conn, version))
}

pub fn schema_version(conn: &Connection) -> Result<i64, Error> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let current = schema_version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(Error::NewerDatabase {
            found: current,
            known: SCHEMA_VERSION,
        });
    }
    // A migration that rebuilds a table other tables point at drops it first, which foreign keys
    // refuse even deferred; so they're off while migrating, as SQLite's own procedure says, and each
    // migration is checked before it commits.
    conn.execute_batch("PRAGMA foreign_keys = OFF;")?;
    let migrated = (|| {
        for (number, sql) in (1..).zip(MIGRATIONS).skip(current as usize) {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute_batch(sql)?;
            let broken: i64 =
                tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            if broken > 0 {
                return Err(Error::Refused(format!(
                    "migration {number} leaves {broken} broken references"
                )));
            }
            tx.pragma_update(None, "user_version", number)?;
            tx.commit()?;
        }
        Ok(())
    })();
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    migrated
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gathr-db-{name}-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("gathr.db")
    }

    #[test]
    fn an_empty_database_gets_every_migration_once() {
        let path = temp_db("empty");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name IN ('events', 'chats', 'messages')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3);
    }

    /// Before 0004 every chat ran on the oldest account, and a new oldest one cleared the sessions,
    /// so a session left then lives in the oldest account's folder and must still resume there.
    #[test]
    fn a_chat_session_from_before_agents_resumes_on_the_oldest_account() {
        let path = temp_db("sessions");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..3] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 3;
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES
                 ('newer', 'claude', 'Newer', 'newer', 2), ('oldest', 'claude', 'Oldest', 'oldest', 1);
             INSERT INTO chats (id, title, created_ts, engine_session) VALUES
                 ('talked', 'Talked', 1, 'session-1'), ('quiet', 'Quiet', 1, NULL);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES ('t', 'talked', 'x', 'done', 1);
             INSERT INTO engine_raw (ts, chat_id, turn_id, direction, frame_type, size) VALUES
                 (1, 'talked', 't', 'out', 'system/init', 1);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let session = |chat| crate::store::engine_session(&conn, chat, None, "oldest").unwrap();
        assert_eq!(session("talked").as_deref(), Some("session-1"));
        assert_eq!(session("quiet"), None);
    }

    /// A chat's session belongs to the agent that ran it (F2): each agent keeps its own session in
    /// each chat, so the session a chat had before is the agent's of the last turn its engine ran,
    /// not of a newer turn that never started.
    #[test]
    fn a_chat_session_moves_to_the_agent_of_its_last_turn() {
        let path = temp_db("agent-sessions");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..4] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 4;
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('work', 'claude', 'Work', 'work', 1);
             INSERT INTO chats (id, title, created_ts, engine_session, engine_account) VALUES
                 ('by-agent', 'A', 1, 'session-a', 'work'), ('no-agent', 'B', 1, 'session-b', 'work');
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id) VALUES
                 ('t1', 'by-agent', 'x', 'done', 1, 'orin'), ('t2', 'by-agent', 'x', 'done', 2, 'cass'),
                 ('t3', 'no-agent', 'x', 'done', 1, NULL),
                 ('t4', 'by-agent', 'x', 'failed', 3, 'vale'), ('t5', 'no-agent', 'x', 'queued', 2, 'pia');
             -- Only turns whose engine ran have its frames; t4 failed before it started, t5 never did.
             INSERT INTO engine_raw (ts, chat_id, turn_id, direction, frame_type, size) VALUES
                 (1, 'by-agent', 't1', 'out', 'system/init', 1), (2, 'by-agent', 't2', 'out', 'system/init', 1),
                 (3, 'no-agent', 't3', 'out', 'system/init', 1);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let session =
            |chat, agent| crate::store::engine_session(&conn, chat, agent, "work").unwrap();
        assert_eq!(
            session("by-agent", Some("cass")).as_deref(),
            Some("session-a")
        );
        assert_eq!(session("by-agent", Some("orin")), None);
        assert_eq!(session("no-agent", None).as_deref(), Some("session-b"));
        assert_eq!(session("no-agent", Some("cass")), None);
        assert_eq!(session("by-agent", Some("vale")), None);
        assert_eq!(session("no-agent", Some("pia")), None);
    }

    /// 0010 rebuilds `tasks`, which other tables point at: every task, its links and waits stay.
    #[test]
    fn tasks_and_what_points_at_them_outlast_the_person_tasks_rebuild() {
        let path = temp_db("person-tasks");
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch("PRAGMA foreign_keys = ON;").unwrap();
        for sql in &MIGRATIONS[..9] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 9;
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, reviewed, created_ts,
                                updated_ts)
                 VALUES (1, 'c', 'lead', 'ann', 'work', 'one', 'done', NULL, 1, 1),
                        (2, 'c', 'lead', 'bo', 'review', 'two', 'held', 'abc123', 2, 2);
             INSERT INTO task_after VALUES (2, 1);
             INSERT INTO waits (id, chat_id, agent_id, task, kind, then_do, state, until_ts, created_ts)
                 VALUES ('w', 'c', 'bo', 2, 'pr', 'merge', 'open', 9, 2);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let kept: (i64, i64, i64, String) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM tasks), (SELECT count(*) FROM task_after),
                        (SELECT count(*) FROM waits WHERE task = 2),
                        (SELECT reviewed FROM tasks WHERE number = 2)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(kept, (2, 1, 1, "abc123".to_string()));
        let broken: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0);
    }

    /// 0028 rebuilds `messages`, which the search index points at by rowid: every message keeps
    /// its rowid and columns, search finds what it found, and a thinking row is never indexed.
    #[test]
    fn messages_keep_their_rowids_and_search_through_the_thinking_rebuild() {
        let path = temp_db("thinking");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..27] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 27;
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES ('t', 'c', 'x', 'done', 1);
             INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of) VALUES
                 ('gone', 'c', 't', 1, 'user', 'text', 'gravel deleted', NULL, NULL),
                 ('asked', 'c', 't', 2, 'user', 'text', 'gravel road', NULL, NULL),
                 ('ran', 'c', 't', 3, 'assistant', 'tool', '{\"tool\":\"gravel\",\"status\":\"ok\"}', NULL, NULL),
                 ('sent', 'c', 't', 4, 'assistant', 'text', 'gravel for Bo', 'bo', NULL),
                 ('told', 'c', NULL, 5, 'gathr', 'text', 'gravel told', NULL, 'sent');
             DELETE FROM messages WHERE id = 'gone';",
        )
        .unwrap();
        let rows = |conn: &Connection| -> Vec<(i64, String, Option<String>, Option<String>)> {
            let mut stmt = conn
                .prepare(
                    "SELECT rowid, id || role || kind || body || ts || coalesce(turn_id, '-'),
                                 to_agent, copy_of FROM messages ORDER BY rowid",
                )
                .unwrap();
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)));
            rows.unwrap().map(Result::unwrap).collect()
        };
        let found = |conn: &Connection| -> Vec<i64> {
            let mut stmt = conn
                .prepare("SELECT rowid FROM message_search WHERE message_search MATCH 'gravel' ORDER BY rowid")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let before = rows(&conn);
        assert_eq!(found(&conn), [2, 4, 5], "the gap a deleted message left");
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(rows(&conn), before);
        assert_eq!(before[2].2.as_deref(), Some("bo"));
        assert_eq!(before[3].3.as_deref(), Some("sent"));
        assert_eq!(found(&conn), [2, 4, 5]);
        // The index still follows the table: a new text is found, a thinking row never is.
        conn.execute_batch(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
                 ('thought', 'c', 't', 6, 'assistant', 'thinking', '{\"status\":\"gravel\"}'),
                 ('later', 'c', 't', 7, 'assistant', 'text', 'gravel later');
             UPDATE messages SET body = 'sand road' WHERE id = 'asked';
             INSERT INTO message_search (message_search) VALUES ('integrity-check');",
        )
        .unwrap();
        assert_eq!(found(&conn), [4, 5, 7]);
        conn.execute("DELETE FROM messages WHERE id = 'sent'", [])
            .unwrap();
        assert_eq!(found(&conn), [5, 7]);
        let plan: String = conn
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM messages WHERE chat_id = 'c' ORDER BY ts",
                [],
                |r| r.get(3),
            )
            .unwrap();
        assert!(plan.contains("messages_by_chat"), "{plan}");
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let path = temp_db("newer");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        match open(&path) {
            Err(Error::NewerDatabase {
                found: 99,
                known: SCHEMA_VERSION,
            }) => {}
            other => panic!("expected a refusal of schema 99, got {other:?}"),
        }
    }
}

#[cfg(test)]
mod engine_wait_migration_test {
    use super::*;
    #[test]
    fn engine_wait_migration_keeps_turn_fields_and_background_references() {
        let path =
            std::env::temp_dir().join(format!("gathr-migrate-{}.sqlite", crate::ids::span_id()));
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..12] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch("PRAGMA user_version = 12;
            INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
            INSERT INTO turns VALUES ('t', 'c', 'trace', 'queued', 2, NULL, 'paused', 'a', 3, 4, 'answer', 'pause', 'cancelled');
            INSERT INTO background VALUES ('b', 't', 'command', 5);").unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let kept: (String, i64, i64, String, String, String) = conn.query_row(
            "SELECT agent_id, position, task, answers, held, stop_why FROM turns WHERE id = 't'", [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).unwrap();
        assert_eq!(
            kept,
            (
                "a".into(),
                3,
                4,
                "answer".into(),
                "pause".into(),
                "cancelled".into()
            )
        );
        assert_eq!(
            conn.query_row("SELECT turn_id FROM background WHERE id = 'b'", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            "t"
        );
        conn.execute("UPDATE turns SET held = 'codex' WHERE id = 't'", [])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| r
                .get::<_, i64>(
                0
            ))
            .unwrap(),
            0
        );
        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
