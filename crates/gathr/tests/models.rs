//! An account's models and effort levels, from its Claude Code's `initialize` (0025), through
//! the real service with the replay engine answering as Claude Code 2.1.318 did.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, sign_in_account};
use gathr::http;
use serde_json::{Value, json};

fn models(service: &TestService, account: &str) -> Value {
    let answer = service.call("GET", &format!("/api/accounts/{account}/models"));
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
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY_ARGS", &args)]);
    let first = service.add_account("First");

    let signed_out = models(&service, &first);
    assert_eq!(signed_out["version"], "2.1.318");
    assert_eq!(
        signed_out["models"].as_array().unwrap().len(),
        5,
        "{signed_out}"
    );
    assert_eq!(models(&service, &first), signed_out);
    assert_eq!(runs(&args), 1, "a second ask started the engine again");

    sign_in_account(&data.0, &first);
    let signed_in = models(&service, &first);
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

    let work = json!({ "engine": "claude", "name": "Work" });
    let work = service.send("POST", "/api/accounts", work).json();
    let path = format!("/api/accounts/{}/models", work["id"].as_str().unwrap());
    let work_models = service.call("GET", &path).json();
    assert_eq!(
        work_models["models"].as_array().map(Vec::len),
        Some(5),
        "a signed-out account got the signed-in one's list: {work_models}"
    );
    assert_eq!(
        models(&service, &first),
        signed_in,
        "Work's ask replaced First's list"
    );
    assert_eq!(runs(&args), 3, "First's list was asked again");
}

/// Starts the service with the replay engine in `mode` (`GATHR_REPLAY_INIT`), asks for the list
/// on a thread, and stops the service once the engine has started.
fn stop_while_asking(data: &Path, mode: &str) -> Duration {
    let args = data.join("args.jsonl");
    let mode = [
        ("GATHR_REPLAY_ARGS", args.as_path()),
        ("GATHR_REPLAY_INIT", Path::new(mode)),
    ];
    let service = TestService::with_engine(data, &mode);
    let path = format!("/api/accounts/{}/models", service.add_account("Test"));
    let (port, token) = (service.info.port, service.info.token.clone());
    let asking = std::thread::spawn(move || http::call(port, &token, "GET", &path));
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
    let missing = data.0.join("no-such-claude.exe");
    let service = TestService::start_with(&data.0, &[("GATHR_CLAUDE_PATH", &missing)]);

    let id = service.add_account("Test");
    let answer = service.call("GET", &format!("/api/accounts/{id}/models"));
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

/// The engine that lists models runs on the account's login and may be renewing it, so it's
/// never killed for taking long to exit (0001): only shutdown kills, after its grace.
#[test]
fn a_model_list_engine_slow_to_exit_is_never_killed() {
    let data = TempDir::new();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY_INIT", Path::new("linger")),
            ("GATHR_REPLAY_LINGER_MS", Path::new("11500")),
        ],
    );
    let path = format!("/api/accounts/{}/models", service.add_account("Test"));
    let listed = service.call("GET", &path);
    assert_eq!(listed.status, 200, "{}", listed.body);
    let sql = "SELECT error_type, json_extract(attrs, '$.\"process.exit.code\"') AS code \
               FROM event_log WHERE name = 'gathr.engine.models.close'";
    let deadline = Instant::now() + Duration::from_secs(20);
    let closed = loop {
        let (ok, rows) = gathr(&data.0, &["diag", "query", sql]);
        assert!(ok, "{rows}");
        if let Some(row) = rows["rows"].get(0) {
            break row.clone();
        }
        assert!(Instant::now() < deadline, "the engine never closed");
        std::thread::sleep(Duration::from_millis(200));
    };
    assert!(
        closed["error_type"].is_null() && closed["code"] == 0,
        "it was killed: {closed}"
    );
}
