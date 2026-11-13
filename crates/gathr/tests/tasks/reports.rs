//! A report is a record, and Gathr works out the rest (0047, docs/scenarios.md 5a to 5d): a turn
//! that ends is never a task that is done, a truthful report is taken in every state, and "left"
//! never becomes "done".

use super::*;

fn note(text: &str) -> Said<'_> {
    Said {
        note: Some(text),
        ..Said::default()
    }
}

/// The lead's turn that gave Ann a task, ended: the lead hears back when the task settles.
fn given_by_the_lead(conn: &mut Connection) -> i64 {
    turn(conn, "lead-turn", "lead", None, "done");
    give(conn, "ann", &[], Some("lead-turn")).0
}

/// 5b; G95, G97, G167: a turn that ends with no word leaves the task open. Gathr asks once.
#[test]
fn a_turn_that_ends_with_no_report_asks_its_owner_once_then_tells_the_giver() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = given_by_the_lead(&mut conn);
    let starts = work(&mut conn, task, None);
    assert_eq!(state(&conn, task).0, State::Ready, "asked, not done");
    assert!(to_lead(&starts).is_empty(), "nobody is told: {starts:?}");
    let ask: Vec<_> = starts.iter().filter(|s| s.agent == "ann").collect();
    assert_eq!(ask.len(), 1, "{starts:?}");
    assert_eq!(ask[0].task, Some(task));
    let wanted = format!("Your last turn on task #{task} (Fix the build) ended with no report.");
    assert!(ask[0].text.starts_with(&wanted), "{}", ask[0].text);
    assert!(tasks::get(&conn, task).unwrap().unwrap().asked);

    let starts = work(&mut conn, task, None);
    let silent = Some("no report: two turns ended without one".to_string());
    assert_eq!(state(&conn, task), (State::Blocked, silent));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let line = format!("#{task} Ann, Fix the build: no report: two turns ended without one");
    assert!(back[0].text.contains(&line), "{}", back[0].text);
}

/// The ask is answered: the report settles the task, and the count starts again.
#[test]
fn a_report_after_the_ask_settles_the_task() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    work(&mut conn, task, None);
    work(&mut conn, task, Some(Finish::Done));
    let t = tasks::get(&conn, task).unwrap().unwrap();
    assert_eq!((t.state, t.asked), (State::Done, false));
    assert_eq!(t.report.unwrap().how, Finish::Done);
}

/// 5c; G47, G117, G124, G133, G178, G184: the owner's report is taken whatever state the task is in.
#[test]
fn a_report_is_taken_in_every_state_a_task_can_be_in() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    // Ready, its turn not started: done settles it, and its queued turn never starts (G124).
    let (ready, _) = give(&mut conn, "ann", &[], None);
    let (_, took, _) =
        tasks::finish(&mut conn, "ann", ready, Finish::Done, &note("PR #9"), T).unwrap();
    assert_eq!(took, tasks::Took::Applied);
    assert_eq!(
        state(&conn, ready),
        (State::Done, Some("PR #9".to_string()))
    );
    assert!(tasks::may_start(&conn, ready).unwrap().is_some());

    // Waiting behind another task (G133).
    let (first, _) = give(&mut conn, "bo", &[], None);
    let (waiting, _) = give(&mut conn, "ann", &[first], None);
    finish(
        &mut conn,
        "ann",
        waiting,
        Finish::Done,
        &note("done with #9"),
    )
    .unwrap();
    assert_eq!(state(&conn, waiting).0, State::Done);

    // Left on a task still in line: the note is kept and it keeps its place.
    let (queued, _) = give(&mut conn, "ann", &[], None);
    finish(
        &mut conn,
        "ann",
        queued,
        Finish::Left,
        &note("the tests remain"),
    )
    .unwrap();
    let t = tasks::get(&conn, queued).unwrap().unwrap();
    assert_eq!(t.state, State::Ready);
    assert_eq!(t.report.unwrap().note.as_deref(), Some("the tests remain"));

    // Blocked, failed and stopped: applied at once (G178).
    for end in ["blocked", "failed", "stopped"] {
        let (task, _) = give(&mut conn, "ann", &[], None);
        match end {
            "blocked" => drop(work(&mut conn, task, Some(Finish::Blocked))),
            "failed" => {
                drop(tasks::ended(&mut conn, task, Outcome::Failed("no engine".into()), T).unwrap())
            }
            _ => {
                tasks::started(&mut conn, task, T).unwrap();
                tasks::ended(&mut conn, task, Outcome::Stopped("Stop now".into()), T).unwrap();
            }
        }
        assert_eq!(state(&conn, task).0.as_str(), end);
        finish(&mut conn, "ann", task, Finish::Done, &note("fixed in #9")).unwrap();
        assert_eq!(
            state(&conn, task).0,
            State::Done,
            "a {end} task reported done"
        );
    }

    // Done already: answered with what is recorded, never an error (G184).
    let (_, took, starts) =
        tasks::finish(&mut conn, "ann", ready, Finish::Done, &note("again"), T).unwrap();
    assert_eq!(took, tasks::Took::Already);
    assert!(starts.is_empty());
    assert_eq!(state(&conn, ready).1.as_deref(), Some("PR #9"), "unchanged");

    // Cancelled: answered, and nothing changes.
    let (cancelled, _) = give(&mut conn, "ann", &[], None);
    tasks::cancel(&mut conn, None, cancelled, "dropped", T).unwrap();
    let (_, took, _) =
        tasks::finish(&mut conn, "ann", cancelled, Finish::Done, &note("late"), T).unwrap();
    assert_eq!(took, tasks::Took::Cancelled);
    assert_eq!(state(&conn, cancelled).0, State::Cancelled);
}

