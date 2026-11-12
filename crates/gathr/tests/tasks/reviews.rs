use super::*;

#[test]
fn a_held_work_task_cannot_use_the_review_refresh_exception() {
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
    tasks::finish(&mut conn, "bo", task, Finish::Done, &said).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok { tools: 1 }, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Held);
    let why = refusal(tasks::finish(&mut conn, "bo", task, Finish::Done, &said));
    assert!(why.contains("not being worked on"), "{why}");
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
    tasks::finish(
        &mut conn,
        "bo",
        unfinished,
        Finish::Left,
        &Said {
            commit: Some("def1234"),
            ..Said::default()
        },
    )
    .unwrap();
    // Recreate the prior schema to exercise the real database upgrade boundary.
    let before =
        "ALTER TABLE tasks DROP COLUMN review_done; DROP INDEX turns_live; DROP INDEX tasks_asked;
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
