//! A reply in a thread (0076) through a real service, on Claude Code's replay stand-in: the reply keeps
//! the message or task it answers, live, in history and after a restart; its agent reads Gathr's line
//! naming what it answers before the person's words; its turn sits in that thread and serves that
//! ask, or joins the agent's running turn; and a target that isn't a message's words or a task of the
//! chat is refused with a reason and a reference. Scenario 4e.

mod common;
#[path = "words/samples.rs"]
mod samples;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, next_live, rows, team, text, turn_on, until};
use gathr::http::Response;
use serde_json::{Value, json};

/// Claude Code's stand-in for `n` turns of the signed-in recording, each turn in `held` waiting for
/// `go` once its message arrives.
fn engine(data: &Path, held: &[usize], n: usize) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let mut all = Vec::new();
    for turn in 0..n {
        all.push(lines[0]);
        if held.contains(&turn) {
            all.push(r#"{"dir":"hold"}"#);
        }
        all.extend(&lines[1..]);
    }
    let file = data.join("turns.jsonl");
    std::fs::write(&file, all.join("\n") + "\n").unwrap();
    file
}

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

fn post(service: &TestService, chat: &str, body: Value) -> Response {
    service.send("POST", &format!("/api/chats/{chat}/messages"), body)
}

/// The person's message to the lead: the turn it opened.
fn say(service: &TestService, chat: &str, words: &str) -> String {
    let sent = post(service, chat, json!({ "text": words }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()["turn"].as_str().unwrap().to_string()
}

fn turn_is(service: &TestService, chat: &str, turn: &str, statuses: &[&str]) -> Value {
    let what = format!("turn {turn} to be {statuses:?}");
    let history = service.wait_for(chat, 30, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && statuses.iter().any(|s| t["status"] == *s))
    });
    let turns = history["turns"].as_array().unwrap();
    turns.iter().find(|t| t["id"] == turn).unwrap().clone()
}

/// The words a turn said, once said: the message.
fn said_in(service: &TestService, chat: &str, turn: &str) -> Value {
    let what = format!("words of turn {turn}");
    let history = service.wait_for(chat, 30, &what, |h| words_of(h, turn).is_some());
    words_of(&history, turn).unwrap()
}

fn words_of(history: &Value, turn: &str) -> Option<Value> {
    let messages = history["messages"].as_array()?;
    let found = messages
        .iter()
        .find(|m| m["turn"] == turn && m["role"] == "assistant" && m["kind"] == "text");
    found.cloned()
}

/// The person's message with these words, as history shows it.
fn person_said(history: &Value, words: &str) -> Value {
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let found = messages
        .iter()
        .find(|m| m["role"] == "user" && m["body"] == words);
    found
        .unwrap_or_else(|| panic!("no message {words:?}: {history}"))
        .clone()
}

fn turn_of(history: &Value, turn: &str) -> Value {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["id"] == turn);
    found
        .unwrap_or_else(|| panic!("no turn {turn}: {history}"))
        .clone()
}

/// What the engine read in `turn`, joined.
fn read_in(data: &Path, turn: &str) -> String {
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{turn}' ORDER BY id"
    );
    let bodies = rows(data, &sql);
    let bodies = bodies.iter().filter_map(|r| r["body"].as_str());
    bodies.collect::<Vec<_>>().join("\n")
}

/// Gathr's line the engine read before the person's words, as JSON escapes it in the frame.
fn escaped(words: &str) -> String {
    let json = serde_json::to_string(words).unwrap();
    json.trim_matches('"').to_string()
}

fn ask_of(data: &Path, turn: &str) -> Value {
    let found = rows(
        data,
        &format!("SELECT ask_id FROM turns WHERE id = '{turn}'"),
    );
    found[0]["ask_id"].clone()
}

