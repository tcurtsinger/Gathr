//! 0059, scenarios 3a and 3f: a work task carries a complete brief, each field within its limit, and
//! its giver can edit it until its first turn starts.

use super::*;
use gathr::tasks::{Brief, Edit};
use serde_json::{Value, json};

const MISSING: &str = "A work task needs why, done, proof and where. Missing: {missing}. `done` says what is true \
                       when it's done, `proof` how that is shown, `where` the folder and branch the work goes in, \
                       or \"no code\".";
const PAIR: &str = "`size` and `lines` go together: `size` names the files or area, `lines` the rough lines it adds.";

fn missing(fields: &str) -> String {
    MISSING.replace("{missing}", fields)
}

/// Lead's task for Ann of `kind`, with `brief`.
fn ask(kind: Kind, brief: Brief<'_>) -> Give<'_> {
    Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind,
        what: "Fix the build",
        brief,
        after: &[],
        pr: None,
        in_passing: false,
    }
}

/// Lead's edit of `task`, changing nothing yet.
fn edit_of(task: i64) -> Edit<'static> {
    Edit {
        chat: "c",
        by: Some("lead"),
        task,
        ..Edit::default()
    }
}

fn count(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

/// The attributes of each `gathr.task.edit` event, oldest first.
fn edits(conn: &Connection) -> Vec<Value> {
    let sql = "SELECT json(attrs) FROM events WHERE name = 'gathr.task.edit' ORDER BY rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    rows.map(|r| serde_json::from_str(&r.unwrap()).unwrap())
        .collect()
}

/// The body of the turn's first Gathr message: the ask it is queued with.
fn ask_of(conn: &Connection, turn: &str) -> String {
    let sql = "SELECT body FROM messages WHERE turn_id = ?1 AND role = 'gathr' ORDER BY ts LIMIT 1";
    conn.query_row(sql, [turn], |r| r.get(0)).unwrap()
}

fn moved<'a>(turn: &'a str, task: i64) -> tasks::Moved<'a> {
    tasks::Moved {
        chat: "c",
        turn,
        agent: Some("ann"),
        task: Some(task),
        trace: T,
        carries_on: None,
    }
}

#[test]
fn a_work_task_without_why_done_proof_or_where_is_refused_naming_each() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let none = refusal(tasks::give(
        &mut conn,
        &ask(Kind::Work, Brief::default()),
        T,
    ));
    assert_eq!(none, missing("`why`, `done`, `proof` and `where`"));
    let some = Brief {
        why: Some("CI is red"),
        done: Some("  "),
        proof: Some("the CI run on the PR"),
        ..Brief::default()
    };
    let two = refusal(tasks::give(&mut conn, &ask(Kind::Work, some), T));
    assert_eq!(two, missing("`done` and `where`"));
    let given = "SELECT count(*) FROM events WHERE name = 'gathr.task.give'";
    assert_eq!(
        (
            count(&conn, "SELECT count(*) FROM tasks"),
            count(&conn, given)
        ),
        (0, 0),
        "a refused task is neither saved nor recorded"
    );

    let (review, _) = tasks::give(&mut conn, &ask(Kind::Review, Brief::default()), T).unwrap();
    assert_eq!(review.state, State::Ready);
    let done = Brief {
        done: Some("the PR is reviewed"),
        ..Brief::default()
    };
    assert_eq!(
        refusal(tasks::give(&mut conn, &ask(Kind::Review, done), T)),
        "A review takes none of done, proof, where or size: Gathr says what its done and proof are."
    );
}

#[test]
fn a_field_over_its_limit_or_a_size_without_lines_is_refused_saying_what_to_do() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (long_done, long_where) = ("d".repeat(301), "w".repeat(201));
    let cases = [
        (
            Brief {
                done: Some(&long_done),
                ..common::COMPLETE
            },
            "`done` is 301 characters: cut 1 to fit 300, and put the details in `why`",
        ),
        (
            Brief {
                place: Some(&long_where),
                ..common::COMPLETE
            },
            "`where` is 201 characters: cut 1 to fit 200, and put the details in `why`",
        ),
        (
            Brief {
                size: Some(("src/build.rs", 0)),
                ..common::COMPLETE
            },
            "`lines` is the rough number of lines it adds: 1 or more.",
        ),
    ];
    for (brief, why) in cases {
        assert_eq!(
            refusal(tasks::give(&mut conn, &ask(Kind::Work, brief), T)),
            why
        );
    }
    assert_eq!(refusal(tasks::sized(Some("src/build.rs"), None)), PAIR);
    assert_eq!(refusal(tasks::sized(None, Some(40))), PAIR);
    assert_eq!(
        tasks::sized(Some("src/build.rs"), Some(40)).unwrap(),
        Some(("src/build.rs", 40))
    );
    assert_eq!(count(&conn, "SELECT count(*) FROM tasks"), 0, "none given");
}

