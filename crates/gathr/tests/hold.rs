//! 0063, Stage 3 part I: building waits until a new ask's plan is approved; a repair goes ahead.
//! The one rule, the refused gives, writes and pushes, and the tasks that wait for a plan, at the
//! SQLite seam.

mod common;
#[path = "hold/give.rs"]
mod give;
#[path = "hold/look.rs"]
mod look;
#[path = "words/samples.rs"]
mod samples;
#[path = "hold/wait.rs"]
mod wait;

use common::TempDir;
use gathr::Error;
use gathr::asks::{self, Sizing};
use gathr::hold;
use gathr::plans::{self, Part, Writing, Wrote};
use gathr::tasks::{self, Brief, Finish, Give, Kind, Moved, Said, Start, State};
use gathr::words::Opened;
use rusqlite::Connection;

const T: &str = "test-trace";
const TAGS: &str = "Add tags to notes";

/// Chat `c` of project `p`: Mira leads, Ann is a member, Bo the Reviewer. Chat `d`, of no project:
/// Mira and Ann. Chat `solo`: Mira alone. The person's message in Mira's turn `T` in `c` starts an
/// ask, unsized, and `T` runs: the database and the ask.
fn room(dir: &TempDir) -> (Connection, String) {
    let mut conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1), ('bo', 'Bo', 'bo', 'acc', 1);
         INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Notes', 'notes', 'x', 1);
         INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'Chat', 1, 'p');
         INSERT INTO chats (id, title, created_ts) VALUES ('d', 'Plain', 1), ('solo', 'Solo', 1);
         INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 2),
             ('c', 'bo', 'reviewer', 3), ('d', 'mira', 'supervisor', 1), ('d', 'ann', 'member', 2),
             ('solo', 'mira', 'supervisor', 1);",
    )
    .unwrap();
    let ask = sent(&mut conn, ("c", "mira"), "T", TAGS);
    (conn, ask)
}

/// The person's message in a new turn of `agent`'s, saved as `send` saves it, on an ask of its own
/// (as when nothing else is open), the turn then running: the message's id, the ask's.
fn sent(conn: &mut Connection, (chat, agent): (&str, &str), turn: &str, text: &str) -> String {
    let tx = conn.transaction().unwrap();
    let on = (Some(agent), None, None);
    gathr::store::add_turn(&tx, chat, turn, T, on, Opened::Person).unwrap();
    let message = gathr::store::add_message(&tx, chat, turn, "user", "text", text).unwrap();
    let id = message["id"].as_str().unwrap().to_string();
    let own = "INSERT OR IGNORE INTO asks (id, chat_id) VALUES (?1, ?2)";
    tx.execute(own, [&id, chat]).unwrap();
    let running = "UPDATE turns SET ask_id = ?2, status = 'running' WHERE id = ?1";
    tx.execute(running, [turn, &id]).unwrap();
    tx.commit().unwrap();
    id
}

/// Whether the one rule holds the turn.
fn held(conn: &Connection, turn: &str) -> bool {
    hold::turn(conn, turn).unwrap().is_some_and(|h| h.held())
}

/// A task given by `giver` to `owner` from `given_in`: a work task with a complete brief, or a
/// review with none.
fn give(
    conn: &mut Connection,
    (giver, owner): (&str, &str),
    given_in: Option<&str>,
    (kind, look_into, pr): (Kind, bool, Option<i64>),
) -> Result<(tasks::Task, Vec<Start>), Error> {
    let g = Give {
        chat: "c",
        giver: Some(giver),
        owner,
        given_in,
        parent: None,
        kind,
        what: if look_into {
            "Survey tag libraries"
        } else {
            "Add the tags table"
        },
        brief: if kind == Kind::Review {
            Brief::default()
        } else {
            common::COMPLETE
        },
        after: &[],
        pr,
        in_passing: false,
        look_into,
    };
    tasks::give(conn, &g, T)
}

/// A work task (building, or a look-into task), given and taken: its number.
fn work(
    conn: &mut Connection,
    (giver, owner): (&str, &str),
    given_in: &str,
    look_into: bool,
) -> i64 {
    let how = (Kind::Work, look_into, None);
    give(conn, (giver, owner), Some(given_in), how)
        .unwrap()
        .0
        .number
}

