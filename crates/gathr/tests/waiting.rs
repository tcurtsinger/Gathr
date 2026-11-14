//! 0050: one answer to what a chat waits on, from saved state plus what the running service knows.
//! Scenarios 6a and 6b on a real database, and two of the room's incidents: everyone waited on a CI
//! job no runner took and nothing said so, and a wait whose look kept failing went unseen.

mod common;

use std::collections::HashMap;

use common::TempDir;
use gathr::tasks::{self, Finish, Give, Kind, Outcome, Said};
use gathr::waiting::{self, Answer, Line, Live, Moves};
use gathr::waits::{self, Leave, Part};
use rusqlite::{Connection, params};

const T: &str = "test-trace";

/// A project's chat with a Supervisor (Lead), Ann, Bo and Cy, who has nothing to do.
fn team(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1),
             ('bo', 'Bo', 'bo', 'acc', 1), ('cy', 'Cy', 'cy', 'acc', 1);
         INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Site', 'site', 'x', 1);
         INSERT INTO chats (id, title, created_ts, project_id) VALUES ('c', 'Chat', 1, 'p');
         INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1),
             ('c', 'bo', 'member', 1), ('c', 'cy', 'member', 1);",
    )
    .unwrap();
    conn
}

fn turn(conn: &Connection, id: &str, agent: &str, task: Option<i64>, status: &str) {
    gathr::store::add_turn(conn, "c", id, T, (Some(agent), task, None)).unwrap();
    gathr::store::set_turn(conn, id, status, None).unwrap();
}

fn give(conn: &mut Connection, owner: &str, after: &[i64], given_in: &str) -> i64 {
    let g = Give {
        chat: "c",
        giver: Some("lead"),
        owner,
        given_in: Some(given_in),
        parent: None,
        kind: Kind::Work,
        what: "Fix the build",
        why: None,
        after,
        pr: None,
        in_passing: false,
    };
    tasks::give(conn, &g, T).unwrap().0.number
}

/// The turn Gathr queued on the task, running now.
fn begin(conn: &mut Connection, task: i64) -> String {
    let sql = "SELECT id FROM turns WHERE task = ?1 AND status = 'queued' ORDER BY rowid LIMIT 1";
    let id: String = conn.query_row(sql, [task], |r| r.get(0)).unwrap();
    gathr::store::set_turn(conn, &id, "running", None).unwrap();
    tasks::started(conn, task, T).unwrap();
    id
}

/// The service as it runs, knowing nothing more than the database.
fn service() -> Live {
    Live {
        running: true,
        ..Live::default()
    }
}

fn answer(conn: &Connection, live: &Live) -> Answer {
    waiting::answer(conn, "c", live, gathr::ids::now_us()).unwrap()
}

fn line<'a>(answer: &'a Answer, kind: &str, has: &str) -> &'a Line {
    let found = answer
        .lines
        .iter()
        .find(|l| l.kind == kind && l.what.contains(has));
    found.unwrap_or_else(|| panic!("no {kind} line with \"{has}\" in {:#?}", answer.lines))
}

/// Scenario 6a: one answer names each agent with its task and whether it works, waits or is free,
/// a free agent included. An agent with an approval waiting and a turn running needs the person.
#[test]
fn one_answer_names_each_agent_with_its_task_and_whether_it_works_waits_or_is_free() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "lead", None, "done");
    let first = give(&mut conn, "ann", &[], "giving");
    give(&mut conn, "bo", &[first], "giving");
    let running = begin(&mut conn, first);

    let said = answer(&conn, &service());
    let stands: Vec<(&str, &str, &str, Option<i64>)> = said
        .agents
        .iter()
        .map(|a| (a.name.as_str(), a.state, a.words.as_str(), a.task))
        .collect();
    assert_eq!(
        stands,
        [
            ("Lead", "waiting", "Waiting for Ann and Bo", None),
            ("Ann", "working", "Working", Some(first)),
            ("Bo", "waiting", "Waiting for Ann", None),
            ("Cy", "free", "Ready", None),
        ]
    );
    let lead = &said.agents[0];
    assert_eq!(lead.on.len(), 2, "whom it waits for: {lead:?}");
    assert!(
        lead.on.iter().all(|on| on.since_us > 0),
        "since when: {lead:?}"
    );

    let asked = Live {
        approvals: vec![running],
        ..service()
    };
    let said = answer(&conn, &asked);
    let ann = &said.agents[1];
    assert_eq!(
        (ann.state, ann.words.as_str()),
        ("needs_you", "Needs your approval")
    );
    assert_eq!(line(&said, "approval", "Ann").moves, Moves::You);
}

