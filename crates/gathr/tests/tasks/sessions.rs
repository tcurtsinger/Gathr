//! Stage 4, first slice (0055): each turn's key, the conversation it gives, the opening of a task's
//! new conversation, and the rows that follow each task's conversation.

use super::*;
use gathr::sessions::{self, Conversation, Key};

/// A second account, so a row can live in another login folder.
fn second_account(conn: &Connection) {
    let sql = "INSERT INTO accounts VALUES ('acc2', 'claude', 'Second', 'second', 1)";
    conn.execute(sql, []).unwrap();
}

/// A task of `owner`'s in chat `c`, last moved at `updated`.
fn task(conn: &Connection, number: i64, owner: &str, what: &str, state: &str, updated: i64) {
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (?1, 'c', 'lead', ?2, 'work', ?3, ?4, 1, ?5)",
        params![number, owner, what, state, updated],
    )
    .unwrap();
}

/// A turn of `agent`'s with no task, answering `answers`.
fn answering(conn: &Connection, id: &str, agent: Option<&str>, answers: &str) {
    gathr::store::add_turn(
        conn,
        "c",
        id,
        T,
        (agent, None, Some(answers)),
        gathr::words::Opened::Person,
    )
    .unwrap();
}

#[test]
fn a_turns_key_is_its_task_the_task_it_answers_or_latest() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 1, "ann", "Write the parser", "working", 1);
    task(&conn, 2, "lead", "Plan the week", "working", 1);
    task(&conn, 3, "bo", "Check the parser", "working", 1);
    turn(&conn, "ann-on-1", "ann", Some(1), "done");
    turn(&conn, "lead-on-2", "lead", Some(2), "running");
    turn(&conn, "bo-on-3", "bo", Some(3), "done");
    turn(&conn, "ann-free", "ann", None, "done");
    answering(&conn, "hear-back", Some("ann"), "ann-on-1");
    answering(&conn, "teammate", Some("ann"), "bo-on-3");
    answering(&conn, "wait-fired", Some("ann"), "ann-free");
    answering(&conn, "lead-heard", Some("lead"), "lead-on-2");
    answering(&conn, "no-agent", None, "ann-on-1");

    let key = |turn: &str| sessions::want(&conn, turn).unwrap();
    assert_eq!(key("ann-on-1"), Key::Task(1), "a task turn");
    assert_eq!(
        key("lead-on-2"),
        Key::Lead,
        "the lead, even on its own task"
    );
    assert_eq!(key("lead-heard"), Key::Lead);
    assert_eq!(
        key("hear-back"),
        Key::Answers(1),
        "answers its own turn on #1"
    );
    assert_eq!(key("teammate"), Key::Latest, "answers another agent's turn");
    assert_eq!(key("wait-fired"), Key::Latest, "left in a turn on no task");
    assert_eq!(key("ann-free"), Key::Latest);
    assert_eq!(key("no-agent"), Key::Latest);
}

#[test]
fn a_task_resumes_its_row_only_on_the_turns_account() {
    let dir = TempDir::new();
    let conn = team(&dir);
    second_account(&conn);
    task(&conn, 1, "ann", "Write the parser", "working", 1);
    task(&conn, 2, "ann", "Write the docs", "working", 1);
    sessions::set(&conn, 1, "ann", "c", "acc", "s-1").unwrap();
    sessions::set(&conn, 2, "ann", "c", "acc2", "s-2").unwrap();

    let choose = |key| sessions::choose(&conn, "ann", "acc", key).unwrap();
    let resumed = Conversation::Resume {
        task: 1,
        session: "s-1".into(),
        work_task: false,
    };
    assert_eq!(choose(Key::Task(1)), resumed);
    assert_eq!(choose(Key::Answers(1)), resumed, "a hear-back on #1");
    assert_eq!(
        choose(Key::Task(2)),
        Conversation::New(2),
        "its row is in another login folder"
    );
    assert_eq!(choose(Key::Answers(2)), Conversation::Latest, "never New");
    task(&conn, 3, "ann", "Ship it", "working", 1);
    assert_eq!(choose(Key::Task(3)), Conversation::New(3), "no row");
    assert_eq!(choose(Key::Answers(3)), Conversation::Latest, "no row");
    assert_eq!(choose(Key::Lead), Conversation::Latest);
    assert_eq!(choose(Key::Latest), Conversation::Latest);
}

