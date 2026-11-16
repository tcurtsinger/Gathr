//! 0054, scenario 10h: a change past its project's line doesn't land until the person answers, and
//! every change that merges is recorded.
use super::*;
use gathr::changes::count::{self, Count};
use gathr::changes::{self, Measure};
use gathr::tasks::person::{self, Answer};
use gathr::waiting::{self, Moves};

/// `git diff --numstat --no-renames -z main...pr` from git 2.55: 2,000 lines in src, a lock file
/// and one under ui, a binary, a generated file and a vendored one.
const DIFF: &str = "8700\t0\tCargo.lock\x00300\t0\tgen/api.rs\x00-\t-\tlogo.png\x001900\t20\tsrc/a.rs\x00\
                    100\t0\tsrc/b.rs\x0050\t0\tui/package-lock.json\x00200\t0\tvendor/lib.js\x00";
/// `git check-attr -z --source=main linguist-generated linguist-vendored -- <paths>` for the same.
const ATTRS: &str = "src/a.rs\x00linguist-generated\x00unspecified\x00src/a.rs\x00linguist-vendored\x00\
                     unspecified\x00gen/api.rs\x00linguist-generated\x00set\x00gen/api.rs\x00linguist-vendored\x00\
                     unspecified\x00vendor/lib.js\x00linguist-generated\x00unspecified\x00vendor/lib.js\x00\
                     linguist-vendored\x00true\x00";
/// `git log --first-parent --diff-merges=first-parent -n 50 --numstat --no-renames -z
/// --format=%x01%H pr` for the same repository: the PR's commit, then the base's.
const LOG: &str = "\x017bea7f368810789ae16b55a6a79c4314c936bf34\x00\n8700\t0\tCargo.lock\x00300\t0\tgen/api.rs\x00\
                   -\t-\tlogo.png\x001900\t20\tsrc/a.rs\x00100\t0\tsrc/b.rs\x0050\t0\tui/package-lock.json\x00\
                   200\t0\tvendor/lib.js\x00\x013c77cb9590bd1cdcdebdd979f32886de592fa572\x00\n2\t0\t.gitattributes\x00\
                   300\t0\tCargo.lock\x0020\t0\tsrc/a.rs\x00";

const H1: &str = "aaaa111122223333";
const H2: &str = "bbbb111122223333";
const GO_ON: &str = "Let it go on";
const SEND_BACK: &str = "Send it back";

#[test]
fn a_change_counts_added_lines_but_not_lock_binary_or_generated_files() {
    let files = count::numstat(DIFF);
    assert_eq!(files.len(), 7, "{files:?}");
    assert_eq!(files[2].added, None, "a binary: {:?}", files[2]);
    let marked = count::marked(ATTRS);
    assert_eq!(marked, ["gen/api.rs", "vendor/lib.js"]);
    let counted = count::counted(&files, &marked);
    let top = vec![
        ("src/a.rs".to_string(), 1900),
        ("src/b.rs".to_string(), 100),
    ];
    assert_eq!(
        counted,
        Count {
            added: 2000,
            removed: 20,
            top
        }
    );
}

/// `count` commits of the base, each adding `each` lines, newest first, in git's log form.
fn log_of(each: &[i64]) -> String {
    let commits = each.iter().enumerate();
    let one = |(i, n): (usize, &i64)| format!("\x01{i:040}\x00\n{n}\t0\tsrc/f{i}.rs\x00");
    commits.map(one).collect()
}

fn line_of(log: &str) -> i64 {
    let commits: Vec<Count> = count::history(log)
        .iter()
        .map(|files| count::counted(files, &[]))
        .collect();
    count::line(&commits)
}

#[test]
fn the_line_is_the_largest_of_the_last_50_and_never_under_3000() {
    let commits = count::history(LOG);
    assert_eq!(commits.len(), 2, "{commits:?}");
    let marked = count::marked(ATTRS);
    let added: Vec<i64> = commits
        .iter()
        .map(|c| count::counted(c, &marked).added)
        .collect();
    assert_eq!(added, [2000, 22]);
    assert_eq!(line_of(LOG), 3000, "fewer than 10 changes");
    assert_eq!(line_of(&log_of(&[100; 9])), 3000);
    let mut fifty = vec![100; 49];
    fifty.insert(20, 9000);
    assert_eq!(line_of(&log_of(&[9000; 9])), 3000, "9 is too few");
    assert_eq!(line_of(&log_of(&fifty)), 9000);
    assert!(!count::over(9000, 9000, None));
    assert!(count::over(9001, 9000, None));
    assert!(!count::over(6200, 3000, Some(3200)));
    assert!(count::over(6201, 3000, Some(3200)));
    assert_eq!(count::grouped(1_234_567), "1,234,567");
    assert_eq!(count::grouped(999), "999");
}

