//! The stale and cancel rules (F11, 0010) on a real database, with the clock passed in: a task
//! nothing moves is marked stale and its Supervisor told, then cancelled unless something can still
//! move it, and a task that can never go on is cancelled at once, in any chat.

mod common;

use common::TempDir;
use gathr::pauses::Pause;
use gathr::tasks::stale::{self, Days};
use gathr::tasks::{self, Give, Kind, State};
use rusqlite::{Connection, params};

const T: &str = "test-trace";
const DAY: i64 = 86_400_000_000;

/// A chat with a Supervisor (`lead`) and a member (`ann`).
fn team(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1);",
    )
    .unwrap();
    conn
}

/// A task for Ann, left `ready` with its queued turn ended, last moved at `moved`.
fn idle_task(conn: &mut Connection, after: &[i64], moved: i64) -> i64 {
    let g = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix the build",
        why: None,
        after,
        pr: None,
    };
    let (task, _) = tasks::give(conn, &g, T).unwrap();
    let n = task.number;
    conn.execute(
        "UPDATE turns SET status = 'failed', created_ts = ?2, ended_ts = ?2 WHERE task = ?1",
        params![n, moved],
    )
    .unwrap();
    conn.execute(
        "UPDATE messages SET ts = ?2 WHERE turn_id IN (SELECT id FROM turns WHERE task = ?1)",
        params![n, moved],
    )
    .unwrap();
    conn.execute(
        "UPDATE tasks SET updated_ts = ?2, created_ts = ?2 WHERE number = ?1",
        params![n, moved],
    )
    .unwrap();
    n
}

fn sweep(conn: &mut Connection, now: i64, pauses: &[Pause]) -> stale::Swept {
    stale::sweep(conn, now, DAY, pauses, T).unwrap()
}

fn task(conn: &Connection, n: i64) -> tasks::Task {
    tasks::get(conn, n).unwrap().unwrap()
}

#[test]
fn a_task_nothing_moves_for_the_stale_days_is_marked_once_and_its_supervisor_told() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let n = idle_task(&mut conn, &[], DAY);
    let swept = sweep(&mut conn, 3 * DAY + DAY / 2, &[]);
    assert!(
        swept.marked.is_empty(),
        "2.5 days isn't stale: {:?}",
        swept.marked
    );
    let swept = sweep(&mut conn, 4 * DAY + 1, &[]);
    assert_eq!(swept.marked, [n]);
    assert_eq!(
        task(&conn, n).stale_us,
        Some(DAY),
        "stale since its last move"
    );
    assert_eq!(swept.starts.len(), 1, "{:?}", swept.starts);
    let told = &swept.starts[0];
    assert_eq!((told.agent.as_str(), told.task), ("lead", None));
    assert!(
        told.text.contains(&format!("#{n}")) && told.text.contains("Ann"),
        "the Supervisor reads which task and whose: {}",
        told.text
    );
    assert!(
        told.text
            .contains("agent tasks auto-cancel after 7 more days without progress unless work is ongoing or paused."),
        "agent stale notice must retain conditional auto-cancel policy: {}",
        told.text
    );
    let again = sweep(&mut conn, 4 * DAY + 2, &[]);
    assert!(
        again.marked.is_empty() && again.starts.is_empty(),
        "told once"
    );
}

#[test]
fn person_only_stale_notice_omits_agent_auto_cancel_policy() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (person, _) = tasks::person::ask(
        &mut conn,
        tasks::person::Ask {
            chat: "c",
            giver: "lead",
            given_in: None,
            parent: None,
            what: Some("Approve the wording?"),
            why: None,
            form: None,
            choices: Vec::new(),
            files: Vec::new(),
            request: None,
        },
        T,
    )
    .unwrap();
    conn.execute(
        "UPDATE tasks SET updated_ts = ?1, created_ts = ?1 WHERE number = ?2",
        params![DAY, person.number],
    )
    .unwrap();
    let swept = sweep(&mut conn, 4 * DAY + 1, &[]);
    let notice = &swept.starts[0].text;
    assert!(
        !notice.contains("cancelled after") && !notice.contains("auto-cancel"),
        "person-only notice must omit agent auto-cancel policy: {notice}"
    );
}

#[test]
fn a_stale_task_that_moves_is_stale_no_more() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let n = idle_task(&mut conn, &[], DAY);
    sweep(&mut conn, 5 * DAY, &[]);
    assert!(task(&conn, n).stale_us.is_some());
    // A message in a turn on it, with no change of state, is a move too.
    gathr::store::add_turn(&conn, "c", "late", T, (Some("ann"), Some(n), None)).unwrap();
    conn.execute(
        "UPDATE turns SET status = 'done', created_ts = ?1, ended_ts = ?1 WHERE id = 'late'",
        [5 * DAY + 1],
    )
    .unwrap();
    sweep(&mut conn, 5 * DAY + 2, &[]);
    assert_eq!(task(&conn, n).stale_us, None);
    assert_eq!(task(&conn, n).state, State::Ready);
}