#[test]
fn gathrs_fix_resumes_its_prs_work_task_when_it_is_the_same_agents_on_the_same_account() {
    let dir = TempDir::new();
    let conn = team(&dir);
    second_account(&conn);
    // #1 Ann's work on PR 7, #2 Bo's review of it, #3 the fix Gathr gave Ann for its notes.
    conn.execute_batch(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, pr, created_ts, updated_ts)
             VALUES (1, 'c', 'lead', 'ann', 'work', 'Build the page', 'held', 7, 1, 1),
                    (2, 'c', 'lead', 'bo', 'review', 'Review the page', 'blocked', 7, 1, 1),
                    (3, 'c', 'bo', 'ann', 'work', 'Fix the review of #7', 'ready', 7, 1, 1),
                    (4, 'c', 'lead', 'ann', 'work', 'Tidy the page', 'ready', 7, 1, 1);
         INSERT INTO review_notes (task, reviewed, kind, file, says, fix, created_ts)
             VALUES (2, 'abc1234', 'wrong', NULL, 'It fails on an empty page', 3, 1);",
    )
    .unwrap();
    let choose = |key| sessions::choose(&conn, "ann", "acc", key).unwrap();
    assert_eq!(
        choose(Key::Task(3)),
        Conversation::New(3),
        "the work task has none"
    );

    sessions::set(&conn, 1, "ann", "c", "acc", "s-work").unwrap();
    let work = Conversation::Resume {
        task: 3,
        session: "s-work".into(),
        work_task: true,
    };
    assert_eq!(choose(Key::Task(3)), work);
    assert_eq!(
        choose(Key::Task(4)),
        Conversation::New(4),
        "a task Gathr didn't give as a fix"
    );

    sessions::set(&conn, 1, "ann", "c", "acc2", "s-work").unwrap();
    assert_eq!(
        choose(Key::Task(3)),
        Conversation::New(3),
        "on another account"
    );

    sessions::set(&conn, 3, "ann", "c", "acc", "s-fix").unwrap();
    let own = Conversation::Resume {
        task: 3,
        session: "s-fix".into(),
        work_task: false,
    };
    assert_eq!(choose(Key::Task(3)), own, "later, its own row");

    // The work task became Bo's: its conversation isn't Ann's to go on in.
    conn.execute("DELETE FROM task_sessions WHERE task = 3", [])
        .unwrap();
    conn.execute("UPDATE tasks SET owner = 'bo' WHERE number = 1", [])
        .unwrap();
    sessions::set(&conn, 1, "bo", "c", "acc", "s-bo").unwrap();
    assert_eq!(choose(Key::Task(3)), Conversation::New(3));
}

const FIRST: &str = "This is a new conversation for task #5. Your earlier turns in this chat aren't in it: \
    read_chat and list_tasks have them.";

#[test]
fn a_new_conversation_opens_with_the_agents_five_newest_other_tasks() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 1, "ann", "Write the parser", "done", 10);
    task(&conn, 2, "ann", "Write the lexer", "done", 70);
    task(&conn, 3, "ann", "Fix the tests", "working", 30);
    task(&conn, 4, "ann", "Add the docs", "ready", 60);
    task(&conn, 5, "ann", "Ship it", "ready", 99);
    task(&conn, 6, "ann", "Rename the crate", "done", 50);
    task(&conn, 7, "ann", "Speed up the build", "done", 40);
    task(&conn, 8, "bo", "Check the parser", "done", 80);
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (9, 'other', 'lead', 'ann', 'work', 'Elsewhere', 'done', 1, 90)",
        [],
    )
    .unwrap();
    let brief = "Task #5 from Lead: Ship it\n\
        Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.";

    let (opening, count) = tasks::opening(&conn, 5, "ann", brief).unwrap();
    let expected = [
        FIRST,
        "Your last tasks in this chat:",
        "- #2 Ann, Write the lexer: done",
        "- #4 Ann, Add the docs: ready",
        "- #6 Ann, Rename the crate: done",
        "- #7 Ann, Speed up the build: done",
        "- #3 Ann, Fix the tests: working",
    ];
    assert_eq!(opening, expected.join("\n"));
    assert_eq!(count, 5);

    // An ask that isn't the brief: the task lost its conversation, so the brief comes too.
    let ask = "Your last turn on task #5 (Ship it) ended with no report. Use finish_task to say done, left or blocked.";
    let (opening, _) = tasks::opening(&conn, 5, "ann", ask).unwrap();
    assert_eq!(opening, [&expected.join("\n"), brief].join("\n"));
}

#[test]
fn an_agent_with_no_other_task_gets_the_first_line_alone() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 5, "ann", "Ship it", "ready", 99);
    task(&conn, 6, "bo", "Check it", "ready", 99);
    let brief = "Task #5 from Lead: Ship it\nUse finish_task with done, left or blocked.";
    let (opening, count) = tasks::opening(&conn, 5, "ann", brief).unwrap();
    assert_eq!((opening.as_str(), count), (FIRST, 0));
}

