//! That an agent thought, as a time only (0042), through the real service: the chat's history
//! and the live socket show the thought opening and closing, a cut-off one stops, a turn that
//! only thought is still empty, and Codex's marks are timed by the quiet before them. The engines
//! are played from `thinking_*.jsonl`, hand-written in the shape of real frames (their first row
//! says so).

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, first_token, gathr, next_live, recording};
use serde_json::{Value, json};

/// A service whose engines replay `name`, with a signed-in account for `engine`, and the chat
/// of one agent on it.
fn service(data: &Path, engine: &str, name: &str) -> (TestService, String, Value) {
    let (replay, played) = (common::replay_engine(), recording(name));
    let env = [
        ("GATHR_CLAUDE_PATH", replay.as_path()),
        ("GATHR_CODEX_PATH", replay.as_path()),
        ("GATHR_REPLAY", played.as_path()),
        ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
    ];
    let service = TestService::start_with(data, &env);
    let body = json!({ "engine": engine, "name": "Thinking test" });
    let account = service.send("POST", "/api/accounts", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    if engine == "codex" {
        let folder = data.join("accounts").join(&account).join("codex");
        std::fs::create_dir_all(&folder).unwrap();
        let login = r#"{"tokens":{"refresh_token":"made-up"}}"#;
        std::fs::write(folder.join("auth.json"), login).unwrap();
    } else {
        common::sign_in_account(data, &account);
    }
    let body = json!({ "name": "Made-up", "account": account });
    let made = service.send("POST", "/api/agents", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let agent = made.json()["id"].clone();
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    (service, chat, agent)
}

/// One turn in the chat: its history once the turn is `status`, and every live event up to the
/// turn's end.
fn one_turn(service: &TestService, chat: &str, agent: &Value, status: &str) -> (Value, Vec<Value>) {
    let mut live = service.live();
    let path = format!("/api/chats/{chat}/messages");
    let body = json!({ "text": "Think it through", "agent": agent });
    let sent = service.send("POST", &path, body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let mut heard = Vec::new();
    loop {
        let event = next_live(&mut live);
        let ended = event["type"] == "turn" && event["status"] != "running";
        heard.push(event);
        if ended {
            break;
        }
    }
    let history = service.wait_for(chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == status
    });
    (history, heard)
}

/// A message's time (`2026-11-12T07:10:23.180Z`), in milliseconds since 1970.
fn ms(message: &Value) -> i64 {
    let time = message["time"].as_str().unwrap();
    let n = |at: std::ops::Range<usize>| time[at].parse::<i64>().unwrap();
    // Howard Hinnant's days_from_civil.
    let (y, m, d) = (n(0..4) - i64::from(n(5..7) <= 2), n(5..7), n(8..10));
    let (era, yoe) = (y.div_euclid(400), y.rem_euclid(400));
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    (((days * 24 + n(11..13)) * 60 + n(14..16)) * 60 + n(17..19)) * 1000 + n(20..23)
}

/// The agent's messages in the history, each as its kind, or its tool's name.
fn kinds(history: &Value) -> Vec<&str> {
    let messages = history["messages"].as_array().unwrap().iter();
    messages
        .filter(|m| m["role"] == "assistant")
        .map(|m| {
            m["body"]["tool"]
                .as_str()
                .unwrap_or(m["kind"].as_str().unwrap())
        })
        .collect()
}

fn thoughts(history: &Value) -> Vec<&Value> {
    let messages = history["messages"].as_array().unwrap().iter();
    messages.filter(|m| m["kind"] == "thinking").collect()
}

/// The turn's `gathr.turn.thinking` events, found from its reference.
fn traced(data: &Path, history: &Value) -> Vec<Value> {
    let reference = history["turns"][0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let events = trace["events"].as_array().unwrap().iter();
    events
        .filter(|e| e["name"] == "gathr.turn.thinking")
        .cloned()
        .collect()
}

#[test]
fn a_claude_thought_opens_live_then_closes_with_its_time_and_tokens_and_no_text() {
    let data = TempDir::new();
    let (service, chat, agent) = service(&data.0, "claude", "thinking_claude");
    let (history, heard) = one_turn(&service, &chat, &agent, "done");
    assert_eq!(kinds(&history), ["thinking", "text"], "{history}");
    let thought = thoughts(&history)[0];
    let body = thought["body"].as_object().expect("the body is an object");
    let keys: Vec<&String> = body.keys().collect();
    assert_eq!(keys, ["duration_ms", "status", "tokens"], "{thought}");
    assert_eq!(
        (&body["status"], &body["tokens"]),
        (&json!("done"), &json!(204))
    );
    assert!(body["duration_ms"].is_i64(), "{thought}");

    // Live, as a tool: a message when it starts, then its body when it ends.
    let of_thought =
        |e: &&Value| e["message"]["id"] == thought["id"] || e["message"] == thought["id"];
    let live: Vec<&Value> = heard.iter().filter(of_thought).collect();
    assert_eq!(live.len(), 2, "{heard:?}");
    assert_eq!(live[0]["type"], "message", "{live:?}");
    assert_eq!(live[0]["message"]["kind"], "thinking", "{live:?}");
    assert_eq!(live[0]["message"]["body"], json!({ "status": "running" }));
    assert_eq!(
        (&live[1]["type"], &live[1]["body"]),
        (&json!("tool"), &thought["body"])
    );

    let events = traced(&data.0, &history);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["status"], "ok", "{events:?}");
    assert_eq!(
        events[0]["attrs"],
        json!({ "gathr.thinking.tokens_estimate": 204 })
    );

    // What the agent reads of its chat holds its words, never that it thought.
    let token = first_token(&data.0);
    let read = call_tool(service.info.port, &token, "read_chat", json!({}));
    let read = common::text(&read);
    let said: Vec<&str> = read
        .lines()
        .filter_map(|l| l.split_once(": "))
        .map(|l| l.1)
        .collect();
    assert_eq!(said, ["Think it through", "Thought it through."], "{read}");
}

/// 0019's forced failure: the engine exits mid-thought. The thought closes as stopped, with the
/// time it ran, and the turn's reference finds it.
#[test]
fn a_thought_cut_off_is_stopped_and_found_from_the_turns_reference() {
    let data = TempDir::new();
    let (service, chat, agent) = service(&data.0, "claude", "thinking_cut_off");
    let (history, _) = one_turn(&service, &chat, &agent, "failed");
    let kept = thoughts(&history);
    assert_eq!(kept.len(), 1, "{history}");
    assert_eq!(kept[0]["body"]["status"], "stopped", "{history}");
    assert_eq!(kept[0]["body"]["tokens"], 40, "{history}");
    let events = traced(&data.0, &history);
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(events[0]["status"], "error", "{events:?}");
    assert_eq!(events[0]["error_type"], "stopped", "{events:?}");
}

/// G167: thinking is no reply and no tool call, so the turn did nothing.
#[test]
fn a_turn_that_only_thought_still_fails_as_empty() {
    let data = TempDir::new();
    let (service, chat, agent) = service(&data.0, "claude", "thinking_only");
    let (history, _) = one_turn(&service, &chat, &agent, "failed");
    let why = history["turns"][0]["why"].as_str().unwrap_or_default();
    assert!(why.contains("no reply and no tool call"), "{history}");
    assert_eq!(thoughts(&history)[0]["body"]["status"], "done", "{history}");
}

/// A count that arrives while the agent's own tool runs opens no thought: it may be a
/// sub-agent's, or one that came late, and nothing would close it until the next words or tool.
#[test]
fn a_count_inside_a_running_step_opens_no_thought() {
    let data = TempDir::new();
    let (service, chat, agent) = service(&data.0, "claude", "thinking_in_a_step");
    let (history, _) = one_turn(&service, &chat, &agent, "done");
    assert_eq!(kinds(&history), ["Agent", "text"], "{history}");
}

/// The service dies mid-turn, as a crash or a kill would end it: the turn fails at the next
/// start, and its thought and its tool, both left running, are stopped with it. How long the
/// thought ran is not known, so it says no time.
#[test]
fn a_restart_stops_the_steps_a_dead_service_left_running() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let (replay, played) = (common::replay_engine(), recording("thinking_claude"));
    let env = [
        ("GATHR_CLAUDE_PATH", replay.as_path()),
        ("GATHR_REPLAY", played.as_path()),
        ("GATHR_REPLAY_HOLD", go.as_path()),
    ];
    let service = TestService::start_with(&data.0, &env);
    let account = service.add_account("Thinking test");
    common::sign_in_account(&data.0, &account);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Think it through" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let history = service.wait_for(&chat, 10, "the thought to open", |h| {
        thoughts(h)
            .first()
            .is_some_and(|t| t["body"]["status"] == "running")
    });
    let turn = history["turns"][0]["id"].as_str().unwrap();
    common::kill(&service.info.pid.to_string());
    let mut service = service;
    service.child.wait().unwrap();
    // The engine it left goes on and ends; the turn also had a tool under way.
    std::fs::write(&go, "").unwrap();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    let running = r#"{"tool":"Bash","status":"running","input":{"text":"cargo check"}}"#;
    gathr::store::add_message(&conn, &chat, turn, "assistant", "tool", running).unwrap();
    // A step whose body isn't JSON stays as it is, and doesn't stop the others' recovery.
    gathr::store::add_message(&conn, &chat, turn, "assistant", "tool", "not json").unwrap();
    drop(conn);

    let service = TestService::start_with(&data.0, &env);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(history["turns"][0]["status"], "failed", "{history}");
    let messages = history["messages"].as_array().unwrap().iter();
    let steps: Vec<&Value> = messages
        .filter(|m| m["kind"] != "text")
        .map(|m| &m["body"])
        .collect();
    let stopped = [
        json!({ "status": "stopped" }),
        json!({ "tool": "Bash", "status": "stopped", "input": { "text": "cargo check" } }),
        json!("not json"),
    ];
    assert_eq!(steps, stopped.iter().collect::<Vec<_>>(), "{history}");
}

