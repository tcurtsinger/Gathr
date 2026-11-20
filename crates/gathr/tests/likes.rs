//! The person's like (0057) through a real service, on Claude Code's replay stand-in: kept once, live
//! and in history, and taken back; delivered to its agent as a short message of the person's would be,
//! into its running turn or a turn of its own, once; a paused chat, a signed-out agent and a removed
//! one; and each message that takes no like refused with a reason and a reference.

mod common;

use std::path::Path;

use common::{TempDir, TestService, gathr, rows, team};
use gathr::http::Response;
use serde_json::{Value, json};

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

/// The person's message to the lead: the turn it opened.
fn say(service: &TestService, chat: &str, text: &str) -> String {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()["turn"].as_str().unwrap().to_string()
}

/// The turn's status once it is one of `statuses`.
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

fn like(service: &TestService, chat: &str, message: &str, body: Value) -> Response {
    let path = format!("/api/chats/{chat}/messages/{message}/like");
    service.send("PUT", &path, body)
}

/// The person's likes in the chat's history: the messages they are on.
fn likes_shown(service: &TestService, chat: &str) -> Vec<Value> {
    let reactions = history(service, chat)["reactions"].clone();
    let reactions = reactions.as_array().cloned().unwrap_or_default();
    let mine = reactions.iter().filter(|r| r["agent"] == "person");
    mine.map(|r| r["message"].clone()).collect()
}

/// The turns a like opened.
fn liked_turns(data: &Path) -> Vec<Value> {
    rows(
        data,
        "SELECT id, status FROM turns WHERE opened_by = 'liked' ORDER BY created_ts",
    )
}

/// A refusal: 409 with `why`, and a reference `gathr diag trace` finds it from (0019).
pub fn refused(data: &Path, answer: &Response, why: &str) {
    assert_eq!(answer.status, 409, "{}", answer.body);
    let said = answer.json();
    assert_eq!(
        (&said["error"], &said["why"]),
        (&json!("Like failed"), &json!(why)),
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
fn a_like_on_what_takes_none_is_refused_with_a_reason_and_a_reference() {
    let data = TempDir::new();
    let steps = common::recording("steps_claude");
    let (service, chat, _) = team(&data.0, &steps, &[]);
    let turn = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &turn, &["done"]);
    let shown = history(&service, &chat);
    let messages = shown["messages"].as_array().unwrap();
    let id = |role: &str, kind: &str| {
        let found = messages
            .iter()
            .find(|m| m["role"] == role && m["kind"] == kind);
        let found = found.unwrap_or_else(|| panic!("no {role} {kind}: {shown}"));
        found["id"].as_str().unwrap().to_string()
    };
    let (mine, step) = (id("user", "text"), id("assistant", "tool"));
    let other = service.new_chat();

    let why = format!("{mine} is your own message: a like goes on an agent's words.");
    refused(&data.0, &like(&service, &chat, &mine, json!({})), &why);
    let why = format!("{step} is a step, not a message: a like goes on an agent's words.");
    refused(&data.0, &like(&service, &chat, &step, json!({})), &why);
    let words = id("assistant", "text");
    let why = format!("There's no message {words} in this chat.");
    refused(&data.0, &like(&service, &other, &words, json!({})), &why);
    assert!(likes_shown(&service, &chat).is_empty());
    assert!(liked_turns(&data.0).is_empty());
}
