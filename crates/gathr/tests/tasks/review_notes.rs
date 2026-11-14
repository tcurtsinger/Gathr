//! 0053, scenario 10g: a review asks both questions, what fails against the ask and what in the
//! change the ask doesn't need. Its notes are kept at the commit reviewed; wrong or unasked notes
//! give the PR's builder a fix in passing, and a done review's suggestions go to the lead.
use super::*;
use gathr::tasks::{NoteKind, NoteSaid};

/// The head of #7 as the PR looks last saw it.
const HEAD: &str = "abc1234def5678";

fn note<'a>(kind: NoteKind, file: Option<&'a str>, says: &'a str) -> NoteSaid<'a> {
    NoteSaid { kind, file, says }
}

/// Lead's turn gives Ann the work on #7 and Bo its review: the work's number, the review's, and the
/// review's brief.
fn work_and_review(conn: &mut Connection) -> (i64, i64, String) {
    turn(conn, "lead-turn", "lead", None, "running");
    let work = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: Some("lead-turn"),
        parent: None,
        kind: Kind::Work,
        what: "Build the page.",
        why: Some("Pages load slowly."),
        after: &[],
        pr: Some(7),
        in_passing: false,
    };
    let work = tasks::give(conn, &work, T).unwrap().0.number;
    tasks::pr_looked(conn, work, "OPEN", Some(HEAD), None, T).unwrap();
    let (review, brief) = review_of(conn, Some(7));
    gathr::store::set_turn(conn, "lead-turn", "done", None).unwrap();
    tasks::turn_ended(conn, "lead-turn", T).unwrap();
    (work, review, brief)
}

/// Lead's review of `pr` for Bo, given in Lead's turn: its number and brief.
fn review_of(conn: &mut Connection, pr: Option<i64>) -> (i64, String) {
    let review = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: Some("lead-turn"),
        parent: None,
        kind: Kind::Review,
        what: "Review the page",
        why: None,
        after: &[],
        pr,
        in_passing: false,
    };
    let (task, starts) = tasks::give(conn, &review, T).unwrap();
    (task.number, starts[0].text.clone())
}

/// Bo's turn on the review: begun, reported as said, and ended through `turn_moved`, as the
/// service ends it. The turn's id and what its end starts.
fn review_turn(
    conn: &mut Connection,
    review: i64,
    how: Finish,
    said: &Said,
) -> (String, Vec<Start>) {
    let id = begin(conn, review);
    finish(conn, "bo", review, how, said).unwrap();
    (id.clone(), end(conn, &id, review))
}

fn end(conn: &mut Connection, turn: &str, review: i64) -> Vec<Start> {
    let moved = tasks::Moved {
        chat: "c",
        turn,
        agent: Some("bo"),
        task: Some(review),
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(conn, &moved, "done", None).unwrap()
}

fn left<'a>(note: &'a str, notes: &'a [NoteSaid<'a>]) -> Said<'a> {
    Said {
        note: Some(note),
        commit: Some(HEAD),
        notes,
        ..Said::default()
    }
}

fn rows(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM review_notes", [], |r| r.get(0))
        .unwrap()
}

/// What each `gathr.review.fix` event says became of a fix, oldest first.
fn outcomes(conn: &Connection) -> Vec<String> {
    let sql = "SELECT json_extract(attrs, '$.\"gathr.review.outcome\"') FROM event_log
               WHERE name = 'gathr.review.fix' ORDER BY id";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

fn tasks_named(conn: &Connection, what: &str) -> Vec<i64> {
    let sql = "SELECT number FROM tasks WHERE what = ?1 ORDER BY number";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([what], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

const UNASKED: &str = "A cache the ask doesn't need";
const FIX: &str = "Fix the review of #7";

#[test]
fn a_reviews_brief_names_its_pr_head_and_the_task_it_answers() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (work, _, brief) = work_and_review(&mut conn);
    let lines: Vec<&str> = brief.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("For a review, include the commit"))
        .expect(&brief);
    let expected = format!(
        "PR #7, at abc1234 as last seen.\n\
         It answers task #{work} from Lead: Build the page. Why: Pages load slowly.\n\
         Say what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true."
    );
    let next: Vec<&str> = lines[at + 1..].iter().take(3).copied().collect();
    assert_eq!(next.join("\n"), expected, "{brief}");
    let (_, brief) = review_of(&mut conn, Some(9));
    assert!(
        brief.contains("\nPR #9. Its head isn't seen yet.\n"),
        "{brief}"
    );
    assert!(
        brief.contains(
            "\nIt answers no task in this chat: this review's own ask, above, is what was asked.\n"
        ),
        "{brief}"
    );
    let (_, brief) = review_of(&mut conn, None);
    assert!(
        !brief.contains("PR #") && !brief.contains("It answers"),
        "{brief}"
    );
    assert!(
        brief.contains("\nSay what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true.\n"),
        "{brief}"
    );
}

#[test]
fn a_done_review_needs_nothing_unasked_and_no_wrong_or_unasked_notes() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    begin(&mut conn, review);
    let why = "A done review says nothing_unasked: true and has no wrong or unasked notes. Report those as left.";
    let silent = Said {
        commit: Some(HEAD),
        ..Said::default()
    };
    let refused = finish(&mut conn, "bo", review, Finish::Done, &silent);
    assert_eq!(refusal(refused), why);
    let notes = [note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED)];
    let with_unasked = Said {
        commit: Some(HEAD),
        nothing_unasked: true,
        notes: &notes,
        ..Said::default()
    };
    let refused = finish(&mut conn, "bo", review, Finish::Done, &with_unasked);
    assert_eq!(refusal(refused), why);
    assert_eq!(rows(&conn), 0);
}

