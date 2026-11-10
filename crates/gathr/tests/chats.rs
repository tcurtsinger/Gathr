//! Chats with Claude Code through the real service, the engine played back from recordings
//! (`tests/frames`) by `examples/replay_engine.rs`. docs/first-slice.md, hand checks 4 to 9.

mod common;

use std::path::Path;
use std::time::Duration;

use common::{TempDir, TestService, gathr, recording};
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
    let missing = data.0.join("no-such-claude.exe");
    let service = TestService::start_with(&data.0, &[("GATHR_CLAUDE_PATH", &missing)]);
    let chat = service.new_chat();

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

/// An engine too old for the switches Gathr passes fails the send by its version, before any
/// turn runs, rather than with whatever it prints about an unknown switch.
#[test]
fn a_claude_code_older_than_gathr_needs_fails_the_send_with_its_version_and_a_reference() {
    let data = TempDir::new();
    let runs = data.0.join("runs.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_ARGS", &runs),
            ("GATHR_REPLAY_VERSION", Path::new("2.1.183 (Claude Code)")),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );
    assert_eq!(sent.status, 500, "{}", sent.body);
    let answer = sent.json();
    assert_eq!(answer["error"], "Send failed");
    let why = "Claude Code 2.1.183 is too old: Gathr needs 2.1.318 or later";
    assert_eq!(answer["why"], why, "{answer}");
    assert!(engine_runs(&runs).is_empty(), "an engine too old still ran");

    let (ok, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let start = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "gathr.engine.start")
        .unwrap_or_else(|| panic!("no engine start in the trace: {trace}"));
    assert_eq!(start["status"], "error", "{start}");
    assert_eq!(start["attrs"]["version"], "2.1.183", "{start}");
}

/// A launcher that never answers `--version` fails the send with a reference, rather than
/// leaving the chat unable to answer Send or Stop.
#[test]
fn a_claude_code_that_never_answers_its_version_fails_the_send_in_seconds() {
    let data = TempDir::new();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_VERSION", Path::new("stall")),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);

    let (port, token) = (service.info.port, service.info.token.clone());
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let body = json!({ "text": "Say hi" });
        let path = format!("/api/chats/{chat}/messages");
        let _ = tx.send(gathr::http::send(port, &token, "POST", &path, &body).unwrap());
    });
    let sent = rx
        .recv_timeout(Duration::from_secs(20))
        .expect("Send unanswered after 20 s with a stalled --version");
    assert_eq!(sent.status, 500, "{}", sent.body);
    let answer = sent.json();
    assert!(
        answer["why"].as_str().unwrap().contains("didn't answer"),
        "{answer}"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let failed = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["name"] == "gathr.engine.start" && e["error_type"] == "engine_version");
    assert!(failed, "no engine_version failure in the trace: {trace}");
}

/// 0019: why the engine stopped is on the failed turn itself, and its reference finds it.
#[test]
fn an_engine_that_exits_mid_turn_fails_it_with_its_last_error_line() {
    let data = TempDir::new();
    let service =
        TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("engine_exits"))]);
    let chat = service.new_chat();
    service.signed_in(&data.0);

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
fn an_engine_that_exits_mid_reply_keeps_the_text_already_shown_and_stops_its_tools() {
    let data = TempDir::new();
    let fixture = recording("exits_mid_reply");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &fixture)]);
    let chat = service.new_chat();
    service.signed_in(&data.0);
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );

    let history = service.wait_for(&chat, 10, "the turn to fail", |h| {
        turn_status(h) == Some("failed")
    });
    let messages = history["messages"].as_array().unwrap();
    assert!(
        messages
            .iter()
            .any(|m| m["role"] == "assistant" && m["body"] == "Already visible answer"),
        "the streamed text is missing from the history: {messages:?}"
    );
    let tool = messages.iter().find(|m| m["kind"] == "tool");
    assert_eq!(
        tool.map(|t| t["body"]["status"].clone()),
        Some(json!("stopped")),
        "the open tool call should show stopped: {messages:?}"
    );
}

#[test]
fn a_message_sent_signed_out_waits_and_a_refused_login_waits_again_without_retrying() {
    let data = TempDir::new();
    let args = data.0.join("engine-args.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_out")),
            ("GATHR_REPLAY_ARGS", &args),
        ],
    );
    let chat = service.new_chat();

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
    service.signed_in(&data.0);
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
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_REPLAY", &recording("steered_after_turn"))],
    );
    service.signed_in(&data.0);
    let chat = service.new_chat();
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
    let fixture = recording("approval");
    let request = std::fs::read_to_string(&fixture)
        .unwrap()
        .lines()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter_map(|row| serde_json::from_str::<Value>(row["line"].as_str().unwrap()).ok())
        .find(|f| f["request"]["subtype"] == "can_use_tool")
        .map(|f| f["request_id"].as_str().unwrap().to_string())
        .expect("the recording asks for an approval");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &fixture)]);
    service.signed_in(&data.0);
    let chat = service.new_chat();
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

