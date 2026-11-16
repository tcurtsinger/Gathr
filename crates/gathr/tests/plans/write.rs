//! H2 and H3: a version is written with its parts, its message and its check, or refused saying
//! what to do; one question to the person is open at a time for an ask that may get a plan.

use super::*;
use gathr::tasks::person::{self, Ask};

const BODY: &str = "Plan: Tags on notes (version 1, 3 parts)\n\
Let notes carry tags, shown in the list and used to filter.\n\
\n\
1. Add the tags table (Ann)\n\
Why: Notes need tags\n\
Done when: A note carries its tags\n\
Proof: tests/tags.rs passes\n\
Where: the project's worktree, on a new branch\n\
Size: src/tags.rs, about 60 lines\n\
\n\
2. Show tags in the list (Bo), after part 1\n\
Why: Notes need tags\n\
Done when: A note carries its tags\n\
Proof: tests/tags.rs passes\n\
Where: the project's worktree, on a new branch\n\
Size: src/tags.rs, about 60 lines\n\
\n\
3. Filter by tag (Ann), after part 2 and #9\n\
Why: Notes need tags\n\
Done when: A note carries its tags\n\
Proof: tests/tags.rs passes\n\
Where: the project's worktree, on a new branch\n\
Size: src/tags.rs, about 60 lines";

/// Mira's question to the person on the ask her `turn` serves.
fn question(conn: &mut Connection, turn: &str, what: &str) -> Result<i64, Error> {
    question_by(conn, "mira", turn, what)
}

fn question_by(conn: &mut Connection, giver: &str, turn: &str, what: &str) -> Result<i64, Error> {
    let a = Ask {
        chat: "c",
        giver,
        given_in: Some(turn),
        parent: None,
        what: Some(what),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    person::ask(conn, a, T).map(|(t, _)| t.number)
}

/// H2.5, H2.7 to H2.9, H4.1.
#[test]
fn a_plan_is_written_as_a_version_with_its_parts_and_its_message() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    // Task #9, done, for part 3 to come after: made by hand, as tasks 1 to 8 aren't needed.
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (9, 'c', 'mira', 'ann', 'work', 'Old work', 'done', 1, 1)",
        [],
    )
    .unwrap();
    let mut parts = three();
    let after_nine = [9];
    parts[2].after_tasks = &after_nine;
    let wrote = write(&mut conn, Some("T"), &parts, None).unwrap();

    let plan: (i64, String, String) = conn
        .query_row("SELECT id, ask_id, lead FROM plans", [], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?))
        })
        .unwrap();
    assert_eq!(plan, (wrote.plan, ask.clone(), "mira".to_string()));
    let version: (i64, String, String, String, i64) = conn
        .query_row(
            "SELECT n, turn_id, title, text, id FROM plan_versions WHERE plan_id = ?1",
            [wrote.plan],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!(
        version,
        (1, "T".into(), TITLE.into(), TEXT.into(), wrote.version)
    );
    let mut stmt = conn
        .prepare(
            "SELECT n, owner, what, why, done, proof, place, size, size_lines, after_parts, after_tasks
             FROM plan_parts WHERE version_id = ?1 ORDER BY n",
        )
        .unwrap();
    type Row = (
        i64,
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
    );
    let rows: Vec<Row> = stmt
        .query_map([wrote.version], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
                r.get(8)?,
                r.get(9)?,
                r.get(10)?,
            ))
        })
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let row = |n: i64, owner: &str, what: &str, after: &str, tasks: &str| -> Row {
        (
            n,
            owner.into(),
            what.into(),
            "Notes need tags".into(),
            "A note carries its tags".into(),
            "tests/tags.rs passes".into(),
            "the project's worktree, on a new branch".into(),
            "src/tags.rs".into(),
            60,
            after.into(),
            tasks.into(),
        )
    };
    assert_eq!(
        rows,
        [
            row(1, "ann", "Add the tags table", "[]", "[]"),
            row(2, "bo", "Show tags in the list", "[1]", "[]"),
            row(3, "ann", "Filter by tag", "[2]", "[9]"),
        ]
    );

    let message: (String, String, Option<String>, String) = conn
        .query_row(
            "SELECT m.role, m.kind, m.turn_id, m.body FROM messages m
             JOIN plan_versions v ON v.message_id = m.id WHERE v.id = ?1",
            [wrote.version],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        message,
        (
            "assistant".into(),
            "text".into(),
            Some("T".into()),
            BODY.into()
        )
    );
    assert_eq!(wrote.message["body"], BODY);
    assert_eq!(
        wrote.answer,
        "Wrote version 1 of plan #1, \"Tags on notes\": 3 parts. Bo checks it in #10; the person sees it after."
    );

    let check = tasks::get(&conn, wrote.check).unwrap().unwrap();
    let seen = (
        check.kind,
        check.giver.as_deref(),
        check.owner.as_deref(),
        check.given_in.as_deref(),
        check.in_passing,
        check.checks,
        check.ask.as_deref(),
        check.what.as_str(),
    );
    assert_eq!(
        seen,
        (
            tasks::Kind::Review,
            Some("mira"),
            Some("bo"),
            Some("T"),
            true,
            Some(wrote.version),
            Some(ask.as_str()),
            "Check plan v1: Tags on notes"
        )
    );
    let (turn, brief) = queued_on(&conn, wrote.check);
    assert_eq!(
        samples::opened(&conn, &turn),
        (Some("plan_check".into()), Some(gathr::words::PLAN_CHECK))
    );
    samples::same("plan_check: first", &sampled(&conn, &brief));
    let start = wrote.starts.iter().find(|s| s.task == Some(wrote.check));
    assert_eq!(start.map(|s| s.text.as_str()), Some(brief.as_str()));
    assert_eq!(
        start.map(|s| &s.message["body"]),
        Some(&serde_json::json!(brief))
    );

    let said = events(&conn, "version");
    let message_id: String = version_field(&conn, wrote.version, "message_id");
    let expected = serde_json::json!({
        "gathr.plan.id": wrote.plan, "gathr.plan.version": 1, "gathr.ask.id": ask,
        "gathr.message.id": message_id, "gathr.plan.parts": 3, "gathr.task.check": wrote.check,
        "gathr.plan.checker": "Bo",
    });
    assert_eq!(said, [expected]);
}