#[test]
fn after_the_cancel_days_a_stale_task_nothing_can_move_is_cancelled_and_restarts() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let n = idle_task(&mut conn, &[], DAY);
    sweep(&mut conn, 5 * DAY, &[]);
    // Marked at day 4 (3 days after its last move), cancelled 7 days after that.
    let swept = sweep(&mut conn, 11 * DAY - 1, &[]);
    assert!(
        swept.cancelled.is_empty(),
        "under 7 days stale: {:?}",
        swept.cancelled
    );
    let swept = sweep(&mut conn, 11 * DAY, &[]);
    assert_eq!(swept.cancelled, [n]);
    let t = task(&conn, n);
    assert_eq!(t.state, State::Cancelled);
    assert_eq!(t.reason.as_deref(), Some("stale since 1970-01-02"));
    let starts = tasks::restart(&mut conn, None, n, T).unwrap();
    assert_eq!(task(&conn, n).state, State::Ready);
    assert_eq!(starts.len(), 1, "a restart starts its owner's turn again");
}

#[test]
fn a_stale_task_something_can_still_move_is_only_flagged() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let waiting = idle_task(&mut conn, &[], DAY);
    conn.execute(
        "INSERT INTO waits (id, chat_id, agent_id, task, kind, target, then_do, state, until_ts, created_ts)
         VALUES ('w', 'c', 'ann', ?1, 'pr', '7', 'carry on', 'open', ?2, ?3)",
        params![waiting, 99 * DAY, DAY],
    )
    .unwrap();
    let with_pr = idle_task(&mut conn, &[], DAY);
    conn.execute("UPDATE tasks SET pr = 9 WHERE number = ?1", [with_pr])
        .unwrap();
    let queued = idle_task(&mut conn, &[], DAY);
    conn.execute(
        "UPDATE turns SET status = 'queued', held = 'usage' WHERE task = ?1",
        [queued],
    )
    .unwrap();
    let paused = idle_task(&mut conn, &[], DAY);
    let person = gathr::tasks::person::ask(
        &mut conn,
        gathr::tasks::person::Ask {
            chat: "c",
            giver: "lead",
            given_in: None,
            parent: None,
            what: Some("Pick a name"),
            why: None,
            form: None,
            choices: Vec::new(),
            files: Vec::new(),
            request: None,
        },
        T,
    )
    .unwrap()
    .0
    .number;
    conn.execute(
        "UPDATE tasks SET updated_ts = ?1, created_ts = ?1 WHERE number = ?2",
        params![DAY, person],
    )
    .unwrap();
    // Ann alone is paused for the last sweep: her task waits on the person's resume.
    let ann = [Pause {
        scope: "agent".into(),
        target: "ann".into(),
        since_us: DAY,
        quiet_us: None,
        stopped: false,
    }];
    let mixed = sweep(&mut conn, 5 * DAY, &[]);
    let notice = &mixed.starts[0].text;
    assert!(
        notice.contains(&format!("#{person} Pick a name (for the person)"))
            && notice
                .contains("agent tasks auto-cancel after 7 more days without progress unless work is ongoing or paused."),
        "mixed notice must identify the person and scope auto-cancel to agents: {notice}"
    );
    for n in [waiting, with_pr, queued, person] {
        assert!(task(&conn, n).stale_us.is_some(), "#{n} is flagged");
    }
    conn.execute("UPDATE waits SET state = 'cancelled' WHERE id = 'w'", [])
        .unwrap();
    conn.execute(
        "INSERT INTO waits (id, chat_id, agent_id, task, kind, target, then_do, state, until_ts, created_ts)
         VALUES ('w2', 'c', 'ann', ?1, 'pr', '7', 'carry on', 'open', ?2, ?3)",
        params![waiting, 99 * DAY, DAY],
    )
    .unwrap();
    conn.execute("UPDATE waits SET created_ts = ?1", [DAY])
        .unwrap();
    let swept = sweep(&mut conn, 12 * DAY, &ann);
    assert!(swept.cancelled.is_empty(), "{:?}", swept.cancelled);
    let swept = sweep(&mut conn, 12 * DAY, &[]);
    assert_eq!(swept.cancelled, [paused], "only the one nothing can move");
    assert_eq!(
        task(&conn, person).state,
        State::Ready,
        "the person's own stays"
    );
}

