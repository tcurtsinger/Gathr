//! A reply in a thread (0076) through a real service, on Claude Code's replay stand-in: the reply keeps
//! the message or task it answers, live, in history and after a restart; its agent reads Gathr's line
//! naming what it answers before the person's words; its turn sits in that thread and serves that
//! ask, or joins the agent's running turn; and a target that isn't a message's words or a task of the
//! chat is refused with a reason and a reference. Scenario 4e.

mod common;

use std::path::Path;

use common::replay::{LetGo, read_in, turns as engine};
use common::{TempDir, TestService, gathr, rows, team, until};
use gathr::http::Response;
use serde_json::{Value, json};

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
