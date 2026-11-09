//! Pause, Stop now, usage holds and background commands through a real service (0010, F10): a
//! pause holds Gathr's turns but never the person's, and outlasts a restart; Stop now ends what
//! runs and leaves a carry-on for Restart; a usage limit holds the turn instead of failing it; and
//! the turn Claude Code starts itself when a background command ends is kept.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, recording, sign_in_account, text, until};
use serde_json::{Value, json};

/// A signed-in chat with a Supervisor (Lead) and a member (Ann): the service, the chat, the lead.
fn team(data: &Path, replay: &Path, more: &[(&str, &Path)]) -> (TestService, String, String) {
    let mut env = vec![
        ("GATHR_REPLAY", replay),
        ("GATHR_REPLAY_HOLD", Path::new("")),
        ("GATHR_REPLAY_ENV", Path::new("")),
    ];
    let (go, tokens) = (data.join("go"), data.join("env.jsonl"));
    env[1].1 = &go;
    env[2].1 = &tokens;
    env.extend_from_slice(more);
    let service = TestService::with_engine(data, &env);
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    (service, chat, lead)
}

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
    common::held_recording(dir)
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

fn token(data: &Path) -> String {
    until(10, "the engine's token", || {
        let text = std::fs::read_to_string(data.join("env.jsonl")).ok()?;
        let line: Value = serde_json::from_str(text.lines().next()?).ok()?;
        line["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

#[test]
fn a_pause_holds_gathrs_turns_not_the_persons_and_outlasts_a_restart() {
    let data = TempDir::new();
    let both = joined(
        &data.0,
        "both.jsonl",
        &[held(&data.0), recording("signed_in")],
    );
    let (service, chat, _) = team(&data.0, &both, &[]);
    let paused = pause(&service, "/api/pauses", &chat);
    assert_eq!(
        paused["pauses"][0]["state"], "paused",
        "nothing runs yet: {paused}"
    );
    assert_eq!(
        send(&service, &chat, "Plan it")["status"],
        "running",
        "the person's own message"
    );
    let lead = token(&data.0);
    let ask = json!({ "to": "Ann", "what": "Write the notes" });
    let given = text(&call_tool(service.info.port, &lead, "give_task", ask));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    let history = service.wait_for(&chat, 10, "Ann's turn held", |h| {
        h["turns"][1]["held"] == "pause"
    });
    assert_eq!(
        history["run"]["state"], "pausing",
        "the lead's turn still runs: {history}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "nothing running", |h| {
        h["turns"][0]["status"] == "done" && h["run"]["state"] == "paused"
    });
    assert_eq!(history["turns"][1]["status"], "queued", "{history}");
    service.stop();

    let again = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let history = again.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(
        history["turns"][1]["held"], "pause",
        "held after a restart: {history}"
    );
    assert_eq!(history["run"]["state"], "paused", "{history}");
    pause(&again, "/api/pauses/resume", &chat);
    again.wait_for(&chat, 20, "Ann's turn after Resume", |h| {
        h["turns"][1]["status"] == "done"
    });
}

#[test]
fn stop_now_ends_the_turn_and_its_carry_on_waits_for_restart() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let parts = [recording("stopped_before_reply"), recording("signed_in")];
    let replay = joined(&data.0, "stop.jsonl", &parts);
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Write 2,000 words on rivers");
    service.wait_for(&chat, 10, "the turn", |h| {
        h["turns"][0]["status"] == "running"
    });
    let stopped = pause(&service, "/api/pauses/stop", &chat);
    assert_eq!(stopped["pauses"][0]["state"], "stopped", "{stopped}");
    let history = service.wait_for(&chat, 10, "the carry-on", |h| {
        h["turns"][0]["status"] == "stopped" && h["turns"][1]["held"] == "pause"
    });
    assert_eq!(history["run"]["state"], "stopped", "{history}");
    assert_eq!(history["turns"][1]["agent"], lead.as_str());
    assert_eq!(
        history["turns"][1]["answers"], history["turns"][0]["id"],
        "{history}"
    );
    let said = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "gathr");
    let said = said
        .map(|m| m["body"].as_str().unwrap())
        .unwrap_or_default();
    assert!(
        said.starts_with("The person stopped your turn with Stop now."),
        "{said}"
    );
    pause(&service, "/api/pauses/resume", &chat);
    let history = service.wait_for(&chat, 20, "the carry-on after Restart", |h| {
        h["turns"][1]["status"] == "done"
    });
    assert_eq!(history["run"]["state"], "running", "{history}");
}

#[test]
fn a_usage_limit_holds_the_turn_until_the_reset_instead_of_failing_it() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &recording("usage_limit"), &[]);
    send(&service, &chat, "Say hi");
    let history = service.wait_for(&chat, 10, "the usage hold", |h| {
        h["turns"][0]["held"] == "usage"
    });
    let turn = &history["turns"][0];
    assert_eq!(
        turn["status"], "queued",
        "sent again at the reset, as no tool ran: {history}"
    );
    assert!(
        turn["why"]
            .as_str()
            .unwrap()
            .starts_with("out of usage until 2100-01-01"),
        "{turn}"
    );
    let lead_seat = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == lead.as_str());
    assert_eq!(
        lead_seat.unwrap()["usage_until_us"],
        4_102_444_800_000_000_i64,
        "{history}"
    );
    let listed = service.call("GET", "/api/pauses").json();
    assert_eq!(listed["usage"][0]["kind"], "five_hour", "{listed}");
    let (ok, events) = gathr(
        &data.0,
        &[
            "diag",
            "query",
            "SELECT name FROM event_log WHERE name = 'gathr.usage.hold'",
        ],
    );
    assert!(
        ok && events["rows"].as_array().is_some_and(|r| r.len() == 1),
        "{events}"
    );
}

#[test]
fn the_turn_claude_code_starts_when_a_background_command_ends_is_kept() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &recording("background"), &[]);
    send(&service, &chat, "Run the pings in the background");
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let paused = pause(&service, "/api/pauses", &chat);
    let still = &paused["pauses"][0];
    assert_eq!(
        still["state"], "pausing",
        "a background command runs: {paused}"
    );
    assert_eq!(
        still["running"]["background"][0]["command"],
        "Run first ping in background"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the engine's own turn", |h| {
        h["turns"][1]["status"] == "done"
    });
    assert_eq!(
        history["turns"][1]["answers"], history["turns"][0]["id"],
        "{history}"
    );
    let last = history["messages"].as_array().unwrap().last().unwrap()["body"].clone();
    assert!(
        last.as_str().unwrap().starts_with("The first ping"),
        "{history}"
    );
    assert_eq!(history["run"]["state"], "paused", "{history}");
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