#[test]
fn the_giver_edits_a_waiting_tasks_title_and_after_and_its_brief_and_order_follow() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (first, _) = give(&mut conn, "bo", &[], None);
    let (second, _) = give(&mut conn, "ann", &[first], None);
    let retitled = Edit {
        what: Some("Fix the Windows build"),
        after: Some(&[]),
        ..edit_of(second)
    };
    let (task, starts, _) = tasks::edit(&mut conn, &retitled, T).unwrap();
    assert_eq!(task.state, State::Ready);
    assert_eq!(starts.len(), 1, "{starts:?}");
    let line = format!("Task #{second} from Lead: Fix the Windows build\n");
    assert!(starts[0].text.starts_with(&line), "{}", starts[0].text);
    assert_eq!(task.edited, ["what", "after"]);
    assert!(task.edited_us.is_some(), "{task:?}");
    let recorded = edits(&conn);
    assert_eq!(recorded.len(), 1, "{recorded:?}");
    assert_eq!(
        (
            &recorded[0]["gathr.task.fields"],
            &recorded[0]["gathr.task.editor"],
            &recorded[0]["gathr.task.state"],
            &recorded[0]["gathr.task.was"],
        ),
        (
            &json!("what,after"),
            &json!("lead"),
            &json!("ready"),
            &json!({ "what": "Fix the build", "after": [first] }),
        )
    );

    // Back in line: blocked for a failed task it comes after, ready once that is taken out.
    let (failing, starts) = give(&mut conn, "bo", &[], None);
    let (blocked, _) = give(&mut conn, "ann", &[failing], None);
    let turn = starts[0].turn.clone();
    let failed = Some("no worktree");
    tasks::turn_moved(&mut conn, &moved(&turn, failing), "failed", failed).unwrap();
    let because = format!("#{failing} is failed");
    assert_eq!(
        state(&conn, blocked),
        (State::Blocked, Some(because.clone()))
    );
    let freed = Edit {
        after: Some(&[]),
        ..edit_of(blocked)
    };
    let (task, starts, _) = tasks::edit(&mut conn, &freed, T).unwrap();
    assert_eq!((task.state, starts.len()), (State::Ready, 1), "{starts:?}");
    let (ready, _) = give(&mut conn, "ann", &[], None);
    let after = [failing];
    let held = Edit {
        after: Some(&after),
        ..edit_of(ready)
    };
    let (task, starts, _) = tasks::edit(&mut conn, &held, T).unwrap();
    assert_eq!((task.state, task.reason), (State::Blocked, Some(because)));
    assert!(starts.is_empty(), "{starts:?}");
}

