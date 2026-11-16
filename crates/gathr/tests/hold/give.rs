//! I1, I2's refusals at the library seam, and I5: what a held turn may give, write and push, and
//! how a look-into task is briefed and reports.

use super::*;
use gathr::tasks::{NoteKind, NoteSaid};
use serde_json::{Value, json};

const WORK: (Kind, bool, Option<i64>) = (Kind::Work, false, None);
const LOOK: (Kind, bool, Option<i64>) = (Kind::Work, true, None);
const WRONG: &str = "\"Add tags to notes\" has no approved plan";

fn count(conn: &Connection, table: &str) -> i64 {
    let sql = format!("SELECT count(*) FROM {table}");
    conn.query_row(&sql, [], |r| r.get(0)).unwrap()
}

/// I1: a held turn's building give is refused, worded by who gives and the plan's state; look-into
/// tasks and reviews are taken; a started building task's give is taken and waits.
#[test]
fn a_held_turn_gives_only_look_into_and_review_tasks() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    let before = (count(&conn, "tasks"), count(&conn, "turns"));
    let lead = format!(
        "{WRONG}, so building waits. If it is a repair, size it repair with size; else write its plan with plan, \
         or give a look-into task (look_into: true) for what the plan needs first."
    );
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        lead
    );
    assert_eq!(
        (count(&conn, "tasks"), count(&conn, "turns")),
        before,
        "nothing given"
    );
    assert!(held_ts(&conn, &m1).is_some(), "the ask was held");
    let refused = events(&conn, "refused");
    assert_eq!(refused.len(), 1, "{refused:?}");
    let e = &refused[0];
    let seen = [
        &e["gathr.ask.id"],
        &e["gathr.turn.id"],
        &e["gen_ai.agent.id"],
        &e["gathr.hold.what"],
        &e["gathr.hold.why"],
        &e["gen_ai.tool.name"],
    ];
    let want = [m1.as_str(), "T", "mira", "give", "unplanned", "give_task"];
    assert_eq!(seen, want.map(serde_json::Value::from).each_ref());
    assert!(e.get("gathr.hold.plan").is_none(), "no plan: {e}");

    asks::resize(&mut conn, &m1, "new", T).unwrap();
    let sized_new = format!(
        "{WRONG}, so building waits: write its plan with plan, or give a look-into task (look_into: true) for \
         what the plan needs first."
    );
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        sized_new
    );
    let ann = queued(&conn, "ann", (None, Some("T")), Opened::Teammate);
    let member = format!(
        "{WRONG}, so building waits: give a look-into task (look_into: true), or tell Mira what you found."
    );
    assert_eq!(
        refusal(give(&mut conn, ("ann", "bo"), Some(&ann), WORK)),
        member
    );

    let look = give(&mut conn, ("mira", "ann"), Some("T"), LOOK).unwrap().0;
    assert!(look.look_into, "taken as a look-into task");
    let look_turn = queued_on(&conn, look.number).unwrap();
    assert!(held(&conn, &look_turn), "its turn is held");
    give(
        &mut conn,
        ("mira", "bo"),
        Some("T"),
        (Kind::Review, false, None),
    )
    .unwrap();
    let recorded = events(&conn, "refused").len();
    let review = refusal(give(
        &mut conn,
        ("mira", "bo"),
        Some("T"),
        (Kind::Review, true, None),
    ));
    assert_eq!(
        review,
        "A review isn't a look-into task: leave out look_into, or give it as work."
    );
    let with_pr = refusal(give(
        &mut conn,
        ("mira", "ann"),
        Some("T"),
        (Kind::Work, true, Some(7)),
    ));
    assert_eq!(with_pr, "A look-into task takes no pr: it changes nothing.");
    assert_eq!(
        events(&conn, "refused").len(),
        recorded,
        "nothing of the hold"
    );
    let from_look = format!(
        "Task #{} is a look-into task, so it gives no building work: give a look-into task (look_into: true), \
         or report what you found.",
        look.number
    );
    assert_eq!(
        refusal(give(&mut conn, ("ann", "bo"), Some(&look_turn), WORK)),
        from_look
    );
    give(&mut conn, ("ann", "bo"), Some(&look_turn), LOOK).unwrap();

    // By the state of the ask's plan.
    let first = plan(&mut conn, "T", "Add the tags table");
    let checking = format!(
        "\"{TAGS}\" has no approved plan yet: its plan is being checked, and building starts once the person \
         approves it. Meanwhile give a look-into task (look_into: true) if the plan needs more."
    );
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        checking
    );
    let member_on = |state: &str| {
        format!(
            "\"{TAGS}\" has no approved plan yet: its plan is {state}, so building waits. Give a look-into task \
             (look_into: true), or tell Mira what you found."
        )
    };
    let ann_says = refusal(give(&mut conn, ("ann", "bo"), Some(&ann), WORK));
    assert_eq!(ann_says, member_on("being checked"));
    agreed(&mut conn, &first);
    let waiting = format!(
        "\"{TAGS}\" has no approved plan yet: its plan waits for the person's Approve. Tell them so, and don't \
         write it again."
    );
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        waiting
    );
    assert_eq!(
        events(&conn, "refused").pop().unwrap()["gathr.hold.plan"],
        "waiting"
    );
    let ann_says = refusal(give(&mut conn, ("ann", "bo"), Some(&ann), WORK));
    assert_eq!(ann_says, member_on("waiting for the person's Approve"));
    plans::send_back(&mut conn, first.plan, 1, "Split it", T).unwrap();
    let revising = format!(
        "\"{TAGS}\" has no approved plan yet: its plan was sent back or its check found problems. Write the next \
         version with plan."
    );
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        revising
    );
    let ann_says = refusal(give(&mut conn, ("ann", "bo"), Some(&ann), WORK));
    assert_eq!(ann_says, member_on("being revised by Mira"));
    plan(&mut conn, "T", "Add the tags column");
    plans::drop(&mut conn, first.plan, 2, T).unwrap();
    assert_eq!(
        refusal(give(&mut conn, ("mira", "ann"), Some("T"), WORK)),
        sized_new,
        "dropped"
    );

    // A started building task's own give is taken, and waits for the plan.
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the export");
    lead_sizes(&mut conn, "T2", "repair");
    let build = work(&mut conn, ("mira", "ann"), "T2", false);
    let build_turn = begin(&mut conn, build);
    asks::resize(&mut conn, &m2, "new", T).unwrap();
    let recorded = events(&conn, "refused").len();
    let sub = give(&mut conn, ("ann", "bo"), Some(&build_turn), WORK)
        .unwrap()
        .0
        .number;
    let waits = (State::Waiting, Some("waits for its plan".to_string()));
    assert_eq!(state(&conn, sub), waits);
    assert_eq!(queued_on(&conn, sub), None, "no turn for it");
    assert_eq!(
        events(&conn, "refused").len(),
        recorded,
        "nothing of the hold"
    );

    // Sized repair, or its plan approved: taken.
    sent(&mut conn, ("c", "mira"), "T3", "Fix the typo");
    refusal(give(&mut conn, ("mira", "ann"), Some("T3"), WORK));
    lead_sizes(&mut conn, "T3", "repair");
    let fixed = give(&mut conn, ("mira", "ann"), Some("T3"), WORK)
        .unwrap()
        .0;
    assert_eq!(fixed.state, State::Ready, "a repair goes ahead");
    let second = plan(&mut conn, "T", "Add the tags table");
    agreed(&mut conn, &second);
    plans::approve(&mut conn, second.plan, 1, T).unwrap();
    let built = give(&mut conn, ("mira", "ann"), Some("T"), WORK).unwrap().0;
    assert_eq!(built.state, State::Ready, "after Approve");
}