#[test]
fn notes_go_with_a_review_and_a_commit_and_each_says_what() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (work, review, _) = work_and_review(&mut conn);
    begin(&mut conn, work);
    let notes = [note(NoteKind::Suggestion, None, "Name it plainly")];
    let on_work = Said {
        note: Some("built"),
        notes: &notes,
        ..Said::default()
    };
    let refused = finish(&mut conn, "ann", work, Finish::Done, &on_work);
    assert_eq!(
        refusal(refused),
        "`notes` and `nothing_unasked` go with a review"
    );
    let on_work = Said {
        note: Some("built"),
        nothing_unasked: true,
        ..Said::default()
    };
    let refused = finish(&mut conn, "ann", work, Finish::Done, &on_work);
    assert_eq!(
        refusal(refused),
        "`notes` and `nothing_unasked` go with a review"
    );

    begin(&mut conn, review);
    let notes = [note(NoteKind::Wrong, None, "The page still loads slowly")];
    let no_commit = Said {
        note: Some("slow"),
        notes: &notes,
        ..Said::default()
    };
    let refused = finish(&mut conn, "bo", review, Finish::Left, &no_commit);
    assert_eq!(
        refusal(refused),
        "Include the reviewed commit with your notes"
    );
    let notes = [note(NoteKind::Wrong, Some("src/page.rs"), "  ")];
    let refused = finish(&mut conn, "bo", review, Finish::Left, &left("slow", &notes));
    assert_eq!(refusal(refused), "Each note says what it is, in `says`");
    assert_eq!(rows(&conn), 0);
}

#[test]
fn new_notes_on_a_done_review_are_refused() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    let done = |notes| Said {
        commit: Some(HEAD),
        nothing_unasked: true,
        notes,
        ..Said::default()
    };
    review_turn(&mut conn, review, Finish::Done, &done(&[]));
    assert_eq!(state(&conn, review).0, State::Done);
    let notes = [note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED)];
    let why =
        format!("Review #{review} is done at abc1234. New notes need a new review: ask its giver.");
    let refused = finish(&mut conn, "bo", review, Finish::Done, &done(&notes));
    assert_eq!(refusal(refused), why);
    let refused = finish(
        &mut conn,
        "bo",
        review,
        Finish::Left,
        &left("a cache", &notes),
    );
    assert_eq!(refusal(refused), why);
    let review_done: bool = conn
        .query_row(
            "SELECT review_done FROM tasks WHERE number = ?1",
            [review],
            |r| r.get(0),
        )
        .unwrap();
    assert!(review_done, "the done review stands");
    assert_eq!(rows(&conn), 0);
    // A plain repeat of done is what it was.
    let (_, took, _) = tasks::finish(&mut conn, "bo", review, Finish::Done, &done(&[]), T).unwrap();
    assert_eq!(took, tasks::Took::Already);
}

#[test]
fn a_review_left_with_an_unasked_note_gives_its_builder_a_fix_in_passing() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    let notes = [
        note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED),
        note(NoteKind::Suggestion, None, "Name it plainly"),
    ];
    let said = left("a cache nobody asked for.", &notes);
    let (turn_id, starts) = review_turn(&mut conn, review, Finish::Left, &said);
    assert_eq!(rows(&conn), 2);
    let fixes = tasks_named(&conn, FIX);
    assert_eq!(fixes.len(), 1, "one fix");
    let fix = tasks::get(&conn, fixes[0]).unwrap().unwrap();
    assert_eq!(
        fix.why.as_deref(),
        Some(
            format!(
                "Review #{review} by Bo, at abc1234, left these notes on PR #7:\n- Unasked: src/cache.rs: {UNASKED}\n\
                 Make what is wrong work and take out what the ask doesn't need, on PR #7. Then report this task done."
            )
            .as_str()
        )
    );
    assert_eq!(
        (
            fix.owner.as_deref(),
            fix.giver.as_deref(),
            fix.parent,
            fix.pr
        ),
        (Some("ann"), Some("bo"), Some(review), Some(7))
    );
    assert_eq!(fix.given_in.as_deref(), Some(turn_id.as_str()));
    assert!(fix.in_passing, "it doesn't hold the review");
    let reason = format!(
        "left: a cache nobody asked for. Gathr gave #{} to Ann to fix it",
        fix.number
    );
    assert_eq!(state(&conn, review), (State::Blocked, Some(reason.clone())));
    let review_done: bool = conn
        .query_row(
            "SELECT review_done FROM tasks WHERE number = ?1",
            [review],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!review_done, "a left review lands nothing");
    let lead = to(&starts, "lead");
    assert_eq!(lead.len(), 1, "the lead hears trouble: {starts:?}");
    assert!(lead[0].contains(&reason), "{}", lead[0]);
    assert_eq!(to(&starts, "ann").len(), 1, "the fix starts: {starts:?}");
    assert_eq!(outcomes(&conn), ["given"]);
}

