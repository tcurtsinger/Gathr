//! 0075 through a real service on Claude Code's replay stand-in: what each turn, task and line said
//! in no turn is about, the main message it sits under, as history, the tasks list and `gathr diag
//! threads` say it, on real sequences of work. Old chats are tied by `task` and `answers` alone,
//! and a link that leads nowhere ties to nothing, found from the history route's reference.

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, gathr, nth, rows, team_on, text, turn_on, until};
use serde_json::{Value, json};

const ASK: &str = "Add tags to notes";

/// Each engine's first turn holds until `go`; every turn after it runs straight through.
fn first_turns_hold(data: &Path) {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let mut all = vec![lines[0].to_string(), r#"{"dir":"hold"}"#.to_string()];
    all.extend(lines[1..].iter().map(|l| l.to_string()));
    for _ in 0..6 {
        all.extend(lines.iter().map(|l| l.to_string()));
    }
    std::fs::write(data.join("held.jsonl"), all.join("\n") + "\n").unwrap();
}

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(std::path::PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

fn listed(service: &TestService, chat: &str) -> Value {
    let listed = service.call("GET", &format!("/api/chats/{chat}/tasks"));
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

/// The person's message, the turn it opened (once it runs) and its id.
fn send(service: &TestService, chat: &str, said: &str) -> (String, String) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": said }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let found = service.wait_for(chat, 30, "the person's message", |h| {
        let messages = h["messages"].as_array().cloned().unwrap_or_default();
        messages
            .iter()
            .any(|m| m["role"] == "user" && m["body"] == said)
    });
    let messages = found["messages"].as_array().unwrap();
    let message = messages
        .iter()
        .find(|m| m["role"] == "user" && m["body"] == said);
    (turn, message.unwrap()["id"].as_str().unwrap().to_string())
}

/// A turn's topic as history gives it.
fn topic_of(history: &Value, turn: &str) -> Value {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["id"] == turn);
    found.unwrap_or_else(|| panic!("no turn {turn}: {history}"))["topic"].clone()
}

/// The topic of the line said in no turn that starts with `start`.
fn line_topic(history: &Value, start: &str) -> Value {
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let line = messages
        .iter()
        .find(|m| m["turn"].is_null() && m["body"].as_str().is_some_and(|b| b.starts_with(start)));
    line.unwrap_or_else(|| panic!("no line {start:?}: {history}"))["topic"].clone()
}

fn task_topic(listed: &Value, number: i64) -> Value {
    let tasks = listed["tasks"].as_array().cloned().unwrap_or_default();
    let found = tasks.iter().find(|t| t["number"] == number);
    found.unwrap_or_else(|| panic!("no task #{number}: {listed}"))["topic"].clone()
}

/// The turns the words of `kind` opened, oldest first.
fn opened(data: &Path, kind: &str) -> Vec<String> {
    let sql = format!("SELECT id FROM turns WHERE opened_by = '{kind}' ORDER BY rowid");
    let found = rows(data, &sql);
    found
        .iter()
        .map(|r| r["id"].as_str().unwrap().to_string())
        .collect()
}

fn token(data: &Path, n: usize) -> String {
    nth(&data.join("env.jsonl"), n)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap_or_else(|| panic!("no token for engine {n}"))
        .to_string()
}

fn part(what: &str) -> Value {
    json!({ "to": "Ann", "what": what, "why": "Notes need tags", "done": "A note carries its tags",
        "proof": "tests/tags.rs passes", "where": "no code", "size": "src/tags.rs", "lines": 60, "after": [] })
}