/// H2.4: each refusal, in its words, writes nothing.
#[test]
fn a_plan_is_refused_saying_what_to_do() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts, archived_ts) VALUES ('old', 'Old', 1, 2), ('solo', 'Solo', 1, NULL);
         INSERT INTO chat_members VALUES ('old', 'mira', 'supervisor', 1), ('old', 'ann', 'member', 1),
             ('solo', 'mira', 'member', 1);",
    )
    .unwrap();
    let refused = |conn: &mut Connection, w: Writing, why: &str| {
        let before = counts(conn);
        assert_eq!(refusal(plans::write(conn, &w, T)), why, "{why}");
        assert_eq!(counts(conn), before, "nothing is written for: {why}");
    };
    let parts = three();
    let base = |chat, agent, turn| Writing {
        chat,
        agent,
        turn,
        title: TITLE,
        text: TEXT,
        parts: &parts,
        answers: None,
    };
    refused(
        &mut conn,
        base("old", "mira", Some("T")),
        "the chat is archived",
    );
    refused(
        &mut conn,
        base("solo", "mira", Some("T")),
        "This chat has one agent, so it has no plans.",
    );
    refused(
        &mut conn,
        base("c", "ann", Some("T")),
        "Only Mira, the chat's Supervisor, writes plans.",
    );
    refused(
        &mut conn,
        base("c", "mira", None),
        "you have no turn running in this chat",
    );
    gathr::store::add_turn(
        &conn,
        "c",
        "bare",
        T,
        (Some("mira"), None, None),
        Opened::Wake,
    )
    .unwrap();
    refused(
        &mut conn,
        base("c", "mira", Some("bare")),
        "This turn serves none of the person's asks, so a plan has nothing to belong to.",
    );
    conn.execute("UPDATE asks SET size = 'repair' WHERE id = ?1", [&ask])
        .unwrap();
    refused(
        &mut conn,
        base("c", "mira", Some("T")),
        "This ask is a repair, which goes ahead without a plan. If it needs one, size it new first.",
    );
    conn.execute("UPDATE asks SET size = 'new' WHERE id = ?1", [&ask])
        .unwrap();
    let q = question(&mut conn, "T", "Tags per note or per folder?").unwrap();
    refused(
        &mut conn,
        base("c", "mira", Some("T")),
        &format!(
            "Question #{q} to the person is open for this ask: write the plan once it's answered."
        ),
    );
    tasks::cancel(&mut conn, Some("mira"), q, "not needed", T).unwrap();
    let mut two_lines = base("c", "mira", Some("T"));
    two_lines.title = "Tags\non notes";
    refused(
        &mut conn,
        two_lines,
        "`title` is one line of 1 to 80 characters",
    );
    let mut none = base("c", "mira", Some("T"));
    none.parts = &[];
    refused(&mut conn, none, "A plan has at least one part.");

    let mut cases: Vec<(Vec<Part>, String)> = Vec::new();
    let mut no_proof = three();
    no_proof[0].proof = None;
    cases.push((
        no_proof,
        "Part 1: A work task needs why, done, proof and where. Missing: `proof`. `done` says what is true when it's \
         done, `proof` how that is shown, `where` the folder and branch the work goes in, or \"no code\"."
            .into(),
    ));
    let mut no_lines = three();
    no_lines[1].lines = None;
    cases.push((
        no_lines,
        "Part 2: a part needs size and lines: the files or area it touches, and the rough lines it adds.".into(),
    ));
    let mut zed = three();
    zed[0].to = "Zed";
    cases.push((
        zed,
        "Part 1: no agent Zed in this chat: its agents are Mira, Ann, Bo".into(),
    ));
    let mut ahead = three();
    ahead[1].after = &[3];
    cases.push((
        ahead,
        "Part 2 comes after part 3: a part comes after earlier parts only.".into(),
    ));
    let mut unknown = three();
    unknown[0].after_tasks = &[99];
    cases.push((unknown, "Part 1: there's no task #99 in this chat".into()));
    let gone = {
        let g = tasks::Give {
            chat: "c",
            giver: Some("mira"),
            owner: "ann",
            given_in: None,
            parent: None,
            kind: tasks::Kind::Work,
            what: "Old work",
            brief: common::COMPLETE,
            after: &[],
            pr: None,
            in_passing: false,
            look_into: false,
        };
        let n = tasks::give(&mut conn, &g, T).unwrap().0.number;
        tasks::cancel(&mut conn, Some("mira"), n, "not needed", T).unwrap();
        [n]
    };
    let mut cancelled = three();
    cancelled[2].after_tasks = &gone;
    cases.push((
        cancelled,
        format!("Part 3 comes after #{}, which is cancelled.", gone[0]),
    ));
    let mut twice = three();
    twice[2].what = " Add the tags table ";
    cases.push((
        twice,
        "Parts 1 and 3 are both \"Add the tags table\": give each part its own what.".into(),
    ));
    for (parts, why) in &cases {
        let mut w = base("c", "mira", Some("T"));
        w.parts = parts.as_slice();
        refused(&mut conn, w, why);
    }

    let answer = |note: i64| Answer {
        note,
        changed: true,
        why: "Fixed".into(),
    };
    let mut early = base("c", "mira", Some("T"));
    let one = [answer(1)];
    early.answers = Some(&one);
    refused(
        &mut conn,
        early,
        "`answers` go with the version after a check that left findings.",
    );

    let first = write(&mut conn, Some("T"), &three(), None).unwrap();
    refused(
        &mut conn,
        base("c", "mira", Some("T")),
        "Version 2 would be the same as version 1: change a part, or answer the findings.",
    );
    findings(&mut conn, first.check);
    let ids = note_ids(&conn, first.check);
    let mut stray = base("c", "mira", Some("T"));
    let wrong = [answer(ids[0]), answer(999)];
    stray.answers = Some(&wrong);
    refused(
        &mut conn,
        stray,
        &format!("Note 999 isn't a finding of #{}.", first.check),
    );
    let mut short = base("c", "mira", Some("T"));
    let half = [answer(ids[0])];
    short.answers = Some(&half);
    refused(
        &mut conn,
        short,
        &format!(
            "Answer each finding of #{}, Bo's check of version 1: note {} has no answer.",
            first.check, ids[1]
        ),
    );
    refused(
        &mut conn,
        base("c", "mira", Some("T")),
        &format!(
            "Answer each finding of #{}, Bo's check of version 1: notes {} and {} have no answer.",
            first.check, ids[0], ids[1]
        ),
    );
}

