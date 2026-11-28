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
    let why = "Claude Code 2.1.183 is too old. Needs 2.1.318 or later.";
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

/// How many times the stand-in answered `--version`.
fn version_runs(file: &Path) -> usize {
    std::fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .count()
}

/// Says "Say hi" in `chat` and waits for its `n`th turn to fail: `engine_exits` ends each engine
/// with its turn, so every send starts a new one.
fn started(service: &TestService, chat: &str, n: usize) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(chat, 10, &format!("turn {n} to fail"), |h| {
        let turns = h["turns"].as_array().map_or(0, Vec::len);
        turns == n && turn_status(h) == Some("failed")
    });
}

/// Claude Code's version is read once per engine file (0024): a second start on the same file,
/// unchanged, runs no `--version`.
#[test]
fn a_second_start_on_the_same_engine_file_reads_its_version_once() {
    let data = TempDir::new();
    let versions = data.0.join("versions.txt");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_VERSIONS", &versions),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);
    started(&service, &chat, 1);
    started(&service, &chat, 2);
    assert_eq!(version_runs(&versions), 1);
}

/// A file that changed where it is (a new size or modified time) is asked again.
#[test]
fn an_engine_file_that_changed_is_asked_its_version_again() {
    let data = TempDir::new();
    let versions = data.0.join("versions.txt");
    let engine = data
        .0
        .join("engine")
        .join(common::replay_engine().file_name().unwrap());
    std::fs::create_dir_all(engine.parent().unwrap()).unwrap();
    std::fs::copy(common::replay_engine(), &engine).unwrap();
    let service = TestService::start_with(
        &data.0,
        &[
            ("GATHR_CLAUDE_PATH", &engine),
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_VERSIONS", &versions),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);
    started(&service, &chat, 1);
    let file = std::fs::File::options().write(true).open(&engine).unwrap();
    let later = std::time::SystemTime::now() + Duration::from_secs(3600);
    file.set_modified(later).unwrap();
    drop(file);
    started(&service, &chat, 2);
    assert_eq!(version_runs(&versions), 2);
}

/// A `--version` that didn't answer in time isn't remembered: the next start asks again, and can
/// run.
#[test]
fn a_version_that_wasnt_read_is_asked_again_at_the_next_start() {
    let data = TempDir::new();
    let (versions, hold) = (data.0.join("versions.txt"), data.0.join("hold"));
    std::fs::write(&hold, "").unwrap();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_VERSIONS", &versions),
            ("GATHR_REPLAY_VERSION_HOLD", &hold),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.status, 500, "{}", sent.body);
    let why = sent.json()["why"].as_str().unwrap_or_default().to_string();
    assert!(why.ends_with("it didn't answer in 5 s"), "{why}");
    std::fs::remove_file(&hold).unwrap();
    started(&service, &chat, 2);
    assert_eq!(version_runs(&versions), 2);
}

/// A version read as too old stays refused on the next start, in the same words and with the same
/// event, without asking the file again.
#[test]
fn a_claude_code_read_as_too_old_is_refused_again_without_asking_again() {
    let data = TempDir::new();
    let versions = data.0.join("versions.txt");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("engine_exits")),
            ("GATHR_REPLAY_VERSIONS", &versions),
            ("GATHR_REPLAY_VERSION", Path::new("2.1.183 (Claude Code)")),
        ],
    );
    let chat = service.new_chat();
    service.signed_in(&data.0);
    let path = format!("/api/chats/{chat}/messages");
    let why = "Claude Code 2.1.183 is too old. Needs 2.1.318 or later.";
    for _ in 0..2 {
        let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
        assert_eq!(
            (sent.status, &sent.json()["why"]),
            (500, &json!(why)),
            "{}",
            sent.body
        );
        let (ok, trace) = gathr(
            &data.0,
            &["diag", "trace", sent.json()["ref"].as_str().unwrap()],
        );
        assert!(ok, "{trace}");
        let events = trace["events"].as_array().unwrap();
        let start = events.iter().find(|e| e["name"] == "gathr.engine.start");
        let start = start.unwrap_or_else(|| panic!("no engine start in the trace: {trace}"));
        let found = (&start["error_type"], &start["attrs"]["version"]);
        assert_eq!(
            found,
            (&json!("engine_version"), &json!("2.1.183")),
            "{start}"
        );
    }
    assert_eq!(version_runs(&versions), 1);
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
        format!("Claude Code exited (code 1): {line}"),
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
    let read_after = common::opened(&data.0, &second_turn);
    assert_eq!(read_after, (json!("person"), Value::Null), "{history}");
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