/// Scenario 6e: the person's ask, the lead's plan and its check, Approve's parts, a part that is
/// blocked, the person's cancel and the last part done. The lead hears back twice: on the trouble,
/// in the ask's thread, and once everything is in, the ask's result. Gathr's lines sit with what
/// they are about.
#[test]
fn an_asks_plan_its_parts_and_the_leads_hearing_back_each_sit_where_they_belong() {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, "claude");
    first_turns_hold(&data.0);
    common::with_reviewer(&service, &chat);
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    let port = service.info.port;
    let (lead_turn, ask) = send(&service, &chat, ASK);
    let lead = common::first_token(&data.0);

    let plan = json!({ "title": ASK, "text": "Notes carry tags.",
        "parts": [part("Add the tags table"), part("Show tags in the list")] });
    let wrote = text(&call_tool(port, &lead, "plan", plan));
    assert!(wrote.starts_with("Wrote version 1 of plan #1"), "{wrote}");
    let check_turn = turn_on(&service, &chat, 1, &["running"]);
    let rev = token(&data.0, 1);
    let shown = service.call("GET", "/api/plans/1").json();
    let version = shown["versions"][0]["message"]
        .as_str()
        .unwrap()
        .to_string();
    let agrees = call_tool(port, &rev, "react", json!({ "message": version }));
    assert_eq!(agrees["isError"], false, "{}", text(&agrees));
    std::fs::write(&go, "").unwrap();
    until(30, "the plan to wait on the person", || {
        let shown = service.call("GET", "/api/plans/1").json();
        (shown["state"] == "waiting").then_some(())
    });
    service.wait_for(&chat, 30, "the lead's first turn to end", |h| {
        h["turns"][0]["status"] == "done"
    });

    // Ann's first turn holds again, so her report is made while it runs.
    std::fs::remove_file(&go).unwrap();
    let approved = service.send("POST", "/api/plans/1/approve", json!({ "version": 1 }));
    assert_eq!(approved.status, 200, "{}", approved.body);
    let ann_turn = turn_on(&service, &chat, 2, &["running"]);
    let ann = token(&data.0, 2);
    let blocked = json!({ "task": 3, "how": "blocked", "note": "it needs the tags table first" });
    let said = text(&call_tool(port, &ann, "finish_task", blocked));
    assert!(said.starts_with("Recorded: blocked."), "{said}");
    until(30, "the lead's hear-back on the trouble", || {
        opened(&data.0, "hear_back").into_iter().next()
    });
    let cancelled = service.send("POST", "/api/tasks/3/cancel", json!({}));
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let done =
        json!({ "task": 2, "how": "done", "note": "the table is in", "file": "src/tags.rs" });
    let said = text(&call_tool(port, &ann, "finish_task", done));
    assert!(said.starts_with("Recorded: done."), "{said}");
    std::fs::write(&go, "").unwrap();
    let backs = until(30, "the lead's last hear-back", || {
        let backs = opened(&data.0, "hear_back");
        (backs.len() == 2).then_some(backs)
    });
    let history = service.wait_for(&chat, 30, "every turn to end", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .all(|t| t["status"] != "queued" && t["status"] != "running")
    });

    let (on_ask, on_version) = (json!(format!("m:{ask}")), json!(format!("m:{version}")));
    let found = [
        ("the person's own turn", topic_of(&history, &lead_turn)),
        ("the plan check's turn", topic_of(&history, &check_turn)),
        ("Ann's turn on part #2", topic_of(&history, &ann_turn)),
        (
            "the hear-back on the trouble",
            topic_of(&history, &backs[0]),
        ),
        ("the closing hear-back", topic_of(&history, &backs[1])),
        ("the check's line", line_topic(&history, "Rev agrees")),
        ("Approve's line", line_topic(&history, "Approved:")),
        (
            "part #2's done line",
            line_topic(&history, "A task is done:\n- #2"),
        ),
    ];
    let want = [
        ("the person's own turn", on_ask.clone()),
        ("the plan check's turn", on_version.clone()),
        ("Ann's turn on part #2", json!("t:2")),
        ("the hear-back on the trouble", on_ask),
        ("the closing hear-back", json!(format!("r:{}", backs[1]))),
        ("the check's line", on_version.clone()),
        ("Approve's line", on_version.clone()),
        ("part #2's done line", json!("t:2")),
    ];
    assert_eq!(found, want, "{history}");
    let listed = listed(&service, &chat);
    let tasks = [1, 2, 3].map(|n| task_topic(&listed, n));
    assert_eq!(tasks, [on_version, json!("t:2"), json!("t:3")], "{listed}");
    assert_eq!(
        listed["asks"][0]["open"], false,
        "the ask is done: {listed}"
    );
}

/// Scenario 6e: a part a member gives from its own task sits in that task's thread, with the turns
/// that do it and the member's hear-back on it.
#[test]
fn a_part_a_member_gives_from_its_task_sits_in_that_task() {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, "claude");
    first_turns_hold(&data.0);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let mut live = service.live();
    send(&service, &chat, "Export notes as Markdown");
    let lead = common::first_token(&data.0);
    common::fix_it(port, &lead);
    let work = common::briefed(json!({ "to": "Ann", "what": "Write the exporter" }));
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann"), "{given}");
    turn_on(&service, &chat, 1, &["running"]);
    let ann = token(&data.0, 1);
    let part = common::briefed(json!({ "to": "Lead", "what": "Name the export menu item" }));
    let given = text(&call_tool(port, &ann, "give_task", part));
    assert!(given.starts_with("Task #2 for Lead"), "{given}");
    std::fs::write(data.0.join("go"), "").unwrap();
    let part_turn = turn_on(&service, &chat, 2, &["running", "done"]);
    let sql = "SELECT u.id FROM turns u JOIN agents a ON a.id = u.agent_id
               WHERE u.opened_by = 'hear_back' AND a.name = 'Ann'";
    let heard = until(60, "Ann's hear-back on #2", || {
        rows(&data.0, sql)
            .first()
            .map(|r| r["id"].as_str().unwrap().to_string())
    });
    let history = history(&service, &chat);
    let found = [topic_of(&history, &part_turn), topic_of(&history, &heard)];
    assert_eq!(found, [json!("t:1"), json!("t:1")], "{history}");
    let listed = listed(&service, &chat);
    assert_eq!(task_topic(&listed, 2), json!("t:1"), "{listed}");
    // The live `turn` event says it from the part's turn's first word, as it is queued.
    let mut sent = std::iter::from_fn(|| Some(common::next_live(&mut live)));
    let queued = sent.find(|u| u["type"] == "turn" && u["turn"] == part_turn.as_str());
    assert_eq!(queued.unwrap()["topic"], "t:1");
}