/// Scenario 6b: one answer lists every hold, each with who can end it and since when.
#[test]
fn one_answer_lists_every_hold_with_who_can_end_it_and_since_when() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "lead", None, "done");
    // Ann works on #1; Bo's #2 comes after it; Ann's #3 is queued behind her own turn.
    let first = give(&mut conn, "ann", &[], "giving");
    let second = give(&mut conn, "bo", &[first], "giving");
    let third = give(&mut conn, "ann", &[], "giving");
    begin(&mut conn, first);
    let queued: String = conn
        .query_row("SELECT id FROM turns WHERE task = ?1", [third], |r| {
            r.get(0)
        })
        .unwrap();
    // Cy's #4 is blocked, and Lead has heard; Cy's #5 is ready with no turn on it.
    let blocked = give(&mut conn, "cy", &[], "giving");
    begin(&mut conn, blocked);
    let left = Said {
        note: Some("the second file"),
        ..Said::default()
    };
    tasks::finish(&mut conn, "cy", blocked, Finish::Left, &left, T).unwrap();
    let ended: String = conn
        .query_row("SELECT id FROM turns WHERE task = ?1", [blocked], |r| {
            r.get(0)
        })
        .unwrap();
    gathr::store::set_turn(&conn, &ended, "done", None).unwrap();
    tasks::ended(&mut conn, blocked, Outcome::Ok, T).unwrap();
    conn.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [blocked])
        .unwrap();
    let idle = give(&mut conn, "cy", &[], "giving");
    conn.execute("DELETE FROM turns WHERE task = ?1", [idle])
        .unwrap();
    // A question for the person, a background command of Ann's, and Gathr updating.
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts)
         VALUES ('c', 'lead', 'person', 'Pick a colour', 'ready', 5, 5)",
        [],
    )
    .unwrap();
    let ann_turn: String = conn
        .query_row("SELECT id FROM turns WHERE task = ?1", [first], |r| {
            r.get(0)
        })
        .unwrap();
    conn.execute(
        "INSERT INTO background (id, turn_id, description, started_ts) VALUES ('b1', ?1, 'npm run build', 7)",
        [&ann_turn],
    )
    .unwrap();

    let live = Live {
        queued: HashMap::from([(queued.clone(), "behind Ann's turn on #1".to_string())]),
        updating: Some("0.1.140".into()),
        ..service()
    };
    let said = answer(&conn, &live);
    let stands = |kind: &str, has: &str| {
        let found = line(&said, kind, has);
        assert!(found.since_us.is_some(), "since when: {found:?}");
        (found.moves, found.from)
    };
    assert_eq!(
        stands("turn", "Ann is working on #1"),
        (Moves::Itself, "saved")
    );
    assert_eq!(
        stands("queued", "behind Ann's turn on #1"),
        (Moves::Itself, "service")
    );
    assert_eq!(
        stands("after", &format!("#{second} (Bo) waits for #{first} (Ann)")),
        (Moves::Others, "saved")
    );
    assert_eq!(
        stands(
            "trouble",
            &format!("#{blocked} (Cy) is blocked: left: the second file")
        ),
        (Moves::Nobody, "saved")
    );
    assert_eq!(
        stands(
            "idle",
            &format!("#{idle} (Cy) is ready, and no turn is on it")
        ),
        (Moves::Nobody, "saved")
    );
    assert_eq!(stands("question", "Pick a colour"), (Moves::You, "saved"));
    assert_eq!(stands("command", "npm run build"), (Moves::Itself, "saved"));
    assert_eq!(line(&said, "update", "0.1.140").moves, Moves::Itself);
    // What waits on the person comes first, then what nobody is on.
    let order: Vec<Moves> = said.lines.iter().map(|l| l.moves).collect();
    let mut sorted = order.clone();
    sorted.sort_by_key(|m| {
        [Moves::You, Moves::Nobody, Moves::Itself, Moves::Others]
            .iter()
            .position(|x| x == m)
    });
    assert_eq!(order, sorted);

    // A queued turn whose queue has said nothing is unknown, and says why; a paused one waits on you.
    let silent = answer(&conn, &service());
    assert!(
        line(&silent, "queued", "Ann").what.contains("unknown"),
        "{:#?}",
        silent.lines
    );
    gathr::pauses::pause(&conn, "agent", "ann", false, T).unwrap();
    conn.execute(
        "UPDATE turns SET held = 'pause', why = 'paused' WHERE id = ?1",
        [&queued],
    )
    .unwrap();
    let paused = answer(&conn, &service());
    assert_eq!(line(&paused, "queued", "paused").moves, Moves::You);
    let ann = paused.agents.iter().find(|a| a.name == "Ann").unwrap();
    assert_eq!(
        (ann.state, ann.words.as_str()),
        ("working", "Working"),
        "a running turn still shows"
    );

    // With the service stopped, what it alone knew is said as it was.
    let stopped = answer(&conn, &Live::default());
    assert!(
        line(&stopped, "turn", "Ann")
            .what
            .contains("was running when the service stopped"),
        "{:#?}",
        stopped.lines
    );
}