/// The chats in project `p`, as landing needs.
fn in_project(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Site', 'site', 'x', 1);
         UPDATE chats SET project_id = 'p';",
    )
    .unwrap();
}

/// A measure of PR `pr` in chat `c` at landing.
fn measure(pr: i64, head: &str, added: i64, line: i64) -> Measure {
    let top = vec![
        ("src/big.rs".to_string(), added - 40),
        ("src/b.rs".to_string(), 40),
    ];
    Measure {
        project: "p".into(),
        chat: "c".into(),
        pr,
        head: head.into(),
        count: Count {
            added,
            removed: 12,
            top,
        },
        line,
        line_of: "project",
        by: "landing",
    }
}

fn measured(conn: &mut Connection, m: &Measure) -> Option<String> {
    changes::measured(conn, m, T).unwrap().0
}

/// Gathr's open questions to the person.
fn questions(conn: &Connection) -> Vec<tasks::Task> {
    let open = person::open(conn, None).unwrap().into_iter();
    open.map(|o| o.task).filter(|t| t.giver.is_none()).collect()
}

fn count_of(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn events(conn: &Connection, name: &str) -> i64 {
    let sql = "SELECT count(*) FROM event_log WHERE name = ?1";
    conn.query_row(sql, [name], |r| r.get(0)).unwrap()
}

/// Ann's work on #5 and Gathr's question at +3,200: the work and the question.
fn asked(conn: &mut Connection, pr: i64) -> (i64, i64) {
    let work = work_on_pr(conn, pr);
    let rests = measured(conn, &measure(pr, H1, 3200, 3000));
    let q = questions(conn).last().expect("a question").number;
    assert_eq!(rests, Some(format!("waits on the person's answer to #{q}")));
    (work, q)
}

fn answer(conn: &mut Connection, q: i64, pick: &[&str], by: Option<&str>) -> person::Sent {
    let a = Answer {
        task: q,
        pick: pick.iter().map(|p| p.to_string()).collect(),
        ..Answer::default()
    };
    person::answer_as(conn, &[a], by, T).unwrap()
}

#[test]
fn an_over_change_is_asked_at_landing_once_with_its_numbers_and_task() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    let work = work_on_pr(&mut conn, 5);
    let before = state(&conn, work);
    let turns = count_of(&conn, "SELECT count(*) FROM turns");
    let rests = measured(&mut conn, &measure(5, H1, 3200, 3000));
    let asked = questions(&conn);
    assert_eq!(asked.len(), 1, "{asked:?}");
    let q = &asked[0];
    assert_eq!(
        rests,
        Some(format!("waits on the person's answer to #{}", q.number))
    );
    assert_eq!(
        (q.owner.as_deref(), q.state, q.pr, q.parent),
        (None, State::Ready, Some(5), Some(work))
    );
    assert_eq!(
        q.what,
        "#5 adds 3,200 lines, over this project's line of 3,000: let it go on?"
    );
    let why = format!(
        "#5 adds 3,200 lines and removes 12. This project's line is 3,000, the least it can be.\n\
         Largest: src/big.rs +3,160, src/b.rs +40.\n\
         It answers task #{work} from Lead: Fix the summary."
    );
    assert_eq!(q.why.as_deref(), Some(why.as_str()));
    let choices: Vec<(&str, Option<&str>)> = q
        .choices
        .iter()
        .map(|c| (c.label.as_str(), c.detail.as_deref()))
        .collect();
    let blocked = format!("#{work} is blocked, and Lead is told.");
    assert_eq!(
        choices,
        [
            (GO_ON, Some("It lands once its review and checks pass.")),
            (SEND_BACK, Some(blocked.as_str()))
        ]
    );
    assert_eq!(state(&conn, work), before, "the work is untouched");
    assert_eq!(
        count_of(&conn, "SELECT count(*) FROM turns"),
        turns,
        "nothing starts"
    );
    assert_eq!(events(&conn, "gathr.change.asked"), 1);

    let again = measured(&mut conn, &measure(5, H1, 3200, 3000));
    assert_eq!(again, rests, "it still rests on the same question");
    assert_eq!(questions(&conn).len(), 1, "asked once");
    assert_eq!(
        events(&conn, "gathr.change.measured"),
        1,
        "same head and line: no write"
    );

    // With no work task on its PR, the why and the choice say so; a line set by a change says so.
    measured(&mut conn, &measure(8, H1, 9500, 9000));
    let q = questions(&conn).pop().unwrap();
    let why = q.why.unwrap();
    assert!(
        why.starts_with(
            "#8 adds 9,500 lines and removes 12. This project's line is 9,000: the most any of its last 50 changes added.\n"
        ),
        "{why}"
    );
    assert!(
        why.ends_with("\nNo task in this chat asks for it."),
        "{why}"
    );
    assert_eq!(
        q.choices[1].detail.as_deref(),
        Some("It doesn't land at this head.")
    );
}