/// Where the login isn't a file (the macOS Keychain), every start holds the account's gate
/// until its first reply. A turn stopped before any reply must let the next one go at once,
/// not after the gate's 30 s. The next message is queued while the stopped turn still holds
/// the gate: the engine holds its answer to the Stop until the test lets it go.
#[cfg(target_os = "macos")]
#[test]
fn a_turn_stopped_before_any_reply_lets_the_next_message_start_at_once() {
    let data = TempDir::new();
    let go = data.0.join("stop-answered");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("stopped_before_reply")),
            ("GATHR_REPLAY_AUTH", Path::new("0")),
            ("GATHR_REPLAY_HOLD", &go),
        ],
    );
    service.add_account("Test");
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let first = service.send(
        "POST",
        &path,
        json!({ "text": "Write 2,000 words on rivers. Use no tools." }),
    );
    assert_eq!(first.json()["status"], "running", "{}", first.body);
    service.send("POST", &format!("/api/chats/{chat}/stop"), json!({}));
    let started = std::time::Instant::now();
    let second = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(second.json()["status"], "queued", "{}", second.body);
    std::fs::write(&go, "").unwrap();

    service.wait_for(&chat, 10, "the next message to start", |h| {
        h["turns"]
            .as_array()
            .is_some_and(|t| t.len() == 2 && t[1]["status"] == "running")
    });
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "it waited {:?} on the stopped turn's gate",
        started.elapsed()
    );
}

/// `signed_in`, held after its first words stream and before the whole block arrives.
fn held_mid_reply(dir: &Path) -> std::path::PathBuf {
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let mut lines: Vec<&str> = frames.lines().collect();
    let delta = lines.iter().position(|l| l.contains("text_delta")).unwrap();
    lines.insert(delta + 1, r#"{"dir":"hold"}"#);
    let path = dir.join("mid-reply.jsonl");
    std::fs::write(&path, lines.join("\n")).unwrap();
    path
}

/// A window that opens the chat mid-reply (a reconnect, a switch back) shows what has streamed so
/// far, not nothing until the reply ends.
#[test]
fn a_chats_history_holds_the_reply_still_streaming() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let replay = held_mid_reply(&data.0);
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_REPLAY", &replay), ("GATHR_REPLAY_HOLD", &go)],
    );
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service
        .send("POST", &path, json!({ "text": "Say hi" }))
        .json();

    let mid = service.wait_for(&chat, 10, "the streamed words", |h| {
        h["streaming"][0]["text"] == "Hi!"
    });
    assert_eq!(mid["streaming"][0]["turn"], sent["turn"], "{mid}");
    std::fs::write(&go, "").unwrap();
    let after = service.wait_for(&chat, 10, "the reply", |h| turn_status(h) == Some("done"));
    assert_eq!(
        after["streaming"],
        json!([]),
        "a finished reply still streams: {after}"
    );
    let last = after["messages"].as_array().unwrap().last().unwrap();
    assert_eq!(last["body"], "Hi!", "{after}");
}

/// Audit 2.2.6: a reply the database won't take fails its turn with a reference, stays readable as
/// `streaming`, and is saved once a write works again. Never dropped while the turn carries on.
#[test]
fn a_reply_that_cant_be_saved_fails_the_turn_and_is_kept_until_it_can() {
    let data = TempDir::new();
    let fixture = recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &fixture)]);
    let chat = service.new_chat();
    service.signed_in(&data.0);
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    let refuse = "CREATE TRIGGER no_reply BEFORE INSERT ON messages WHEN NEW.role = 'assistant' \
                  AND NEW.kind = 'text' BEGIN SELECT RAISE(ABORT, 'forced'); END";
    db.execute(refuse, []).unwrap();
    let say = |text: &str| {
        service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({ "text": text }),
        )
    };
    say("Say hi");

    let history = service.wait_for(&chat, 10, "the turn to fail", |h| {
        turn_status(h) == Some("failed")
    });
    let turn = history["turns"].as_array().unwrap().last().unwrap().clone();
    let why = turn["why"].as_str().unwrap_or_default();
    assert!(
        why.contains("wasn't saved"),
        "the turn doesn't say why: {turn}"
    );
    let kept = history["streaming"].as_array().cloned().unwrap_or_default();
    assert!(
        kept.iter()
            .any(|s| s["text"].as_str().is_some_and(|t| t.starts_with("Hi!"))),
        "the reply is gone: {history}"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", turn["ref"].as_str().unwrap()]);
    assert!(
        ok && trace.to_string().contains("forced"),
        "the reference doesn't find the failed save: {trace}"
    );

    db.execute("DROP TRIGGER no_reply", []).unwrap();
    say("Again");
    let history = service.wait_for(&chat, 10, "the kept reply to be saved", |h| {
        let mut replies = h["messages"].as_array().into_iter().flatten();
        replies.any(|m| m["role"] == "assistant" && m["kind"] == "text")
    });
    let first = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant")
        .unwrap();
    assert_eq!(
        first["turn"], turn["id"],
        "the kept reply was saved on another turn: {history}"
    );
}