/// H2.7 3, H4.11.
#[test]
fn a_new_version_cancels_the_last_check_and_says_no_check_line() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let first = write(&mut conn, Some("T"), &three(), None).unwrap();
    let (queued, _) = queued_on(&conn, first.check);
    let mut next = three();
    next[1].what = "Show tags on each note";
    next.push(part("Bo", "Tag old notes", &[1]));
    let second = write(&mut conn, Some("T"), &next, None).unwrap();

    assert_eq!((second.plan, second.n), (first.plan, 2));
    let parts: i64 = conn
        .query_row(
            "SELECT count(*) FROM plan_parts WHERE version_id = ?1",
            [second.version],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(parts, 4);
    assert_eq!(
        task_state(&conn, first.check),
        (
            tasks::State::Cancelled,
            Some("version 2 replaced it".into())
        )
    );
    let stop: Option<String> = conn
        .query_row("SELECT stop_why FROM turns WHERE id = ?1", [&queued], |r| {
            r.get(0)
        })
        .unwrap();
    let why = format!(
        "its task #{} was cancelled: version 2 replaced it",
        first.check
    );
    assert_eq!(stop, Some(why));
    let line: Option<String> = version_field(&conn, first.version, "check_line");
    let sent: Option<i64> = version_field(&conn, first.version, "sent_ts");
    assert_eq!((line, sent), (None, None));
    assert_eq!(lines(&conn, "c"), Vec::<String>::new());
    let check = tasks::get(&conn, second.check).unwrap().unwrap();
    let given = (check.checks, check.owner.as_deref(), check.state);
    assert_eq!(
        given,
        (Some(second.version), Some("bo"), tasks::State::Ready)
    );
}

