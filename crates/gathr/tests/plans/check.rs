//! H4: who checks a version, how a check agrees or objects, how its version reaches the person,
//! and that only Gathr and the person end a check.

use super::*;

/// One part: Ann adds the table.
fn one() -> Vec<Part<'static>> {
    vec![part("Ann", "Add the tags table", &[])]
}

/// `plan` from Mira's `turn` in `chat`.
fn write_to(
    conn: &mut Connection,
    chat: &str,
    turn: &str,
    parts: &[Part],
    answers: Option<&[Answer]>,
) -> Wrote {
    let w = Writing {
        chat,
        agent: "mira",
        turn: Some(turn),
        title: TITLE,
        text: TEXT,
        parts,
        answers,
    };
    plans::write(conn, &w, T).unwrap()
}

fn owner(conn: &Connection, task: i64) -> String {
    tasks::get(conn, task).unwrap().unwrap().owner.unwrap()
}

/// The thumbs-up of `agent` on `message`, and what it did to a check.
fn thumbs_up(conn: &mut Connection, agent: &str, message: &str) -> Option<plans::Agreed> {
    let tx = conn.transaction().unwrap();
    gathr::reactions::add(&tx, "c", message, agent, T).unwrap();
    let agreed = plans::agreed(&tx, "c", message, agent, T).unwrap();
    tx.commit().unwrap();
    agreed
}

/// The version's message.
fn message_of(conn: &Connection, version: i64) -> String {
    version_field(conn, version, "message_id")
}

/// Mira's turns queued, by what opened them.
fn queued_for_mira(conn: &Connection) -> Vec<(String, String)> {
    let sql = "SELECT u.opened_by, m.body FROM turns u JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
               WHERE u.agent_id = 'mira' AND u.status = 'queued' ORDER BY u.rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap();
    rows.map(Result::unwrap).collect()
}

fn answers() -> Vec<Answer> {
    vec![
        Answer {
            note: 1,
            changed: true,
            why: "The list reads tags now".into(),
        },
        Answer {
            note: 2,
            changed: false,
            why: "The person asked for export last week".into(),
        },
    ]
}

/// H4.2.
#[test]
fn the_check_goes_to_the_reviewer_else_the_free_member_that_checked_least() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    gathr::store::add_turn(
        &conn,
        "c",
        "bo-busy",
        T,
        (Some("bo"), None, None),
        Opened::Person,
    )
    .unwrap();
    set_status(&conn, "bo-busy", "running");
    let reviewed = write_to(&mut conn, "c", "T", &one(), None);
    assert_eq!(
        owner(&conn, reviewed.check),
        "bo",
        "the Reviewer, busy or not"
    );

    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('t2', 'Two', 1);
         INSERT INTO chat_members VALUES ('t2', 'mira', 'supervisor', 1), ('t2', 'ann', 'member', 2),
             ('t2', 'cy', 'member', 3);",
    )
    .unwrap();
    asked(&mut conn, "t2", "U1", "Add tags");
    let first = write_to(&mut conn, "t2", "U1", &one(), None);
    assert_eq!(
        owner(&conn, first.check),
        "ann",
        "both free, neither has checked: the first added"
    );
    let (ann_turn, _) = queued_on(&conn, first.check);
    set_status(&conn, &ann_turn, "done");
    asked(&mut conn, "t2", "U2", "Add folders");
    let second = write_to(&mut conn, "t2", "U2", &one(), None);
    assert_eq!(
        owner(&conn, second.check),
        "cy",
        "Ann has checked once, Cy never"
    );

    // Ann has checked twice, Cy once, but Cy's check turn is queued: Ann is the free one.
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts, checks)
         VALUES ('t2', 'mira', 'ann', 'review', 'Check plan v1', 'done', 1, 1, ?1)",
        [first.version],
    )
    .unwrap();
    asked(&mut conn, "t2", "U3", "Add search");
    let third = write_to(&mut conn, "t2", "U3", &one(), None);
    assert_eq!(owner(&conn, third.check), "ann", "Cy is busy");

    // Ann's findings: the next version goes to Ann again, though Cy is free and has checked less.
    let (cy_turn, _) = queued_on(&conn, second.check);
    set_status(&conn, &cy_turn, "done");
    let (ann_turn, _) = queued_on(&conn, third.check);
    let m = Moved {
        chat: "t2",
        turn: &ann_turn,
        agent: Some("ann"),
        task: Some(third.check),
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(&mut conn, &m, "running", None).unwrap();
    let notes = [tasks::NoteSaid {
        kind: tasks::NoteKind::Wrong,
        file: None,
        says: "Search needs an index",
    }];
    let said = Said {
        note: Some("One gap"),
        notes: &notes,
        ..Said::default()
    };
    report(&mut conn, "ann", third.check, Finish::Left, &said).unwrap();
    tasks::turn_moved(&mut conn, &m, "done", None).unwrap();
    let id = note_ids(&conn, third.check)[0];
    let answered = [Answer {
        note: id,
        changed: true,
        why: "Added the index".into(),
    }];
    let again = write_to(&mut conn, "t2", "U3", &one(), Some(&answered));
    assert_eq!(
        owner(&conn, again.check),
        "ann",
        "who left findings checks again"
    );
}

