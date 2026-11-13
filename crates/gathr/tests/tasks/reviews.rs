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
