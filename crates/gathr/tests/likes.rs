//! The person's like (0057) through a real service, on Claude Code's replay stand-in: kept once, live
//! and in history, and taken back; delivered to its agent as a short message of the person's would be,
//! into its running turn or a turn of its own, once; a paused chat, a signed-out agent and a removed
//! one; and each message that takes no like refused with a reason and a reference.

#[path = "likes/acts.rs"]
mod acts;
mod common;
#[path = "words/samples.rs"]
mod samples;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, next_live, rows, team, until};
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

/// The agent's words its `turn` said, once said: the message's id.
fn said_in(service: &TestService, chat: &str, turn: &str) -> String {
    let what = format!("words of turn {turn}");
    let history = service.wait_for(chat, 30, &what, |h| words_of(h, turn).is_some());
    words_of(&history, turn).unwrap()
}

fn words_of(history: &Value, turn: &str) -> Option<String> {
    let messages = history["messages"].as_array()?;
    let found = messages
        .iter()
        .find(|m| m["turn"] == turn && m["role"] == "assistant" && m["kind"] == "text")?;
    found["id"].as_str().map(String::from)
}

fn like(service: &TestService, chat: &str, message: &str, body: Value) -> Response {
    let path = format!("/api/chats/{chat}/messages/{message}/like");
    service.send("PUT", &path, body)
}