#[test]
fn a_smaller_head_withdraws_the_question() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    let (_, q) = asked(&mut conn, 5);
    let rests = measured(&mut conn, &measure(5, H2, 500, 3000));
    assert_eq!(rests, None, "it lands");
    let why = Some("#5 is now +500, under this project's line of 3,000".to_string());
    assert_eq!(state(&conn, q), (State::Cancelled, why));
    assert_eq!(events(&conn, "gathr.change.withdrawn"), 1);
    assert!(questions(&conn).is_empty());
}

#[test]
fn a_cancelled_question_rests_until_a_new_head() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    let (_, q) = asked(&mut conn, 5);
    tasks::cancel(&mut conn, None, q, "not now", T).unwrap();
    let rests = measured(&mut conn, &measure(5, H1, 3200, 3000));
    assert_eq!(
        rests,
        Some(format!("waits on a new head: the person cancelled #{q}"))
    );
    assert!(questions(&conn).is_empty(), "nothing asked again");
    let rests = measured(&mut conn, &measure(5, H2, 3300, 3000));
    let q2 = questions(&conn);
    assert_eq!(q2.len(), 1, "a new head is asked: {q2:?}");
    assert_eq!(
        rests,
        Some(format!("waits on the person's answer to #{}", q2[0].number))
    );
}

/// The `pr` line of chat `c` as the waiting answer gives it.
fn pr_line(conn: &Connection) -> (String, Moves) {
    let answer = waiting::answer(conn, "c", &waiting::Live::default(), 1).unwrap();
    let line = answer.lines.iter().find(|l| l.kind == "pr");
    let line = line.unwrap_or_else(|| panic!("no pr line: {:?}", answer.lines));
    (line.what.clone(), line.moves)
}

/// Bo's review of #5 for Ann, reported done.
fn review_done(conn: &mut Connection) -> i64 {
    turn(conn, "giving", "ann", None, "done");
    let review = review_by_bo(conn, 5, "giving");
    reviewed(conn, review);
    review
}

#[test]
fn while_asked_the_pr_waits_on_you() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    review_done(&mut conn);
    let (_, q) = asked(&mut conn, 5);
    let expected = format!("#5 to land: waits on your answer to #{q}");
    assert_eq!(pr_line(&conn), (expected, Moves::You));
    tasks::cancel(&mut conn, None, q, "not now", T).unwrap();
    let expected = format!("#5 to land: you cancelled #{q}; waits on a new head");
    assert_eq!(pr_line(&conn), (expected, Moves::Others));
}

#[test]
fn only_the_person_answers_gathrs_question_and_only_with_one_pick() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    let (_, q) = asked(&mut conn, 5);
    let by_ann = answer(&mut conn, q, &[GO_ON], Some("ann"));
    let only = format!("#{q} is Gathr's question to the person: only the person answers it");
    assert_eq!(by_ann.refused, [(q, only)]);
    let one = format!("Answer #{q} with one of its choices");
    let words = Answer {
        task: q,
        text: Some("fine by me".into()),
        ..Answer::default()
    };
    let in_words = person::answer(&mut conn, &[words], T).unwrap();
    assert_eq!(in_words.refused, [(q, one.clone())]);
    let both = answer(&mut conn, q, &[GO_ON, SEND_BACK], None);
    assert_eq!(both.refused, [(q, one)]);
    let other = answer(&mut conn, q, &["Ship it"], None);
    assert_eq!(other.refused, [(q, format!("#{q} has no choice Ship it"))]);
    let refusals =
        "SELECT count(*) FROM event_log WHERE name = 'gathr.task.answer' AND status = 'error'";
    assert_eq!(
        count_of(&conn, refusals),
        4,
        "each refusal has its reference"
    );
    let answered = answer(&mut conn, q, &[GO_ON], None);
    assert!(answered.refused.is_empty(), "{:?}", answered.refused);
    assert_eq!(state(&conn, q).0, State::Done);
}