/// 5a: "done" says what to look at, "left" what is left, "blocked" on what.
#[test]
fn a_report_that_says_too_little_is_turned_away_saying_what_is_missing() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let bare = Said::default();
    assert_eq!(
        refusal(finish(&mut conn, "ann", task, Finish::Done, &bare)),
        "Say what to look at when work is done: a PR, a commit, a file or a note"
    );
    assert_eq!(
        refusal(finish(&mut conn, "ann", task, Finish::Left, &bare)),
        "Say what is left, in `note`"
    );
    assert_eq!(
        refusal(finish(&mut conn, "ann", task, Finish::Blocked, &bare)),
        "Say what blocks it, in `note`, or the task it waits on, in `on_task`"
    );
    assert_eq!(state(&conn, task).0, State::Ready, "nothing changed");
    // A file is something to look at; so is the task's own PR.
    let file = Said {
        file: Some("docs/plan.md"),
        ..Said::default()
    };
    let done = finish(&mut conn, "ann", task, Finish::Done, &file).unwrap();
    assert_eq!(done.report.unwrap().see.as_deref(), Some("docs/plan.md"));
    let with_pr = fix_with_pr(&mut conn);
    finish(&mut conn, "ann", with_pr, Finish::Done, &bare).unwrap();
}

/// S65, G188: "left" with nothing open stops there and is told. It never starts its owner again.
#[test]
fn left_with_nothing_open_stops_and_tells_the_giver() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = given_by_the_lead(&mut conn);
    let t1 = begin(&mut conn, task);
    let said = note("the drivers and the PR remain");
    let (_, took, _) = tasks::finish(&mut conn, "ann", task, Finish::Left, &said, T).unwrap();
    assert_eq!(took, tasks::Took::Kept, "its turn is running");
    assert_eq!(state(&conn, task).0, State::Working);
    gathr::store::set_turn(&conn, &t1, "done", None).unwrap();
    let starts = tasks::ended(&mut conn, task, Outcome::Ok, T).unwrap();
    let left = Some("left: the drivers and the PR remain".to_string());
    assert_eq!(state(&conn, task), (State::Blocked, left));
    assert!(starts.iter().all(|s| s.agent != "ann"), "{starts:?}");
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let line = format!("#{task} Ann, Fix the build: left: the drivers and the PR remain");
    assert!(back[0].text.contains(&line), "{}", back[0].text);

    // Restarted by its giver, it carries on with no report standing.
    tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    let t = tasks::get(&conn, task).unwrap().unwrap();
    assert_eq!(t.state, State::Ready);
    assert!(t.report.is_none(), "{:?}", t.report);
}

/// A "done" stands through a later turn on the task (a wait that fires, a hear-back). A "left"
/// doesn't: its owner is at work again, and that turn must end with its own report.
#[test]
fn done_stands_through_a_later_turn_and_left_does_not() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = fix_with_pr(&mut conn);
    work(&mut conn, fix, Some(Finish::Done));
    let held = Some("done, waiting on #69 to merge".to_string());
    assert_eq!(state(&conn, fix), (State::Held, held.clone()));
    work(&mut conn, fix, None);
    assert_eq!(
        state(&conn, fix),
        (State::Held, held),
        "still reported done"
    );

    let left = fix_with_pr(&mut conn);
    work(&mut conn, left, Some(Finish::Left));
    let held = Some("left, waiting on #69 to merge".to_string());
    assert_eq!(state(&conn, left), (State::Held, held));
    work(&mut conn, left, None);
    let unsaid = Some("waiting on #69 to merge".to_string());
    assert_eq!(state(&conn, left), (State::Held, unsaid));
    assert!(tasks::get(&conn, left).unwrap().unwrap().report.is_none());
}

/// G188, live #148: a PR that merges doesn't make a "left" task done. Its owner is woken with what
/// it left, and the merged PR no longer holds the task.
#[test]
fn a_pr_that_merges_does_not_make_a_left_task_done() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = fix_with_pr(&mut conn);
    let t1 = begin(&mut conn, fix);
    finish(
        &mut conn,
        "ann",
        fix,
        Finish::Left,
        &note("the second driver"),
    )
    .unwrap();
    gathr::store::set_turn(&conn, &t1, "done", None).unwrap();
    tasks::ended(&mut conn, fix, Outcome::Ok, T).unwrap();
    let starts = tasks::pr_looked(&mut conn, fix, "MERGED", None, None, T).unwrap();
    assert_eq!(state(&conn, fix).0, State::Ready, "not done");
    let ann: Vec<_> = starts.iter().filter(|s| s.agent == "ann").collect();
    assert_eq!(ann.len(), 1, "{starts:?}");
    let carry = "#69 merged, and you left this work: the second driver";
    assert!(ann[0].text.contains(carry), "{}", ann[0].text);
    work(&mut conn, fix, Some(Finish::Done));
    assert_eq!(
        state(&conn, fix).0,
        State::Done,
        "the merged PR holds nothing"
    );
}