#[test]
fn a_task_whose_after_task_or_blocker_is_cancelled_is_cancelled_at_once() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let first = idle_task(&mut conn, &[], DAY);
    let second = idle_task(&mut conn, &[first], DAY);
    let blocked = idle_task(&mut conn, &[], DAY);
    conn.execute(
        "UPDATE tasks SET state = 'blocked', blocked_on = ?1 WHERE number = ?2",
        [first, blocked],
    )
    .unwrap();
    tasks::cancel(&mut conn, None, first, "not needed", T).unwrap();
    assert_eq!(
        (task(&conn, second).state, task(&conn, second).reason),
        (State::Cancelled, Some(format!("#{first} was cancelled")))
    );
    assert_eq!(task(&conn, blocked).state, State::Cancelled);
}

/// The task's turn runs and ends so.
fn run(conn: &mut Connection, n: i64, outcome: tasks::Outcome) {
    tasks::started(conn, n, T).unwrap();
    tasks::ended(conn, n, outcome, T).unwrap();
}

#[test]
fn a_dependant_already_blocked_is_cancelled_when_another_of_its_after_tasks_is() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (failed, open) = (
        idle_task(&mut conn, &[], DAY),
        idle_task(&mut conn, &[], DAY),
    );
    let both = idle_task(&mut conn, &[failed, open], DAY);
    run(&mut conn, failed, tasks::Outcome::Failed("it broke".into()));
    assert_eq!(
        task(&conn, both).state,
        State::Blocked,
        "blocked by #{failed}"
    );
    tasks::cancel(&mut conn, None, open, "not needed", T).unwrap();
    assert_eq!(
        (task(&conn, both).state, task(&conn, both).reason),
        (State::Cancelled, Some(format!("#{open} was cancelled")))
    );
}

#[test]
fn a_failed_or_stopped_task_can_be_cancelled() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (failed, stopped) = (
        idle_task(&mut conn, &[], DAY),
        idle_task(&mut conn, &[], DAY),
    );
    run(&mut conn, failed, tasks::Outcome::Failed("it broke".into()));
    run(
        &mut conn,
        stopped,
        tasks::Outcome::Stopped("stopped".into()),
    );
    for n in [failed, stopped] {
        tasks::cancel(&mut conn, None, n, "not needed", T).unwrap();
        assert_eq!(task(&conn, n).state, State::Cancelled, "#{n}");
    }
}

#[test]
fn archiving_a_chat_cancels_a_task_in_another_chat_blocked_on_one_of_its_tasks() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c2', 'Other', 1);
         INSERT INTO chat_members VALUES ('c2', 'lead', 'supervisor', 1), ('c2', 'ann', 'member', 1);",
    )
    .unwrap();
    let here = idle_task(&mut conn, &[], DAY);
    let g = Give {
        chat: "c2",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Ship it",
        why: None,
        after: &[],
        pr: None,
    };
    let there = tasks::give(&mut conn, &g, T).unwrap().0.number;
    conn.execute(
        "UPDATE tasks SET state = 'blocked', blocked_on = ?1 WHERE number = ?2",
        [here, there],
    )
    .unwrap();
    gathr::archive::archive(&mut conn, "c", T).unwrap();
    let t = task(&conn, there);
    assert_eq!(
        (t.state, t.reason),
        (
            State::Cancelled,
            Some(format!(
                "The task blocking this one, #{here}, was cancelled"
            ))
        )
    );
}

#[test]
fn a_waiting_task_is_never_stale_while_its_after_task_is_open() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let first = idle_task(&mut conn, &[], DAY);
    let second = idle_task(&mut conn, &[first], DAY);
    let swept = sweep(&mut conn, 20 * DAY, &[]);
    assert_eq!(swept.marked, [first]);
    assert_eq!(swept.cancelled, [first]);
    assert_eq!(
        task(&conn, second).state,
        State::Cancelled,
        "then it can't go on"
    );
}

#[test]
fn the_days_are_saved_and_refused_out_of_range() {
    let dir = TempDir::new();
    let conn = team(&dir);
    assert_eq!(
        stale::days(&conn).unwrap(),
        Days {
            stale: 3,
            cancel: 7
        }
    );
    stale::set_days(
        &conn,
        Days {
            stale: 2,
            cancel: 10,
        },
    )
    .unwrap();
    assert_eq!(
        stale::days(&conn).unwrap(),
        Days {
            stale: 2,
            cancel: 10
        }
    );
    for bad in [
        Days {
            stale: 0,
            cancel: 7,
        },
        Days {
            stale: 3,
            cancel: 366,
        },
    ] {
        match stale::set_days(&conn, bad) {
            Err(gathr::Error::Refused(_)) => {}
            other => panic!("expected a refusal, got {other:?}"),
        }
    }
}
