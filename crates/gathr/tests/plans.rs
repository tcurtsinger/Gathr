//! 0062, Stage 3 part H: the lead's plan, a record of versions written with `plan`, each checked
//! before the person sees it, at the SQLite seam.

#[path = "plans/check.rs"]
mod check;
mod common;
#[path = "plans/decide.rs"]
mod decide;
#[path = "words/samples.rs"]
mod samples;
#[path = "plans/write.rs"]
mod write;

use common::TempDir;
use gathr::Error;
use gathr::plans::{self, Answer, Part, Writing, Wrote};
use gathr::tasks::{self, Finish, Moved, Said, Start};
use gathr::words::Opened;
use rusqlite::{Connection, params};

const T: &str = "test-trace";
const TITLE: &str = "Tags on notes";
const TEXT: &str = "Let notes carry tags, shown in the list and used to filter.";

/// Chat `c`: Mira leads, Ann is a member, Bo the Reviewer. Cy is an agent in no chat yet. The
/// person's message in Mira's turn `T` starts an ask, and `T` runs: the database and the ask.
fn room(dir: &TempDir) -> (Connection, String) {
    let mut conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1),
             ('bo', 'Bo', 'bo', 'acc', 1), ('cy', 'Cy', 'cy', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 2),
             ('c', 'bo', 'reviewer', 3);",
    )
    .unwrap();
    let ask = asked(&mut conn, "c", "T", "Add tags to notes");
    (conn, ask)
}

/// The person's message in a new turn of Mira's in `chat`, which then runs, on an ask of its own:
/// the ask.
fn asked(conn: &mut Connection, chat: &str, turn: &str, text: &str) -> String {
    let tx = conn.transaction().unwrap();
    gathr::store::add_turn(
        &tx,
        chat,
        turn,
        T,
        (Some("mira"), None, None),
        Opened::Person,
    )
    .unwrap();
    let message = gathr::store::add_message(&tx, chat, turn, "user", "text", text).unwrap();
    let id = message["id"].as_str().unwrap().to_string();
    tx.execute(
        "INSERT OR IGNORE INTO asks (id, chat_id) VALUES (?1, ?2)",
        [&id, chat],
    )
    .unwrap();
    tx.execute(
        "UPDATE turns SET ask_id = ?2, status = 'running' WHERE id = ?1",
        [turn, &id],
    )
    .unwrap();
    tx.commit().unwrap();
    id
}

/// Three parts: Ann adds the table, Bo shows the tags after it, Ann filters after part 2.
fn three() -> Vec<Part<'static>> {
    vec![
        part("Ann", "Add the tags table", &[]),
        part("Bo", "Show tags in the list", &[1]),
        part("Ann", "Filter by tag", &[2]),
    ]
}

fn part<'a>(to: &'a str, what: &'a str, after: &'a [i64]) -> Part<'a> {
    Part {
        to,
        what,
        why: Some("Notes need tags"),
        done: Some("A note carries its tags"),
        proof: Some("tests/tags.rs passes"),
        place: Some("the project's worktree, on a new branch"),
        size: Some("src/tags.rs"),
        lines: Some(60),
        after,
        after_tasks: &[],
    }
}

/// `plan` from Mira's `turn` in chat `c`.
fn write(
    conn: &mut Connection,
    turn: Option<&str>,
    parts: &[Part],
    answers: Option<&[Answer]>,
) -> Result<Wrote, Error> {
    let w = Writing {
        chat: "c",
        agent: "mira",
        turn,
        title: TITLE,
        text: TEXT,
        parts,
        answers,
    };
    plans::write(conn, &w, T)
}

