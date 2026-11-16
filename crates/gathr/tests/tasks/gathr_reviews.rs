//! 0060, scenario 3g: a work task's done report that first names its PR gives the chat's Reviewer
//! that PR's review, as the builder would give it, once per PR; a second review of a PR with an
//! open one is refused, naming it.
use super::*;
use gathr::tasks::{NoteKind, NoteSaid, Took};
use gathr::waiting::{self, Live, Moves};
use gathr::words;

/// Bo is the chat's Reviewer.
fn reviewing(conn: &Connection) {
    conn.execute(
        "UPDATE chat_members SET role = 'reviewer' WHERE chat_id = 'c' AND agent_id = 'bo'",
        [],
    )
    .unwrap();
}

/// A report on the task from `turn`, by its owner, naming `pr`: the review Gathr gave and what
/// the report starts.
fn said_in(
    conn: &mut Connection,
    task: i64,
    how: Finish,
    pr: Option<i64>,
    turn: &str,
) -> (Option<(i64, String)>, Vec<Start>) {
    let owner = tasks::get(conn, task).unwrap().unwrap().owner.unwrap();
    let said = Said {
        note: Some("see the change"),
        pr,
        turn: Some(turn),
        ..Said::default()
    };
    let (_, _, starts, given) = tasks::finish(conn, &owner, task, how, &said, T).unwrap();
    (given, starts)
}

/// A turn of the owner's on the task, begun, and its report naming `pr`: the review Gathr gave and
/// the turn, still running.
fn report(
    conn: &mut Connection,
    task: i64,
    how: Finish,
    pr: Option<i64>,
) -> (Option<(i64, String)>, String) {
    let id = begin(conn, task);
    (said_in(conn, task, how, pr, &id).0, id)
}