/// Bo's review left with an unasked note, and the fix Gathr gave Ann.
fn left_with_a_fix(conn: &mut Connection) -> (i64, i64) {
    let (_, review, _) = work_and_review(conn);
    let notes = [note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED)];
    review_turn(
        conn,
        review,
        Finish::Left,
        &left("a cache nobody asked for", &notes),
    );
    let fixes = tasks_named(conn, FIX);
    assert_eq!(fixes.len(), 1, "one fix");
    (review, fixes[0])
}

#[test]
fn the_fixs_end_wakes_the_reviewer_on_its_review() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (review, fix) = left_with_a_fix(&mut conn);
    let starts = work(&mut conn, fix, Some(Finish::Done));
    let held = Some("done, waiting on #7 to merge".to_string());
    assert_eq!(state(&conn, fix), (State::Held, held));
    let bo: Vec<&Start> = starts.iter().filter(|s| s.agent == "bo").collect();
    assert_eq!(bo.len(), 1, "{starts:?}");
    assert_eq!(bo[0].task, Some(review), "on the review itself");
}

#[test]
fn a_wrong_note_after_gathrs_fix_gets_a_second_fix() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (review, fix) = left_with_a_fix(&mut conn);
    let notes = [note(
        NoteKind::Wrong,
        Some("src/page.rs"),
        "The page still loads slowly",
    )];
    review_turn(&mut conn, review, Finish::Left, &left("still slow", &notes));
    assert_eq!(outcomes(&conn), ["given", "given"]);
    let fixes = tasks_named(&conn, FIX);
    assert_eq!(fixes.len(), 2, "a second fix");
    let second = tasks::get(&conn, fixes[1]).unwrap().unwrap();
    assert!(second.number > fix);
    let why = second.why.unwrap();
    assert!(
        why.contains("\n- Wrong: src/page.rs: The page still loads slowly\n"),
        "{why}"
    );
}

#[test]
fn a_refused_fix_doesnt_fail_the_turn() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    conn.execute("DELETE FROM chat_members WHERE agent_id = 'ann'", [])
        .unwrap();
    let notes = [note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED)];
    let (turn_id, _) = review_turn(&mut conn, review, Finish::Left, &left("a cache", &notes));
    let status: String = conn
        .query_row("SELECT status FROM turns WHERE id = ?1", [&turn_id], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "done");
    assert_eq!(
        state(&conn, review),
        (State::Blocked, Some("left: a cache".into()))
    );
    let outcomes = outcomes(&conn);
    assert_eq!(outcomes.len(), 1, "{outcomes:?}");
    assert!(outcomes[0].starts_with("refused: "), "{outcomes:?}");
    assert!(tasks_named(&conn, FIX).is_empty());
}

#[test]
fn a_done_reviews_suggestions_are_its_reason_and_start_no_turn() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    let notes = [
        note(NoteKind::Suggestion, Some("src/x.rs"), "Name it plainly"),
        note(NoteKind::Suggestion, None, "Split the file later"),
    ];
    let done = Said {
        note: Some("looks good"),
        commit: Some(HEAD),
        nothing_unasked: true,
        notes: &notes,
        ..Said::default()
    };
    let (_, starts) = review_turn(&mut conn, review, Finish::Done, &done);
    let reason =
        "looks good\nSuggestion: src/x.rs: Name it plainly\nSuggestion: Split the file later";
    assert_eq!(state(&conn, review), (State::Done, Some(reason.into())));
    let line = format!("A task is done:\n- #{review} Bo, Review the page: done: {reason}");
    let said: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE role = 'gathr' AND body = ?1",
            [&line],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(said, 1, "the done line");
    assert!(to(&starts, "ann").is_empty(), "{starts:?}");
    assert!(tasks_named(&conn, FIX).is_empty());
    assert!(outcomes(&conn).is_empty());
}

