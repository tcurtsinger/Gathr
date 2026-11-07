//! An account's models and effort levels, from its Claude Code's `initialize` (0025), through
//! the real service with the replay engine answering as Claude Code 2.1.318 did.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, replay_engine, sign_in};
use gathr::http;
use serde_json::{Value, json};

fn models(service: &TestService) -> Value {
    let answer = service.call("GET", "/api/accounts/main/models");
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()
}

fn model<'a>(list: &'a Value, id: &str) -> &'a Value {
    let found = list["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["id"] == id);
    found.unwrap_or_else(|| panic!("{id} isn't listed: {list}"))
}

fn runs(args_file: &std::path::Path) -> usize {
    std::fs::read_to_string(args_file)
        .unwrap_or_default()
        .lines()
        .count()
}

#[test]
fn the_list_comes_from_the_engine_once_and_again_after_sign_in() {
    let data = TempDir::new();
    let args = data.0.join("args.jsonl");
    let service = TestService::start_with(&data.0, &[("GATHR_REPLAY_ARGS", &args)]);
    let saved = service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    assert_eq!(saved.status, 200, "{}", saved.body);

    let signed_out = models(&service);
    assert_eq!(signed_out["version"], "2.1.318");
    assert_eq!(
        signed_out["models"].as_array().unwrap().len(),
        5,
        "{signed_out}"
    );
    assert_eq!(models(&service), signed_out);
    assert_eq!(runs(&args), 1, "a second ask started the engine again");

    sign_in(&data.0);
    let signed_in = models(&service);
    assert_eq!(runs(&args), 2, "signing in didn't ask the engine again");
    assert_eq!(
        signed_in["models"].as_array().unwrap().len(),
        13,
        "{signed_in}"
    );
    let opus = model(&signed_in, "opus");
    assert_eq!(opus["name"], "Opus 5.5");
    assert_eq!(
        opus["efforts"],
        json!(["low", "medium", "high", "xhigh", "max"])
    );
    assert_eq!(
        model(&signed_in, "claude-haiku-4-5-20251001")["efforts"],
        json!([]),
        "a model with no effort levels lists none"
    );
}

/// Starts the service with the replay engine in `mode` (`GATHR_REPLAY_INIT`), asks for the list
/// on a thread, and stops the service once the engine has started.
fn stop_while_asking(data: &Path, mode: &str) -> Duration {
    let args = data.join("args.jsonl");
    let mode = [
        ("GATHR_REPLAY_ARGS", args.as_path()),
        ("GATHR_REPLAY_INIT", Path::new(mode)),
    ];
    let service = TestService::start_with(data, &mode);
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    let (port, token) = (service.info.port, service.info.token.clone());
    let asking =
        std::thread::spawn(move || http::call(port, &token, "GET", "/api/accounts/main/models"));
    let deadline = Instant::now() + Duration::from_secs(10);
    while runs(&args) == 0 {
        assert!(Instant::now() < deadline, "the engine never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    std::thread::sleep(Duration::from_millis(300));
    let began = Instant::now();
    service.stop();
    let took = began.elapsed();
    let _ = asking.join();
    took
}

fn closes(data: &Path) -> Vec<Value> {
    let sql = "SELECT status, error_type FROM event_log WHERE name = 'gathr.engine.models.close'";
    let (ok, rows) = gathr(data, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

#[test]
fn shutdown_ends_an_ask_the_engine_never_answers() {
    let data = TempDir::new();
    let took = stop_while_asking(&data.0, "silent");
    assert!(
        took < Duration::from_secs(5),
        "the service took {took:?} to stop"
    );
    let closes = closes(&data.0);
    assert_eq!(
        closes.len(),
        1,
        "the engine's close wasn't recorded: {closes:?}"
    );
    assert_eq!(closes[0]["status"], "ok", "{closes:?}");
}

#[test]
fn shutdown_waits_for_an_engine_closing_after_its_answer_then_ends_it() {
    let data = TempDir::new();
    let took = stop_while_asking(&data.0, "linger");
    assert!(
        took < Duration::from_secs(15),
        "the service took {took:?} to stop"
    );
    let closes = closes(&data.0);
    assert_eq!(
        closes.len(),
        1,
        "the engine's close wasn't recorded: {closes:?}"
    );
    assert_eq!(closes[0]["error_type"], "killed", "{closes:?}");
}

#[test]
fn a_list_that_fails_has_a_reference_that_traces() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let missing = data.0.join("no-such-claude.exe");
    service.send("PUT", "/api/engines/claude", json!({ "path": missing }));

    let answer = service.call("GET", "/api/accounts/main/models");
    assert_eq!(answer.status, 500, "{}", answer.body);
    let answer = answer.json();
    assert_eq!(answer["error"], "Models failed");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let asked = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "gathr.engine.models")
        .unwrap_or_else(|| panic!("no gathr.engine.models in the trace: {trace}"));
    assert_eq!(asked["status"], "error");
    assert!(
        asked["attrs"]["why"]
            .as_str()
            .unwrap()
            .contains("no-such-claude"),
        "{asked}"
    );
}