/// I1.4, I1.5: Gathr's own building gives on a held ask wait for its plan; reviews and checks don't,
/// and Approve puts what waited back in line.
#[test]
fn gathrs_own_building_gives_wait_for_the_plan() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    asks::resize(&mut conn, &m1, "repair", T).unwrap();
    let build = work(&mut conn, ("mira", "ann"), "T", false);
    let build_turn = begin(&mut conn, build);
    asks::resize(&mut conn, &m1, "new", T).unwrap();
    let said = Said {
        note: Some("see the PR"),
        pr: Some(41),
        turn: Some(&build_turn),
        ..Said::default()
    };
    tasks::finish(&mut conn, "ann", build, Finish::Done, &said, T).unwrap();
    moved(&mut conn, (&build_turn, "ann"), Some(build), "done");
    let review: i64 = conn
        .query_row(
            "SELECT number FROM tasks WHERE kind = 'review' AND pr = 41",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state(&conn, review).0, State::Ready, "E's review");
    let review_turn = begin(&mut conn, review);
    let notes = [NoteSaid {
        kind: NoteKind::Wrong,
        file: Some("src/tags.rs"),
        says: "Tags aren't saved",
    }];
    let left = Said {
        note: Some("One gap"),
        commit: Some("abc1234"),
        notes: &notes,
        ..Said::default()
    };
    tasks::finish(&mut conn, "bo", review, Finish::Left, &left, T).unwrap();
    moved(&mut conn, (&review_turn, "bo"), Some(review), "done");
    let fix: i64 = conn
        .query_row(
            "SELECT number FROM tasks WHERE parent = ?1",
            [review],
            |r| r.get(0),
        )
        .unwrap();
    let waits = (State::Waiting, Some("waits for its plan".to_string()));
    assert_eq!(state(&conn, fix), waits, "Gathr's fix");
    assert_eq!(queued_on(&conn, fix), None);

    let wrote = plan(&mut conn, "T", "Add the tags table");
    assert_eq!(state(&conn, wrote.check).0, State::Ready, "H's check");
    assert!(queued_on(&conn, wrote.check).is_some());
    agreed(&mut conn, &wrote);
    plans::approve(&mut conn, wrote.plan, 1, T).unwrap();
    let part: i64 = conn
        .query_row(
            "SELECT task FROM plan_parts WHERE version_id = ?1",
            [wrote.version],
            |r| r.get(0),
        )
        .unwrap();
    for (what, task) in [("Approve's part", part), ("the fix, back in line", fix)] {
        assert_eq!(state(&conn, task), (State::Ready, None), "{what}");
        assert!(queued_on(&conn, task).is_some(), "{what}'s brief");
    }

    // Readied again on a held ask: a restart, a PR merged with no report.
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the export");
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts,
                            ask_id, started_ts, pr)
         VALUES (50, 'c', 'mira', 'ann', 'T2', 'work', 'Never ran', 'failed', 1, 1, ?1, NULL, NULL),
                (51, 'c', 'mira', 'ann', 'T2', 'work', 'Ran once', 'failed', 1, 1, ?1, 1, NULL),
                (52, 'c', 'mira', 'ann', 'T2', 'work', 'Merged unseen', 'ready', 1, 1, ?1, NULL, 42)",
        [&m2],
    )
    .unwrap();
    let starts = tasks::restart(&mut conn, Some("mira"), 50, T).unwrap();
    assert_eq!(
        (state(&conn, 50), starts.len()),
        (waits.clone(), 0),
        "not started: waits"
    );
    let starts = tasks::restart(&mut conn, Some("mira"), 51, T).unwrap();
    assert_eq!(
        (state(&conn, 51).0, starts.len()),
        (State::Ready, 1),
        "started: runs"
    );
    tasks::pr_looked(&mut conn, 52, "MERGED", None, None, T).unwrap();
    assert_eq!(state(&conn, 52), waits, "its PR merged");
    assert_eq!(queued_on(&conn, 52), None);
}

