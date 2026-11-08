//! One engine per agent (0001, 0010 Sessions): an agent runs one turn at a time across its chats,
//! keeps a session in each chat, and its engine closes the gentle way to switch or when idle,
//! never killed: one that won't exit holds the switch and shows as a hold on the agent.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, recording, sign_in_account};
use serde_json::{Value, json};

/// Every start's arguments, in order.
fn runs(args_file: &Path) -> Vec<Vec<String>> {
    let runs = std::fs::read_to_string(args_file).unwrap_or_default();
    runs.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn resumed(args: &[String]) -> bool {
    args.iter().any(|a| a == "--resume")
}

fn statuses(history: &Value) -> Vec<String> {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let status = |t: &Value| t["status"].as_str().unwrap_or_default().to_string();
    turns.iter().map(status).collect()
}

/// `signed_in` with the engine held after the message arrives, until `go` exists.
fn held_recording(dir: &Path) -> PathBuf {
    let held = dir.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    held
}

/// A signed-in account and an agent on it: the agent's id.
fn agent(service: &TestService, data: &Path, name: &str) -> String {
    let account = service.add_account(&format!("{name}'s"));
    sign_in_account(data, &account);
    let added = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    added.json()["id"].as_str().unwrap().to_string()
}

fn send(service: &TestService, chat: &str, agent: &str) -> Value {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

fn query(data: &Path, sql: &str) -> Vec<Value> {
    let (ok, rows) = gathr(data, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

fn until<T>(secs: u64, what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "waited {secs} s for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn an_agent_runs_one_turn_at_a_time_across_chats_and_keeps_a_session_in_each() {
    let data = TempDir::new();
    let (go, args) = (data.0.join("go"), data.0.join("args.jsonl"));
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ARGS", &args),
        ],
    );
    let (orin, cass) = (
        agent(&service, &data.0, "Orin"),
        agent(&service, &data.0, "Cass"),
    );
    let (a, b, c) = (service.new_chat(), service.new_chat(), service.new_chat());

    assert_eq!(send(&service, &a, &orin)["status"], "running");
    assert_eq!(
        send(&service, &b, &orin)["status"],
        "queued",
        "a second turn for a busy agent started in another chat"
    );
    // Another agent isn't held up by Orin's turn.
    assert_eq!(send(&service, &c, &cass)["status"], "running");
    assert_eq!(send(&service, &c, &orin)["status"], "queued");

    // Stop in one chat leaves the agent's turns in other chats alone.
    let stopped = service.send("POST", &format!("/api/chats/{c}/stop"), json!({}));
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    let history = service.call("GET", &format!("/api/chats/{a}")).json();
    assert_eq!(statuses(&history), ["running"], "{history}");
    let history = service.call("GET", &format!("/api/chats/{b}")).json();
    assert_eq!(statuses(&history), ["queued"], "{history}");

    std::fs::write(&go, "").unwrap();
    service.wait_for(&b, 10, "chat B's turn, after A's", |h| {
        statuses(h) == ["done"]
    });
    send(&service, &a, &orin);
    service.wait_for(&a, 10, "chat A's second turn", |h| {
        statuses(h) == ["done", "done"]
    });
    send(&service, &a, &cass);
    service.wait_for(&a, 10, "Cass's first turn in chat A", |h| {
        statuses(h).len() == 3 && statuses(h)[2] == "done"
    });

    // Orin in A (new), Cass in C (new), Orin in B (new), Orin in A (resumed), Cass in A (new).
    let runs = runs(&args);
    let resumes: Vec<bool> = runs.iter().map(|r| resumed(r)).collect();
    assert_eq!(resumes, [false, false, false, true, false], "{runs:?}");
}

#[test]
fn a_switch_held_by_an_engine_that_wont_exit_shows_as_a_hold_and_is_never_killed() {
    let data = TempDir::new();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_INIT", Path::new("linger")),
            ("GATHR_REPLAY_LINGER_MS", Path::new("2500")),
            ("GATHR_HOLD_AFTER_MS", Path::new("300")),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let (a, b) = (service.new_chat(), service.new_chat());
    send(&service, &a, &orin);
    service.wait_for(&a, 10, "the first turn", |h| statuses(h) == ["done"]);

    // Chat B needs its own session, so A's engine closes; it lingers past the hold time.
    let started = Instant::now();
    let waiting = send(&service, &b, &orin);
    assert_eq!(waiting["status"], "queued");
    let hold = until(5, "the agent to show a hold", || {
        let agents = service.call("GET", "/api/agents").json();
        let hold = agents["agents"][0]["hold"].clone();
        (!hold.is_null()).then_some(hold)
    });
    assert!(hold["why"].is_string(), "{hold}");
    let history = service.call("GET", &format!("/api/chats/{b}")).json();
    assert_eq!(
        statuses(&history),
        ["queued"],
        "the next turn started while the old engine was still there: {history}"
    );

    service.wait_for(&b, 10, "the turn after the hold", |h| {
        statuses(h) == ["done"]
    });
    assert!(
        started.elapsed() >= Duration::from_millis(2000),
        "the lingering engine was cut short after {:?}",
        started.elapsed()
    );
    let agents = service.call("GET", "/api/agents").json();
    assert!(agents["agents"][0]["hold"].is_null(), "{agents}");
    let closes = query(
        &data.0,
        "SELECT error_type, json_extract(attrs, '$.\"process.exit.code\"') AS code \
         FROM event_log WHERE name = 'gathr.engine.close'",
    );
    assert_eq!(closes.len(), 1, "{closes:?}");
    assert!(
        closes[0]["error_type"].is_null() && closes[0]["code"] == 0,
        "the held engine didn't exit on its own: {closes:?}"
    );
    let holds = query(
        &data.0,
        "SELECT ref FROM event_log WHERE name = 'gathr.engine.hold'",
    );
    assert_eq!(holds.len(), 1, "the hold wasn't recorded: {holds:?}");
    // The waiting turn's reference explains why it waited (0019).
    let reference = waiting["ref"].as_str().unwrap();
    let (_, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(
        trace.to_string().contains("gathr.engine.hold"),
        "the turn's reference doesn't find the hold: {trace}"
    );
}

#[test]
fn an_idle_engine_closes_gently_and_the_next_turn_resumes_its_session() {
    let data = TempDir::new();
    let args = data.0.join("args.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_IDLE_CLOSE_MS", Path::new("300")),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let chat = service.new_chat();
    send(&service, &chat, &orin);
    service.wait_for(&chat, 10, "the first turn", |h| statuses(h) == ["done"]);

    let sql = "SELECT error_type, json_extract(attrs, '$.why') AS why, \
               json_extract(attrs, '$.\"process.exit.code\"') AS code \
               FROM event_log WHERE name = 'gathr.engine.close'";
    let closed = until(5, "the idle close", || query(&data.0, sql).pop());
    assert!(
        closed["error_type"].is_null() && closed["code"] == 0,
        "{closed}"
    );
    assert!(
        closed["why"].as_str().unwrap_or_default().contains("idle"),
        "{closed}"
    );

    send(&service, &chat, &orin);
    service.wait_for(&chat, 10, "the turn after the idle close", |h| {
        statuses(h) == ["done", "done"]
    });
    let runs = runs(&args);
    assert_eq!(runs.len(), 2, "{runs:?}");
    assert!(resumed(&runs[1]), "the session wasn't resumed: {runs:?}");
}