fn ci_wait(conn: &mut Connection, agent: &str) -> waits::Wait {
    let leave = Leave {
        chat: "c",
        agent,
        task: None,
        left_in: None,
        cwd: Some("."),
        then_do: "merge",
        parts: vec![Part {
            kind: "ci".into(),
            target: "157".into(),
            repo: None,
        }],
    };
    waits::leave(conn, &leave, T).unwrap()
}

/// The room, 2026-11-12: every agent waited on a macOS job no runner took, and nothing said so. And
/// a wait whose look kept failing went unseen (constructed). A wait's line says what its last look
/// saw and when, and after three failed looks nobody is moving it.
#[test]
fn a_wait_says_what_its_last_look_saw_and_a_look_that_keeps_failing_moves_nothing() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let wait = ci_wait(&mut conn, "bo");
    let queued = "1 of 2 checks on #157 at 0123456 finished: 1 passed; queued: UI (macOS)";
    waits::looked(&conn, &wait.id, None, Some(queued), None).unwrap();

    let said = answer(&conn, &service());
    let found = line(&said, "wait", "Bo waits for CI on #157");
    assert_eq!(found.moves, Moves::Itself);
    assert_eq!(found.seen.as_deref(), Some(queued));
    assert!(
        found.looked_us.is_some() && found.since_us.is_some(),
        "{found:?}"
    );
    assert_eq!(found.on.as_deref(), Some("CI on #157"));
    let bo = said.agents.iter().find(|a| a.name == "Bo").unwrap();
    assert_eq!(
        (bo.state, bo.words.as_str()),
        ("waiting", "Waiting for CI on #157")
    );

    for _ in 0..waits::SHOWN_AFTER {
        waits::looked(&conn, &wait.id, None, None, Some("gh: not signed in")).unwrap();
    }
    let said = answer(&conn, &service());
    let found = line(&said, "wait", "Bo waits for CI on #157");
    assert_eq!(found.moves, Moves::Nobody, "three looks failed");
    assert_eq!(found.error.as_deref(), Some("gh: not signed in"));
    assert_eq!(
        found.seen.as_deref(),
        Some(queued),
        "what it last saw stands"
    );
}