/// H4.9, H4.10, H4.11.
#[test]
fn a_thumbs_up_from_the_checker_on_its_version_finishes_the_check() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write_to(&mut conn, "c", "T", &one(), None);
    moved(&mut conn, ("T", "mira"), None, "done");
    let message = message_of(&conn, wrote.version);

    assert!(
        thumbs_up(&mut conn, "ann", &message).is_none(),
        "Ann isn't the checker"
    );
    assert_eq!(task_state(&conn, wrote.check).0, tasks::State::Ready);

    let turn = begin(&mut conn, wrote.check);
    let agreed = thumbs_up(&mut conn, "bo", &message).unwrap();
    assert_eq!(
        (agreed.took, agreed.check, agreed.version),
        (tasks::Took::Kept, wrote.check, 1)
    );
    let check = tasks::get(&conn, wrote.check).unwrap().unwrap();
    assert_eq!(check.report.map(|r| r.how), Some(Finish::Done));
    assert_eq!(
        check.state,
        tasks::State::Working,
        "kept until the turn ends"
    );
    assert_eq!(lines(&conn, "c"), Vec::<String>::new());

    let starts = moved(&mut conn, (&turn, "bo"), Some(wrote.check), "done");
    assert_eq!(task_state(&conn, wrote.check).0, tasks::State::Done);
    let said = lines(&conn, "c");
    assert_eq!(
        said,
        ["Bo agrees with version 1 of the plan \"Tags on notes\"."]
    );
    let line: Option<String> = version_field(&conn, wrote.version, "check_line");
    let line_id: String = conn
        .query_row(
            "SELECT id FROM messages WHERE turn_id IS NULL AND role = 'gathr'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(line, Some(line_id));
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert!(sent.is_some(), "the version reached the person");
    assert!(starts.iter().all(|s| s.agent != "mira"), "{starts:?}");
    assert_eq!(queued_for_mira(&conn), Vec::<(String, String)>::new());
    let heard: bool = conn
        .query_row(
            "SELECT heard FROM tasks WHERE number = ?1",
            [wrote.check],
            |r| r.get(0),
        )
        .unwrap();
    assert!(heard, "the check is heard with no turn");

    assert_eq!(plan_state(&conn, "c", wrote.plan), plans::State::Waiting);
    assert_eq!(
        plans::open_state(&conn, &ask).unwrap(),
        Some(plans::State::Waiting)
    );
    let chats = gathr::store::chats(&conn, false).unwrap();
    let c = chats
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "c")
        .unwrap();
    assert_eq!(c["needs"], true);
    let live = gathr::waiting::Live {
        running: true,
        ..Default::default()
    };
    let answer = gathr::waiting::answer(&conn, "c", &live, gathr::ids::now_us()).unwrap();
    let plan = answer.lines.iter().find(|l| l.kind == "plan").unwrap();
    let seen = (
        plan.what.as_str(),
        plan.moves,
        plan.agent.as_deref(),
        plan.since_us,
    );
    assert_eq!(
        seen,
        (
            "A plan for you: Tags on notes",
            gathr::waiting::Moves::You,
            Some("mira"),
            sent
        )
    );

    // No turn on the check: it is done at once. A thumbs-up on a replaced version does nothing.
    asked(&mut conn, "c", "T5", "Add folders");
    let first = write_to(&mut conn, "c", "T5", &one(), None);
    let mut changed = one();
    changed[0].what = "Add the folders table";
    let second = write_to(&mut conn, "c", "T5", &changed, None);
    let replaced = message_of(&conn, first.version);
    assert!(thumbs_up(&mut conn, "bo", &replaced).is_none());
    let open = tasks::get(&conn, second.check).unwrap().unwrap();
    assert_eq!(
        (open.state, open.report.is_none()),
        (tasks::State::Ready, true)
    );
    let newest = message_of(&conn, second.version);
    let agreed = thumbs_up(&mut conn, "bo", &newest).unwrap();
    assert_eq!(agreed.took, tasks::Took::Applied);
    assert_eq!(task_state(&conn, second.check).0, tasks::State::Done);
}