fn row(conn: &Connection, pr: i64) -> (Option<i64>, Option<i64>, Option<i64>) {
    let sql = "SELECT question, asked, let_through FROM changes WHERE pr = ?1";
    conn.query_row(sql, [pr], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
}

#[test]
fn let_it_go_on_lets_the_count_shown_through_and_asks_again_past_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    // A bigger head before the answer: it rests on the open question, and past the count shown
    // "Let it go on" asks again.
    let (_, q) = asked(&mut conn, 5);
    let rests = measured(&mut conn, &measure(5, H2, 6201, 3000));
    assert_eq!(rests, Some(format!("waits on the person's answer to #{q}")));
    let sent = answer(&mut conn, q, &[GO_ON], None);
    assert!(sent.starts.is_empty(), "{:?}", sent.starts);
    let q2 = questions(&conn);
    assert_eq!(q2.len(), 1, "{q2:?}");
    assert!(
        q2[0].what.starts_with("#5 adds 6,201 lines"),
        "{}",
        q2[0].what
    );
    assert_eq!(row(&conn, 5), (Some(q2[0].number), Some(6201), Some(3200)));
    assert_eq!(events(&conn, "gathr.change.answered"), 1);

    let (_, q) = asked(&mut conn, 6);
    measured(&mut conn, &measure(6, H2, 6200, 3000));
    answer(&mut conn, q, &[GO_ON], None);
    assert_eq!(row(&conn, 6), (None, Some(3200), Some(3200)));
    assert_eq!(questions(&conn).len(), 1, "only #5's");
    assert_eq!(
        measured(&mut conn, &measure(6, H2, 6200, 3000)),
        None,
        "it lands"
    );
}

#[test]
fn send_it_back_blocks_the_work_until_its_giver_restarts_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    review_done(&mut conn);
    let work = held_on_its_merge(&mut conn, 5);
    measured(&mut conn, &measure(5, H1, 3200, 3000));
    let q = questions(&conn)[0].number;
    let sent = answer(&mut conn, q, &[SEND_BACK], None);
    let why = format!(
        "The person sent #5 back as bigger than asked: +3,200 lines; this project's line is 3,000. \
         Narrow #{work}'s ask and restart it, or cancel it."
    );
    assert_eq!(state(&conn, work), (State::Blocked, Some(why.clone())));
    assert_eq!(to(&sent.starts, "lead").len(), 1, "{:?}", sent.starts);
    let expected = "#5 to land: you sent it back; waits on a new head".to_string();
    assert_eq!(pr_line(&conn), (expected, Moves::Others));
    // The reason ends its own sentence: what the chat waits on adds no second full stop.
    let answer = waiting::answer(&conn, "c", &waiting::Live::default(), 1).unwrap();
    let trouble: Vec<&str> = (answer.lines.iter())
        .filter(|l| l.kind == "trouble")
        .map(|l| l.what.as_str())
        .collect();
    assert_eq!(trouble.len(), 1, "{trouble:?}");
    assert!(
        trouble[0].ends_with("or cancel it. Lead hears of it next")
            || trouble[0].ends_with("or cancel it. Lead was told"),
        "{trouble:?}"
    );
    let rests = measured(&mut conn, &measure(5, H1, 3200, 3000));
    assert_eq!(
        rests.as_deref(),
        Some("was sent back by the person at aaaa111")
    );
    assert!(
        questions(&conn).is_empty(),
        "the head sent back isn't asked again"
    );

    let restarted = tasks::restart(&mut conn, Some("lead"), work, T).unwrap();
    assert_eq!(restarted.len(), 1, "{restarted:?}");
    let carried = format!("Continue from here: {why} Check what is already done");
    assert!(
        restarted[0].text.contains(&carried),
        "{}",
        restarted[0].text
    );
    // A new head is measured: over, it's asked again.
    measured(&mut conn, &measure(5, H2, 3300, 3000));
    assert_eq!(questions(&conn).len(), 1);

    // Every blocked task carries its reason into its restart.
    let (task, _) = give(&mut conn, "bo", &[], None);
    let said = Said {
        note: Some("needs the lead's call on naming"),
        ..Said::default()
    };
    begin(&mut conn, task);
    finish(&mut conn, "bo", task, Finish::Blocked, &said).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Blocked);
    let restarted = tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    let text = &restarted[0].text;
    assert!(
        text.contains("Continue from here: needs the lead's call on naming."),
        "{text}"
    );
}

