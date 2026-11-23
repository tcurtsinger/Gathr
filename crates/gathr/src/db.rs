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
    include_str!("../migrations/0067_questions.sql"),
    include_str!("../migrations/0068_drop_old_tables.sql"),
    include_str!("../migrations/0069_no_agent_pause.sql"),
    include_str!("../migrations/0070_agent_description.sql"),
    include_str!("../migrations/0071_agent_marks.sql"),
    include_str!("../migrations/0072_tools.sql"),
    include_str!("../migrations/0073_tool_sign_in.sql"),
    include_str!("../migrations/0074_tool_client_id.sql"),
    include_str!("../migrations/0075_no_reviewer.sql"),
    include_str!("../migrations/0076_closed_questions.sql"),
    include_str!("../migrations/0077_pc_tools.sql"),
    include_str!("../migrations/0078_blocks.sql"),
    include_str!("../migrations/0079_schedules.sql"),
    include_str!("../migrations/0080_schedule_name.sql"),
    include_str!("../migrations/0081_stays.sql"),
    include_str!("../migrations/0082_added_by.sql"),
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

    pub(super) fn temp_db(name: &str) -> std::path::PathBuf {
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

    /// 0071 drops the old faces' columns: every agent keeps its row and id, and has no picture picked.
    #[test]
    fn agents_keep_their_rows_through_the_marks_and_pick_none() {
        let path = temp_db("marks");
        let conn = Connection::open(&path).unwrap();
        let before = 70;
        for sql in &MIGRATIONS[..before] {
            conn.execute_batch(sql).unwrap();
        }
        conn.pragma_update(None, "user_version", before as i64)
            .unwrap();
        conn.execute_batch(
            "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, model, effort, created_ts, approvals,
                                 folder_name, face_seed, skin, background, picture, description) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 'opus', 'high', 1, 'full', 'Mira', 'x7', 3, '#7FB5A8',
                  'data:image/png;base64,AA', 'Everyday help'),
                 ('bo', 'Bo', 'bo', 'acc', NULL, NULL, 2, 'auto', NULL, 'Bo', NULL, NULL, NULL, NULL);
             INSERT INTO agents (id, name, name_key, created_ts, removed_ts) VALUES ('ann', 'Ann', 'ann', 3, 4);",
        )
        .unwrap();
        let rows = |conn: &Connection| -> Vec<String> {
            let sql = "SELECT id || name || name_key || coalesce(account_id, '-') || coalesce(model, '-')
                       || coalesce(effort, '-') || created_ts || approvals || coalesce(folder_name, '-')
                       || coalesce(removed_ts, '-') || coalesce(description, '-') FROM agents ORDER BY id";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let kept = rows(&conn);
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(rows(&conn), kept);
        let mut stmt = conn
            .prepare("SELECT name FROM pragma_table_info('agents') ORDER BY cid")
            .unwrap();
        let columns: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        for gone in ["face_seed", "gender", "skin", "background", "picture"] {
            assert!(!columns.iter().any(|c| c == gone), "{gone} is still there");
        }
        let mira = crate::agents::get(&conn, "mira").unwrap().unwrap();
        assert_eq!((mira.shape, mira.eyes, mira.colour), (None, None, None));
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
mod drop_old_tables_test {
    use super::*;

    /// The kept tables' kept columns, rowid first where a table has one.
    const KEPT: &[(&str, &str)] = &[
        (
            "chats",
            "rowid, id, title, created_ts, project, project_id, archived_ts",
        ),
        (
            "turns",
            "rowid, id, chat_id, trace_id, status, created_ts, ended_ts, why, agent_id, position, answers, held, opened_by, opened_version",
        ),
        (
            "messages",
            "rowid, id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of, picks, reply_to",
        ),
        ("background", "rowid, id, turn_id, description, started_ts"),
        ("reads", "chat_id, topic, seen_ts, folded"),
        (
            "projects",
            "rowid, id, name, name_key, folder, created_ts, folder_name",
        ),
        (
            "project_members",
            "rowid, project_id, agent_id, role, position",
        ),
        (
            "agents",
            "rowid, id, name, name_key, account_id, created_ts",
        ),
        (
            "agent_context",
            "rowid, chat_id, agent_id, used, updated_ts",
        ),
        (
            "questions",
            "rowid, id, chat_id, agent, question, choices, pick, words, asked_ts, answered_ts",
        ),
        ("board_lines", "rowid, id, chat_id, owner, text, state"),
        (
            "reactions",
            "rowid, message_id, agent_id, chat_id, created_ts",
        ),
    ];

    fn kept(conn: &Connection) -> Vec<String> {
        let mut all = Vec::new();
        for (table, columns) in KEPT {
            let sql = format!("SELECT '{table}' || json_array({columns}) FROM {table} ORDER BY 1");
            let mut stmt = conn.prepare(&sql).unwrap();
            let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
            all.extend(rows.map(Result::unwrap));
        }
        all
    }

    /// 0068 drops the old machinery's tables and columns: every kept row stays with its rowid and
    /// values, search still finds its words, and no reference is left broken.
    #[test]
    fn the_old_tables_go_and_every_kept_row_stays() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-drop-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..67] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 67;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 2);
             INSERT INTO projects (id, name, name_key, folder, created_ts, folder_name, ci, merge_method)
                 VALUES ('p', 'Notes', 'notes', 'x', 1, 'Notes', 'ci', 'squash');
             INSERT INTO project_members VALUES ('p', 'mira', 'supervisor', 0), ('p', 'ann', 'member', 1);
             INSERT INTO chats (id, title, created_ts, project, project_id, room) VALUES
                 ('c', 'Walk', 1, 'x', 'p', 1), ('d', 'Map', 2, NULL, NULL, 0);
             INSERT INTO asks (id, chat_id, size) VALUES ('a', 'c', 'new');
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, position, task,
                                answers, held, stop_why, opened_by, ask_id, result_of, placed, session_id)
                 VALUES ('t1', 'c', 'x', 'done', 5, 'mira', 1, 7, NULL, NULL, 'stopped', 'person', 'a',
                         NULL, 'main', 's1'),
                        ('t0', 'c', 'y', 'queued', 5, 'ann', NULL, NULL, 't1', 'pause', NULL, 'teammate',
                         NULL, 't1', NULL, NULL);
             INSERT INTO background VALUES ('b', 't1', 'Made-up command', 6);
             INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, about, reply_task, reply_to) VALUES
                 ('m1', 'c', 't1', 1, 'user', 'text', 'Plan the gravel walk', 't:7', 7, NULL),
                 ('gone', 'c', 't1', 2, 'assistant', 'text', 'gravel deleted', NULL, NULL, NULL),
                 ('m2', 'c', 't1', 3, 'assistant', 'text', 'The gravel map is ready', 't:7', NULL, 'm1'),
                 ('m3', 'd', NULL, 4, 'user', 'text', 'Another road', NULL, NULL, NULL);
             DELETE FROM messages WHERE id = 'gone';
             INSERT INTO reactions VALUES ('m2', 'ann', 'c', 7);
             INSERT INTO reads VALUES ('c', '', 3, NULL), ('c', 't:7', 3, 1);
             INSERT INTO agent_context (chat_id, agent_id, used, updated_ts, told) VALUES ('c', 'mira', 9, 8, 1);
             INSERT INTO questions (chat_id, agent, question, choices, asked_ts) VALUES
                 ('c', 'mira', 'Which name?', '[{\"label\":\"Ridge\"}]', 9);
             INSERT INTO board_lines (chat_id, owner, text, state) VALUES ('c', 'ann', 'Draw the map', 'doing');
             INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts, ask_id)
                 VALUES (7, 'c', 'mira', 'ann', 'work', 'Draw it', 'done', 1, 1, 'a');
             INSERT INTO task_after VALUES (7, 7);
             INSERT INTO turn_answers VALUES ('t1', 7);
             INSERT INTO task_sessions VALUES (7, 'ann', 'c', 'acc', 's2', NULL, 0, 0);
             INSERT INTO plans (id, chat_id, ask_id, lead, created_ts) VALUES (1, 'c', 'a', 'mira', 1);
             INSERT INTO plan_versions (id, plan_id, n, message_id, turn_id, title, text, created_ts)
                 VALUES (1, 1, 1, 'm2', 't1', 'Walk', 'Draw it', 1);
             INSERT INTO waits (id, chat_id, agent_id, task, kind, then_do, state, until_ts, created_ts)
                 VALUES ('w', 'c', 'ann', 7, 'pr', 'merge', 'open', 9, 2);
             INSERT INTO quiet (chat_id) VALUES ('c');
             INSERT INTO check_runs VALUES ('r', 'p', 'abc', 'passed', NULL, 0, 'x', 1, 2);
             INSERT INTO check_steps VALUES ('r', 0, 'cargo test', 0, 5, 'ok');
             INSERT INTO project_checks VALUES ('p', 0, 'cargo test', 'person', 1);",
        )
        .unwrap();
        let before = kept(&conn);
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        assert_eq!(kept(&conn), before);
        let found: Vec<i64> = conn
            .prepare("SELECT rowid FROM message_search WHERE message_search MATCH 'gravel' ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(found, [1, 3]);
        let broken: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0);
        let left: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name IN ('tasks', 'task_after', 'waits', 'asks',
                     'plans', 'plan_versions', 'plan_parts', 'held_turns', 'turn_answers', 'task_sessions',
                     'review_notes', 'landings', 'changes', 'quiet', 'pr_cleanup', 'project_checks',
                     'project_check_allowed', 'check_runs', 'check_steps', 'check_caches', 'turns_next')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(left, 0);
    }

    /// 0069 removes the pauses of one agent; a chat's and the app's stay.
    #[test]
    fn an_agents_pause_goes_and_a_chats_and_the_apps_stay() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-agent-pause-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..68] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 68;
             INSERT INTO pauses (scope, target, since_ts) VALUES
                 ('app', '', 1), ('chat', 'c', 2), ('agent', 'mira', 3);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let left: Vec<String> = conn
            .prepare("SELECT scope FROM pauses ORDER BY since_ts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(left, ["app", "chat"]);
    }

    /// 0070 gives agents a description: every agent from before stays, with none.
    #[test]
    fn agents_from_before_stay_with_no_description() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-description-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..69] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 69;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1), ('bo', 'Bo', 'bo', 'acc', 2);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let rows: Vec<String> = conn
            .prepare("SELECT rowid || ' ' || id || ' ' || coalesce(description, '-') FROM agents ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(rows, ["1 mira -", "2 bo -"]);
    }

    /// 0072 adds the tools table, empty: every agent and setting from before stays as it was.
    #[test]
    fn tools_arrive_empty_and_what_was_there_stays() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-tools-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..71] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 71;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts, shape) VALUES
                 ('mira', 'Mira', 'mira', 'acc', 1, 'round');
             INSERT INTO settings (key, value) VALUES ('skills_off', '[\"old-notes\"]');",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let kept: String = conn
            .query_row(
                "SELECT a.id || ' ' || a.shape || ' ' || s.value FROM agents a, settings s
                 WHERE s.key = 'skills_off'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, "mira round [\"old-notes\"]");
        let tools: i64 = conn
            .query_row("SELECT count(*) FROM tools", [], |r| r.get(0))
            .unwrap();
        assert_eq!(tools, 0);
    }

    /// 0073 adds a tool's sign-in, empty: a tool from before keeps its name, link and key.
    #[test]
    fn a_tool_from_before_keeps_its_key_and_has_no_sign_in() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-sign-in-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..72] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 72;
             INSERT INTO tools (id, name, link, enabled, key, added_ts) VALUES
                 ('t1', 'Tracker', 'https://tracker.example.com/mcp', 1, x'0102', 5);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let kept: String = conn
            .query_row(
                "SELECT name || ' ' || link || ' ' || hex(key) || ' ' || (sign_in IS NULL) FROM tools",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, "Tracker https://tracker.example.com/mcp 0102 1");
    }

    /// 0074 adds a tool's Client ID, empty: a tool from before keeps its name, link and sign-in.
    #[test]
    fn a_tool_from_before_keeps_its_sign_in_and_has_no_client_id() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-client-id-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..73] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 73;
             INSERT INTO tools (id, name, link, enabled, sign_in, added_ts) VALUES
                 ('t1', 'Tracker', 'https://tracker.example.com/mcp', 1, x'0102', 5);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let kept: String = conn
            .query_row(
                "SELECT name || ' ' || link || ' ' || hex(sign_in) || ' ' || (client_id IS NULL)
                 FROM tools",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kept, "Tracker https://tracker.example.com/mcp 0102 1");
    }

    /// 0075 makes every reviewer a member: it stays on its chat and its project, the lead stays.
    #[test]
    fn a_reviewer_from_before_stays_as_a_member() {
        let path = std::env::temp_dir()
            .join(format!("gathr-db-no-reviewer-{}", crate::ids::span_id()))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..74] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 74;
             INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
             INSERT INTO agents (id, name, name_key, account_id, created_ts, approvals)
                 VALUES ('mira', 'Mira', 'mira', 'acc', 1, 'full'), ('rev', 'Rev', 'rev', 'acc', 2, 'full');
             INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Notes', 'notes', 'x', 1);
             INSERT INTO project_members VALUES ('p', 'mira', 'supervisor', 0), ('p', 'rev', 'reviewer', 1);
             INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'C', 1, 'p');
             INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'rev', 'reviewer', 2);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let roles: String = conn
            .query_row(
                "SELECT (SELECT group_concat(agent_id || ' ' || role, ', ') FROM project_members)
                     || '; ' || (SELECT group_concat(agent_id || ' ' || role, ', ') FROM chat_members)",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            roles,
            "mira supervisor, rev member; mira supervisor, rev member"
        );
    }

    /// 0076 closes the open questions of a chat archived before, from when it was archived; a listed
    /// chat's open question and an answered one stay as they were.
    #[test]
    fn an_archived_chats_open_question_is_closed() {
        let path = std::env::temp_dir()
            .join(format!(
                "gathr-db-closed-questions-{}",
                crate::ids::span_id()
            ))
            .join("gathr.db");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..75] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 75;
             INSERT INTO chats (id, title, created_ts, archived_ts) VALUES ('old', 'Old', 1, 50), ('on', 'On', 1, NULL);
             INSERT INTO questions (chat_id, agent, question, asked_ts, answered_ts) VALUES
                 ('old', 'mira', 'Open?', 2, NULL), ('old', 'mira', 'Answered?', 3, 4), ('on', 'mira', 'Open?', 5, NULL);",
        )
        .unwrap();
        drop(conn);
        let conn = open(&path).unwrap();
        let closed: String = conn
            .query_row(
                "SELECT group_concat(id || ' ' || coalesce(closed_ts, '-'), ', ') FROM questions",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(closed, "1 50, 2 -, 3 -");
    }

    /// 0078 rebuilds `messages` for a block: every message keeps its rowid and columns, its indexes
    /// and search come back, a thumbs-up still points at its message, and a block is never indexed.
    #[test]
    fn messages_keep_everything_through_the_blocks_rebuild() {
        let path = super::tests::temp_db("blocks");
        let conn = Connection::open(&path).unwrap();
        for sql in &MIGRATIONS[..77] {
            conn.execute_batch(sql).unwrap();
        }
        conn.execute_batch(
            "PRAGMA user_version = 77;
             INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
             INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES ('t', 'c', 'x', 'done', 1);
             INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body, to_agent, copy_of, picks, reply_to)
             VALUES ('gone', 'c', 't', 1, 'user', 'text', 'tomato deleted', NULL, NULL, NULL, NULL),
                 ('asked', 'c', 't', 2, 'user', 'text', 'tomato bed', NULL, NULL, '[{\"at\":0}]', NULL),
                 ('ran', 'c', 't', 3, 'assistant', 'tool', '{\"tool\":\"tomato\"}', NULL, NULL, NULL, NULL),
                 ('said', 'c', 't', 4, 'assistant', 'text', 'tomato seeds', 'bo', 'asked', NULL, 'asked');
             DELETE FROM messages WHERE id = 'gone';
             INSERT INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES ('said', 'person', 'c', 5);",
        )
        .unwrap();
        let rows = |conn: &Connection| -> Vec<String> {
            let sql = "SELECT rowid || id || chat_id || turn_id || ts || role || kind || body
                         || coalesce(to_agent, '-') || coalesce(copy_of, '-') || coalesce(picks, '-')
                         || coalesce(reply_to, '-') FROM messages ORDER BY rowid";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let schema = |conn: &Connection| -> Vec<String> {
            let sql = "SELECT name FROM sqlite_schema WHERE tbl_name = 'messages' ORDER BY name";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let found = |conn: &Connection| -> Vec<i64> {
            let sql = "SELECT rowid FROM message_search WHERE message_search MATCH 'tomato' ORDER BY rowid";
            let mut stmt = conn.prepare(sql).unwrap();
            let rows = stmt.query_map([], |r| r.get(0)).unwrap();
            rows.map(Result::unwrap).collect()
        };
        let (before, names) = (rows(&conn), schema(&conn));
        assert_eq!(found(&conn), [2, 4]);
        drop(conn);

        let conn = open(&path).unwrap();
        assert_eq!(rows(&conn), before);
        assert_eq!(schema(&conn), names);
        assert_eq!(found(&conn), [2, 4]);
        conn.execute_batch(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
                 ('shown', 'c', 't', 6, 'assistant', 'block', '{\"kind\":\"table\",\"title\":\"tomato\"}');
             INSERT INTO message_search (message_search) VALUES ('integrity-check');",
        )
        .unwrap();
        assert_eq!(found(&conn), [2, 4]);
        let broken: i64 = conn
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(broken, 0);
    }
}
