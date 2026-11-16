//! I4: a change of size holds or releases an ask's tasks that haven't started, and the person's
//! change is told; a task waiting for its plan wakes nobody and is cancelled only with no plan.

use super::*;
use gathr::quiet::{self, Times};
use gathr::tasks::stale;
use gathr::waiting::{self, Live, Moves};

const S: i64 = 1_000_000;
const DAY: i64 = 86_400 * S;
const WAITS: &str = "waits for its plan";

/// A turn opened by a resize: its id, what it answers, its ask, its version and its text.
type Told = (String, Option<String>, Option<String>, Option<i64>, String);

/// The turns opened by a resize, oldest first.
fn told(conn: &Connection) -> Vec<Told> {
    let sql = "SELECT u.id, u.answers, u.ask_id, u.opened_version, m.body FROM turns u
               JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
               WHERE u.opened_by = 'resized' ORDER BY u.rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    });
    rows.unwrap().map(Result::unwrap).collect()
}

fn stop_why(conn: &Connection, turn: &str) -> Option<String> {
    let sql = "SELECT stop_why FROM turns WHERE id = ?1";
    conn.query_row(sql, [turn], |r| r.get(0)).unwrap()
}

/// A task Mira gives `owner` from `turn`, after `after`: its number.
fn after(conn: &mut Connection, owner: &str, turn: &str, after: &[i64]) -> i64 {
    let g = Give {
        chat: "c",
        giver: Some("mira"),
        owner,
        given_in: Some(turn),
        parent: None,
        kind: Kind::Work,
        what: "Show tags in the list",
        brief: common::COMPLETE,
        after,
        pr: None,
        in_passing: false,
        look_into: false,
    };
    tasks::give(conn, &g, T).unwrap().0.number
}