/// A PR reviewed clean shows where Gathr's last look left it (0049): moving by itself while its
/// checks run, and nobody's once it is left to its reviewer.
#[test]
fn a_pr_to_land_says_where_gathrs_last_look_left_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "lead", None, "done");
    let review = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: Some("giving"),
        parent: None,
        kind: Kind::Review,
        what: "Review #157",
        why: None,
        after: &[],
        pr: Some(157),
        in_passing: false,
    };
    let review = tasks::give(&mut conn, &review, T).unwrap().0.number;
    begin(&mut conn, review);
    let done = Said {
        commit: Some("0123456"),
        ..Said::default()
    };
    tasks::finish(&mut conn, "bo", review, Finish::Done, &done, T).unwrap();
    let ran: String = conn
        .query_row("SELECT id FROM turns WHERE task = ?1", [review], |r| {
            r.get(0)
        })
        .unwrap();
    gathr::store::set_turn(&conn, &ran, "done", None).unwrap();
    tasks::ended(&mut conn, review, Outcome::Ok, T).unwrap();

    let said = answer(&conn, &service());
    assert_eq!(
        line(&said, "pr", "#157").moves,
        Moves::Itself,
        "not looked at yet"
    );

    let going = "CI is still going on 0123456 (queued: UI (macOS))";
    gathr::landings::looked(&conn, ("c", 157, "0123456"), going, true).unwrap();
    let said = answer(&conn, &service());
    let found = line(&said, "pr", going);
    assert_eq!(found.moves, Moves::Itself);
    assert!(
        found.since_us.is_some() && found.looked_us.is_some(),
        "{found:?}"
    );

    let failed = "CI on 0123456 has a failure, which the project doesn't require to pass";
    gathr::landings::looked(&conn, ("c", 157, "0123456"), failed, false).unwrap();
    let said = answer(&conn, &service());
    assert_eq!(line(&said, "pr", failed).moves, Moves::Nobody);

    conn.execute(
        "UPDATE tasks SET pr_closed = 1 WHERE number = ?1",
        params![review],
    )
    .unwrap();
    let said = answer(&conn, &service());
    assert!(
        said.lines.iter().all(|l| l.kind != "pr"),
        "merged: {:#?}",
        said.lines
    );
}

/// Found using it on seeded data: a member's request the Supervisor filed as a question to the
/// person waits for that answer, and never reads as something nobody is on. A held task with
/// nothing open says the reason it has.
#[test]
fn a_request_filed_for_the_person_waits_for_their_answer() {
    let dir = TempDir::new();
    let conn = team(&dir);
    conn.execute_batch(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, reason, created_ts, updated_ts)
         VALUES (66, 'c', 'ann', 'lead', 'person_request', 'Ask about the notice', 'held',
                 'done, waiting on #67 (the person)', 5, 5),
                (68, 'c', 'lead', 'bo', 'work', 'Fix it', 'held', 'waiting on a check', 5, 5);
         INSERT INTO tasks (number, chat_id, giver, kind, what, state, request, created_ts, updated_ts)
         VALUES (67, 'c', 'lead', 'person', 'What should the notice say?', 'ready', 66, 6, 6);",
    )
    .unwrap();
    let said = answer(&conn, &service());
    let waits = line(&said, "given", "#66 (Lead) waits for your answer to #67");
    assert_eq!(waits.moves, Moves::Others);
    assert_eq!(
        line(&said, "question", "What should the notice say?").moves,
        Moves::You
    );
    let stale = line(&said, "idle", "#68 (Bo) is held (waiting on a check)");
    assert_eq!(stale.moves, Moves::Nobody);
}

/// Found in #185's review. A turn queued behind one that waits for a sign-in waits on the person,
/// whichever chat that turn is in, and its agent isn't free. With the service stopped nothing
/// moves by itself, and the answer says so first.
#[test]
fn a_turn_behind_a_sign_in_waits_on_the_person_and_nothing_moves_while_the_service_is_stopped() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "signing", "ann", None, "waiting_sign_in");
    turn(&conn, "next", "ann", None, "queued");
    ci_wait(&mut conn, "bo");
    let words = "behind this agent's queued turn";
    let live = Live {
        queued: HashMap::from([("next".to_string(), words.to_string())]),
        behind: HashMap::from([("next".to_string(), "signing".to_string())]),
        ..service()
    };
    let said = answer(&conn, &live);
    let behind = line(&said, "queued", words);
    assert_eq!(behind.moves, Moves::You, "{behind:?}");
    assert!(
        behind.what.contains("waits for you to sign in"),
        "{behind:?}"
    );
    let ann = said.agents.iter().find(|a| a.name == "Ann").unwrap();
    assert_eq!(
        (ann.state, ann.words.as_str()),
        ("waiting", "Waiting for you to sign in")
    );
    assert_eq!(line(&said, "wait", "Bo waits for CI").moves, Moves::Itself);

    let stopped = answer(&conn, &Live::default());
    assert_eq!(stopped.lines[0].kind, "stopped", "{:#?}", stopped.lines);
    let moving: Vec<&Line> = stopped
        .lines
        .iter()
        .filter(|l| l.moves == Moves::Itself)
        .collect();
    assert!(moving.is_empty(), "moving with no service: {moving:#?}");
    // In words too it comes first, before what waits on the person.
    let text = waiting::text(&stopped, gathr::ids::now_us());
    assert!(text.starts_with("Gathr's service isn't running"), "{text}");
    assert!(text.contains("Waiting on you:"), "{text}");
}

