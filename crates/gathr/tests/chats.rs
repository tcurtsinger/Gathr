//! Chats with Claude Code through the real service, the engine played back from recordings
//! (`tests/frames`) by `examples/replay_engine.rs`. docs/first-slice.md, hand checks 4 to 9.

mod common;

use std::path::Path;
use std::time::Duration;

use common::{TempDir, TestService, gathr, recording, replay_engine, sign_in};
use serde_json::{Value, json};

fn turn_status(history: &Value) -> Option<&str> {
    history["turns"].as_array()?.last()?["status"].as_str()
}

fn engine_runs(args_file: &Path) -> Vec<Value> {
    std::fs::read_to_string(args_file)
        .unwrap_or_default()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

#[test]
fn a_send_with_no_engine_at_the_set_path_fails_with_a_reference_that_traces() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::start(&data.0);
    let chat = service.new_chat();
    let missing = data.0.join("no-such-claude.exe");
    let saved = service.send("PUT", "/api/engines/claude", json!({ "path": missing }));
    assert_eq!(saved.status, 200, "{}", saved.body);

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "hello" }),
    );
    assert_eq!(sent.status, 500, "{}", sent.body);
    let answer = sent.json();
    assert_eq!(answer["error"], "Send failed");
    assert!(
        answer["why"].as_str().unwrap().contains("isn't a file"),
        "{answer}"
    );

    let reference = answer["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let names: Vec<&str> = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["name"].as_str().unwrap())
        .collect();
    for expected in ["gathr.chat.send", "gathr.engine.start", "gathr.api.request"] {
        assert!(
            names.contains(&expected),
            "{expected} missing from the trace: {names:?}"
        );
    }
    let start = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "gathr.engine.start")
        .unwrap();
    assert_eq!(start["status"], "error");
    assert!(
        start["attrs"]["why"]
            .as_str()
            .unwrap()
            .contains("no-such-claude"),
        "{start}"
    );
}

#[test]
fn a_message_sent_signed_out_waits_and_a_refused_login_waits_again_without_retrying() {
    let data = TempDir::new();
    let args = data.0.join("engine-args.jsonl");
    let service = TestService::start_with(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_out")),
            ("GATHR_REPLAY_ARGS", &args),
        ],
    );
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    assert_eq!(sent.json()["status"], "waiting_sign_in");
    assert!(
        engine_runs(&args).is_empty(),
        "the engine started with no login"
    );

    // A login appears; the engine refuses it, as Claude Code does ("Not logged in").
    sign_in(&data.0);
    service.wait_for(&chat, 10, "the engine to start", |_| {
        !engine_runs(&args).is_empty()
    });
    service.wait_for(&chat, 10, "the turn to wait for sign-in again", |h| {
        turn_status(h) == Some("waiting_sign_in") && engine_runs(&args).len() == 1
    });
    std::thread::sleep(Duration::from_secs(5));
    assert_eq!(
        engine_runs(&args).len(),
        1,
        "a refused login started the engine again"
    );
    let refused = "SELECT count(*) AS n FROM event_log WHERE name = 'invoke_agent claude' AND error_type = 'signed_out'";
    let (_, rows) = gathr(&data.0, &["diag", "query", refused]);
    assert_eq!(
        rows["rows"][0]["n"], 1,
        "the engine's refusal wasn't recorded once: {rows}"
    );
    assert_eq!(
        turn_status(&service.call("GET", &format!("/api/chats/{chat}")).json()),
        Some("waiting_sign_in")
    );
}