/// I5: a look-into task's brief, its report, and no review after it.
#[test]
fn a_look_into_task_is_briefed_reports_no_pr_and_gets_no_review() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let g = Give {
        chat: "c",
        giver: Some("mira"),
        owner: "ann",
        given_in: Some("T"),
        parent: None,
        kind: Kind::Work,
        what: "Survey tag libraries",
        brief: Brief {
            why: Some("Notes need tags"),
            done: Some("Three libraries compared"),
            proof: Some("a table in notes/tags.md"),
            place: Some("no code"),
            size: None,
        },
        after: &[],
        pr: None,
        in_passing: false,
        look_into: true,
    };
    let (look, starts) = tasks::give(&mut conn, &g, T).unwrap();
    samples::same("brief: look into", &starts[0].text);
    let with_pr = Said {
        note: Some("Compared three"),
        pr: Some(7),
        ..Said::default()
    };
    let refused = refusal(tasks::finish(
        &mut conn,
        "ann",
        look.number,
        Finish::Done,
        &with_pr,
        T,
    ));
    let want = format!(
        "Task #{} is a look-into task: it names no PR. Put what you found in note, or the file you wrote in file.",
        look.number
    );
    assert_eq!(refused, want);
    assert_eq!(count(&conn, "tasks WHERE kind = 'review'"), 0, "no review");
    let said = Said {
        note: Some("Compared three: notes/tags.md"),
        ..Said::default()
    };
    let (done, ..) = tasks::finish(&mut conn, "ann", look.number, Finish::Done, &said, T).unwrap();
    assert_eq!(done.state, State::Done);
    assert_eq!(serde_json::to_value(&done).unwrap()["look_into"], true);
    conn.execute(
        "UPDATE tasks SET look_into = 0 WHERE number = ?1",
        [look.number],
    )
    .unwrap();
    let as_work = serde_json::to_value(tasks::get(&conn, look.number).unwrap().unwrap()).unwrap();
    assert!(
        as_work.get("look_into").is_none(),
        "a work task's JSON: {as_work}"
    );
}