#[test]
fn an_edit_of_a_ready_task_rewrites_its_queued_brief_keeps_its_carry_and_one_that_now_waits_drops_it()
 {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, starts) = give(&mut conn, "ann", &[], None);
    let turn = starts[0].turn.clone();
    // Queued before 0059: its words were version 1.
    conn.execute("UPDATE turns SET opened_version = 1 WHERE id = ?1", [&turn])
        .unwrap();
    let turns = count(&conn, "SELECT count(*) FROM turns");
    let redone = Edit {
        done: Some("CI on main and the PR are green"),
        ..edit_of(task)
    };
    let (_, starts, message) = tasks::edit(&mut conn, &redone, T).unwrap();
    assert!(starts.is_empty(), "no new turn: {starts:?}");
    assert_eq!(count(&conn, "SELECT count(*) FROM turns"), turns);
    let brief = ask_of(&conn, &turn);
    assert!(
        brief.starts_with("Task #1 from Lead: Fix the build\nWhy: CI is red\n")
            && brief.contains("\nDone when: CI on main and the PR are green\n"),
        "{brief}"
    );
    assert_eq!(message.map(|m| m["body"].clone()), Some(json!(brief)));
    assert_eq!(
        samples::opened(&conn, &turn),
        (Some("brief".into()), Some(2))
    );

    let (other, _) = give(&mut conn, "bo", &[], None);
    let after = [other];
    let waits = Edit {
        after: Some(&after),
        ..edit_of(task)
    };
    let (edited, _, _) = tasks::edit(&mut conn, &waits, T).unwrap();
    assert_eq!(edited.state, State::Waiting);
    assert_eq!(
        tasks::stop_why(&conn, &turn).unwrap(),
        Some(format!("its task #{task} was edited to wait on #{other}"))
    );

    // A turn that failed before it ran: the restart's brief carries why, and an edit keeps it.
    let (carried, starts) = give(&mut conn, "ann", &[], None);
    let failed = Some("no worktree");
    let first = starts[0].turn.clone();
    tasks::turn_moved(&mut conn, &moved(&first, carried), "failed", failed).unwrap();
    let starts = tasks::restart(&mut conn, Some("lead"), carried, T).unwrap();
    let queued = starts[0].turn.clone();
    let proved = Edit {
        proof: Some("the build log"),
        ..edit_of(carried)
    };
    tasks::edit(&mut conn, &proved, T).unwrap();
    let brief = ask_of(&conn, &queued);
    assert!(
        brief.contains("\nProof: the build log\n")
            && brief.contains("\nContinue from here: no worktree. Check what is already done"),
        "{brief}"
    );

    // The runner claims it: its ask is read again, and the task has started.
    assert_eq!(tasks::claim_ask(&mut conn, &queued).unwrap(), Some(brief));
    let started = tasks::get(&conn, carried).unwrap().unwrap().started_us;
    assert!(started.is_some(), "claimed, so started");
    assert_eq!(tasks::claim_ask(&mut conn, &queued).unwrap(), None);
    let again = tasks::get(&conn, carried).unwrap().unwrap().started_us;
    assert_eq!(again, started, "a second claim writes nothing");
    let late = Edit {
        what: Some("Fix the build now"),
        ..edit_of(carried)
    };
    assert_eq!(
        refusal(tasks::edit(&mut conn, &late, T)),
        format!(
            "Task #{carried} has started, so it can't be edited: message Ann with the change, or cancel it and give a new task."
        )
    );

    // A turn that ran and was put back in the queue keeps what the runner holds.
    let (ran, starts) = give(&mut conn, "ann", &[], None);
    let turn = starts[0].turn.clone();
    tasks::turn_moved(&mut conn, &moved(&turn, ran), "running", None).unwrap();
    gathr::store::add_message(
        &conn,
        "c",
        &turn,
        "gathr",
        "text",
        "Bo: the log is in the PR",
    )
    .unwrap();
    gathr::store::set_turn(&conn, &turn, "queued", None).unwrap();
    let started = tasks::get(&conn, ran).unwrap().unwrap().started_us;
    assert_eq!(tasks::claim_ask(&mut conn, &turn).unwrap(), None);
    let after = tasks::get(&conn, ran).unwrap().unwrap().started_us;
    assert_eq!((started.is_some(), after), (true, started));
}

#[test]
fn a_turn_dropped_for_its_edit_leaves_its_task_as_the_edit_left_it_and_tells_nobody() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-1", "lead", None, "done");
    let (task, starts) = give(&mut conn, "ann", &[], Some("lead-1"));
    let marked = starts[0].turn.clone();
    let (other, _) = give(&mut conn, "bo", &[], None);
    let after = [other];
    let waits = Edit {
        after: Some(&after),
        ..edit_of(task)
    };
    tasks::edit(&mut conn, &waits, T).unwrap();
    let freed = Edit {
        after: Some(&[]),
        ..edit_of(task)
    };
    let (_, starts, _) = tasks::edit(&mut conn, &freed, T).unwrap();
    let new = starts[0].turn.clone();

    // The runner drops the marked turn at its gate, as `dropped` does.
    let why = tasks::stop_why(&conn, &marked).unwrap();
    assert!(why.is_some(), "the turn the first edit left is marked");
    let starts = tasks::turn_moved(&mut conn, &moved(&marked, task), "stopped", why.as_deref());
    let starts = starts.unwrap();
    assert!(
        starts.iter().all(|s| !s.hear_back),
        "nobody hears back: {starts:?}"
    );
    assert_eq!(state(&conn, task), (State::Ready, None));
    let stopped = "SELECT count(*) FROM events WHERE name = 'gathr.task.state'
                   AND json_extract(json(attrs), '$.\"gathr.task.state\"') = 'stopped'";
    assert_eq!(count(&conn, stopped), 0, "the task was never stopped");
    assert_eq!(
        tasks::stop_why(&conn, &new).unwrap(),
        None,
        "its new turn may run"
    );
    tasks::turn_moved(&mut conn, &moved(&new, task), "running", None).unwrap();
    assert_eq!(state(&conn, task).0, State::Working);
}