fn refusal(result: Result<impl std::fmt::Debug, Error>) -> String {
    match result {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// The newest turn queued on `task`.
fn queued_on(conn: &Connection, task: i64) -> Option<String> {
    let sql =
        "SELECT id FROM turns WHERE task = ?1 AND status = 'queued' ORDER BY rowid DESC LIMIT 1";
    conn.query_row(sql, [task], |r| r.get(0)).ok()
}

/// A turn moves to `status` as the runner moves it, with what follows.
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

/// The task's queued turn starts running, as the runner starts it: the turn.
fn begin(conn: &mut Connection, task: i64) -> String {
    let turn = queued_on(conn, task).unwrap();
    let owner = tasks::get(conn, task).unwrap().unwrap().owner.unwrap();
    moved(conn, (&turn, &owner), Some(task), "running");
    turn
}

fn state(conn: &Connection, task: i64) -> (State, Option<String>) {
    let t = tasks::get(conn, task).unwrap().unwrap();
    (t.state, t.reason)
}

/// The ask's `held_ts`.
fn held_ts(conn: &Connection, ask: &str) -> Option<i64> {
    let sql = "SELECT held_ts FROM asks WHERE id = ?1";
    conn.query_row(sql, [ask], |r| r.get(0)).unwrap()
}

/// Each `gathr.hold.<name>` event's attributes, oldest first.
fn events(conn: &Connection, name: &str) -> Vec<serde_json::Value> {
    let sql = "SELECT json(attrs) FROM events WHERE name = ?1 ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([format!("gathr.hold.{name}")], |r| r.get::<_, String>(0));
    let rows = rows.unwrap().map(Result::unwrap);
    rows.map(|a| serde_json::from_str(&a).unwrap()).collect()
}

/// Mira sizes the ask her `turn` serves.
fn lead_sizes(conn: &mut Connection, turn: &str, size: &str) -> Vec<Start> {
    let s = Sizing {
        chat: "c",
        agent: "mira",
        turn: Some(turn),
        size,
        message: None,
        trace: T,
    };
    asks::size(conn, &s).unwrap().1
}

/// Mira writes a plan of one part from `turn`, its part named `what`.
fn plan(conn: &mut Connection, turn: &str, what: &str) -> Wrote {
    let part = Part {
        to: "Ann",
        what,
        why: Some("Notes need tags"),
        done: Some("A note carries its tags"),
        proof: Some("tests/tags.rs passes"),
        place: Some("the project's worktree, on a new branch"),
        size: Some("src/tags.rs"),
        lines: Some(60),
        after: &[],
        after_tasks: &[],
    };
    let w = Writing {
        chat: "c",
        agent: "mira",
        turn: Some(turn),
        title: "Tags on notes",
        text: "Let notes carry tags.",
        parts: &[part],
    };
    plans::write(conn, &w, T).unwrap()
}

/// The version reaches the person, its round run late (0080): it waits on them.
fn agreed(conn: &mut Connection, wrote: &Wrote) {
    let ten = 600_000_000;
    plans::overdue(conn, "c", gathr::ids::now_us() + ten, ten, T).unwrap();
    let sent: Option<i64> = conn
        .query_row(
            "SELECT sent_ts FROM plan_versions WHERE id = ?1",
            [wrote.version],
            |r| r.get(0),
        )
        .unwrap();
    assert!(sent.is_some());
}
/// A turn Gathr queues in chat `c` for `agent`, on `task`, answering `answers`: its id.
fn queued(
    conn: &Connection,
    agent: &str,
    (task, answers): (Option<i64>, Option<&str>),
    opened: Opened,
) -> String {
    let start = Start::queue(conn, ("c", agent), (task, answers), opened, "Go on".into());
    start.unwrap().turn
}

/// I0, I0.4 to I0.6: the one rule, turn by turn.
#[test]
fn the_one_rule_holds_a_new_asks_turns_and_a_look_into_task_for_its_whole_life() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    let mut seen = vec![("T, unsized", held(&conn, "T"), true)];
    asks::resize(&mut conn, &m1, "new", T).unwrap();
    seen.push(("T, sized new", held(&conn, "T"), true));
    asks::resize(&mut conn, &m1, "repair", T).unwrap();
    seen.push(("T, a repair", held(&conn, "T"), false));
    let look = work(&mut conn, ("mira", "ann"), "T", true);
    let look_turn = queued_on(&conn, look).unwrap();
    seen.push((
        "a look-into task's turn on a repair ask",
        held(&conn, &look_turn),
        true,
    ));
    let back = queued(&conn, "mira", (None, Some("T")), Opened::HearBack);
    seen.push((
        "Mira's hear-back, on the repair ask",
        held(&conn, &back),
        false,
    ));
    conn.execute("UPDATE asks SET size = NULL WHERE id = ?1", [&m1])
        .unwrap();
    seen.push((
        "Mira's hear-back, on an unsized ask",
        held(&conn, &back),
        true,
    ));

    // Its plan, state by state.
    asks::resize(&mut conn, &m1, "new", T).unwrap();
    let first = plan(&mut conn, "T", "Add the tags table");
    seen.push(("T, its plan being checked", held(&conn, "T"), true));
    let check = queued(&conn, "ann", (None, Some("T")), Opened::Check);
    seen.push(("a member's check of it", held(&conn, &check), true));
    agreed(&mut conn, &first);
    seen.push(("T, its plan waiting on the person", held(&conn, "T"), true));
    plans::send_back(&mut conn, first.plan, 1, "Split it", T).unwrap();
    seen.push(("T, its plan sent back", held(&conn, "T"), true));
    plan(&mut conn, "T", "Add the tags column");
    plans::drop(&mut conn, first.plan, 2, T).unwrap();
    seen.push(("T, its plan dropped", held(&conn, "T"), true));
    let second = plan(&mut conn, "T", "Add the tags table");
    agreed(&mut conn, &second);
    let approved = plans::approve(&mut conn, second.plan, 1, T).unwrap();
    seen.push(("T, its plan approved", held(&conn, "T"), false));
    let part = approved
        .starts
        .iter()
        .find(|s| s.agent == "ann" && s.task.is_some());
    seen.push((
        "Approve's part's turn",
        held(&conn, &part.unwrap().turn),
        false,
    ));
    seen.push((
        "the look-into task's turn after Approve",
        held(&conn, &look_turn),
        true,
    ));

    // Building tasks, a review, and a hear-back on a building task.
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the export");
    lead_sizes(&mut conn, "T2", "repair");
    let build = work(&mut conn, ("mira", "ann"), "T2", false);
    let build_turn = begin(&mut conn, build);
    asks::resize(&mut conn, &m2, "new", T).unwrap();
    seen.push((
        "a building task's turn, its ask made new while it works",
        held(&conn, &build_turn),
        false,
    ));
    let own = queued(
        &conn,
        "ann",
        (Some(build), Some(&build_turn)),
        Opened::HearBack,
    );
    seen.push((
        "Ann's hear-back on her building task, on a new ask",
        held(&conn, &own),
        false,
    ));
    conn.execute("UPDATE asks SET size = NULL WHERE id = ?1", [&m2])
        .unwrap();
    seen.push((
        "a building task's turn on an unsized ask",
        held(&conn, &build_turn),
        false,
    ));
    let m3 = sent(&mut conn, ("c", "mira"), "T3", "Check the import");
    let review = give(
        &mut conn,
        ("mira", "bo"),
        Some("T3"),
        (Kind::Review, false, None),
    );
    let review_turn = queued_on(&conn, review.unwrap().0.number).unwrap();
    seen.push((
        "a review task's turn on an unsized ask",
        held(&conn, &review_turn),
        true,
    ));
    assert!(held_ts(&conn, &m3).is_none(), "a review isn't refused");

    // Turns that serve no ask.
    for (what, opened) in [
        ("a wake", Opened::Wake),
        ("a silent turn told", Opened::Silent),
        ("stale tasks told", Opened::Stale),
        ("a landing hand-off", Opened::Landing),
        ("a compaction", Opened::Compaction),
        ("the person's answer to Gathr's question", Opened::Answered),
    ] {
        let turn = queued(&conn, "mira", (None, None), opened);
        seen.push((what, held(&conn, &turn), false));
    }
    conn.execute_batch(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id) VALUES ('raw', 'c', 'x', 'running', 1, 'mira');",
    )
    .unwrap();
    seen.push(("a turn from before asks", held(&conn, "raw"), false));
    sent(&mut conn, ("solo", "mira"), "S", TAGS);
    seen.push(("a turn in a one-agent chat", held(&conn, "S"), false));
    sent(&mut conn, ("d", "ann"), "A", "Fix the typo");
    seen.push((
        "Ann's own turn on an unsized ask the person sent her",
        held(&conn, "A"),
        true,
    ));
    sent(&mut conn, ("d", "mira"), "D", "Fix the label");
    seen.push(("Mira's turn in chat d, unsized", held(&conn, "D"), true));
    conn.execute(
        "DELETE FROM chat_members WHERE chat_id = 'd' AND agent_id = 'ann'",
        [],
    )
    .unwrap();
    seen.push(("a team chat that lost its members", held(&conn, "D"), false));
    assert!(hold::turn(&conn, "nope").unwrap().is_none(), "no such turn");

    let wrong: Vec<_> = seen.iter().filter(|(_, got, want)| got != want).collect();
    assert!(wrong.is_empty(), "held, as (what, got, wanted): {wrong:#?}");
}