/// H4.3, H4.7, H4.8.
#[test]
fn a_check_left_with_findings_wakes_the_lead_and_the_next_version_answers_each() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write_to(&mut conn, "c", "T", &one(), None);
    moved(&mut conn, ("T", "mira"), None, "done");
    let turn = begin(&mut conn, wrote.check);

    let suggestion = [tasks::NoteSaid {
        kind: tasks::NoteKind::Suggestion,
        file: None,
        says: "Name it labels",
    }];
    let only = Said {
        note: Some("A thought"),
        notes: &suggestion,
        ..Said::default()
    };
    assert_eq!(
        refusal(report(&mut conn, "bo", wrote.check, Finish::Left, &only)),
        "A check that agrees gives a thumbs-up with react; left needs a wrong or unasked note."
    );
    findings(&mut conn, wrote.check);
    let wrong = [tasks::NoteSaid {
        kind: tasks::NoteKind::Wrong,
        file: None,
        says: "Missed a caller",
    }];
    let committed = Said {
        note: Some("Two gaps"),
        notes: &wrong,
        commit: Some("abc1234"),
        ..Said::default()
    };
    assert_eq!(
        refusal(report(
            &mut conn,
            "bo",
            wrote.check,
            Finish::Left,
            &committed
        )),
        "A plan check takes no pr, commit or signed_off: Gathr records the version you checked."
    );
    let done_wrong = Said {
        notes: &wrong,
        nothing_unasked: true,
        ..Said::default()
    };
    assert_eq!(
        refusal(report(
            &mut conn,
            "bo",
            wrote.check,
            Finish::Done,
            &done_wrong
        )),
        "A done review says nothing_unasked: true and has no wrong or unasked notes. Report those as left."
    );

    moved(&mut conn, (&turn, "bo"), Some(wrote.check), "done");
    assert_eq!(
        task_state(&conn, wrote.check),
        (
            tasks::State::Blocked,
            Some(
                "left: Two gaps. Findings: note 1, wrong, src/notes.rs: The list query misses tags; note 2, unasked: \
                 Export isn't asked. Answer each in the next version's answers."
                    .into()
            )
        )
    );
    let (all, fixes): (i64, i64) = conn
        .query_row(
            "SELECT (SELECT count(*) FROM tasks), (SELECT count(*) FROM events WHERE name = 'gathr.review.fix')",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!((all, fixes), (1, 0), "no fix is given");
    let woken = queued_for_mira(&conn);
    assert_eq!(woken.len(), 1, "{woken:?}");
    assert_eq!(woken[0].0, "hear_back");
    samples::same("hear_back: a plan check left", &woken[0].1);
    assert_eq!(
        lines(&conn, "c"),
        ["Bo has 2 findings on version 1 of the plan \"Tags on notes\"."]
    );
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert_eq!(sent, None);
    assert_eq!(plan_state(&conn, "c", wrote.plan), plans::State::Revising);
    assert_eq!(
        plans::open_state(&conn, &ask).unwrap(),
        Some(plans::State::Revising)
    );

    let hear: String = conn
        .query_row(
            "SELECT id FROM turns WHERE agent_id = 'mira' AND status = 'queued'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    set_status(&conn, &hear, "running");
    assert_eq!(
        refusal(write(&mut conn, Some(&hear), &one(), None)),
        "Answer each finding of #1, Bo's check of version 1: notes 1 and 2 have no answer."
    );
    let next = write(&mut conn, Some(&hear), &one(), Some(&answers())).unwrap();
    assert_eq!(owner(&conn, next.check), "bo");
    let (_, brief) = queued_on(&conn, next.check);
    samples::same("plan_check: again", &sampled(&conn, &brief));
    assert_eq!(task_state(&conn, wrote.check).0, tasks::State::Cancelled);
}

/// H4.11, H4.12.
#[test]
fn a_check_that_fails_or_runs_late_reaches_the_person() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let wrote = write_to(&mut conn, "c", "T", &one(), None);
    moved(&mut conn, ("T", "mira"), None, "done");
    let turn = begin(&mut conn, wrote.check);
    moved(&mut conn, (&turn, "bo"), Some(wrote.check), "failed");
    let late = "Bo didn't finish checking version 1 of the plan \"Tags on notes\".";
    assert_eq!(lines(&conn, "c"), [late]);
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert!(sent.is_some());
    assert_eq!(queued_for_mira(&conn), Vec::<(String, String)>::new());

    asked(&mut conn, "c", "T6", "Add folders");
    let open = write_to(&mut conn, "c", "T6", &one(), None);
    let written: i64 = version_field(&conn, open.version, "created_ts");
    let ten = 600_000_000;
    assert_eq!(
        plans::overdue(&mut conn, "c", written + ten - 1_000_000, ten, T).unwrap(),
        0
    );
    assert_eq!(lines(&conn, "c"), [late]);
    assert_eq!(
        plans::overdue(&mut conn, "c", written + ten, ten, T).unwrap(),
        1
    );
    assert_eq!(lines(&conn, "c"), [late, late]);
    let sent: Option<i64> = version_field(&conn, open.version, "sent_ts");
    assert!(sent.is_some());
    let checked = events(&conn, "checked");
    assert_eq!(
        checked
            .last()
            .map(|e| (&e["gathr.plan.outcome"], &e["gathr.plan.sent"])),
        Some((&serde_json::json!("late"), &serde_json::json!(true)))
    );
    assert_eq!(
        plans::overdue(&mut conn, "c", written + 2 * ten, ten, T).unwrap(),
        0
    );
    let message = message_of(&conn, open.version);
    thumbs_up(&mut conn, "bo", &message).unwrap();
    assert_eq!(task_state(&conn, open.check).0, tasks::State::Done);
    assert_eq!(lines(&conn, "c"), [late, late], "the line isn't said twice");

    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c3', 'Three', 1);
         INSERT INTO chat_members VALUES ('c3', 'mira', 'supervisor', 1), ('c3', 'ann', 'member', 2),
             ('c3', 'bo', 'reviewer', 3);",
    )
    .unwrap();
    asked(&mut conn, "c3", "T7", "Add search");
    let archived = write_to(&mut conn, "c3", "T7", &one(), None);
    gathr::archive::archive(&mut conn, "c3", T).unwrap();
    assert_eq!(task_state(&conn, archived.check).0, tasks::State::Cancelled);
    let said = lines(&conn, "c3");
    assert!(said.iter().all(|l| !l.contains("plan")), "{said:?}");
    let line: Option<String> = version_field(&conn, archived.version, "check_line");
    assert_eq!(line, None);
}

