//! What ends a turn (G167): a turn ends on its own `result`, never on the result Claude Code sends
//! for a notice about a background command, and a turn that ends with nothing said and no tool run
//! never settles its task. A restart that cuts a turn stops it: it never fails it.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, held_recording, recording};
use serde_json::{Value, json};

/// `signed_in`'s rows, as the recording has them.
fn rows() -> Vec<String> {
    let text = std::fs::read_to_string(recording("signed_in")).unwrap();
    text.lines().map(String::from).collect()
}

fn out(frame: Value) -> String {
    json!({ "dir": "out", "line": frame.to_string() }).to_string()
}

fn written(dir: &Path, rows: Vec<String>) -> PathBuf {
    let file = dir.join("turn_end.jsonl");
    std::fs::write(&file, rows.join("\n") + "\n").unwrap();
    file
}

fn one_turn(data: &Path, recording: &Path) -> Value {
    let service = TestService::with_engine(data, &[("GATHR_REPLAY", recording)]);
    service.signed_in(data);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] != "running" && h["turns"][0]["status"] != "queued"
    });
    // Anything still to come from the engine has had its moment.
    std::thread::sleep(std::time::Duration::from_millis(500));
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

/// Live, Claude Code 2.1.322 (G167): a resumed session first told of a background command the
/// stop had ended, then sent a result for that notice alone (`origin.kind: task-notification`, no
/// model call), and only then ran the turn on Gathr's message. Gathr took that result as its turn's.
#[test]
fn a_notices_result_doesnt_end_the_turn_it_arrives_in() {
    let data = TempDir::new();
    let rows = rows();
    let session = "a404d9af-ce93-4b65-bb15-1b300040a9f1";
    let notice = json!({ "type": "system", "subtype": "task_notification", "status": "stopped",
        "task_id": "made-up", "output_file": "", "session_id": session,
        "summary": "Background shell command didn't finish before the previous session ended" });
    let result = json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 0,
        "result": "", "origin": { "kind": "task-notification" }, "session_id": session,
        "duration_ms": 67, "usage": { "input_tokens": 0, "output_tokens": 0 } });
    let mut played = vec![rows[0].clone(), out(notice), rows[1].clone(), out(result)];
    played.extend(rows[1..].iter().cloned());
    let history = one_turn(&data.0, &written(&data.0, played));

    let turns = history["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 1, "{history}");
    assert_eq!(turns[0]["status"], "done", "{history}");
    let replies: Vec<&str> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "assistant")
        .filter_map(|m| m["body"].as_str())
        .collect();
    assert_eq!(replies, ["Hi!"], "{history}");
}

/// A turn a restart cut didn't fail: its message reached the agent, which carries on. It reads stopped, cut by
/// the restart (`end`), and its reference leads to the cut. On no task, it carries on all the same.
#[test]
fn a_turn_a_restart_cut_reads_stopped_not_failed_and_carries_on() {
    let data = TempDir::new();
    let held = held_recording(&data.0, "signed_in");
    let go = data.0.join("go");
    let env = [
        ("GATHR_REPLAY", held.as_path()),
        ("GATHR_REPLAY_HOLD", go.as_path()),
    ];
    let service = TestService::with_engine(&data.0, &env);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(&chat, 10, "the turn to run", |h| {
        h["turns"][0]["status"] == "running"
    });
    service.stop();

    let service = TestService::with_engine(&data.0, &env);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let turn = &history["turns"][0];
    let ended = (turn["status"].as_str(), turn["end"].as_str());
    assert_eq!(ended, (Some("stopped"), Some("restart")), "{history}");
    let reference = turn["ref"].as_str().unwrap();
    let (ok, traced) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{traced}");
    assert!(traced.to_string().contains("gathr.turn.cut"), "{traced}");

    std::fs::write(&go, "").unwrap();
    let history = service.wait_for(&chat, 10, "the carry-on's end", |h| {
        h["turns"][1]["status"] == "done"
    });
    let turns = history["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 2, "one carry-on: {history}");
    assert_eq!(turns[1]["answers"], turns[0]["id"], "{history}");
    assert!(turns[1]["task"].is_null(), "{history}");
}