/// Codex says only that it thought, once it has: each mark is timed by the quiet before it. Of
/// the fixture's four, the one after a second of quiet and the one a second after the command
/// ended are kept; the one right after another and the one while the command ran are not.
#[test]
fn a_codex_mark_is_a_thought_as_long_as_the_quiet_before_it_or_nothing() {
    let data = TempDir::new();
    let (service, chat, agent) = service(&data.0, "codex", "thinking_codex");
    let (history, heard) = one_turn(&service, &chat, &agent, "done");
    assert_eq!(
        kinds(&history),
        ["thinking", "Shell", "thinking", "text"],
        "{history}"
    );
    for thought in thoughts(&history) {
        let body = &thought["body"];
        assert_eq!(body["status"], "done", "{thought}");
        let took = body["duration_ms"].as_i64().unwrap();
        assert!(took >= 1_000, "kept under a second: {thought}");
        assert_eq!(body.get("tokens"), None, "Codex gives no count: {thought}");
        // Its time is when it began, as a tool's is: the quiet's start, not the mark.
        let all = history["messages"].as_array().unwrap();
        let at = all.iter().position(|m| m["id"] == thought["id"]).unwrap();
        let (before, after) = (ms(&all[at - 1]), ms(&all[at + 1]));
        let began = ms(thought);
        assert!(
            before <= began && began + took <= after + 50,
            "{before} {began} +{took} {after}"
        );
        // It arrives whole: one live message, already done.
        let live = heard.iter().filter(|e| e["message"]["id"] == thought["id"]);
        let live: Vec<&Value> = live.collect();
        assert_eq!(live.len(), 1, "{heard:?}");
        assert_eq!(live[0]["message"]["body"], *body, "{live:?}");
    }
    // Each took its own quiet, not the turn so far: together they fit in the turn's time, which
    // a second mark timed from the turn's start would not.
    let kept = thoughts(&history);
    let took: i64 = kept
        .iter()
        .map(|t| t["body"]["duration_ms"].as_i64().unwrap())
        .sum();
    let all = history["messages"].as_array().unwrap();
    let turn_ms = ms(all.last().unwrap()) - ms(&all[0]);
    assert!(took <= turn_ms + 50, "{took} of {turn_ms}: {history}");
    assert_eq!(traced(&data.0, &history).len(), 2);
}