/// The record's columns for a PR, as text.
fn record(conn: &Connection, pr: i64) -> Vec<Option<String>> {
    let sql = "SELECT builder_id, builder_engine, builder_model, reviewer_id, reviewer_engine, reviewer_model,
                      CAST(unasked AS TEXT), first_reviewed, CAST(by_gathr AS TEXT), CAST(landed_ts > 0 AS TEXT)
               FROM changes WHERE pr = ?1";
    conn.query_row(sql, [pr], |r| (0..10).map(|i| r.get(i)).collect())
        .unwrap()
}

#[test]
fn a_pr_ending_outside_gathr_ends_its_question_and_merged_is_recorded_once() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_project(&conn);
    conn.execute("UPDATE agents SET model = 'opus' WHERE id = 'ann'", [])
        .unwrap();
    let (_, q) = asked(&mut conn, 5);
    // Bo's review leaves an unasked note at one commit, then is done at another.
    turn(&conn, "giving", "ann", None, "done");
    let review = review_by_bo(&mut conn, 5, "giving");
    begin(&mut conn, review);
    let notes = [tasks::NoteSaid {
        kind: tasks::NoteKind::Unasked,
        file: None,
        says: "A cache nobody asked for",
    }];
    let left = Said {
        note: Some("one thing"),
        commit: Some("abc1234"),
        notes: &notes,
        ..Said::default()
    };
    finish(&mut conn, "bo", review, Finish::Left, &left).unwrap();
    let done = Said {
        commit: Some("def5678"),
        nothing_unasked: true,
        ..Said::default()
    };
    finish(&mut conn, "bo", review, Finish::Done, &done).unwrap();

    changes::ended(&mut conn, "c", 5, "MERGED", Some(H1), false, T).unwrap();
    assert_eq!(
        state(&conn, q),
        (State::Cancelled, Some("#5 merged".into()))
    );
    let text = |s: &str| Some(s.to_string());
    let expected = vec![
        text("ann"),
        text("claude"),
        text("opus"),
        text("bo"),
        text("claude"),
        None,
        text("1"),
        text("abc1234"),
        text("0"),
        text("1"),
    ];
    assert_eq!(record(&conn, 5), expected);
    assert_eq!(events(&conn, "gathr.change.merged_unasked"), 1);
    assert_eq!(events(&conn, "gathr.change.landed"), 1);
    changes::ended(&mut conn, "c", 5, "MERGED", Some(H1), false, T).unwrap();
    assert_eq!(events(&conn, "gathr.change.landed"), 1, "recorded once");

    // Closed, its row goes.
    measured(&mut conn, &measure(6, H1, 100, 3000));
    changes::ended(&mut conn, "c", 6, "CLOSED", Some(H1), false, T).unwrap();
    assert_eq!(
        count_of(&conn, "SELECT count(*) FROM changes WHERE pr = 6"),
        0
    );
    assert_eq!(events(&conn, "gathr.change.closed"), 1);
    // A look that sees Gathr's push merged before Gathr settles it still records it as Gathr's;
    // with no push of Gathr's, the same look records it as by hand.
    measured(&mut conn, &measure(7, H1, 100, 3000));
    changes::pushed(&conn, "p", 7).unwrap();
    changes::ended(&mut conn, "c", 7, "MERGED", Some(H1), false, T).unwrap();
    assert_eq!(record(&conn, 7)[8], text("1"), "pushed by Gathr");
    measured(&mut conn, &measure(8, H1, 100, 3000));
    changes::ended(&mut conn, "c", 8, "MERGED", Some(H1), false, T).unwrap();
    assert_eq!(record(&conn, 8)[8], text("0"), "merged by hand");
}