/// Ends `turn` well through `turn_moved`, as the service ends it: what that starts.
fn ended(conn: &mut Connection, turn: &str) -> Vec<Start> {
    let (agent, task): (String, Option<i64>) = conn
        .query_row(
            "SELECT agent_id, task FROM turns WHERE id = ?1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let moved = tasks::Moved {
        chat: "c",
        turn,
        agent: Some(&agent),
        task,
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(conn, &moved, "done", None).unwrap()
}

/// Bo's turn on the review, reported done and ended: what its end starts.
fn review_done(conn: &mut Connection, review: i64) -> Vec<Start> {
    let id = begin(conn, review);
    let said = Said {
        commit: Some("abc1234"),
        nothing_unasked: true,
        ..Said::default()
    };
    finish(conn, "bo", review, Finish::Done, &said).unwrap();
    ended(conn, &id)
}

/// Each `gathr.review.give`: its outcome, PR and review, oldest first.
fn decisions(conn: &Connection) -> Vec<(String, i64, Option<i64>)> {
    let sql = "SELECT json_extract(attrs, '$.\"gathr.review.outcome\"'), json_extract(attrs, '$.\"gathr.review.pr\"'),
                      json_extract(attrs, '$.\"gathr.task.review\"')
               FROM event_log WHERE name = 'gathr.review.give' ORDER BY id";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)));
    rows.unwrap().map(Result::unwrap).collect()
}

fn last(conn: &Connection) -> (String, i64, Option<i64>) {
    decisions(conn).pop().expect("a gathr.review.give")
}

/// The chat's review tasks, oldest first.
fn reviews(conn: &Connection) -> Vec<i64> {
    let sql = "SELECT number FROM tasks WHERE kind = 'review' ORDER BY number";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// A review of `pr` given by `giver` from `given_in`, with `parent` and `after`.
fn review_from(
    conn: &mut Connection,
    (giver, given_in): (&str, Option<&str>),
    parent: Option<i64>,
    after: &[i64],
    pr: Option<i64>,
) -> Result<i64, Error> {
    let g = Give {
        chat: "c",
        giver: Some(giver),
        owner: "bo",
        given_in,
        parent,
        kind: Kind::Review,
        what: "Review the change",
        brief: Brief::default(),
        after,
        pr,
        in_passing: false,
        look_into: false,
    };
    tasks::give(conn, &g, T).map(|(t, _)| t.number)
}

#[test]
fn a_done_report_that_first_names_its_pr_gives_the_reviewer_its_review_after_the_work() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewing(&conn);
    turn(&conn, "lead-turn", "lead", None, "running");
    let (work, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    assert_eq!(work, 1);
    let ann = begin(&mut conn, work);
    let said = Said {
        note: Some("see #7"),
        pr: Some(7),
        turn: Some(&ann),
        ..Said::default()
    };
    let (_, took, starts, given) =
        tasks::finish(&mut conn, "ann", work, Finish::Done, &said, "report-trace").unwrap();
    assert_eq!(took, Took::Kept);
    assert_eq!(given, Some((2, "Bo".to_string())), "the review it gave");
    let r = tasks::get(&conn, 2).unwrap().unwrap();
    let as_given = (
        r.kind,
        r.owner.as_deref(),
        r.giver.as_deref(),
        r.given_in.as_deref(),
        r.parent,
    );
    assert_eq!(
        as_given,
        (
            Kind::Review,
            Some("bo"),
            Some("ann"),
            Some(ann.as_str()),
            Some(work)
        )
    );
    let shape = (
        r.what.as_str(),
        r.pr,
        r.in_passing,
        r.after.clone(),
        r.state,
    );
    assert_eq!(
        shape,
        ("Review #7", Some(7), true, vec![work], State::Waiting)
    );
    assert!(
        to(&starts, "bo").is_empty(),
        "it waits on the work: {starts:?}"
    );

    let sql = "SELECT json_extract(attrs, '$.\"gathr.review.outcome\"'), trace_id FROM event_log
               WHERE name = 'gathr.review.give'";
    let rows: Vec<(String, String)> = {
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
        rows.map(Result::unwrap).collect()
    };
    assert_eq!(rows, [("given".to_string(), "report-trace".to_string())]);
    assert_eq!(decisions(&conn), [("given".to_string(), 7, Some(2))]);
    let gives: i64 = conn
        .query_row(
            "SELECT count(*) FROM event_log WHERE name = 'gathr.task.give' AND CAST(task_id AS INTEGER) = 2
             AND trace_id = 'report-trace' AND json_extract(attrs, '$.\"gathr.task.giver\"') = 'ann'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(gives, 1, "its give, by Ann, on the report's trace");

    let starts = ended(&mut conn, &ann);
    let held = Some("done, waiting on #7 to merge".to_string());
    assert_eq!(state(&conn, work), (State::Held, held));
    assert_eq!(state(&conn, 2).0, State::Ready);
    let bo: Vec<&Start> = starts.iter().filter(|s| s.agent == "bo").collect();
    assert_eq!(bo.len(), 1, "{starts:?}");
    assert_eq!(bo[0].task, Some(2));
    let opened = samples::opened(&conn, &bo[0].turn);
    assert_eq!(
        opened,
        (Some("review_brief".to_string()), Some(words::REVIEW_BRIEF))
    );
}

#[test]
fn which_reports_give_a_review_and_which_dont() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewing(&conn);
    let not_done = |pr| ("not_done".to_string(), pr, None);

    // Left with a first PR: none, and the work's merge line still wakes the lead.
    let (left, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, left, Finish::Left, Some(8));
    assert_eq!((given, reviews(&conn)), (None, vec![]));
    assert_eq!(decisions(&conn), [not_done(8)]);
    ended(&mut conn, &turn);
    let (given, turn) = report(&mut conn, left, Finish::Done, None);
    ended(&mut conn, &turn);
    assert_eq!(
        (given, reviews(&conn)),
        (None, vec![]),
        "a later done names no PR first"
    );
    assert_eq!(decisions(&conn).len(), 1, "and records nothing");
    let live = Live {
        running: true,
        ..Live::default()
    };
    let said = waiting::answer(&conn, "c", &live, gathr::ids::now_us()).unwrap();
    let merge = said
        .lines
        .iter()
        .find(|l| l.kind == "merge")
        .expect("a merge line");
    assert!(
        merge.what.ends_with(", which has no review"),
        "{}",
        merge.what
    );
    assert_eq!(merge.moves, Moves::Nobody);

    // Blocked with a first PR: none.
    let (blocked, _) = give(&mut conn, "ann", &[], None);
    let (given, _) = report(&mut conn, blocked, Finish::Blocked, Some(9));
    assert_eq!((given, reviews(&conn)), (None, vec![]));
    assert_eq!(decisions(&conn).last(), Some(&not_done(9)));

    // A refused report: nothing recorded, and the work keeps no PR.
    let (refused, _) = give(&mut conn, "ann", &[], None);
    begin(&mut conn, refused);
    let wrong = Said {
        note: Some("see #10"),
        pr: Some(10),
        on_task: Some(left),
        ..Said::default()
    };
    let why = refusal(tasks::finish(
        &mut conn,
        "ann",
        refused,
        Finish::Done,
        &wrong,
        T,
    ));
    assert_eq!(why, "`on_task` goes with blocked");
    assert_eq!(decisions(&conn).len(), 2);
    assert_eq!(tasks::get(&conn, refused).unwrap().unwrap().pr, None);

    // Given with its PR: its report names no PR first.
    let with_pr = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix #11",
        brief: common::COMPLETE,
        after: &[],
        pr: Some(11),
        in_passing: false,
        look_into: false,
    };
    let with_pr = tasks::give(&mut conn, &with_pr, T).unwrap().0.number;
    let (given, _) = report(&mut conn, with_pr, Finish::Done, Some(11));
    assert_eq!((given, reviews(&conn)), (None, vec![]));
    assert_eq!(decisions(&conn).len(), 2);

    // A second report naming the same PR says nothing (D5).
    let (twice, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, twice, Finish::Done, Some(12));
    let first = given.expect("the first report gives one").0;
    let (given, _) = said_in(&mut conn, twice, Finish::Done, Some(12), &turn);
    assert_eq!((given, reviews(&conn)), (None, vec![first]));
    assert_eq!(decisions(&conn).len(), 3);

    // Done while a task Ann gave from her turn on the work, not in passing, is open: the review
    // waits until that task is done and Ann's hear-back releases the work.
    let (built, _) = give(&mut conn, "ann", &[], None);
    let turn = begin(&mut conn, built);
    let part = Give {
        chat: "c",
        giver: Some("ann"),
        owner: "lead",
        given_in: Some(&turn),
        parent: Some(built),
        kind: Kind::Work,
        what: "Write the notes",
        brief: common::COMPLETE,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: false,
    };
    let part = tasks::give(&mut conn, &part, T).unwrap().0.number;
    let (given, _) = said_in(&mut conn, built, Finish::Done, Some(13), &turn);
    let review = given.expect("given").0;
    ended(&mut conn, &turn);
    assert_eq!(
        state(&conn, review).0,
        State::Waiting,
        "the work is held on #{part} too"
    );
    let starts = work(&mut conn, part, Some(Finish::Done));
    let held = Some("done, waiting on #13 to merge".to_string());
    assert_eq!(state(&conn, built), (State::Held, held));
    assert_eq!(state(&conn, review).0, State::Ready);
    assert_eq!(to(&starts, "bo").len(), 1, "{starts:?}");
}

#[test]
fn a_review_is_given_once_per_pr_and_not_when_one_covers_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewing(&conn);
    let (first, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, first, Finish::Done, Some(7));
    let gathrs = given.expect("given").0;
    ended(&mut conn, &turn);
    let covered = |by: i64, pr: i64| ("covered".to_string(), pr, Some(by));

    // Gathr's review of 7 is open.
    let (again, _) = give(&mut conn, "ann", &[], None);
    report(&mut conn, again, Finish::Done, Some(7));
    assert_eq!(last(&conn), covered(gathrs, 7));
    assert_eq!(reviews(&conn), [gathrs]);

    // Gathr's review of 7 is done.
    review_done(&mut conn, gathrs);
    assert_eq!(state(&conn, gathrs).0, State::Done);
    let (later, _) = give(&mut conn, "ann", &[], None);
    report(&mut conn, later, Finish::Done, Some(7));
    assert_eq!(last(&conn), covered(gathrs, 7));
    assert_eq!(reviews(&conn), [gathrs]);

    // Ann gave a review with no PR from her turn on the work.
    let (fourth, _) = give(&mut conn, "ann", &[], None);
    let turn = begin(&mut conn, fourth);
    let hers = review_from(&mut conn, ("ann", Some(&turn)), Some(fourth), &[], None).unwrap();
    said_in(&mut conn, fourth, Finish::Done, Some(8), &turn);
    assert_eq!(last(&conn), covered(hers, 8));
    assert_eq!(reviews(&conn), [gathrs, hers]);

    // Lead gave a review with no PR after the work.
    let (fifth, _) = give(&mut conn, "ann", &[], None);
    let leads = review_from(&mut conn, ("lead", None), None, &[fifth], None).unwrap();
    report(&mut conn, fifth, Finish::Done, Some(9));
    assert_eq!(last(&conn), covered(leads, 9));
    assert_eq!(reviews(&conn), [gathrs, hers, leads]);

    // A cancelled review covers nothing.
    let (sixth, _) = give(&mut conn, "ann", &[], None);
    let (given, _) = report(&mut conn, sixth, Finish::Done, Some(13));
    let cancelled = given.expect("given").0;
    tasks::cancel(&mut conn, Some("ann"), cancelled, "not needed", T).unwrap();
    let (seventh, _) = give(&mut conn, "ann", &[], None);
    let (given, _) = report(&mut conn, seventh, Finish::Done, Some(13));
    let anew = given.expect("given again").0;
    assert_eq!(last(&conn), ("given".to_string(), 13, Some(anew)));
    assert_eq!(reviews(&conn), [gathrs, hers, leads, cancelled, anew]);

    // No Reviewer in the chat.
    conn.execute(
        "UPDATE chat_members SET role = 'member' WHERE agent_id = 'bo'",
        [],
    )
    .unwrap();
    let (eighth, _) = give(&mut conn, "ann", &[], None);
    let (given, _) = report(&mut conn, eighth, Finish::Done, Some(10));
    assert_eq!(
        (given, last(&conn)),
        (None, ("no_reviewer".to_string(), 10, None))
    );

    // The Reviewer's own work.
    reviewing(&conn);
    let (bos, _) = give(&mut conn, "bo", &[], None);
    let (given, _) = report(&mut conn, bos, Finish::Done, Some(14));
    assert_eq!(
        (given, last(&conn)),
        (None, ("own_review".to_string(), 14, None))
    );
    assert_eq!(reviews(&conn).len(), 5);
}