/// Scenario 6d: a running turn that has made no progress past the limit isn't moving anything,
/// and says since when. The service says which turns are silent; the answer is built with that.
#[test]
fn a_silent_turn_is_not_moving_and_says_since_when() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "lead", None, "done");
    let task = give(&mut conn, "ann", &[], "giving");
    let running = begin(&mut conn, task);
    let now = gathr::ids::now_us();
    let since = now - 12 * 60_000_000;
    let live = Live {
        silent: HashMap::from([(running, since)]),
        ..service()
    };
    let said = waiting::answer(&conn, "c", &live, now).unwrap();
    let found = line(&said, "turn", "Ann is working");
    assert_eq!((found.moves, found.silent_us), (Moves::Nobody, Some(since)));
    let shown = serde_json::to_value(found).unwrap();
    assert_eq!(shown["silent_us"], since, "{shown}");
    assert_eq!(
        shown.get("turn"),
        None,
        "the turn's id is the service's own"
    );
    let text = waiting::text(&said, now);
    let words = format!(
        "Not moving:
- Ann is working on #{task}, for under a minute, silent for 12 min"
    );
    assert!(text.contains(&words), "{text}");
    let ann = said.agents.iter().find(|a| a.name == "Ann").unwrap();
    assert_eq!(ann.state, "working", "it still runs");

    let said = waiting::answer(&conn, "c", &service(), now).unwrap();
    let found = line(&said, "turn", "Ann is working");
    assert_eq!((found.moves, found.silent_us), (Moves::Itself, None));
    assert!(!waiting::text(&said, now).contains("silent"));
}

/// Scenario 6h: a task held by a pause of the person's, on its owner, the chat or the app, waits
/// on them, as a paused agent's wait does. Nobody else's task changes.
#[test]
fn a_task_whose_owner_is_paused_waits_on_the_person() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "giving", "lead", None, "done");
    let anns = give(&mut conn, "ann", &[], "giving");
    let bos = give(&mut conn, "bo", &[], "giving");
    conn.execute("DELETE FROM turns WHERE task IS NOT NULL", [])
        .unwrap();
    let stands = |conn: &Connection, has: &str| {
        let said = answer(conn, &service());
        let found = line(&said, "idle", has);
        (found.moves, found.what.ends_with(", paused"))
    };
    let (anns, bos) = (format!("#{anns} (Ann)"), format!("#{bos} (Bo)"));
    for (scope, target) in [("agent", "ann"), ("chat", "c"), ("app", "")] {
        assert_eq!(stands(&conn, &anns), (Moves::Nobody, false), "{scope}");
        gathr::pauses::pause(&conn, scope, target, false, T).unwrap();
        assert_eq!(stands(&conn, &anns), (Moves::You, true), "{scope}");
        let others = match scope {
            "agent" => (Moves::Nobody, false),
            _ => (Moves::You, true),
        };
        assert_eq!(stands(&conn, &bos), others, "{scope}");
        gathr::pauses::resume(&conn, scope, target, T).unwrap();
    }
}

/// The answer in words, as the `waiting` tool and `gathr diag waiting --text` give it.
#[test]
fn the_answer_reads_in_words_grouped_by_who_can_end_each_hold() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    ci_wait(&mut conn, "bo");
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts)
         VALUES ('c', 'lead', 'person', 'Pick a colour', 'ready', 5, 5)",
        [],
    )
    .unwrap();
    let now = gathr::ids::now_us();
    let said = waiting::answer(&conn, "c", &service(), now).unwrap();
    let text = waiting::text(&said, now);
    let (you, itself) = (
        text.find("Waiting on you:").expect(&text),
        text.find("Moving by itself:").expect(&text),
    );
    assert!(you < itself, "{text}");
    assert!(
        text.contains("Pick a colour") && text.contains("Bo waits for CI on #157"),
        "{text}"
    );
    assert!(
        text.contains("Agents: ") && text.contains("Cy is free"),
        "{text}"
    );

    let empty = TempDir::new();
    let conn = team(&empty);
    let said = waiting::answer(&conn, "c", &service(), now).unwrap();
    assert!(
        waiting::text(&said, now).starts_with("Nothing is waiting"),
        "{}",
        waiting::text(&said, now)
    );
}