fn refusal(result: Result<impl std::fmt::Debug, Error>) -> String {
    match result {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A column of the version.
fn version_field<V: rusqlite::types::FromSql>(conn: &Connection, version: i64, column: &str) -> V {
    let sql = format!("SELECT {column} FROM plan_versions WHERE id = ?1");
    conn.query_row(&sql, [version], |r| r.get(0)).unwrap()
}

/// The queued turn on `task`, and its message.
fn queued_on(conn: &Connection, task: i64) -> (String, String) {
    let sql =
        "SELECT u.id, m.body FROM turns u JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
               WHERE u.task = ?1 AND u.status = 'queued' ORDER BY u.rowid DESC LIMIT 1";
    conn.query_row(sql, [task], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
}

/// The text with each message id of the chat's versions read as `<id>`, as the samples have it.
fn sampled(conn: &Connection, text: &str) -> String {
    let mut stmt = conn
        .prepare("SELECT message_id FROM plan_versions")
        .unwrap();
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    ids.iter()
        .fold(text.to_string(), |t, id| t.replace(id, "<id>"))
}

/// What Gathr said in no turn in `chat`, oldest first.
fn lines(conn: &Connection, chat: &str) -> Vec<String> {
    let sql = "SELECT body FROM messages WHERE chat_id = ?1 AND turn_id IS NULL AND role = 'gathr' ORDER BY ts, id";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([chat], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// Each `gathr.plan.<name>` event's attributes, oldest first.
fn events(conn: &Connection, name: &str) -> Vec<serde_json::Value> {
    let sql = "SELECT json(attrs) FROM events WHERE name = ?1 ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([format!("gathr.plan.{name}")], |r| r.get::<_, String>(0));
    let rows = rows.unwrap().map(Result::unwrap);
    rows.map(|a| serde_json::from_str(&a).unwrap()).collect()
}

/// A turn of `agent`'s moves to `status`, with what follows it.
fn moved(
    conn: &mut Connection,
    (turn, agent): (&str, &str),
    task: Option<i64>,
    status: &str,
) -> Vec<Start> {
    let m = Moved {
        chat: "c",
        turn,
        agent: Some(agent),
        task,
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(conn, &m, status, None).unwrap()
}

/// The check's queued turn starts running: its id.
fn begin(conn: &mut Connection, check: i64) -> String {
    let (turn, _) = queued_on(conn, check);
    moved(conn, (&turn, "bo"), Some(check), "running");
    turn
}

/// A report by `agent` on `task`.
fn report(
    conn: &mut Connection,
    agent: &str,
    task: i64,
    how: Finish,
    said: &Said,
) -> Result<tasks::Took, Error> {
    tasks::finish(conn, agent, task, how, said, T).map(|(_, took, ..)| took)
}

/// Bo leaves the check with a wrong and an unasked note.
fn findings(conn: &mut Connection, check: i64) {
    let notes = [
        tasks::NoteSaid {
            kind: tasks::NoteKind::Wrong,
            file: Some("src/notes.rs"),
            says: "The list query misses tags",
        },
        tasks::NoteSaid {
            kind: tasks::NoteKind::Unasked,
            file: None,
            // Ends in a full stop, as agents often write: the reason mustn't double it.
            says: "Export isn't asked.",
        },
    ];
    let said = Said {
        note: Some("Two gaps"),
        notes: &notes,
        ..Said::default()
    };
    report(conn, "bo", check, Finish::Left, &said).unwrap();
}

/// The ids of the newest report's notes on `check`.
fn note_ids(conn: &Connection, check: i64) -> Vec<i64> {
    let mut stmt = conn
        .prepare("SELECT id FROM review_notes WHERE task = ?1 ORDER BY id")
        .unwrap();
    let rows = stmt.query_map([check], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

fn task_state(conn: &Connection, task: i64) -> (tasks::State, Option<String>) {
    let t = tasks::get(conn, task).unwrap().unwrap();
    (t.state, t.reason)
}

fn plan_state(conn: &Connection, chat: &str, plan: i64) -> plans::State {
    let all = plans::in_chat(conn, chat).unwrap();
    all.into_iter().find(|p| p.id == plan).unwrap().state
}

/// How many of each: plans, versions, parts, the agents' messages and tasks.
fn counts(conn: &Connection) -> (i64, i64, i64, i64, i64) {
    conn.query_row(
        "SELECT (SELECT count(*) FROM plans), (SELECT count(*) FROM plan_versions),
                (SELECT count(*) FROM plan_parts), (SELECT count(*) FROM messages WHERE role = 'assistant'),
                (SELECT count(*) FROM tasks)",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .unwrap()
}

fn set_status(conn: &Connection, turn: &str, status: &str) {
    conn.execute(
        "UPDATE turns SET status = ?2 WHERE id = ?1",
        params![turn, status],
    )
    .unwrap();
}