#[test]
fn an_edit_is_refused_with_its_reason_and_changes_nothing() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let (ran, starts) = give(&mut conn, "ann", &[], None);
    let turn = starts[0].turn.clone();
    tasks::turn_moved(&mut conn, &moved(&turn, ran), "running", None).unwrap();
    let asked = tasks::person::Ask {
        chat: "c",
        giver: "lead",
        given_in: None,
        parent: None,
        what: Some("Which port?"),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    let question = tasks::person::ask(&mut conn, asked, T).unwrap().0.number;
    let (cancelled, _) = give(&mut conn, "ann", &[], None);
    tasks::cancel(&mut conn, Some("lead"), cancelled, "not needed", T).unwrap();
    let (later, _) = give(&mut conn, "bo", &[task], None);
    let elsewhere = Give {
        chat: "other",
        ..ask(Kind::Work, common::COMPLETE)
    };
    let elsewhere = tasks::give(&mut conn, &elsewhere, T).unwrap().0.number;

    let (itself, looped, outside) = ([task], [later], [elsewhere]);
    let cases: Vec<(Edit, String)> = vec![
        (
            Edit {
                what: Some("Fix it"),
                ..edit_of(ran)
            },
            format!(
                "Task #{ran} has started, so it can't be edited: message Ann with the change, or cancel it and give a new task."
            ),
        ),
        (
            Edit {
                by: Some("ann"),
                what: Some("Fix it"),
                ..edit_of(task)
            },
            format!("Only task #{task}'s giver may edit it, and that's Lead."),
        ),
        (
            Edit {
                what: Some("Which port now?"),
                ..edit_of(question)
            },
            format!(
                "Task #{question} is a question for the person: edit_task changes work and review tasks."
            ),
        ),
        (
            Edit {
                what: Some("Fix it"),
                ..edit_of(cancelled)
            },
            format!(
                "Task #{cancelled} is cancelled: only a waiting, ready or blocked task can be edited."
            ),
        ),
        (
            Edit {
                what: Some("Fix it"),
                ..edit_of(elsewhere)
            },
            format!("there's no task #{elsewhere} in this chat"),
        ),
        (
            Edit {
                after: Some(&itself),
                ..edit_of(task)
            },
            format!("Task #{task} can't wait on itself."),
        ),
        (
            Edit {
                after: Some(&looped),
                ..edit_of(task)
            },
            format!("#{later} waits on #{task} already, so #{task} can't wait on it."),
        ),
        (
            Edit {
                after: Some(&outside),
                ..edit_of(task)
            },
            format!("there's no task #{elsewhere} in this chat"),
        ),
        (
            Edit {
                lines: Some(40),
                ..edit_of(task)
            },
            PAIR.to_string(),
        ),
        (
            Edit {
                what: Some("Fix the build"),
                ..edit_of(task)
            },
            format!("Nothing to change: task #{task} already reads so."),
        ),
        (
            Edit {
                done: Some(""),
                ..edit_of(task)
            },
            missing("`done`"),
        ),
    ];
    let as_is = |conn: &Connection| {
        let tasks: Vec<Value> = [task, ran, question, cancelled, later]
            .iter()
            .map(|&n| serde_json::to_value(tasks::get(conn, n).unwrap()).unwrap())
            .collect();
        let events = count(conn, "SELECT count(*) FROM events");
        let turns = count(conn, "SELECT count(*) FROM turns");
        (tasks, events, turns)
    };
    for (edit, why) in cases {
        let before = as_is(&conn);
        assert_eq!(refusal(tasks::edit(&mut conn, &edit, T)), why);
        assert_eq!(as_is(&conn), before, "{why}");
    }
}