fn unlike(service: &TestService, chat: &str, message: &str) -> Response {
    service.call(
        "DELETE",
        &format!("/api/chats/{chat}/messages/{message}/like"),
    )
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

/// The next live reaction event.
fn live_reaction(socket: &mut std::net::TcpStream) -> Value {
    loop {
        let event = next_live(socket);
        if event["type"] == "reaction" {
            return event;
        }
    }
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

/// Gathr's line in `turn` that tells of a like on `message`.
fn line_in(history: &Value, turn: &str, message: &str) -> Option<String> {
    let messages = history["messages"].as_array()?;
    let line = messages
        .iter()
        .find(|m| m["turn"] == turn && m["role"] == "gathr" && m["copy_of"] == message)?;
    line["body"].as_str().map(String::from)
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
fn a_like_is_kept_once_reaches_an_idle_agent_once_and_is_taken_back() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    let turn = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &turn, &["done"]);
    let message = said_in(&service, &chat, &turn);
    let mut socket = service.live();

    let given = like(&service, &chat, &message, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    assert_eq!(given.json()["did"], "delivered", "{}", given.body);
    let event = live_reaction(&mut socket);
    assert_eq!(
        (
            &event["chat"],
            &event["reaction"]["message"],
            &event["reaction"]["agent"]
        ),
        (&json!(chat), &json!(message), &json!("person")),
        "{event}"
    );
    assert_eq!(likes_shown(&service, &chat), [json!(message)]);

    let woken = given.json()["turn"]["turn"].as_str().unwrap().to_string();
    turn_is(&service, &chat, &woken, &["done"]);
    assert_eq!(
        common::opened(&data.0, &json!(woken)),
        (json!("liked"), json!(gathr::words::LIKED))
    );
    let shown = history(&service, &chat);
    let line = line_in(&shown, &woken, &message)
        .unwrap_or_else(|| panic!("no line of Gathr's tells of the like: {shown}"));
    let read = read_in(&data.0, &woken);
    let escaped = serde_json::to_string(&line).unwrap();
    assert!(
        read.contains(escaped.trim_matches('"')),
        "the engine read: {read}"
    );
    let time = shown["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == message);
    let time = time.unwrap()["time"].as_str().unwrap();
    let sampled = line.replace(&message, "<id>").replace(time, "<time>");
    samples::same("liked: a message", &sampled);
    let theirs = shown["messages"].as_array().unwrap().iter();
    let person = theirs
        .filter(|m| m["turn"] == woken && m["role"] == "user")
        .count();
    assert_eq!(
        person, 0,
        "no message of the person's in the woken turn: {shown}"
    );

    // Given twice: one mark, one delivery.
    let again = like(&service, &chat, &message, json!({}));
    assert_eq!(again.json()["did"], "standing", "{}", again.body);
    assert_eq!(likes_shown(&service, &chat), [json!(message)]);
    assert_eq!(liked_turns(&data.0).len(), 1, "{:?}", liked_turns(&data.0));

    // Taken back: the mark goes, live too, and nothing the like did is undone. Twice is no error.
    let back = unlike(&service, &chat, &message);
    assert_eq!(back.json()["removed"], true, "{}", back.body);
    let event = live_reaction(&mut socket);
    assert_eq!(
        (&event["removed"], &event["reaction"]["message"]),
        (&json!(true), &json!(message)),
        "{event}"
    );
    assert!(likes_shown(&service, &chat).is_empty());
    turn_is(&service, &chat, &woken, &["done"]);
    let twice = unlike(&service, &chat, &message);
    assert_eq!(twice.status, 200, "{}", twice.body);
    assert_eq!(twice.json()["removed"], false, "{}", twice.body);

    // Given again: the mark alone; the agent was told once.
    let marked = like(&service, &chat, &message, json!({}));
    assert_eq!(marked.json()["did"], "marked", "{}", marked.body);
    assert!(marked.json()["turn"].is_null(), "{}", marked.body);
    assert_eq!(likes_shown(&service, &chat), [json!(message)]);
    assert_eq!(liked_turns(&data.0).len(), 1, "{:?}", liked_turns(&data.0));
}

#[test]
fn a_like_on_a_working_agents_message_joins_its_running_turn() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &engine(&data.0, &[1], 2), &[]);
    let _go = acts::LetGo(data.0.join("go"));
    let first = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &first, &["done"]);
    let message = said_in(&service, &chat, &first);
    let running = say(&service, &chat, "And the tests");
    turn_is(&service, &chat, &running, &["running"]);

    let given = like(&service, &chat, &message, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    let turn = &given.json()["turn"];
    assert_eq!(
        (&turn["turn"], &turn["steered"]),
        (&json!(running), &json!(true)),
        "{}",
        given.body
    );
    assert!(liked_turns(&data.0).is_empty(), "no turn of its own");
    let shown = history(&service, &chat);
    let line = line_in(&shown, &running, &message)
        .unwrap_or_else(|| panic!("the line isn't in the running turn: {shown}"));
    let escaped = serde_json::to_string(&line).unwrap();
    until(10, "the engine to be sent the line", || {
        read_in(&data.0, &running)
            .contains(escaped.trim_matches('"'))
            .then_some(())
    });
}

#[test]
fn a_like_runs_in_a_paused_chat_waits_for_sign_in_and_reaches_no_removed_agent() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[], 4), &[]);
    let first = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &first, &["done"]);
    let second = say(&service, &chat, "And the tests");
    turn_is(&service, &chat, &second, &["done"]);
    let (one, two) = (
        said_in(&service, &chat, &first),
        said_in(&service, &chat, &second),
    );

    // Paused: the person's like still runs, as their message does.
    let scope = json!({ "scope": "chat", "target": chat });
    assert_eq!(
        service.send("POST", "/api/pauses", scope.clone()).status,
        200
    );
    let given = like(&service, &chat, &one, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    let woken = given.json()["turn"]["turn"].as_str().unwrap().to_string();
    let ran = turn_is(&service, &chat, &woken, &["done", "running"]);
    assert!(ran["held"].is_null(), "{ran}");
    turn_is(&service, &chat, &woken, &["done"]);
    let three = said_in(&service, &chat, &woken);
    let resumed = service.send("POST", "/api/pauses/resume", scope);
    assert_eq!(resumed.status, 200, "{}", resumed.body);

    // Signed out: it waits for a sign-in, as a message does.
    let accounts = std::fs::read_dir(data.0.join("accounts")).unwrap();
    for account in accounts {
        let login = account
            .unwrap()
            .path()
            .join("claude")
            .join(".credentials.json");
        let _ = std::fs::remove_file(login);
    }
    let given = like(&service, &chat, &two, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    let waits = given.json()["turn"]["turn"].as_str().unwrap().to_string();
    turn_is(&service, &chat, &waits, &["waiting_sign_in"]);

    // Removed: the mark alone.
    let deleted = service.call("DELETE", &format!("/api/agents/{lead}"));
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    let marked = like(&service, &chat, &three, json!({}));
    assert_eq!(marked.status, 200, "{}", marked.body);
    assert_eq!(marked.json()["did"], "marked", "{}", marked.body);
    assert_eq!(liked_turns(&data.0).len(), 2, "{:?}", liked_turns(&data.0));
    let shown = likes_shown(&service, &chat);
    assert!(shown.contains(&json!(three)), "{shown:?}");
}

/// Found in the proof: the line asks an agent with nothing to add to end without replying, and a turn that
/// said and did nothing failed, with Retry and the chat marked as unable to run.
#[test]
fn a_woken_turn_that_says_nothing_ends_done_not_failed() {
    let data = TempDir::new();
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let said = frames.trim_end().to_string();
    let quiet: Vec<String> = frames
        .lines()
        .filter(|l| !l.contains("content_block") && !l.contains(r#"\"type\":\"assistant\""#))
        .map(|l| l.replace("Hi!", ""))
        .collect();
    let file = data.0.join("quiet.jsonl");
    std::fs::write(&file, format!("{said}\n{}\n", quiet.join("\n"))).unwrap();
    let (service, chat, _) = team(&data.0, &file, &[]);
    let turn = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &turn, &["done"]);
    let message = said_in(&service, &chat, &turn);

    let given = like(&service, &chat, &message, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    let woken = given.json()["turn"]["turn"].as_str().unwrap().to_string();
    let ended = turn_is(&service, &chat, &woken, &["done", "failed"]);
    assert_eq!(
        (&ended["status"], &ended["why"]),
        (&json!("done"), &Value::Null),
        "{ended}"
    );
    assert!(words_of(&history(&service, &chat), &woken).is_none());
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