#[test]
fn a_second_review_of_a_pr_with_an_open_one_is_refused_naming_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewing(&conn);
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(7));
    assert_eq!(given.map(|g| g.0), Some(2));
    let gives = |conn: &Connection| -> i64 {
        conn.query_row(
            "SELECT count(*) FROM event_log WHERE name = 'gathr.task.give'",
            [],
            |r| r.get(0),
        )
        .unwrap()
    };
    let before = (reviews(&conn), gives(&conn));
    let second = review_from(&mut conn, ("lead", None), None, &[], Some(7));
    assert_eq!(
        refusal(second),
        "PR #7 already has an open review: #2, Bo's, given by Ann. Give one once #2 has ended or its giver cancels it."
    );
    assert_eq!((reviews(&conn), gives(&conn)), before, "no task, no give");
    review_from(&mut conn, ("lead", None), None, &[], None)
        .expect("a review with no PR names none");

    ended(&mut conn, &turn);
    review_done(&mut conn, 2);
    assert_eq!(state(&conn, 2).0, State::Done);
    review_from(&mut conn, ("lead", None), None, &[], Some(7)).expect("a second look");

    // A review left with a wrong note is blocked with Gathr's fix: it isn't open.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(15));
    let review = given.expect("given").0;
    ended(&mut conn, &turn);
    let id = begin(&mut conn, review);
    let notes = [NoteSaid {
        kind: NoteKind::Wrong,
        file: None,
        says: "It fails",
    }];
    let left = Said {
        note: Some("fails"),
        commit: Some("abc1234"),
        notes: &notes,
        ..Said::default()
    };
    finish(&mut conn, "bo", review, Finish::Left, &left).unwrap();
    ended(&mut conn, &id);
    assert_eq!(state(&conn, review).0, State::Blocked);
    review_from(&mut conn, ("lead", None), None, &[], Some(15)).expect("its review is blocked");
}

