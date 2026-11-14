use super::*;

#[test]
fn a_work_tasks_commit_is_what_to_look_at_never_a_review() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let work = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix",
        why: None,
        after: &[],
        pr: Some(5),
        in_passing: false,
    };
    let task = tasks::give(&mut conn, &work, T).unwrap().0.number;
    tasks::started(&mut conn, task, T).unwrap();
    let said = Said {
        commit: Some("abc1234"),
        ..Said::default()
    };
    finish(&mut conn, "bo", task, Finish::Done, &said).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Held);
    // Its owner may report it from any turn (G174). A commit with that is what to look at, no review.
    let again = Said {
        commit: Some("def5678"),
        ..Said::default()
    };
    let finished = finish(&mut conn, "bo", task, Finish::Done, &again).unwrap();
    assert_eq!(finished.reviewed, None, "{finished:?}");
    let see = finished.report.as_ref().and_then(|r| r.see.as_deref());
    assert_eq!(see, Some("def5678"), "{finished:?}");
    assert_eq!(finished.state, State::Held);
}

#[test]
fn upgrading_keeps_completed_reviews_without_approving_unfinished_ones() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "ann", None, "running");
    let done = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, done);
    let unfinished = review_by_bo(&mut conn, 6, "giving");
    tasks::started(&mut conn, unfinished, T).unwrap();
    finish(
        &mut conn,
        "bo",
        unfinished,
        Finish::Left,
        &Said {
            commit: Some("def1234"),
            note: Some("the second file"),
            ..Said::default()
        },
    )
    .unwrap();
    // Recreate the prior schema to exercise the real database upgrade boundary.
    let before =
        "ALTER TABLE tasks DROP COLUMN review_done; DROP INDEX turns_live; DROP INDEX tasks_asked;
                  ALTER TABLE tasks DROP COLUMN finish_see; ALTER TABLE tasks DROP COLUMN finish_ts;
                  ALTER TABLE tasks RENAME COLUMN asks TO idle_lefts;
                  ALTER TABLE tasks DROP COLUMN in_passing; ALTER TABLE tasks DROP COLUMN heard_as;
                  ALTER TABLE tasks DROP COLUMN judged;
                  DROP TABLE friction;
                  ALTER TABLE waits DROP COLUMN seen;
                  ALTER TABLE tasks DROP COLUMN signed_off; DROP TABLE landings;
                  DROP TABLE quiet; DROP INDEX messages_person;
                  ALTER TABLE agent_context DROP COLUMN told;
                  ALTER TABLE agent_context DROP COLUMN window_of;
                  ALTER TABLE agent_context DROP COLUMN ran;
                  PRAGMA user_version = 25;";
    conn.execute_batch(before).unwrap();
    drop(conn);
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    for (task, expected) in [(done, true), (unfinished, false)] {
        let approved: bool = conn
            .query_row(
                "SELECT review_done FROM tasks WHERE number = ?1",
                [task],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(approved, expected, "completed evidence for task {task}");
    }
}

/// G170: a review that is done is recorded again at a newer commit on the same task. No fresh
/// review task is needed only to re-record an approval.
#[test]
fn a_done_review_is_recorded_again_at_a_newer_commit() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "ann", None, "done");
    let review = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, review);
    assert_eq!(state(&conn, review).0, State::Done);
    tasks::pr_looked(&mut conn, review, "OPEN", Some("def5678aaaa"), None, T).unwrap();
    assert!(tasks::get(&conn, review).unwrap().unwrap().outdated);

    let newer = Said {
        commit: Some("def5678"),
        ..Said::default()
    };
    let (again, took, starts) =
        tasks::finish(&mut conn, "bo", review, Finish::Done, &newer, T).unwrap();
    assert_eq!(took, tasks::Took::Applied);
    assert!(starts.is_empty(), "nobody is told twice: {starts:?}");
    assert_eq!(
        (again.state, again.reviewed.as_deref(), again.outdated),
        (State::Done, Some("def5678"), false)
    );
    // The same commit again changes nothing.
    let (_, took, _) = tasks::finish(&mut conn, "bo", review, Finish::Done, &newer, T).unwrap();
    assert_eq!(took, tasks::Took::Already);
    // Once the PR has merged or closed there is nothing left to review.
    tasks::pr_looked(&mut conn, review, "MERGED", Some("def5678aaaa"), None, T).unwrap();
    let late = Said {
        commit: Some("0123abc"),
        ..Said::default()
    };
    let (kept, took, _) = tasks::finish(&mut conn, "bo", review, Finish::Done, &late, T).unwrap();
    assert_eq!(took, tasks::Took::Already);
    assert_eq!(kept.reviewed.as_deref(), Some("def5678"));
}

/// The look's verdict is for one commit reviewed and one head. A review recorded again at another
/// commit reads out of date until it is judged, and a verdict that arrives for the commit reviewed
/// before is not taken.
#[test]
fn a_looks_verdict_counts_only_for_the_commit_reviewed_now() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "ann", None, "done");
    let review = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, review);
    let head = Some("def5678aaaa");
    // Only the base came in after abc1234: the look says so, and that is kept for this pair.
    tasks::pr_looked(&mut conn, review, "OPEN", head, Some(("abc1234", true)), T).unwrap();
    let t = tasks::get(&conn, review).unwrap().unwrap();
    assert_eq!(
        (t.outdated, t.judged.as_deref()),
        (false, Some("abc1234:def5678aaaa"))
    );

    // A commit named in fewer than seven characters could be another one: it isn't taken.
    for short in ["d", "def567", "not-a-commit"] {
        let said = Said {
            commit: Some(short),
            ..Said::default()
        };
        assert_eq!(
            refusal(tasks::finish(
                &mut conn,
                "bo",
                review,
                Finish::Done,
                &said,
                T
            )),
            "Give 7 to 40 characters of the commit you reviewed, as git names it"
        );
    }

    // Recorded again at a commit that isn't the head: out of date until the look judges it.
    let older = Said {
        commit: Some("0000aaa"),
        ..Said::default()
    };
    let (t, _, _) = tasks::finish(&mut conn, "bo", review, Finish::Done, &older, T).unwrap();
    assert_eq!((t.outdated, t.judged), (true, None));

    // A verdict worked out for abc1234 while that happened is for another review: not taken.
    tasks::pr_looked(&mut conn, review, "OPEN", head, Some(("abc1234", true)), T).unwrap();
    let t = tasks::get(&conn, review).unwrap().unwrap();
    assert_eq!(
        (t.outdated, t.judged),
        (true, None),
        "judged at the next look"
    );

    tasks::pr_looked(&mut conn, review, "OPEN", head, Some(("0000aaa", false)), T).unwrap();
    let t = tasks::get(&conn, review).unwrap().unwrap();
    assert_eq!(
        (t.outdated, t.judged.as_deref()),
        (true, Some("0000aaa:def5678aaaa"))
    );
}
