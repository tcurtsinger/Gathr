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
    // Signed out too: a wrong path fails the send rather than holding it for a sign-in.
    let data = TempDir::new();
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

/// 0019: why the engine stopped is on the failed turn itself, and its reference finds it.
#[test]
fn an_engine_that_exits_mid_turn_fails_it_with_its_last_error_line() {
    let data = TempDir::new();
    let service = TestService::start_with(&data.0, &[("GATHR_REPLAY", &recording("engine_exits"))]);
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    sign_in(&data.0);

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let history = service.wait_for(&chat, 10, "the turn to fail", |h| {
        turn_status(h) == Some("failed")
    });
    let turn = history["turns"].as_array().unwrap().last().unwrap().clone();
    let line = "error: unknown option '--forward-subagent-text'";
    let why = turn["why"].as_str().unwrap_or_default();
    assert_eq!(
        why,
        format!("Claude Code exited during the turn (exit code 1): {line}"),
        "{turn}"
    );

    let (ok, trace) = gathr(&data.0, &["diag", "trace", turn["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    assert!(
        trace.to_string().contains("unknown option"),
        "the reference doesn't find the error line: {trace}"
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

/// G13, as Claude Code 2.1.318 does it with no step to stop at: it ends the turn, then reads
/// the message sent into it and answers in a turn of its own. Recorded live.
#[test]
fn a_message_sent_into_a_turn_and_read_after_it_gets_its_own_turn_and_answer() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::start_with(
        &data.0,
        &[("GATHR_REPLAY", &recording("steered_after_turn"))],
    );
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    let path = format!("/api/chats/{chat}/messages");
    let first = service.send(
        "POST",
        &path,
        json!({ "text": "Count slowly from 1 to 200, one number per line. Use no tools." }),
    );
    assert_eq!(first.json()["status"], "running", "{}", first.body);
    let steer = "Change of plan: stop counting now and reply with only the word STEERED.";
    let second = service.send("POST", &path, json!({ "text": steer }));
    assert_eq!(second.json()["steered"], true, "{}", second.body);

    let history = service.wait_for(&chat, 10, "two turns, both done", |h| {
        let turns = h["turns"].as_array().map_or(0, |t| t.len());
        turns == 2
            && h["turns"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["status"] == "done")
    });
    let second_turn = history["turns"][1]["id"].clone();
    let in_second: Vec<&Value> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["turn"] == second_turn)
        .collect();
    assert_eq!(
        in_second.len(),
        2,
        "the second turn should hold the message and its answer: {in_second:?}"
    );
    assert_eq!(in_second[0]["body"], steer);
    assert!(
        in_second[1]["body"].as_str().unwrap().contains("STEERED"),
        "{:?}",
        in_second[1]
    );
}

/// A tool approval asked over `--permission-prompt-tool stdio` and answered through the API.
/// Recorded live: Claude Code 2.1.318 asked to run Write, was allowed, and ran it.
#[test]
fn an_approval_answered_through_the_api_lets_the_tool_run() {
    let data = TempDir::new();
    sign_in(&data.0);
    let fixture = recording("approval");
    let request = std::fs::read_to_string(&fixture)
        .unwrap()
        .lines()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter_map(|row| serde_json::from_str::<Value>(row["line"].as_str().unwrap()).ok())
        .find(|f| f["request"]["subtype"] == "can_use_tool")
        .map(|f| f["request_id"].as_str().unwrap().to_string())
        .expect("the recording asks for an approval");
    let service = TestService::start_with(&data.0, &[("GATHR_REPLAY", &fixture)]);
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    let text = "Create a file named note.txt in the current folder containing the word hi. Use the Write tool.";
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": text }),
    );

    let approve = format!("/api/chats/{chat}/approvals/{request}");
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let answered = loop {
        let answer = service.send("POST", &approve, json!({ "allow": true }));
        if answer.status == 200 || std::time::Instant::now() > deadline {
            break answer;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    assert_eq!(
        answered.status, 200,
        "the approval was never open: {}",
        answered.body
    );
    let history = service.wait_for(&chat, 10, "the turn to end", |h| {
        turn_status(h) == Some("done")
    });
    let tool = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["kind"] == "tool")
        .expect("no tool message");
    assert_eq!(tool["body"]["tool"], "Write");
    assert_eq!(tool["body"]["status"], "ok");
    assert_eq!(
        service
            .send("POST", &approve, json!({ "allow": true }))
            .status,
        404,
        "an answered approval stays open"
    );
}