#[test]
fn gathrs_review_goes_as_one_the_builder_gave() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    reviewing(&conn);

    // The work cancelled while Ann's turn on it runs, after her report.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(20));
    let review = given.expect("given").0;
    tasks::cancel(&mut conn, Some("lead"), work, "not needed", T).unwrap();
    let why = Some(format!("#{work} was cancelled"));
    assert_eq!(state(&conn, review), (State::Cancelled, why.clone()));
    let starts = ended(&mut conn, &turn);
    assert!(to(&starts, "bo").is_empty(), "{starts:?}");
    assert_eq!(state(&conn, review), (State::Cancelled, why));

    // Bo's review done, and his turn ended, before any merge: Ann isn't woken.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(21));
    let review = given.expect("given").0;
    ended(&mut conn, &turn);
    let starts = review_done(&mut conn, review);
    assert!(to(&starts, "ann").is_empty(), "{starts:?}");
    let held = Some("done, waiting on #21 to merge".to_string());
    assert_eq!(state(&conn, work), (State::Held, held));

    // Reported from a turn on another task: given in Ann's turn on the work.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let on_work = begin(&mut conn, work);
    gathr::store::set_turn(&conn, &on_work, "done", None).unwrap();
    let (other, _) = give(&mut conn, "ann", &[], None);
    let on_other = begin(&mut conn, other);
    let (given, _) = said_in(&mut conn, work, Finish::Done, Some(11), &on_other);
    let review = given.expect("given").0;
    let r = tasks::get(&conn, review).unwrap().unwrap();
    assert_eq!(r.given_in.as_deref(), Some(on_work.as_str()));
    let other_was = state(&conn, other);
    let starts = review_done(&mut conn, review);
    assert!(to(&starts, "ann").is_empty(), "{starts:?}");
    assert_eq!(state(&conn, other), other_was);

    // The merge first, while Bo's turn runs: Ann hears the review is done, as today.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(22));
    let review = given.expect("given").0;
    ended(&mut conn, &turn);
    begin(&mut conn, review);
    let said = Said {
        commit: Some("abc1234"),
        nothing_unasked: true,
        ..Said::default()
    };
    finish(&mut conn, "bo", review, Finish::Done, &said).unwrap();
    let mut starts = tasks::pr_looked(&mut conn, work, "MERGED", None, None, T).unwrap();
    starts.extend(tasks::pr_looked(&mut conn, review, "MERGED", None, None, T).unwrap());
    assert_eq!(state(&conn, review).0, State::Done);
    let heard: Vec<&Start> = starts
        .iter()
        .filter(|s| s.agent == "ann" && s.hear_back)
        .collect();
    assert_eq!(heard.len(), 1, "{starts:?}");

    // Left with a wrong note: Gathr's fix goes to Ann, given by Bo, and Ann hears the review is
    // blocked with it.
    let (work, _) = give(&mut conn, "ann", &[], None);
    let (given, turn) = report(&mut conn, work, Finish::Done, Some(23));
    let review = given.expect("given").0;
    ended(&mut conn, &turn);
    let id = begin(&mut conn, review);
    let notes = [NoteSaid {
        kind: NoteKind::Wrong,
        file: None,
        says: "It fails",
    }];
    let left = Said {
        note: Some("fails"),
        commit: Some("abc1234"),
        notes: &notes,
        ..Said::default()
    };
    finish(&mut conn, "bo", review, Finish::Left, &left).unwrap();
    let starts = ended(&mut conn, &id);
    let fix: i64 = conn
        .query_row(
            "SELECT number FROM tasks WHERE what = 'Fix the review of #23'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let f = tasks::get(&conn, fix).unwrap().unwrap();
    let as_given = (
        f.owner.as_deref(),
        f.giver.as_deref(),
        f.in_passing,
        f.parent,
    );
    assert_eq!(as_given, (Some("ann"), Some("bo"), true, Some(review)));
    let outcome: String = conn
        .query_row(
            "SELECT json_extract(attrs, '$.\"gathr.review.outcome\"') FROM event_log
             WHERE name = 'gathr.review.fix' ORDER BY id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(outcome, "given");
    let heard: Vec<&Start> = starts
        .iter()
        .filter(|s| s.agent == "ann" && s.hear_back)
        .collect();
    assert_eq!(heard.len(), 1, "{starts:?}");
    let blocked = format!("Gathr gave #{fix} to Ann to fix it");
    assert!(
        heard[0].text.contains(&format!("#{review}")) && heard[0].text.contains(&blocked),
        "{}",
        heard[0].text
    );
}