/// Question 2 of the plan behind 0075: a quiet room wakes its lead on work that is all of one ask,
/// so the wake serves that ask and sits in its thread. The task nobody is on is written while the
/// service is down, as a stop can leave one (as `quiet_room.rs` does).
#[test]
fn a_quiet_rooms_wake_on_one_asks_work_sits_in_that_ask() {
    let data = TempDir::new();
    let replay = common::recording("signed_in");
    let (go, tokens) = (data.0.join("go"), data.0.join("env.jsonl"));
    let looks = ("GATHR_WAIT_EVERY_MS", Path::new("200"));
    let (service, chat, _) = common::team(&data.0, &replay, &[looks]);
    let (turn, ask) = send(&service, &chat, "Fix the build");
    service.wait_for(&chat, 30, "the lead's turn to end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let scope = json!({ "scope": "chat", "target": chat });
    assert_eq!(
        service.send("POST", "/api/pauses", scope.clone()).status,
        200
    );
    service.stop();
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    let now = gathr::ids::now_us();
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts, ask_id)
         SELECT ?1, (SELECT id FROM agents WHERE name = 'Lead'), id, ?2, 'work', 'Fix the build', 'ready', ?3, ?3, ?4
         FROM agents WHERE name = 'Ann'",
        rusqlite::params![chat, turn, now, ask],
    )
    .unwrap();
    drop(conn);
    let env = [
        ("GATHR_REPLAY", replay.as_path()),
        ("GATHR_REPLAY_HOLD", go.as_path()),
        ("GATHR_REPLAY_ENV", tokens.as_path()),
        looks,
    ];
    let service = TestService::with_engine(&data.0, &env);
    assert_eq!(
        service.send("POST", "/api/pauses/resume", scope).status,
        200
    );
    let wake = until(30, "the lead's wake", || {
        opened(&data.0, "wake").into_iter().next()
    });
    let history = history(&service, &chat);
    assert_eq!(
        topic_of(&history, &wake),
        json!(format!("m:{ask}")),
        "{history}"
    );
}

/// A chat from before 0055, its turns from before `opened_by` and asks: the upgrade keeps every
/// rowid and the search index, and its turns are tied by `task` and `answers` alone; one with
/// neither is its own main message.
#[test]
fn an_old_chat_is_tied_by_task_and_answers_alone_after_the_upgrade() {
    let data = TempDir::new();
    let db = gathr::data::DataDir::new(&data.0).db();
    std::fs::create_dir_all(&data.0).unwrap();
    let conn = gathr::db::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('acc', 'claude', 'Old', 'old', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('mira', 'Mira', 'mira', 'acc', 1), ('rowan', 'Rowan', 'rowan', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Old team', 1);
         INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES
             ('c', 'mira', 'supervisor', 1), ('c', 'rowan', 'member', 1);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, position) VALUES
             ('asked', 'c', 'aaaaaaaa', 'done', 10, 'mira', 1);
         INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts)
             VALUES (1, 'c', 'mira', 'rowan', 'asked', 'work', 'Sort the gravel bars', 'done', 11, 11);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, position, task, answers) VALUES
             ('work', 'c', 'bbbbbbbb', 'done', 20, 'rowan', 2, 1, 'asked'),
             ('back', 'c', 'cccccccc', 'done', 30, 'mira', 3, NULL, 'asked'),
             ('lone', 'c', 'dddddddd', 'done', 40, 'mira', 4, NULL, NULL);
         INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
             ('m1', 'c', 'asked', 10, 'user', 'text', 'Sort the gravel bars by size'),
             ('m2', 'c', 'asked', 11, 'assistant', 'text', 'Rowan takes it'),
             ('m3', 'c', 'work', 20, 'gathr', 'text', 'Task #1 from Mira: Sort the gravel bars'),
             ('m4', 'c', 'work', 21, 'assistant', 'text', 'Sorted, herons first'),
             ('m5', 'c', 'back', 30, 'gathr', 'text', 'The task you gave is settled'),
             ('m6', 'c', 'lone', 40, 'gathr', 'text', 'No progress in 3 days');
         ALTER TABLE turns DROP COLUMN result_of; ALTER TABLE landings DROP COLUMN turn;
         ALTER TABLE messages DROP COLUMN about;
         PRAGMA user_version = 54;",
    )
    .unwrap();
    let before = rows_of(
        &conn,
        "SELECT rowid || ':' || id FROM messages ORDER BY rowid",
    );
    let indexed = rows_of(
        &conn,
        "SELECT rowid FROM message_search WHERE message_search MATCH 'herons'",
    );
    drop(conn);

    let service = TestService::start(&data.0);
    let history = history(&service, "c");
    let conn = rusqlite::Connection::open(&db).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, gathr::db::SCHEMA_VERSION);
    assert_eq!(
        rows_of(
            &conn,
            "SELECT rowid || ':' || id FROM messages ORDER BY rowid"
        ),
        before
    );
    let found = rows_of(
        &conn,
        "SELECT rowid FROM message_search WHERE message_search MATCH 'herons'",
    );
    assert_eq!(found, indexed, "the search index moved");
    let topics = ["asked", "work", "back", "lone"].map(|t| topic_of(&history, t));
    assert_eq!(
        topics,
        [json!("m:m1"), json!("t:1"), json!("m:m1"), json!("b:lone")],
        "{history}"
    );
    let listed = listed(&service, "c");
    assert_eq!(task_topic(&listed, 1), json!("t:1"), "{listed}");
}