/// Made-up folders: Mira's worktree inside her own folder, and Shared.
fn place<'a>(own: Option<&'a str>, shared: Option<&'a str>) -> hold::Place<'a> {
    hold::Place {
        worktree: std::path::Path::new("/agents/mira/notes"),
        own: own.map(std::path::Path::new),
        shared: shared.map(std::path::Path::new),
    }
}

/// The deny reason for a write, or none.
fn denied(conn: &Connection, turn: &str, own: Option<&str>) -> Option<String> {
    let place = place(own, Some("/Shared"));
    hold::write_held(conn, turn, &place)
        .unwrap()
        .map(|(why, _)| why)
}

/// I2.2, I2.3, I2.5: a held turn's write and push answers, by who writes and the plan's state.
#[test]
fn a_held_turns_write_and_push_are_refused_and_recorded() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    let base = format!(
        "Held until the plan for \"{TAGS}\" is approved: don't change files in /agents/mira/notes. Write notes, \
         experiments and mock-ups in /agents/mira or /Shared instead."
    );
    let place = place(Some("/agents/mira"), Some("/Shared"));
    let (why, mut write) = hold::write_held(&conn, "T", &place).unwrap().unwrap();
    assert_eq!(
        why,
        format!("{base} If it is a repair, size it repair with size.")
    );
    let (why, push) = hold::push_held(&conn, "T").unwrap().unwrap();
    assert_eq!(
        why,
        format!("held: \"{TAGS}\" has no approved plan, so nothing is pushed for it")
    );
    assert!(held_ts(&conn, &m1).is_none(), "a read writes nothing");
    write.tool = Some("Write".into());
    hold::refused(&mut conn, &write).unwrap();
    hold::refused(&mut conn, &push).unwrap();
    assert!(held_ts(&conn, &m1).is_some());
    let what: Vec<_> = events(&conn, "refused")
        .iter()
        .map(|e| {
            (
                e["gathr.hold.what"].clone(),
                e["gen_ai.tool.name"].clone(),
                e["gathr.turn.id"].clone(),
            )
        })
        .collect();
    let want = [
        (json!("write"), json!("Write"), json!("T")),
        (json!("push"), Value::Null, json!("T")),
    ];
    assert_eq!(what, want);
    let neither = format!(
        "Held until the plan for \"{TAGS}\" is approved: don't change files in /agents/mira/notes. Write notes, \
         experiments and mock-ups in your own folder or Shared instead."
    );
    let bare = place_none();
    let (why, _) = hold::write_held(&conn, "T", &bare).unwrap().unwrap();
    assert_eq!(
        why,
        format!("{neither} If it is a repair, size it repair with size.")
    );

    asks::resize(&mut conn, &m1, "new", T).unwrap();
    assert_eq!(
        denied(&conn, "T", Some("/agents/mira")).as_deref(),
        Some(base.as_str()),
        "sized new"
    );
    let ann = queued(&conn, "ann", (None, Some("T")), Opened::Teammate);
    let ann_base = base.replace("/agents/mira or", "/agents/ann or");
    assert_eq!(
        denied(&conn, &ann, Some("/agents/ann")),
        Some(ann_base.clone()),
        "Ann"
    );
    let first = plan(&mut conn, "T", "Add the tags table");
    let clauses = [
        " Its plan is being checked: building starts once the person approves it.",
        " Its plan waits for the person's Approve: tell them so, and don't write it again.",
        " Write the next version of its plan with plan.",
    ];
    let mut seen = vec![denied(&conn, "T", Some("/agents/mira"))];
    let mut ann_seen = vec![denied(&conn, &ann, Some("/agents/ann"))];
    agreed(&mut conn, &first);
    seen.push(denied(&conn, "T", Some("/agents/mira")));
    ann_seen.push(denied(&conn, &ann, Some("/agents/ann")));
    plans::send_back(&mut conn, first.plan, 1, "Split it", T).unwrap();
    seen.push(denied(&conn, "T", Some("/agents/mira")));
    ann_seen.push(denied(&conn, &ann, Some("/agents/ann")));
    assert_eq!(seen, clauses.map(|c| Some(format!("{base}{c}"))));
    assert_eq!(
        ann_seen,
        [
            Some(ann_base.clone()),
            Some(ann_base.clone()),
            Some(ann_base)
        ]
    );

    // A look-into task's turn, held on a repair ask too.
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the export");
    lead_sizes(&mut conn, "T2", "repair");
    let look = work(&mut conn, ("mira", "ann"), "T2", true);
    let look_turn = queued_on(&conn, look).unwrap();
    let look_words = format!(
        "Task #{look} is a look-into task: don't change files in /agents/mira/notes. Write notes, experiments and \
         mock-ups in /agents/mira or /Shared instead."
    );
    assert_eq!(
        denied(&conn, &look_turn, Some("/agents/mira")),
        Some(look_words)
    );
    let (why, _) = hold::push_held(&conn, &look_turn).unwrap().unwrap();
    assert_eq!(why, "held: a look-into task pushes nothing");

    // Not held: a repair's turn, and a chat with no project.
    let recorded = events(&conn, "refused").len();
    assert!(
        hold::write_held(&conn, "T2", &place).unwrap().is_none(),
        "a repair"
    );
    assert!(
        hold::push_held(&conn, "T2").unwrap().is_none(),
        "a repair's push"
    );
    sent(&mut conn, ("d", "mira"), "D", "Fix the label");
    assert!(held(&conn, "D"), "held, though its chat has no project");
    assert!(
        hold::write_held(&conn, "D", &place).unwrap().is_none(),
        "no project"
    );
    assert_eq!(events(&conn, "refused").len(), recorded, "nothing recorded");
    assert!(held_ts(&conn, &m2).is_none());
}

fn place_none() -> hold::Place<'static> {
    hold::Place {
        worktree: std::path::Path::new("/agents/mira/notes"),
        own: None,
        shared: None,
    }
}
