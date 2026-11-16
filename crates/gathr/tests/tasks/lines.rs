//! 0064, J2 and J3: a building task's line comes from its ask's size and its own lines, and a Fix's
//! line from what its project's landed Fixes added.
use super::*;
use gathr::changes::count::Count;
use gathr::changes::{self, Measure, report};

const H: &str = "cccc111122223333";

/// Chat `c` in project `p`; project `q` holds no chat.
fn in_projects(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Site', 'site', 'x', 1),
             ('q', 'Other', 'other', 'y', 1);
         UPDATE chats SET project_id = 'p';",
    )
    .unwrap();
}

/// Puts `task` on an ask of `size` (none: unsized).
fn on_ask(conn: &Connection, task: i64, size: Option<&str>) {
    let id = format!("ask-{task}");
    conn.execute(
        "INSERT INTO asks (id, chat_id, size) VALUES (?1, 'c', ?2)",
        params![id, size],
    )
    .unwrap();
    conn.execute(
        "UPDATE tasks SET ask_id = ?2 WHERE number = ?1",
        params![task, id],
    )
    .unwrap();
}

/// Ann's work on PR `pr`, on an ask of `ask` when given, measured at `added` lines in `project`
/// and, when `landed`, merged.
fn change(
    conn: &mut Connection,
    (project, pr): (&str, i64),
    ask: Option<Option<&str>>,
    added: i64,
    landed: bool,
) {
    let work = work_on_pr(conn, pr);
    if let Some(size) = ask {
        on_ask(conn, work, size);
    }
    let m = Measure {
        project: project.into(),
        chat: "c".into(),
        pr,
        head: H.into(),
        count: Count {
            added,
            removed: 0,
            top: Vec::new(),
        },
        line: 3000,
        line_of: "project",
        by: "landing",
    };
    changes::measured(conn, &m, T).unwrap();
    if landed {
        changes::ended(conn, "c", pr, "MERGED", Some(H), false, T).unwrap();
    }
}

/// A repair's landed change of `added` lines in project `p`, on PR `pr`.
fn fix(conn: &mut Connection, pr: i64, added: i64) {
    change(conn, ("p", pr), Some(Some("repair")), added, true);
}

/// Work given to Ann on an ask of `ask` (none: no ask), with `lines` in its brief.
fn sized(conn: &mut Connection, ask: Option<Option<&str>>, lines: Option<i64>) -> i64 {
    let brief = Brief {
        size: lines.map(|n| ("src/export.rs", n)),
        ..common::COMPLETE
    };
    let g = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix the export",
        brief,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: false,
    };
    let task = tasks::give(conn, &g, T).unwrap().0.number;
    if let Some(size) = ask {
        on_ask(conn, task, size);
    }
    task
}

fn line(conn: &Connection, task: i64) -> Option<(i64, &'static str)> {
    report::task_line(conn, task).unwrap()
}

#[test]
fn a_fixs_line_is_the_most_of_its_projects_last_50_landed_fixes_never_under_1000() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    let probe = sized(&mut conn, Some(Some("repair")), None);
    for pr in 1..=9 {
        fix(&mut conn, pr, 400);
    }
    assert_eq!(
        line(&conn, probe),
        Some((1000, "fix")),
        "9 Fixes are too few"
    );
    // None of these is a landed Fix of this project: each would be a tenth, at 2,000.
    change(&mut conn, ("p", 20), Some(Some("new")), 2000, true);
    change(&mut conn, ("p", 21), Some(None), 2000, true);
    change(&mut conn, ("p", 22), None, 2000, true);
    change(&mut conn, ("q", 23), Some(Some("repair")), 2000, true);
    change(&mut conn, ("p", 24), Some(Some("repair")), 2000, false);
    assert_eq!(line(&conn, probe), Some((1000, "fix")));
    fix(&mut conn, 10, 400);
    assert_eq!(line(&conn, probe), Some((1000, "fix")), "never under 1,000");
    fix(&mut conn, 11, 1400);
    assert_eq!(line(&conn, probe), Some((1400, "fix")));
    for pr in 100..149 {
        fix(&mut conn, pr, 400);
    }
    assert_eq!(line(&conn, probe), Some((1400, "fix")), "the 50th newest");
    fix(&mut conn, 149, 400);
    assert_eq!(line(&conn, probe), Some((1000, "fix")), "the 51st newest");
}

#[test]
fn a_landed_change_records_its_asks_size() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    change(&mut conn, ("p", 5), Some(Some("repair")), 100, true);
    change(&mut conn, ("p", 6), Some(Some("new")), 100, true);
    change(&mut conn, ("p", 7), None, 100, true);
    let sizes: Vec<(i64, Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT c.pr, c.size, (SELECT e.attrs ->> '$.\"gathr.ask.size\"' FROM event_log e
                 WHERE e.name = 'gathr.change.landed' AND e.attrs ->> '$.pr' = c.pr)
             FROM changes c ORDER BY c.pr",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let text = |s: &str| Some(s.to_string());
    assert_eq!(
        sizes,
        [
            (5, text("repair"), text("repair")),
            (6, text("new"), text("new")),
            (7, None, None)
        ]
    );
}

#[test]
fn a_tasks_line_follows_its_asks_size_and_its_lines() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    let cases = [
        (Some(Some("repair")), None, Some((1000, "fix"))),
        (Some(Some("repair")), Some(100), Some((1000, "fix"))),
        (Some(Some("new")), Some(100), Some((400, "part"))),
        (Some(Some("new")), Some(300), Some((900, "part"))),
        (Some(None), Some(50), Some((350, "part"))),
        (Some(Some("new")), None, None),
        (None, Some(100), None),
    ];
    for (ask, lines, expected) in cases {
        let task = sized(&mut conn, ask, lines);
        assert_eq!(line(&conn, task), expected, "{ask:?} with {lines:?}");
    }
}
