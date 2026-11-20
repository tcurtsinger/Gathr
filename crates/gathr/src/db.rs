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
    include_str!("../migrations/0034_review_judged.sql"),
    include_str!("../migrations/0035_friction.sql"),
    include_str!("../migrations/0036_wait_seen.sql"),
    include_str!("../migrations/0037_landings.sql"),
    include_str!("../migrations/0038_landing_looks.sql"),
    include_str!("../migrations/0039_quiet.sql"),
    include_str!("../migrations/0040_review_notes.sql"),
    include_str!("../migrations/0041_changes.sql"),
    include_str!("../migrations/0042_task_sessions.sql"),
    include_str!("../migrations/0043_turn_opened.sql"),
    include_str!("../migrations/0044_reactions.sql"),
    include_str!("../migrations/0045_task_briefs.sql"),
    include_str!("../migrations/0046_asks.sql"),
    include_str!("../migrations/0047_plans.sql"),
    include_str!("../migrations/0048_hold.sql"),
    include_str!("../migrations/0049_held_turns.sql"),
    include_str!("../migrations/0050_change_lines.sql"),
    include_str!("../migrations/0051_removed_agents.sql"),
    include_str!("../migrations/0052_agent_faces.sql"),
    include_str!("../migrations/0053_cut_turns_stopped.sql"),
    include_str!("../migrations/0054_message_picks.sql"),
    include_str!("../migrations/0055_topics.sql"),
    include_str!("../migrations/0056_replies.sql"),
    include_str!("../migrations/0057_reads.sql"),
    include_str!("../migrations/0058_chat_of_one.sql"),
    include_str!("../migrations/0059_placed.sql"),
    include_str!("../migrations/0060_open_checks.sql"),
    include_str!("../migrations/0061_failed_at.sql"),
    include_str!("../migrations/0062_turn_session.sql"),
    include_str!("../migrations/0063_summed_sessions.sql"),
    include_str!("../migrations/0064_board_lines.sql"),
    include_str!("../migrations/0065_chat_room.sql"),
    include_str!("../migrations/0066_agent_picture.sql"),
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

    /// 0045: a giver may edit a task until it starts; a task that ran before counts as started.
    /// Blocked by its `after`, or cancelled, a task may never have run: only its turns tell.
    #[test]
    fn tasks_that_ran_before_0045_count_as_started() {
        let path = temp_db("briefs");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..44] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 44;
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
                 VALUES (1, 'c', 'lead', 'ann', 'work', 'one', 'working', 1, 7),
                        (2, 'c', 'lead', 'bo', 'work', 'two', 'waiting', 2, 8),
                        (3, 'c', 'lead', 'bo', 'work', 'three', 'blocked', 3, 9),
                        (4, 'c', 'lead', 'bo', 'work', 'four', 'cancelled', 4, 10),
                        (5, 'c', 'lead', 'ann', 'work', 'five', 'blocked', 5, 11);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, task) VALUES
                 ('ran', 'c', 'x', 'done', 5, 5), ('dropped', 'c', 'x', 'queued', 4, 4);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let mut stmt = conn
            .prepare("SELECT number, started_ts FROM tasks ORDER BY number")
            .unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        let started: Vec<(i64, Option<i64>)> = rows.map(Result::unwrap).collect();
        assert_eq!(
            started,
            [(1, Some(7)), (2, None), (3, None), (4, None), (5, Some(11))]
        );
    }

    /// 0046: no backfill. Work in flight at the upgrade serves no ask, and the person's next
    /// message starts one of its own though that work is open.
    #[test]
    fn turns_and_tasks_from_before_0046_serve_no_ask() {
        let path = temp_db("asks");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..45] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 45;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id)
                 VALUES ('old', 'c', 'x', 'running', 1, 'lead');
             INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts)
                 VALUES (1, 'c', 'lead', 'ann', 'old', 'work', 'one', 'working', 1, 1);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let asks = |conn: &Connection| -> (i64, Option<String>, Option<String>) {
            conn.query_row(
                "SELECT (SELECT count(*) FROM asks), (SELECT ask_id FROM turns WHERE id = 'old'),
                        (SELECT ask_id FROM tasks WHERE number = 1)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
        };
        assert_eq!(asks(&conn), (0, None, None));
        let opened = crate::words::Opened::Answered;
        crate::store::add_turn(
            &conn,
            "c",
            "after",
            "x",
            (Some("lead"), None, Some("old")),
            opened,
        )
        .unwrap();
        let ask_of = |turn: &str| -> Option<String> {
            let sql = "SELECT ask_id FROM turns WHERE id = ?1";
            conn.query_row(sql, [turn], |r| r.get(0)).unwrap()
        };
        assert_eq!(ask_of("after"), None, "a turn answering one from before");
        let person = crate::words::Opened::Person;
        crate::store::add_turn(&conn, "c", "new", "x", (Some("lead"), None, None), person).unwrap();
        let said = crate::store::add_message(&conn, "c", "new", "user", "text", "Go on").unwrap();
        let said = said["id"].as_str().map(String::from);
        assert_eq!(
            ask_of("new"),
            said,
            "the old work is no ask's: a new one starts"
        );
    }

    /// 0047 (0062): nothing to fill. No task from before is a plan check, no ask has a plan, and a
    /// question open from before holds a second one on its ask.
    #[test]
    fn plans_come_with_0047_and_nothing_else_changes() {
        let path = temp_db("plans");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..46] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 46;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 1);
             INSERT INTO messages (id, chat_id, ts, role, kind, body) VALUES ('m1', 'c', 1, 'user', 'text', 'Add tags');
             INSERT INTO asks (id, chat_id) VALUES ('m1', 'c');
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, ask_id)
                 VALUES ('t', 'c', 'x', 'running', 1, 'mira', 'm1');
             INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts, ask_id)
                 VALUES (1, 'c', 'mira', 'ann', 'work', 'a', 'waiting', 1, 1, 'm1'),
                        (2, 'c', 'mira', 'ann', 'work', 'b', 'ready', 1, 1, 'm1'),
                        (3, 'c', 'mira', 'ann', 'work', 'c', 'working', 1, 1, 'm1'),
                        (4, 'c', 'mira', 'ann', 'work', 'd', 'held', 1, 1, 'm1'),
                        (5, 'c', 'mira', 'ann', 'review', 'e', 'done', 1, 1, 'm1'),
                        (6, 'c', 'mira', 'ann', 'review', 'f', 'blocked', 1, 1, 'm1'),
                        (7, 'c', 'mira', 'ann', 'work', 'g', 'failed', 1, 1, 'm1'),
                        (8, 'c', 'mira', 'ann', 'work', 'h', 'stopped', 1, 1, 'm1'),
                        (9, 'c', 'mira', 'ann', 'work', 'i', 'cancelled', 1, 1, 'm1'),
                        (10, 'c', 'mira', NULL, 'person', 'Which colour?', 'ready', 1, 1, 'm1');",
        )
        .unwrap();
        let rows = |conn: &Connection| -> String {
            let sql = "SELECT json_group_array(json_array(number, chat_id, giver, owner, kind, what, state,
                              created_ts, updated_ts, ask_id)) FROM (SELECT * FROM tasks ORDER BY number)";
            conn.query_row(sql, [], |r| r.get(0)).unwrap()
        };
        let before = rows(&conn);
        drop(conn);
        let mut conn = open(&path).unwrap();
        assert_eq!(rows(&conn), before);
        let found: (i64, i64, i64, i64) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM plans), (SELECT count(*) FROM plan_versions),
                        (SELECT count(*) FROM plan_parts), (SELECT count(*) FROM tasks WHERE checks IS NOT NULL)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert_eq!(found, (0, 0, 0, 0));
        let ask = crate::tasks::person::Ask {
            chat: "c",
            giver: "mira",
            given_in: Some("t"),
            parent: None,
            what: Some("Which size?"),
            why: None,
            form: None,
            choices: Vec::new(),
            files: Vec::new(),
            request: None,
        };
        match crate::tasks::person::ask(&mut conn, ask, "x") {
            Err(Error::Refused(why)) => assert_eq!(
                why,
                "Question #10 to the person is still open for this ask. Until its plan is approved, ask one at a \
                 time: wait for its answer, or cancel #10 first."
            ),
            other => panic!("expected the second question refused, got {other:?}"),
        }
    }

    /// 0048 (0063): no task is a look-into task, no ask was held, and nothing is moved: a ready
    /// building task's queued brief on an unsized ask stays, and its turn isn't held.
    #[test]
    fn the_hold_comes_with_0048_and_moves_nothing_from_before() {
        let path = temp_db("hold");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..47] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 47;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 1);
             INSERT INTO messages (id, chat_id, ts, role, kind, body) VALUES ('m1', 'c', 1, 'user', 'text', 'Add tags'),
                 ('m2', 'c', 2, 'user', 'text', 'Fix it'), ('m3', 'c', 3, 'user', 'text', 'Rename it');
             INSERT INTO asks (id, chat_id, size) VALUES ('m1', 'c', NULL), ('m2', 'c', 'repair'), ('m3', 'c', 'new');
             INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, reason, created_ts, updated_ts, ask_id)
                 VALUES (1, 'c', 'mira', 'ann', 'work', 'a', 'waiting', NULL, 1, 1, 'm1'),
                        (2, 'c', 'mira', 'ann', 'work', 'b', 'ready', NULL, 1, 1, 'm1'),
                        (3, 'c', 'mira', 'ann', 'work', 'c', 'working', NULL, 1, 1, 'm3'),
                        (4, 'c', 'mira', 'ann', 'work', 'd', 'held', 'its merge', 1, 1, 'm3'),
                        (5, 'c', 'mira', 'ann', 'review', 'e', 'done', NULL, 1, 1, 'm2'),
                        (6, 'c', 'mira', 'ann', 'work', 'f', 'blocked', 'stuck', 1, 1, 'm1'),
                        (7, 'c', 'mira', 'ann', 'work', 'g', 'failed', 'boom', 1, 1, 'm3'),
                        (8, 'c', 'mira', 'ann', 'work', 'h', 'stopped', 'stop', 1, 1, 'm1'),
                        (9, 'c', 'mira', 'ann', 'work', 'i', 'cancelled', 'no', 1, 1, 'm2');
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, task, ask_id)
                 VALUES ('brief', 'c', 'x', 'queued', 1, 'ann', 2, 'm1');",
        )
        .unwrap();
        let rows = |conn: &Connection| -> String {
            let sql = "SELECT json_group_array(json_array(number, state, reason, ask_id)) FROM (SELECT * FROM tasks ORDER BY number)";
            conn.query_row(sql, [], |r| r.get(0)).unwrap()
        };
        let before = rows(&conn);
        drop(conn);
        let conn = open(&path).unwrap();
        assert_eq!(rows(&conn), before, "no task moved");
        let found: (i64, i64, Option<String>) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM tasks WHERE look_into != 0),
                        (SELECT count(*) FROM asks WHERE held_ts IS NOT NULL),
                        (SELECT stop_why FROM turns WHERE id = 'brief')",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(found, (0, 0, None));
        let held = crate::hold::turn(&conn, "brief").unwrap().unwrap();
        assert!(!held.held(), "a building task's queued brief isn't held");
    }

    /// 0049 (0063): no held turn has a start record, so one running across the upgrade isn't
    /// looked at when it ends.
    #[test]
    fn held_turns_come_empty() {
        let path = temp_db("held-turns");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..48] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 48;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
             INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Notes', 'notes', 'x', 1);
             INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'C', 1, 'p');
             INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 1);
             INSERT INTO messages (id, chat_id, ts, role, kind, body) VALUES ('m1', 'c', 1, 'user', 'text', 'Add tags');
             INSERT INTO asks (id, chat_id) VALUES ('m1', 'c');
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, ask_id)
                 VALUES ('t', 'c', 'x', 'running', 1, 'mira', 'm1');
             INSERT INTO agent_folders (path, kind, chat_id, agent_id, made_ts) VALUES ('w', 'worktree', 'c', 'mira', 1);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let rows: i64 = conn
            .query_row("SELECT count(*) FROM held_turns", [], |r| r.get(0))
            .unwrap();
        assert_eq!(rows, 0, "held turns came with rows");
        let ending = crate::hold::look::ending(&conn, "t").unwrap();
        assert!(ending.is_none(), "a turn from before 0049 was looked at");
    }

    /// 0051 rebuilds `agents`, which teams point at, and `agent_folders`: every row stays with its
    /// columns, the name stays unique among agents not removed, and a folder may have no chat.
    #[test]
    fn agents_and_their_folders_outlast_the_removed_agents_rebuild() {
        let path = temp_db("removed-agents");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..50] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 50;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, model, effort, created_ts, approvals, folder_name)
                 VALUES ('mira', 'Mira', 'mira', 'acc', 'opus', 'high', 1, 'full', 'Mira'),
                        ('ann', 'Ann', 'ann', 'acc', NULL, NULL, 2, 'auto', NULL);
             INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Notes', 'notes', 'x', 1);
             INSERT INTO project_members VALUES ('p', 'mira', 'supervisor', 0), ('p', 'ann', 'member', 1);
             INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'C', 1, 'p');
             INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 2);
             INSERT INTO agent_folders (path, kind, chat_id, agent_id, project, made_ts, due_ts, trace, skipped, removed_ts)
                 VALUES ('w', 'worktree', 'c', 'mira', 'x', 1, 2, 'tr', 'in_use', NULL);",
        )
        .unwrap();
        let agents = |conn: &Connection| -> Vec<String> {
            let sql = "SELECT id || name || name_key || account_id || coalesce(model, '-') || coalesce(effort, '-')
                       || created_ts || approvals || coalesce(folder_name, '-') FROM agents ORDER BY id";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let before = agents(&conn);
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(agents(&conn), before);
        let kept: (i64, i64, String) = conn
            .query_row(
                "SELECT (SELECT count(*) FROM chat_members), (SELECT count(*) FROM project_members),
                        (SELECT path || kind || chat_id || agent_id || project || made_ts || due_ts || trace || skipped
                         FROM agent_folders)",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(kept, (2, 2, "wworktreecmirax12trin_use".to_string()));
        let broken: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0);
        // A removed agent lets go of its account and its name; a live one needs both.
        crate::agents::mark_removed(&conn, "ann").unwrap();
        conn.execute_batch(
            "INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('ann2', 'Ann', 'ann', 'acc', 3);
             INSERT INTO agent_folders (path, kind, made_ts) VALUES ('Notes/Mira', 'folder', 3);",
        )
        .unwrap();
        let taken = "INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('x', 'ANN', 'ann', 'acc', 4)";
        assert!(conn.execute(taken, []).is_err(), "a live name taken twice");
        let no_account =
            "INSERT INTO agents (id, name, name_key, created_ts) VALUES ('y', 'Bo', 'bo', 4)";
        assert!(
            conn.execute(no_account, []).is_err(),
            "a live agent with no account"
        );
        conn.execute("DELETE FROM chat_members WHERE agent_id = 'ann'", [])
            .unwrap();
        conn.execute("DELETE FROM project_members WHERE agent_id = 'ann'", [])
            .unwrap();
        crate::agents::mark_removed(&conn, "mira").unwrap();
        conn.execute("DELETE FROM agents WHERE id = 'ann2'", [])
            .unwrap();
        conn.execute("DELETE FROM accounts WHERE id = 'acc'", [])
            .unwrap();
    }

    /// 0052 (0074): an agent from before has its name as its face's seed, and no gender or skin tone.
    #[test]
    fn an_agents_face_starts_from_its_name() {
        let path = temp_db("faces");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..51] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 51;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('mira', 'Mira', 'mira', 'acc', 1);
             INSERT INTO agents (id, name, name_key, created_ts, removed_ts) VALUES ('ann', 'Ann', 'ann', 2, 3);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let sql = "SELECT id || ' ' || face_seed || ' ' || coalesce(gender, '-') || ' ' || coalesce(skin, '-')
                   || ' ' || coalesce(background, '-') FROM agents ORDER BY id";
        let mut stmt = conn.prepare(sql).unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, ["ann Ann - - -", "mira Mira - - -"]);
        // A background is #RRGGBB in upper case or nothing, whatever wrote it.
        let set = |value: &str| {
            let sql = "UPDATE agents SET background = ?1 WHERE id = 'mira'";
            conn.execute(sql, [value]).is_ok()
        };
        let tried = ["#7FB5A8", "#7fb5a8", "#7FB5A", "7FB5A8F", "#7FB5AG"].map(set);
        assert_eq!(tried, [true, false, false, false, false]);
    }

    /// 0066: an agent from before has no picture, and removing an agent drops its picture. The last migration
    /// is held back by position, so the test stays right when an earlier number lands beside it.
    #[test]
    fn an_agent_from_before_has_no_picture_and_removing_drops_one() {
        let path = temp_db("picture");
        let conn = Connection::open(&path).unwrap();
        let before = MIGRATIONS.len() - 1;
        for sql in &MIGRATIONS[..before] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", before as i64)
            .unwrap();
        conn.execute_batch(
            "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('bo', 'Bo', 'bo', 'acc', 2);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let pictures = || -> Vec<Option<String>> {
            let mut stmt = conn
                .prepare("SELECT picture FROM agents ORDER BY id")
                .unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        assert_eq!(pictures(), [None, None]);
        let set = "UPDATE agents SET picture = 'data:image/png;base64,AA'";
        conn.execute(set, []).unwrap();
        crate::agents::mark_removed(&conn, "bo").unwrap();
        assert_eq!(pictures(), [None, Some("data:image/png;base64,AA".into())]);
    }

    /// 0053: a turn a restart cut before reads stopped, as one cut now does; a turn that failed stays failed.
    #[test]
    fn a_turn_a_restart_cut_before_0053_reads_stopped() {
        let path = temp_db("cut");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..52] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 52;
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, why, created_ts) VALUES
                 ('cut', 'c', 'a', 'failed', 'the service stopped during this turn', 1),
                 ('broke', 'c', 'b', 'failed', 'the engine exited', 2);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let mut stmt = conn
            .prepare("SELECT id || ' ' || status FROM turns ORDER BY created_ts")
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, ["cut stopped", "broke failed"]);
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