/// Work nobody reported done isn't done because its PR merged: its owner is woken and asked, or,
/// while its turn still runs, that turn's end settles the task by what it says.
#[test]
fn a_pr_that_merges_finishes_work_only_if_its_owner_reported_it_done() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let silent = fix_with_pr(&mut conn);
    work(&mut conn, silent, None);
    let held = Some("waiting on #69 to merge".to_string());
    assert_eq!(state(&conn, silent), (State::Held, held));
    let starts = tasks::pr_looked(&mut conn, silent, "MERGED", None, None, T).unwrap();
    assert_eq!(state(&conn, silent).0, State::Ready, "asked, not done");
    let carry = "#69 merged, and this task has no report that it's done";
    assert!(starts[0].text.contains(carry), "{}", starts[0].text);

    // Not started yet, its first turn still queued: the merge queues no second one.
    let queued = fix_with_pr(&mut conn);
    let starts = tasks::pr_looked(&mut conn, queued, "MERGED", None, None, T).unwrap();
    assert!(starts.is_empty(), "{starts:?}");
    assert_eq!(state(&conn, queued).0, State::Ready);

    // Reported done: the merge finishes it.
    let reported = fix_with_pr(&mut conn);
    work(&mut conn, reported, Some(Finish::Done));
    tasks::pr_looked(&mut conn, reported, "MERGED", None, None, T).unwrap();
    let merged = Some("#69 merged".to_string());
    assert_eq!(state(&conn, reported), (State::Done, merged));

    let at_work = fix_with_pr(&mut conn);
    let t1 = begin(&mut conn, at_work);
    let starts = tasks::pr_looked(&mut conn, at_work, "MERGED", None, None, T).unwrap();
    assert!(starts.is_empty(), "no second turn is queued: {starts:?}");
    assert_eq!(
        state(&conn, at_work).0,
        State::Working,
        "its turn still runs"
    );
    finish(
        &mut conn,
        "ann",
        at_work,
        Finish::Left,
        &note("the second half"),
    )
    .unwrap();
    gathr::store::set_turn(&conn, &t1, "done", None).unwrap();
    tasks::ended(&mut conn, at_work, Outcome::Ok, T).unwrap();
    let left = Some("left: the second half".to_string());
    assert_eq!(state(&conn, at_work), (State::Blocked, left));
}

/// A hold that ends with no "done" reported never marks the task done: its owner is woken, and that
/// is the one ask.
#[test]
fn a_hold_that_ends_with_no_report_wakes_its_owner() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let t1 = begin(&mut conn, task);
    let running = [("b1".to_string(), "cargo test".to_string())];
    gathr::background::set(&conn, ("c", Some("ann")), &running, Some(&t1)).unwrap();
    gathr::store::set_turn(&conn, &t1, "done", None).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok, T).unwrap();
    let held = Some("waiting on a background command (cargo test)".to_string());
    assert_eq!(state(&conn, task), (State::Held, held));

    turn(&conn, "t2", "ann", None, "running");
    let (_, starts) =
        tasks::background_changed(&mut conn, ("c", Some("ann")), &[], Some("t2"), true, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Ready, "woken, not done");
    assert_eq!(starts.len(), 1, "{starts:?}");
    assert!(
        starts[0]
            .text
            .contains("what this task waited on has ended"),
        "{}",
        starts[0].text
    );
    work(&mut conn, task, None);
    assert_eq!(
        state(&conn, task).0,
        State::Blocked,
        "the wake was the one ask"
    );
}

/// A task set after one that was left starts when that one is restarted and finishes. Today it
/// stays blocked for good, as after a failure or a stop.
#[test]
fn a_task_set_after_one_that_stopped_starts_when_that_one_finishes() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (first, _) = give(&mut conn, "ann", &[], None);
    let (second, _) = give(&mut conn, "bo", &[first], None);
    let (third, _) = give(&mut conn, "bo", &[second], None);
    work(&mut conn, first, Some(Finish::Left));
    assert_eq!(
        state(&conn, second),
        (State::Blocked, Some(format!("#{first} is blocked")))
    );
    assert_eq!(state(&conn, third).0, State::Blocked, "down the chain");
    tasks::restart(&mut conn, Some("lead"), first, T).unwrap();
    let starts = work(&mut conn, first, Some(Finish::Done));
    assert_eq!(state(&conn, second).0, State::Ready);
    let bo: Vec<_> = starts.iter().filter(|s| s.agent == "bo").collect();
    assert_eq!(bo.len(), 1, "{starts:?}");
    assert_eq!(bo[0].task, Some(second));
    work(&mut conn, second, Some(Finish::Done));
    assert_eq!(state(&conn, third).0, State::Ready, "and the one after it");
}