/// I4.2 to I4.5: the person's change to new holds what hasn't started and tells the lead; back to
/// repair releases it and tells the agent the ask went to. The lead's own change and a split hold
/// too, and tell nobody.
#[test]
fn a_change_to_new_holds_tasks_not_started_and_the_lead_is_told() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    asks::resize(&mut conn, &m1, "repair", T).unwrap();
    let one = work(&mut conn, ("mira", "ann"), "T", false);
    let brief = queued_on(&conn, one).unwrap();
    let two = after(&mut conn, "bo", "T", &[one]);
    let three = work(&mut conn, ("mira", "ann"), "T", false);
    let running = begin(&mut conn, three);
    let four = work(&mut conn, ("mira", "bo"), "T", true);
    assert!(
        told(&conn).is_empty(),
        "unsized to repair, nothing held: nobody told"
    );

    let (_, starts) = asks::resize(&mut conn, &m1, "new", T).unwrap();
    assert_eq!(state(&conn, one), (State::Waiting, Some(WAITS.into())));
    assert_eq!(
        stop_why(&conn, &brief).as_deref(),
        Some("its task #1 waits for its plan")
    );
    // Its marked turn is dropped when it comes up, never run: #1 waits for its plan from now.
    let live = Live {
        running: true,
        queued: [(brief.clone(), "behind this agent's turn on #3".into())].into(),
        ..Live::default()
    };
    let said = waiting::answer(&conn, "c", &live, gathr::ids::now_us()).unwrap();
    let on_one: Vec<_> = said.lines.iter().filter(|l| l.task == Some(one)).collect();
    let on_one: Vec<_> = on_one.iter().map(|l| (l.kind, l.what.as_str())).collect();
    assert_eq!(on_one, [("plan_wait", "#1 (Ann) waits for its plan")]);
    assert_eq!(state(&conn, two), (State::Waiting, None), "#2 waits on #1");
    assert_eq!(state(&conn, three).0, State::Working, "started: runs on");
    assert_eq!(state(&conn, four).0, State::Ready, "a look-into task");
    assert!(!held(&conn, &running), "a building task's turn");
    let place = hold::Place {
        worktree: std::path::Path::new("/agents/ann/notes"),
        own: None,
        shared: None,
    };
    assert!(hold::write_held(&conn, &running, &place).unwrap().is_none());
    assert!(hold::push_held(&conn, &running).unwrap().is_none());
    assert!(events(&conn, "refused").is_empty());
    let mira = told(&conn);
    assert_eq!(mira.len(), 1, "{mira:?}");
    let (turn, answers, ask, version, text) = &mira[0];
    assert_eq!(
        (answers.as_deref(), ask.as_deref(), *version),
        (Some("T"), Some(m1.as_str()), Some(1))
    );
    samples::same("resized: new with tasks", text);
    assert!(
        starts.iter().any(|s| &s.turn == turn && s.agent == "mira"),
        "{starts:?}"
    );
    let wait = events(&conn, "wait");
    assert_eq!(wait.len(), 1, "{wait:?}");
    assert_eq!(
        (&wait[0]["gathr.hold.how"], &wait[0]["gathr.task.count"]),
        (&serde_json::json!("held"), &serde_json::json!(1))
    );
    moved(&mut conn, (&brief, "ann"), Some(one), "stopped");
    assert_eq!(
        state(&conn, one),
        (State::Waiting, Some(WAITS.into())),
        "its marked turn moves nothing"
    );

    let (_, starts) = asks::resize(&mut conn, &m1, "repair", T).unwrap();
    assert_eq!(state(&conn, one), (State::Ready, None));
    let again = queued_on(&conn, one).unwrap();
    assert_ne!(again, brief, "a new brief");
    assert!(starts.iter().any(|s| s.turn == again));
    let mira = told(&conn);
    assert_eq!(mira.len(), 2, "{mira:?}");
    samples::same("resized: repair with tasks", &mira[1].4);
    assert_eq!(events(&conn, "wait")[1]["gathr.hold.how"], "released");

    // A repair whose work is all done, made new: nobody told.
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the label");
    lead_sizes(&mut conn, "T2", "repair");
    let done = work(&mut conn, ("mira", "ann"), "T2", false);
    conn.execute_batch(&format!(
        "UPDATE turns SET status = 'done' WHERE task = {done} OR id = 'T2';
         UPDATE tasks SET state = 'done' WHERE number = {done};"
    ))
    .unwrap();
    let (_, starts) = asks::resize(&mut conn, &m2, "new", T).unwrap();
    assert_eq!(
        (starts.len(), told(&conn).len()),
        (0, 2),
        "a correction tells nobody"
    );

    // A repair with work running and nothing to hold, made new.
    let m3 = sent(&mut conn, ("c", "mira"), "T3", "Fix the export");
    asks::resize(&mut conn, &m3, "repair", T).unwrap();
    asks::resize(&mut conn, &m3, "new", T).unwrap();
    samples::same("resized: new", &told(&conn)[2].4);

    // An unsized ask Gathr held, made a repair with nothing waiting.
    let m4 = sent(&mut conn, ("c", "mira"), "T4", "Fix the typo");
    refusal(give(
        &mut conn,
        ("mira", "ann"),
        Some("T4"),
        (Kind::Work, false, None),
    ));
    asks::resize(&mut conn, &m4, "repair", T).unwrap();
    samples::same("resized: repair", &told(&conn)[3].4);

    // The lead's own change to new holds, and tells nobody.
    sent(&mut conn, ("c", "mira"), "T5", "Rename the menu");
    lead_sizes(&mut conn, "T5", "repair");
    let five = work(&mut conn, ("mira", "ann"), "T5", false);
    let starts = lead_sizes(&mut conn, "T5", "new");
    assert_eq!(state(&conn, five), (State::Waiting, Some(WAITS.into())));
    assert_eq!((starts.len(), told(&conn).len()), (0, 4));

    // A message steered into a turn, split off with the task given after it.
    sent(&mut conn, ("c", "mira"), "T6", "Fix the search");
    lead_sizes(&mut conn, "T6", "repair");
    let steered =
        gathr::store::add_message(&conn, "c", "T6", "user", "text", "Add colours").unwrap();
    let steered = steered["id"].as_str().unwrap().to_string();
    let six = work(&mut conn, ("mira", "ann"), "T6", false);
    let s = Sizing {
        chat: "c",
        agent: "mira",
        turn: Some("T6"),
        size: "new",
        message: Some(&steered),
        trace: T,
    };
    asks::size(&mut conn, &s).unwrap();
    assert_eq!(
        state(&conn, six),
        (State::Waiting, Some(WAITS.into())),
        "split off and new"
    );
    asks::resize(&mut conn, &steered, "repair", T).unwrap();
    let last = told(&conn).pop().unwrap();
    let answers = (last.1.as_deref(), last.2.as_deref());
    assert_eq!(
        answers,
        (Some("T6"), Some(steered.as_str())),
        "answers T6, serves the split-off ask"
    );
}