#[test]
fn a_second_dispute_over_unasked_code_reaches_the_lead() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (review, _) = left_with_a_fix(&mut conn);
    let notes = [note(
        NoteKind::Unasked,
        Some("src/cache.rs"),
        "The cache is still there",
    )];
    let (_, starts) = review_turn(
        &mut conn,
        review,
        Finish::Left,
        &left("the cache stays", &notes),
    );
    assert_eq!(tasks_named(&conn, FIX).len(), 1, "no second fix");
    assert_eq!(outcomes(&conn), ["given", "asked_once"]);
    let reason = "left: the cache stays".to_string();
    assert_eq!(state(&conn, review), (State::Blocked, Some(reason.clone())));
    let lead = to(&starts, "lead");
    assert_eq!(lead.len(), 1, "{starts:?}");
    assert!(lead[0].contains(&reason), "{}", lead[0]);
}

#[test]
fn no_fix_when_the_reviewer_gave_one_or_no_task_names_the_pr() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    let id = begin(&mut conn, review);
    let own = Give {
        chat: "c",
        giver: Some("bo"),
        owner: "ann",
        given_in: Some(&id),
        parent: Some(review),
        kind: Kind::Work,
        what: "Take the cache out",
        why: None,
        after: &[],
        pr: Some(7),
        in_passing: true,
    };
    tasks::give(&mut conn, &own, T).unwrap();
    let notes = [note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED)];
    finish(
        &mut conn,
        "bo",
        review,
        Finish::Left,
        &left("a cache", &notes),
    )
    .unwrap();
    end(&mut conn, &id, review);
    assert_eq!(outcomes(&conn), ["reviewer_gave_one"]);

    let (on_nine, _) = review_of(&mut conn, Some(9));
    review_turn(&mut conn, on_nine, Finish::Left, &left("a cache", &notes));
    assert_eq!(outcomes(&conn), ["reviewer_gave_one", "no_work_task"]);
    assert!(tasks_named(&conn, FIX).is_empty());
}

#[test]
fn again_adds_notes_and_a_replaced_report_isnt_shown() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (_, review, _) = work_and_review(&mut conn);
    let id = begin(&mut conn, review);
    let first = [note(NoteKind::Wrong, None, "It fails")];
    finish(
        &mut conn,
        "bo",
        review,
        Finish::Left,
        &left("fails", &first),
    )
    .unwrap();
    let second = [
        note(NoteKind::Unasked, Some("src/cache.rs"), UNASKED),
        note(NoteKind::Suggestion, None, "Name it plainly"),
    ];
    finish(
        &mut conn,
        "bo",
        review,
        Finish::Left,
        &left("a cache", &second),
    )
    .unwrap();
    assert_eq!(rows(&conn), 3);
    let shown = |conn: &Connection| -> Vec<(NoteKind, Option<String>, String)> {
        let t = tasks::get(conn, review).unwrap().unwrap();
        t.notes
            .into_iter()
            .map(|n| (n.kind, n.file, n.says))
            .collect()
    };
    assert_eq!(
        shown(&conn),
        [
            (
                NoteKind::Unasked,
                Some("src/cache.rs".into()),
                UNASKED.into()
            ),
            (NoteKind::Suggestion, None, "Name it plainly".into())
        ]
    );
    let done = |commit, notes| Said {
        commit: Some(commit),
        nothing_unasked: true,
        notes,
        ..Said::default()
    };
    let d = [note(NoteKind::Suggestion, None, "Later, a smaller page")];
    finish(&mut conn, "bo", review, Finish::Done, &done(HEAD, &d)).unwrap();
    end(&mut conn, &id, review);
    assert_eq!(rows(&conn), 4);
    assert_eq!(
        shown(&conn),
        [(NoteKind::Suggestion, None, "Later, a smaller page".into())]
    );
    // Already recorded: nothing is added.
    let e = [note(NoteKind::Suggestion, None, "Another idea")];
    let (_, took, _) =
        tasks::finish(&mut conn, "bo", review, Finish::Done, &done(HEAD, &e), T).unwrap();
    assert_eq!(took, tasks::Took::Already);
    assert_eq!(rows(&conn), 4);
    // Done again at a newer commit: its notes are added, and shown.
    let f = [note(
        NoteKind::Suggestion,
        Some("src/page.rs"),
        "Cut the comment",
    )];
    let again = done("def5678", &f);
    let (_, took, _) = tasks::finish(&mut conn, "bo", review, Finish::Done, &again, T).unwrap();
    assert_eq!(took, tasks::Took::Applied);
    assert_eq!(rows(&conn), 5);
    assert_eq!(
        shown(&conn),
        [(
            NoteKind::Suggestion,
            Some("src/page.rs".into()),
            "Cut the comment".into()
        )]
    );
}