/// H4.3, H4.6's carry.
#[test]
fn a_check_is_ended_only_by_gathr_or_the_person() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let wrote = write_to(&mut conn, "c", "T", &one(), None);
    let check = wrote.check;
    let edit = tasks::Edit {
        chat: "c",
        by: Some("mira"),
        task: check,
        what: Some("Check it all"),
        ..Default::default()
    };
    assert_eq!(
        refusal(tasks::edit(&mut conn, &edit, T)),
        format!("Task #{check} is a plan check: Gathr writes it.")
    );
    assert_eq!(
        refusal(tasks::cancel(
            &mut conn,
            Some("mira"),
            check,
            "not needed",
            T
        )),
        format!(
            "Task #{check} is plan #{}'s check: Gathr ends it when a newer version is written or the plan is decided.",
            wrote.plan
        )
    );
    assert_eq!(lines(&conn, "c"), Vec::<String>::new());
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert_eq!(sent, None);

    tasks::cancel(&mut conn, None, check, "I'll read it now", T).unwrap();
    assert_eq!(
        lines(&conn, "c"),
        ["Bo didn't finish checking version 1 of the plan \"Tags on notes\"."]
    );
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert!(sent.is_some());
    let restart = format!("Task #{check} is a plan check: a new version gets a new check.");
    assert_eq!(
        refusal(tasks::restart(&mut conn, Some("mira"), check, T)),
        restart
    );
    assert_eq!(refusal(tasks::restart(&mut conn, None, check, T)), restart);

    asked(&mut conn, "c", "T8", "Add folders");
    let held = write_to(&mut conn, "c", "T8", &one(), None);
    let turn = begin(&mut conn, held.check);
    let command = [("cmd-1".to_string(), "cargo test".to_string())];
    gathr::background::set(&conn, ("c", Some("bo")), &command, Some(&turn)).unwrap();
    moved(&mut conn, (&turn, "bo"), Some(held.check), "done");
    assert_eq!(task_state(&conn, held.check).0, tasks::State::Held);
    tasks::engine_gone(&mut conn, ("c", Some("bo"))).unwrap();
    let (_, brief) = queued_on(&conn, held.check);
    samples::same("plan_check: carry", &sampled(&conn, &brief));
}