/// I4.1a: a task waiting for its plan follows the plan: no wake for the lead, the person told once
/// the chat is stuck, and cancelled as stale only when its ask has no plan open.
#[test]
fn a_task_waiting_for_its_plan_wakes_nobody_and_is_cancelled_only_with_no_plan() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    asks::resize(&mut conn, &m1, "repair", T).unwrap();
    let one = work(&mut conn, ("mira", "ann"), "T", false);
    asks::resize(&mut conn, &m1, "new", T).unwrap();
    conn.execute(
        "UPDATE turns SET status = 'stopped' WHERE status IN ('queued', 'running')",
        [],
    )
    .unwrap();
    assert_eq!(state(&conn, one), (State::Waiting, Some(WAITS.into())));

    let live = Live {
        running: true,
        ..Live::default()
    };
    let now = gathr::ids::now_us();
    let answer = waiting::answer(&conn, "c", &live, now).unwrap();
    let lines: Vec<_> = answer
        .lines
        .iter()
        .map(|l| (l.kind, l.what.as_str(), l.moves, l.on.as_deref()))
        .collect();
    assert_eq!(
        lines,
        [(
            "plan_wait",
            "#1 (Ann) waits for its plan",
            Moves::Others,
            Some("Mira")
        )]
    );

    let times = Times {
        tell_after_us: 300 * S,
        look_us: 120 * S,
    };
    for at in [0, 120, 240, 360] {
        let starts = quiet::check(&mut conn, "c", &live, now + at * S, times, T).unwrap();
        assert!(starts.is_empty(), "no wake at +{at}s: {starts:?}");
    }
    let woke: i64 = conn
        .query_row(
            "SELECT count(*) FROM events WHERE name = 'gathr.quiet.woke'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(woke, 0, "the lead isn't woken");
    let said: Vec<String> = {
        let sql =
            "SELECT body FROM messages WHERE chat_id = 'c' AND turn_id IS NULL AND role = 'gathr'";
        let mut stmt = conn.prepare(sql).unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    assert!(
        said.len() == 1 && said[0].contains("#1 (Ann) waits for its plan"),
        "the person is told: {said:?}"
    );

    let swept = stale::sweep(&mut conn, now + 3 * DAY + S, DAY, &[], T).unwrap();
    assert_eq!(swept.marked, [one], "marked stale");
    assert!(
        swept.starts.iter().any(|s| s.agent == "mira"),
        "Mira is told"
    );
    let wrote = plan(&mut conn, "T", "Add the tags table");
    agreed(&mut conn, &wrote);
    let swept = stale::sweep(&mut conn, now + 10 * DAY + S, DAY, &[], T).unwrap();
    assert!(
        !swept.cancelled.contains(&one),
        "its plan waits on the person"
    );
    plans::drop(&mut conn, wrote.plan, 1, T).unwrap();
    let swept = stale::sweep(&mut conn, now + 10 * DAY + S, DAY, &[], T).unwrap();
    assert!(swept.cancelled.contains(&one), "no plan: cancelled");
    let why = state(&conn, one).1.unwrap_or_default();
    assert!(why.starts_with("stale since "), "{why}");
}