#[test]
fn a_conversation_brings_back_its_fill_and_whether_it_was_told_and_a_new_one_starts_empty() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 1, "ann", "Write the parser", "working", 1);
    sessions::set(&conn, 1, "ann", "c", "acc", "s-1").unwrap();
    gathr::context::filled(&conn, "c", "ann", 120_000).unwrap();
    sessions::used(&conn, 1, "ann", 120_000).unwrap();
    sessions::told(&conn, 1, "ann", true).unwrap();

    let shown = gathr::context::switched(&conn, "c", "ann", None).unwrap();
    assert!(shown["used"].is_null(), "a new conversation: {shown}");
    let told: bool = conn
        .query_row(
            "SELECT told FROM agent_context WHERE agent_id = 'ann'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!told, "a new conversation hasn't been told");

    let shown = gathr::context::switched(&conn, "c", "ann", Some((1, "s-1"))).unwrap();
    assert_eq!(shown["used"], 120_000, "#1's fill: {shown}");
    let told: bool = conn
        .query_row(
            "SELECT told FROM agent_context WHERE agent_id = 'ann'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert!(told, "#1's conversation was told already");

    sessions::told(&conn, 1, "ann", false).unwrap();
    sessions::set(&conn, 1, "ann", "c", "acc", "s-other").unwrap();
    let shown = gathr::context::switched(&conn, "c", "ann", Some((1, "s-other"))).unwrap();
    assert!(
        shown["used"].is_null(),
        "another conversation's fill isn't #1's: {shown}"
    );
}

#[test]
fn a_refused_id_is_forgotten_in_both_tables_for_that_agent_only() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 1, "ann", "Write the parser", "working", 1);
    task(&conn, 2, "ann", "Write the docs", "working", 1);
    task(&conn, 3, "bo", "Check the parser", "working", 1);
    sessions::set(&conn, 1, "ann", "c", "acc", "s-gone").unwrap();
    sessions::set(&conn, 2, "ann", "c", "acc", "s-kept").unwrap();
    sessions::set(&conn, 3, "bo", "c", "acc", "s-gone").unwrap();
    gathr::store::set_engine_session(&conn, "c", Some("ann"), "s-gone", "acc").unwrap();
    gathr::store::set_engine_session(&conn, "c", Some("bo"), "s-gone", "acc").unwrap();

    sessions::forget(&conn, "c", Some("ann"), "s-gone").unwrap();
    let rows = |sql: &str| -> Vec<String> {
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([], |r| r.get(0)).unwrap();
        rows.map(Result::unwrap).collect()
    };
    let tasks = rows("SELECT task || ':' || session_id FROM task_sessions ORDER BY task");
    assert_eq!(tasks, ["2:s-kept", "3:s-gone"]);
    let latest = rows("SELECT agent_key FROM engine_sessions ORDER BY agent_key");
    assert_eq!(latest, ["bo"]);
}

/// 0042's backfill: a task under way stays in its owner's one session; a settled task, and one whose
/// turn never ran, get none.
#[test]
fn an_open_task_that_ran_keeps_its_owners_session_after_the_migration() {
    let dir = TempDir::new();
    let conn = team(&dir);
    task(&conn, 1, "ann", "Write the parser", "working", 1);
    task(&conn, 2, "ann", "Write the lexer", "done", 1);
    task(&conn, 3, "ann", "Write the docs", "ready", 1);
    task(&conn, 4, "bo", "Check the parser", "blocked", 1);
    turn(&conn, "ran-1", "ann", Some(1), "done");
    turn(&conn, "ran-2", "ann", Some(2), "done");
    turn(&conn, "queued-3", "ann", Some(3), "queued");
    turn(&conn, "failed-4", "bo", Some(4), "failed");
    gathr::store::set_engine_session(&conn, "c", Some("ann"), "s-ann", "acc").unwrap();
    gathr::store::set_engine_session(&conn, "c", Some("bo"), "s-bo", "acc").unwrap();
    conn.execute_batch(
        "DROP TABLE task_sessions; ALTER TABLE turns DROP COLUMN opened_by;
         ALTER TABLE turns DROP COLUMN opened_version; DROP TABLE reactions;
         ALTER TABLE tasks DROP COLUMN done; ALTER TABLE tasks DROP COLUMN proof;
         ALTER TABLE tasks DROP COLUMN place; ALTER TABLE tasks DROP COLUMN size;
         ALTER TABLE tasks DROP COLUMN size_lines; ALTER TABLE tasks DROP COLUMN started_ts;
         ALTER TABLE tasks DROP COLUMN carry; ALTER TABLE tasks DROP COLUMN edited_ts;
         ALTER TABLE tasks DROP COLUMN edited;
         DROP INDEX tasks_checking; ALTER TABLE tasks DROP COLUMN checks;
         DROP TABLE plan_parts; DROP TABLE plan_versions; DROP TABLE plans;
         DROP INDEX turns_by_ask; DROP INDEX tasks_by_ask; ALTER TABLE turns DROP COLUMN ask_id;
         ALTER TABLE tasks DROP COLUMN ask_id; DROP TABLE asks; ALTER TABLE tasks DROP COLUMN look_into;
         DROP TABLE held_turns; ALTER TABLE changes DROP COLUMN size; ALTER TABLE changes DROP COLUMN line_of;
         PRAGMA user_version = 41;",
    )
    .unwrap();
    drop(conn);

    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    let mut stmt = conn
        .prepare(
            "SELECT task || ' ' || agent_id || ' ' || account_id || ' ' || session_id
             FROM task_sessions ORDER BY task",
        )
        .unwrap();
    let rows: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(rows, ["1 ann acc s-ann", "4 bo acc s-bo"]);
}