/// Claude Code 2.1.332 read three messages sent into a turn as one, joined with line breaks: all three count
/// as read, move to the turn that answers them, and the next turn starts. Made from `steered_after_turn`.
#[test]
fn messages_read_together_after_a_turn_count_as_read_and_the_next_turn_starts() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("read_merged"))]);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let first = service.send(
        "POST",
        &path,
        json!({ "text": "Count slowly from 1 to 200, one number per line. Use no tools." }),
    );
    assert_eq!(first.json()["status"], "running", "{}", first.body);
    let sent = [
        "Also note the folder name.",
        "And keep the list short.",
        "Then reply with only the word MERGED.",
    ];
    for text in sent {
        let steer = service.send("POST", &path, json!({ "text": text }));
        assert_eq!(steer.json()["steered"], true, "{}", steer.body);
    }
    let done = |n: usize| {
        move |h: &Value| {
            let turns = h["turns"].as_array().unwrap();
            turns.len() == n && turns.iter().all(|t| t["status"] == "done")
        }
    };
    let history = service.wait_for(&chat, 10, "the read turn, done", done(2));
    let read_turn = history["turns"][1]["id"].clone();
    let in_read: Vec<&Value> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["turn"] == read_turn)
        .collect();
    let bodies: Vec<&str> = in_read.iter().filter_map(|m| m["body"].as_str()).collect();
    assert_eq!(
        bodies.len(),
        4,
        "the three messages and their answer: {bodies:?}"
    );
    assert_eq!(bodies[..3], sent, "{bodies:?}");
    assert!(bodies[3].contains("MERGED"), "{bodies:?}");

    let next = service.send(
        "POST",
        &path,
        json!({ "text": "Now reply with only the word NEXT." }),
    );
    assert_eq!(next.json()["status"], "running", "{}", next.body);
    service.wait_for(&chat, 10, "the next turn, done", done(3));
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

/// `signed_in` with its reply in two text blocks, as one model call can answer (Vale, #84).
fn two_block_reply(dir: &Path) -> std::path::PathBuf {
    let rows = std::fs::read_to_string(recording("signed_in")).unwrap();
    let blocks = json!([{ "type": "text", "text": "FIRST BLOCK" }, { "type": "text", "text": "SECOND BLOCK" }]);
    let rows: Vec<String> = rows
        .lines()
        .map(|row| {
            let mut row: Value = serde_json::from_str(row).unwrap();
            let mut frame: Value = serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
            if frame["type"] == "assistant" {
                frame["message"]["content"] = blocks.clone();
                row["line"] = frame.to_string().into();
            }
            row.to_string()
        })
        .collect();
    let fixture = dir.join("two-blocks.jsonl");
    std::fs::write(&fixture, rows.join("\n") + "\n").unwrap();
    fixture
}

/// Audit 2.2.6: a reply the database won't take fails its turn with a reference, stays readable as
/// `streaming`, and is saved once a write works again, every block in order. Never dropped while
/// the turn carries on.
#[test]
fn a_reply_that_cant_be_saved_fails_the_turn_and_is_kept_until_it_can() {
    let data = TempDir::new();
    let fixture = two_block_reply(&data.0);
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
    let kept = history["streaming"].to_string();
    assert!(
        kept.contains("FIRST BLOCK") && kept.contains("SECOND BLOCK"),
        "a block of the reply is gone: {history}"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", turn["ref"].as_str().unwrap()]);
    assert!(
        ok && trace.to_string().contains("forced"),
        "the reference doesn't find the failed save: {trace}"
    );

    db.execute("DROP TRIGGER no_reply", []).unwrap();
    say("Again");
    let history = service.wait_for(&chat, 10, "the kept reply to be saved", |h| {
        let replies = h["messages"].as_array().into_iter().flatten();
        replies
            .filter(|m| m["role"] == "assistant" && m["kind"] == "text")
            .count()
            >= 2
    });
    let replies: Vec<&Value> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "assistant")
        .collect();
    let saved: Vec<(&Value, &Value)> = replies.iter().map(|m| (&m["body"], &m["turn"])).collect();
    assert_eq!(
        saved,
        [
            (&json!("FIRST BLOCK"), &turn["id"]),
            (&json!("SECOND BLOCK"), &turn["id"])
        ],
        "the kept blocks weren't saved in order on their turn: {history}"
    );
}

/// A key streamed in two pieces never reaches the raw record in either (Vale, #84): each piece
/// alone looks like nothing, so the run is redacted joined before it's written, even with stderr
/// between the pieces (pauses fix its place, as in Vale's second look).
#[test]
fn a_key_split_across_streamed_frames_is_never_stored() {
    let data = TempDir::new();
    let rows = std::fs::read_to_string(recording("signed_in")).unwrap();
    let split = |text: &str| {
        let frame = json!({ "type": "stream_event", "event": { "type": "content_block_delta", "index": 0,
            "delta": { "type": "text_delta", "text": text } }, "parent_tool_use_id": null });
        json!({ "dir": "out", "line": frame.to_string() }).to_string()
    };
    let mut lines: Vec<String> = Vec::new();
    for row in rows.lines() {
        if row.contains("text_delta") {
            lines.push(split("Your key: sk-pr"));
            for (dir, line) in [
                ("sleep", "150"),
                ("err", "a harmless warning"),
                ("sleep", "150"),
            ] {
                lines.push(json!({ "dir": dir, "line": line }).to_string());
            }
            lines.push(split("oj-0123456789abcdefghijklmnop"));
        } else {
            lines.push(row.to_string());
        }
    }
    let fixture = data.0.join("split-key.jsonl");
    std::fs::write(&fixture, lines.join("\n") + "\n").unwrap();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &fixture)]);
    let chat = service.new_chat();
    service.signed_in(&data.0);
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );
    service.wait_for(&chat, 10, "the turn to end", |h| {
        turn_status(h) == Some("done")
    });

    let (ok, raw) = gathr(&data.0, &["diag", "query", "SELECT body FROM engine_raw"]);
    assert!(ok, "{raw}");
    let raw = raw.to_string();
    assert!(
        raw.contains("[redacted:key]"),
        "the split key wasn't redacted: {raw}"
    );
    for piece in ["sk-pr", "oj-0123456789"] {
        assert!(
            !raw.contains(piece),
            "{piece} reached the raw record: {raw}"
        );
    }
}

