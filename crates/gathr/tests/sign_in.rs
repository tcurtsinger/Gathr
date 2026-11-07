//! Signing in from Settings > Engines: `claude auth login` run by the service, with no terminal.
//! The replay engine answers `auth login` the way Claude Code 2.1.318 does (task 893).

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, replay_engine};
use serde_json::{Value, json};

fn service(data: &TempDir) -> TestService {
    let service = TestService::start(&data.0);
    let path = json!({ "path": replay_engine() });
    service.send("PUT", "/api/engines/claude", path);
    service
}

/// Polls Claude Code's state until `done` holds for it, or fails after 10 s.
fn engine_until(service: &TestService, what: &str, done: impl Fn(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let engine = service.call("GET", "/api/engines/claude").json();
        if done(&engine) {
            return engine;
        }
        assert!(
            Instant::now() < deadline,
            "waited 10 s for {what}: {engine}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn a_pasted_code_signs_the_account_in() {
    let data = TempDir::new();
    let service = service(&data);
    let started = service.call("POST", "/api/engines/claude/sign-in");
    assert_eq!(started.status, 200, "{}", started.body);
    let url = started.json()["url"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    assert!(
        url.starts_with("https://"),
        "no sign-in page: {}",
        started.body
    );

    let engine = service.call("GET", "/api/engines/claude").json();
    assert_eq!(engine["account"]["login"], "signed_out", "{engine}");
    assert_eq!(engine["sign_in"], json!({ "state": "waiting", "url": url }));

    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", "/api/engines/claude/sign-in/code", code);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let engine = engine_until(&service, "the sign-in to finish", |e| {
        e["sign_in"].is_null()
    });
    assert_eq!(engine["account"]["login"], "signed_in", "{engine}");
}

#[test]
fn a_refused_code_fails_with_a_reference_that_finds_why() {
    let data = TempDir::new();
    let service = service(&data);
    service.call("POST", "/api/engines/claude/sign-in");
    let code = json!({ "code": "wrong#made-up-state" });
    service.send("POST", "/api/engines/claude/sign-in/code", code);

    let engine = engine_until(&service, "the sign-in to fail", |e| {
        e["sign_in"]["state"] == "failed"
    });
    assert_eq!(engine["account"]["login"], "signed_out", "{engine}");
    let why = engine["sign_in"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("wasn't accepted"), "{engine}");
    let reference = engine["sign_in"]["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    assert!(
        trace.to_string().contains("wasn't accepted"),
        "the reference doesn't find why: {trace}"
    );
}

#[test]
fn cancel_ends_the_wait_and_sign_in_starts_again() {
    let data = TempDir::new();
    let service = service(&data);
    service.call("POST", "/api/engines/claude/sign-in");
    let cancelled = service.call("DELETE", "/api/engines/claude/sign-in");
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let engine = service.call("GET", "/api/engines/claude").json();
    assert!(engine["sign_in"].is_null(), "still signing in: {engine}");

    let again = service.call("POST", "/api/engines/claude/sign-in");
    assert_eq!(again.status, 200, "{}", again.body);
}

#[test]
fn a_code_with_no_sign_in_running_fails() {
    let data = TempDir::new();
    let service = service(&data);
    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", "/api/engines/claude/sign-in/code", code);
    assert_eq!(sent.status, 409, "{}", sent.body);
    assert_eq!(sent.json()["error"], "Sign in failed", "{}", sent.body);
}

#[test]
fn sign_in_fails_with_why_when_claude_code_isnt_there() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let missing = data.0.join("no-such-claude.exe");
    service.send("PUT", "/api/engines/claude", json!({ "path": missing }));
    let started = service.call("POST", "/api/engines/claude/sign-in");
    assert_eq!(started.status, 500, "{}", started.body);
    let answer = started.json();
    assert_eq!(answer["error"], "Sign in failed", "{answer}");
    assert!(
        answer["why"].as_str().unwrap().contains("isn't a file"),
        "{answer}"
    );
}