/// H3.1, H3.2.
#[test]
fn only_one_question_to_the_person_is_open_for_a_planned_ask() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let first = question(&mut conn, "T", "Tags per note or per folder?").unwrap();
    let before = counts(&conn);
    let second = question(&mut conn, "T", "Coloured tags?");
    assert_eq!(
        refusal(second),
        format!(
            "Question #{first} to the person is still open for this ask. Until its plan is approved, ask one at a \
             time: wait for its answer, or cancel #{first} first."
        )
    );
    assert_eq!(counts(&conn), before, "no task is left");

    // A member's request isn't a question to the person.
    gathr::store::add_turn(
        &conn,
        "c",
        "ann-T",
        T,
        (Some("ann"), None, Some("T")),
        Opened::Brief,
    )
    .unwrap();
    set_status(&conn, "ann-T", "running");
    question_by(&mut conn, "ann", "ann-T", "Which colour?").unwrap();

    let repair = asked(&mut conn, "c", "T2", "Fix the typo");
    conn.execute("UPDATE asks SET size = 'repair' WHERE id = ?1", [&repair])
        .unwrap();
    question(&mut conn, "T2", "Which typo?").unwrap();
    question(&mut conn, "T2", "Which page?").unwrap();

    let approved = asked(&mut conn, "c", "T3", "Add folders");
    conn.execute_batch(&format!(
        "INSERT INTO plans (id, chat_id, ask_id, lead, created_ts) VALUES (50, 'c', '{approved}', 'mira', 1);
         INSERT INTO plan_versions (plan_id, n, message_id, turn_id, title, text, created_ts, decided)
             VALUES (50, 1, 'x', 'T3', 'Folders', 'x', 1, 'approved');"
    ))
    .unwrap();
    question(&mut conn, "T3", "Nested folders?").unwrap();
    question(&mut conn, "T3", "Folder icons?").unwrap();

    // Gathr's own question on an ask holds neither the lead's question nor its plan.
    let gathrs = asked(&mut conn, "c", "T4", "Add search");
    conn.execute(
        "INSERT INTO tasks (chat_id, kind, what, state, form, created_ts, updated_ts, ask_id)
         VALUES ('c', 'person', 'Split PR #5?', 'ready', 'choice', 1, 1, ?1)",
        [&gathrs],
    )
    .unwrap();
    write(&mut conn, Some("T4"), &three(), None).unwrap();
    let mira = question(&mut conn, "T4", "Search titles only?").unwrap();
    let mut next = three();
    next[0].what = "Add the search index";
    assert_eq!(
        refusal(write(&mut conn, Some("T4"), &next, None)),
        format!(
            "Question #{mira} to the person is open for this ask: write the plan once it's answered."
        )
    );
}