fn member(history: &Value, name: &str) -> String {
    let members = history["members"].as_array().cloned().unwrap_or_default();
    let found = members.iter().find(|m| m["name"] == name);
    found.unwrap_or_else(|| panic!("no member {name}: {history}"))["agent"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The next live message event of the person's.
fn live_message(socket: &mut std::net::TcpStream) -> Value {
    loop {
        let event = next_live(socket);
        if event["type"] == "message" && event["message"]["role"] == "user" {
            return event["message"].clone();
        }
    }
}

/// A refusal: 400 with `why`, and a reference `gathr diag trace` finds it from (0019).
fn refused(data: &Path, answer: &Response, why: &str) {
    assert_eq!(answer.status, 400, "{}", answer.body);
    let said = answer.json();
    assert_eq!(
        (&said["error"], &said["why"]),
        (&json!("Send failed"), &json!(why)),
        "{said}"
    );
    let reference = said["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events.iter().find(|e| e["name"] == "gathr.api.request");
    let failed = failed.map(|e| e["attrs"]["reason"].clone());
    assert!(ok && failed == Some(json!(why)), "{reference}: {trace}");
}

#[test]
fn a_reply_keeps_its_target_reaches_its_author_with_the_line_and_sits_in_that_thread() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    let asked = say(&service, &chat, "Plan the river walk");
    turn_is(&service, &chat, &asked, &["done"]);
    let message = said_in(&service, &chat, &asked);
    let id = message["id"].as_str().unwrap();
    let ask = person_said(&history(&service, &chat), "Plan the river walk")["id"].clone();
    let mut socket = service.live();

    let words = "Does it pass the old mill?";
    let sent = post(
        &service,
        &chat,
        json!({ "text": words, "agent": lead, "reply_to": id }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let live = live_message(&mut socket);
    assert_eq!(
        (&live["body"], &live["reply_to"]),
        (&json!(words), &json!(id)),
        "{live}"
    );
    turn_is(&service, &chat, &turn, &["done"]);

    let shown = history(&service, &chat);
    let reply = person_said(&shown, words);
    assert_eq!(
        (&reply["reply_to"], &reply["reply_task"]),
        (&json!(id), &Value::Null)
    );
    assert_eq!(
        common::opened(&data.0, &json!(turn)),
        (json!("reply"), json!(gathr::words::REPLY))
    );
    let line = format!(
        "The person replies to your message (message {id}, {}): \"Hi!\"",
        message["time"].as_str().unwrap()
    );
    let read = read_in(&data.0, &turn);
    let both = format!("{line}\n\n{words}");
    assert!(read.contains(&escaped(&both)), "the engine read: {read}");
    let sampled = line
        .replace(id, "<id>")
        .replace(message["time"].as_str().unwrap(), "<time>");
    samples::same("reply: a message", &sampled);
    // In the thread of the message it answers, and the ask that message served.
    let target_topic = turn_of(&shown, &asked)["topic"].clone();
    assert_eq!(turn_of(&shown, &turn)["topic"], target_topic, "{shown}");
    assert_eq!(target_topic, json!(format!("m:{}", ask.as_str().unwrap())));
    assert_eq!(ask_of(&data.0, &turn), ask);
    common::topics_agree(&data.0, &chat);

    service.stop();
    let again = TestService::start(&data.0);
    let kept = person_said(&history(&again, &chat), words);
    assert_eq!(kept["reply_to"], json!(id), "after a restart: {kept}");
}

#[test]
fn a_reply_about_a_task_goes_to_its_owner_and_sits_in_the_tasks_thread() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    let (service, chat, _) = team(&data.0, &engine(&data.0, &[0], 8), &[]);
    let port = service.info.port;
    let asked = say(&service, &chat, "Export the trail notes");
    let lead = common::first_token(&data.0);
    common::fix_it(port, &lead);
    let work = common::briefed(json!({ "to": "Ann", "what": "Write the exporter" }));
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann"), "{given}");
    std::fs::write(&go, "").unwrap();
    turn_on(&service, &chat, 1, &["done"]);
    turn_is(&service, &chat, &asked, &["done"]);
    let quiet = |h: &Value| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        !turns
            .iter()
            .any(|t| t["status"] == "queued" || t["status"] == "running")
    };
    service.wait_for(&chat, 30, "every turn to end", quiet);
    let state =
        |data: &Path| rows(data, "SELECT state FROM tasks WHERE number = 1")[0]["state"].clone();
    let before = state(&data.0);
    let ann = member(&history(&service, &chat), "Ann");

    let words = "Keep the photos out of it";
    let sent = post(
        &service,
        &chat,
        json!({ "text": words, "agent": ann, "reply_task": 1 }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let ended = turn_is(&service, &chat, &turn, &["done"]);
    assert_eq!(ended["agent"], json!(ann));
    assert_eq!(ended["task"], Value::Null, "the reply works on no task");
    let shown = history(&service, &chat);
    assert_eq!(person_said(&shown, words)["reply_task"], json!(1));
    assert_eq!(turn_of(&shown, &turn)["topic"], json!("t:1"), "{shown}");
    let line = "The person replies about your task #1 \"Write the exporter\".";
    samples::same("reply: a task", line);
    let read = read_in(&data.0, &turn);
    assert!(
        read.contains(&escaped(&format!("{line}\n\n{words}"))),
        "the engine read: {read}"
    );
    let task_ask = rows(&data.0, "SELECT ask_id FROM tasks WHERE number = 1")[0]["ask_id"].clone();
    assert_eq!(ask_of(&data.0, &turn), task_ask);
    assert_eq!(state(&data.0), before, "the task's state moved");
    common::topics_agree(&data.0, &chat);
}

#[test]
fn a_reply_to_an_agent_at_work_joins_its_running_turn() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[1], 2), &[]);
    let _go = LetGo(data.0.join("go"));
    let first = say(&service, &chat, "Plan the river walk");
    turn_is(&service, &chat, &first, &["done"]);
    let id = said_in(&service, &chat, &first)["id"].clone();
    let running = say(&service, &chat, "And the picnic spot");
    turn_is(&service, &chat, &running, &["running"]);

    let words = "Shade matters more than the view";
    let sent = post(
        &service,
        &chat,
        json!({ "text": words, "agent": lead, "reply_to": id }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let joined = sent.json();
    assert_eq!(
        (&joined["turn"], &joined["steered"]),
        (&json!(running), &json!(true)),
        "{joined}"
    );
    let reply = person_said(&history(&service, &chat), words);
    assert_eq!((&reply["turn"], &reply["reply_to"]), (&json!(running), &id));
    until(10, "the engine to be sent the line and the words", || {
        let read = read_in(&data.0, &running);
        (read.contains("The person replies to your message") && read.contains(words)).then_some(())
    });
}

#[test]
fn a_reply_to_what_isnt_a_message_or_a_task_of_the_chat_is_refused_with_a_reference() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    let asked = say(&service, &chat, "Plan the river walk");
    turn_is(&service, &chat, &asked, &["done"]);
    let other = service.send("POST", "/api/chats", json!({}));
    assert_eq!(other.status, 200, "{}", other.body);
    let other = other.json()["id"].as_str().unwrap().to_string();
    let sent = post(
        &service,
        &other,
        json!({ "text": "Somewhere else", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let elsewhere = sent.json()["turn"].as_str().unwrap().to_string();
    turn_is(&service, &other, &elsewhere, &["done"]);
    let theirs = person_said(&history(&service, &other), "Somewhere else")["id"].clone();
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    conn.execute(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body)
         VALUES ('step1', ?1, ?2, 1, 'assistant', 'tool', '{}')",
        [&chat, &asked],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (90, ?1, NULL, ?2, 'work', 'Count the stiles', 'ready', 1, 1)",
        [&other, &lead],
    )
    .unwrap();
    drop(conn);
    let reply = |target: Value| {
        let mut body = json!({ "text": "Why?", "agent": lead });
        body.as_object_mut()
            .unwrap()
            .extend(target.as_object().unwrap().clone());
        post(&service, &chat, body)
    };

    let step = reply(json!({ "reply_to": "step1" }));
    refused(
        &data.0,
        &step,
        "step1 is a step, not a message: a reply answers a message's words.",
    );
    let away = reply(json!({ "reply_to": theirs }));
    let why = format!("No message {} in this chat.", theirs.as_str().unwrap());
    refused(&data.0, &away, &why);
    let task = reply(json!({ "reply_task": 90 }));
    refused(&data.0, &task, "No task #90 in this chat.");
    let both = reply(json!({ "reply_to": "step1", "reply_task": 90 }));
    refused(
        &data.0,
        &both,
        "A reply answers one message or one task, not both.",
    );
    let turns = rows(&data.0, "SELECT id FROM turns WHERE opened_by = 'reply'");
    assert!(turns.is_empty(), "a refused reply started {turns:?}");
}

/// A database at 0055 upgrades by adding the reply's two columns: every rowid and the search index
/// stay, and a message from before is a reply to nothing.
#[test]
fn the_upgrade_from_0055_keeps_every_rowid_and_the_search_index() {
    let data = TempDir::new();
    let db = gathr::data::DataDir::new(&data.0).db();
    std::fs::create_dir_all(&data.0).unwrap();
    let conn = gathr::db::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Old chat', 1);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, position) VALUES
             ('t1', 'c', 'aaaaaaaa', 'done', 10, 1);
         INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
             ('m1', 'c', 't1', 10, 'user', 'text', 'Where do the herons nest'),
             ('m2', 'c', 't1', 11, 'assistant', 'text', 'Herons nest by the weir');
         ALTER TABLE messages DROP COLUMN reply_to; ALTER TABLE messages DROP COLUMN reply_task;
         PRAGMA user_version = 55;",
    )
    .unwrap();
    let ids = "SELECT rowid || ':' || id FROM messages ORDER BY rowid";
    let found = "SELECT rowid FROM message_search WHERE message_search MATCH 'herons'";
    let (before, indexed) = (rows_of(&conn, ids), rows_of(&conn, found));
    drop(conn);

    let service = TestService::start(&data.0);
    let shown = history(&service, "c");
    let conn = rusqlite::Connection::open(&db).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, gathr::db::SCHEMA_VERSION);
    assert_eq!(rows_of(&conn, ids), before);
    assert_eq!(rows_of(&conn, found), indexed, "the search index moved");
    let first = &shown["messages"][0];
    assert_eq!(
        (&first["id"], &first["reply_to"], &first["reply_task"]),
        (&json!("m1"), &Value::Null, &Value::Null),
        "{shown}"
    );
}

fn rows_of(conn: &rusqlite::Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([], |r| r.get::<_, rusqlite::types::Value>(0))
        .unwrap();
    rows.map(|r| format!("{:?}", r.unwrap())).collect()
}
