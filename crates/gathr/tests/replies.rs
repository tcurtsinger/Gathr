//! A reply in a thread (0076) through a real service, on Claude Code's replay stand-in: the reply keeps
//! the message or task it answers, live, in history and after a restart; its agent reads Gathr's line
//! naming what it answers before the person's words; its turn sits in that thread and serves that
//! ask, or joins the agent's running turn; and a target that isn't a message's words or a task of the
//! chat is refused with a reason and a reference. Scenario 4e.

mod common;

use common::replay::{LetGo, read_in, turns as engine};
use common::{TempDir, TestService, team, until};
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