fn rows_of(conn: &rusqlite::Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([], |r| r.get::<_, rusqlite::types::Value>(0))
        .unwrap();
    rows.map(|r| format!("{:?}", r.unwrap())).collect()
}

/// The PR's forced failure (0019): on the seeded team chat, a turn whose `answers` names a turn
/// that is gone ties to nothing, `gathr diag threads` says why, and the history route's reference
/// finds it.
#[test]
fn a_turn_whose_answers_is_gone_ties_to_nothing_and_is_found_from_the_historys_reference() {
    let data = TempDir::new();
    let (ok, seeded) = gathr(&data.0, &["seed"]);
    assert!(ok, "{seeded}");
    let db = gathr::data::DataDir::new(&data.0).db();
    let conn = rusqlite::Connection::open(&db).unwrap();
    let chat: String = conn
        .query_row(
            "SELECT id FROM chats WHERE title = 'Seed: team at work'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let messages: i64 = conn
        .query_row(
            "SELECT count(*) FROM messages WHERE chat_id = ?1",
            [&chat],
            |r| r.get(0),
        )
        .unwrap();
    assert!((2_000..2_100).contains(&messages), "{messages} messages");
    let lost: String = conn
        .query_row(
            "SELECT id FROM turns WHERE chat_id = ?1 AND opened_by = 'hear_back' AND task IS NULL
             AND result_of IS NULL ORDER BY rowid LIMIT 1",
            [&chat],
            |r| r.get(0),
        )
        .unwrap();
    conn.execute("UPDATE turns SET answers = 'gone' WHERE id = ?1", [&lost])
        .unwrap();
    drop(conn);

    let (ok, threads) = gathr(&data.0, &["diag", "threads", "--chat", &chat]);
    assert!(ok, "{threads}");
    let turns = threads["turns"].as_array().cloned().unwrap_or_default();
    let row = turns
        .iter()
        .find(|t| t["turn"] == lost.as_str())
        .cloned()
        .unwrap_or_default();
    let want =
        json!({ "topic": format!("b:{lost}"), "why": "tied to nothing: answers gone not found" });
    assert_eq!(
        json!({ "topic": row["topic"], "why": row["why"] }),
        want,
        "{threads}"
    );
    assert_eq!(threads["counts"]["broken"], 1, "{}", threads["in_words"]);
    for fact in [
        "handed_out",
        "parent",
        "question",
        "version",
        "result",
        "person",
    ] {
        assert!(
            threads.to_string().contains(&format!("\"{fact}\"")),
            "no {fact} tie: {}",
            threads["in_words"]
        );
    }

    let service = TestService::start(&data.0);
    let read = service.call("GET", &format!("/api/chats/{chat}"));
    assert_eq!(read.status, 200, "{}", read.body);
    let reference = read
        .headers
        .lines()
        .find_map(|l| l.strip_prefix("x-gathr-ref: "))
        .unwrap_or_else(|| panic!("no reference: {}", read.headers))
        .trim()
        .to_string();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    let found = trace.to_string();
    assert!(
        ok && found.contains("gathr.chat.topics") && found.contains("answers gone not found"),
        "not found from the history's reference {reference}: {trace}"
    );
}
