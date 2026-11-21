//! Pause, Stop now, usage holds and background commands through a real service (0010, F10): a
//! pause holds Gathr's turns but never the person's, and outlasts a restart; Stop now ends what
//! runs and leaves a carry-on for Restart; a usage limit holds the turn instead of failing it; and
//! the turn Claude Code starts itself when a background command ends is kept.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, recording, sign_in_account, team, until};
use serde_json::{Value, json};

/// Recordings played one after the other by the same engine, one turn each.
fn joined(dir: &Path, name: &str, parts: &[PathBuf]) -> PathBuf {
    let text: Vec<String> = parts
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap().trim_end().to_string())
        .collect();
    let file = dir.join(name);
    std::fs::write(&file, text.join("\n") + "\n").unwrap();
    file
}

fn held(dir: &Path) -> PathBuf {
    common::held_recording(dir, "signed_in")
}

fn send(service: &TestService, chat: &str, text: &str) -> Value {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": text }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

fn pause(service: &TestService, path: &str, chat: &str) -> Value {
    let paused = service.send("POST", path, json!({ "scope": "chat", "target": chat }));
    assert_eq!(paused.status, 200, "{}", paused.body);
    paused.json()
}

#[test]
fn stop_now_keeps_the_carry_on_in_a_chat_sent_to_no_agent() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let parts = [recording("stopped_before_reply"), recording("signed_in")];
    let replay = joined(&data.0, "stop.jsonl", &parts);
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &replay)]);
    let account = service.add_account("Main");
    sign_in_account(&data.0, &account);
    let chat = service.new_chat();
    send(&service, &chat, "Write 2,000 words on rivers");
    service.wait_for(&chat, 10, "the turn", |h| {
        h["turns"][0]["status"] == "running"
    });
    pause(&service, "/api/pauses/stop", &chat);
    let history = service.wait_for(&chat, 10, "the carry-on", |h| {
        h["turns"][0]["status"] == "stopped" && h["turns"][1]["held"] == "pause"
    });
    assert_eq!(history["turns"][1]["agent"], Value::Null, "{history}");
    pause(&service, "/api/pauses/resume", &chat);
    service.wait_for(&chat, 20, "the carry-on after Restart", |h| {
        h["turns"][1]["status"] == "done"
    });
}

/// A turn held by a pause that is gone at the next start, as one agent's after 0069, starts then.
#[test]
fn a_turn_held_by_a_pause_that_is_gone_starts_at_the_next_start() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let stopped = recording("stopped_before_reply");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &stopped)]);
    let account = service.add_account("Main");
    sign_in_account(&data.0, &account);
    let chat = service.new_chat();
    send(&service, &chat, "Write 2,000 words on rivers");
    service.wait_for(&chat, 10, "the turn", |h| {
        h["turns"][0]["status"] == "running"
    });
    pause(&service, "/api/pauses/stop", &chat);
    service.wait_for(&chat, 10, "the held carry-on", |h| {
        h["turns"][1]["held"] == "pause"
    });
    service.stop();
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute("DELETE FROM pauses", []).unwrap();
    drop(db);

    let answers = recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &answers)]);
    service.wait_for(&chat, 20, "the carry-on after the start", |h| {
        h["turns"][1]["status"] == "done"
    });
}

#[test]
fn an_engine_that_closed_leaves_no_background_command_behind() {
    let data = TempDir::new();
    let full = std::fs::read_to_string(recording("background")).unwrap();
    let first: Vec<&str> = full
        .lines()
        .take_while(|l| !l.contains(r#""dir":"hold""#))
        .collect();
    let replay = data.0.join("first.jsonl");
    std::fs::write(&replay, first.join("\n") + "\n").unwrap();
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Run the pings in the background");
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let members = json!([{ "agent": lead, "role": "supervisor" }]);
    let other = service.send("POST", "/api/chats", json!({ "members": members }));
    let other = other.json()["id"].as_str().unwrap().to_string();
    send(&service, &other, "Run the pings in the background");
    service.wait_for(&other, 10, "the turn in another chat", |h| {
        h["turns"][0]["status"] == "done"
    });
    let paused = pause(&service, "/api/pauses", &chat);
    assert_eq!(
        paused["pauses"][0]["state"], "paused",
        "the first chat's engine closed with its command: {paused}"
    );
}

/// What the Agents panel lists: each command the engine still runs, under the agent that started it,
/// until the engine no longer lists it.
#[test]
fn a_background_command_is_in_the_chats_history_until_the_engine_no_longer_lists_it() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &recording("background"), &[]);
    send(&service, &chat, "Run the pings in the background");
    let history = service.wait_for(&chat, 10, "the command", |h| {
        h["turns"][0]["status"] == "done"
            && h["background"].as_array().is_some_and(|b| !b.is_empty())
    });
    let command = &history["background"][0];
    assert_eq!(command["agent"], lead.as_str(), "{history}");
    assert_eq!(command["description"], "Run first ping in background");
    assert!(command["started"].is_string(), "{history}");

    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 20, "the command gone", |h| {
        h["background"].as_array().is_some_and(|b| b.is_empty())
    });
}

#[test]
fn a_turn_with_no_progress_is_flagged_never_stopped() {
    let data = TempDir::new();
    let stall = [("GATHR_STALL_AFTER_MS", Path::new("200"))];
    let (service, chat, _) = team(&data.0, &held(&data.0), &stall);
    send(&service, &chat, "Plan it");
    let sql = "SELECT name, error_type FROM event_log WHERE name = 'gathr.turn.stalled'";
    let found = until(10, "the stall flag", || {
        let (_, events) = gathr(&data.0, &["diag", "query", sql]);
        let rows = events["rows"].as_array().cloned().unwrap_or_default();
        (!rows.is_empty()).then_some(rows)
    });
    assert_eq!(found.len(), 1, "flagged once: {found:?}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(
        history["turns"][0]["status"], "running",
        "never stopped: {history}"
    );
}