/// The background model (Claude Code's `haiku`, the replay engine's print mode) names a chat from a
/// first message of 40 characters or more; a failure or a rubbish answer leaves the first line's
/// title, a short message starts no run, a rename by the person stays, and no event holds the words.
#[test]
fn the_background_model_names_a_new_chat_once_and_never_over_the_person() {
    let data = TempDir::new();
    let (answer, args) = (data.0.join("answer"), data.0.join("args"));
    std::fs::write(&answer, "\"Garden plan for spring.\"\n").unwrap();
    let replay = recording("signed_in");
    let env = [
        ("GATHR_REPLAY", replay.as_path()),
        ("GATHR_REPLAY_ANSWER", answer.as_path()),
        ("GATHR_REPLAY_ARGS", args.as_path()),
    ];
    let service = TestService::with_engine(&data.0, &env);
    service.signed_in(&data.0);
    let long =
        |n: u32| format!("Made-up long message {n}: how should the garden beds be laid out?");
    let title = |chat: &str| {
        let list = service.call("GET", "/api/chats").json();
        let row = list
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == chat)
            .cloned();
        row.unwrap()["title"].as_str().unwrap().to_string()
    };
    let outcome = |chat: &str| {
        let sql = format!(
            "SELECT json_extract(attrs, '$.outcome') AS outcome, json_extract(attrs, '$.why') AS why \
             FROM event_log WHERE name = 'gathr.background.name' AND conversation_id = '{chat}'"
        );
        common::until(20, "the background name's event", || {
            common::rows(&data.0, &sql).first().cloned()
        })
    };
    let runs = || {
        engine_runs(&args)
            .iter()
            .filter(|run| {
                run.as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a == "--no-session-persistence")
            })
            .count()
    };
    let send = |text: &str| {
        let chat = service.new_chat();
        let sent = service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({ "text": text }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        chat
    };

    let named = send(&long(1));
    assert_eq!(outcome(&named)["outcome"], "named");
    assert_eq!(title(&named), "Garden plan for spring");
    // The words go on its input, never in its arguments, which any process can read.
    let run = engine_runs(&args)
        .into_iter()
        .rev()
        .find(|r| r.to_string().contains("--no-session-persistence"));
    let run = run.unwrap().to_string();
    assert!(
        run.contains("--model=haiku") && !run.contains("garden beds"),
        "{run}"
    );

    let rubbish = "Too long a name ".repeat(10);
    for (said, n) in [("fail", 2), (rubbish.as_str(), 3)] {
        std::fs::write(&answer, said).unwrap();
        let chat = send(&long(n));
        assert_eq!(outcome(&chat)["outcome"], "failed", "{said}");
        let kept = title(&chat);
        assert!(
            kept.starts_with(&format!("Made-up long message {n}")),
            "{kept}"
        );
    }

    let before = runs();
    let short = send("Made-up short one");
    assert_eq!(outcome(&short)["outcome"], "skipped");
    assert_eq!(runs(), before, "a short first message started a run");
    assert_eq!(title(&short), "Made-up short one");

    // Renamed while the model is still answering: the person's name stays.
    std::fs::write(&answer, "wait").unwrap();
    let renamed = send(&long(4));
    common::until(20, "the run to start", || (runs() > before).then_some(()));
    let path = format!("/api/chats/{renamed}");
    let refused = service.send("PATCH", &path, json!({ "title": "  " }));
    assert_eq!(refused.status, 409, "{}", refused.body);
    let done = service.send("PATCH", &path, json!({ "title": " Made-up own name " }));
    assert_eq!(done.status, 200, "{}", done.body);
    std::fs::write(&answer, "Garden plan for spring").unwrap();
    assert_eq!(outcome(&renamed)["why"], "the chat was renamed");
    assert_eq!(title(&renamed), "Made-up own name");

    let words = common::rows(
        &data.0,
        "SELECT attrs FROM event_log WHERE attrs LIKE '%garden beds%'",
    );
    assert!(
        words.is_empty(),
        "the message's words are in an event: {words:?}"
    );
}
