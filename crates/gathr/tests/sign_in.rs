//! Signing an account in from Settings > Engines: `claude auth login` run by the service, with no terminal.
//! The replay engine answers `auth login` the way Claude Code 2.1.318 does (task 893).

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{TempDir, TestService, account_until, gathr};
use serde_json::{Value, json};

/// The service, and an account added on it.
fn service(data: &TempDir) -> (TestService, String) {
    let service = TestService::with_engine(&data.0, &[]);
    let id = service.add_account("Test");
    (service, id)
}

fn engine_until(
    service: &TestService,
    id: &str,
    what: &str,
    done: impl Fn(&Value) -> bool,
) -> Value {
    account_until(service, id, what, done)
}

#[test]
fn a_pasted_code_signs_the_account_in() {
    let data = TempDir::new();
    let (service, id) = service(&data);
    let started = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
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

    let engine = service.call("GET", &format!("/api/accounts/{id}")).json();
    assert_eq!(engine["login"], "signed_out", "{engine}");
    assert_eq!(
        engine["sign_in"],
        json!({ "state": "waiting", "url": url, "paste": true })
    );

    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let engine = engine_until(&service, &id, "the sign-in to finish", |e| {
        e["sign_in"].is_null()
    });
    assert_eq!(engine["login"], "signed_in", "{engine}");
}

#[test]
fn a_refused_code_fails_with_a_reference_that_finds_why() {
    let data = TempDir::new();
    let (service, id) = service(&data);
    service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    let code = json!({ "code": "wrong#made-up-state" });
    service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);

    let engine = engine_until(&service, &id, "the sign-in to fail", |e| {
        e["sign_in"]["state"] == "failed"
    });
    assert_eq!(engine["login"], "signed_out", "{engine}");
    let why = engine["sign_in"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("wasn't accepted"), "{engine}");
    let reference = engine["sign_in"]["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    assert!(
        trace.to_string().contains("wasn't accepted"),
        "the reference doesn't find why: {trace}"
    );
    let shown = service.call("GET", &format!("/api/trace/{reference}"));
    assert!(
        shown.body.contains("wasn't accepted"),
        "Diagnostics doesn't find why: {}",
        shown.body
    );
}

/// A Cancel that comes while Claude Code is saving the login a pasted code gave it waits for it to
/// finish: a login cut short there is a spent token (0002, audit 2.1.3).
#[test]
fn cancel_after_a_pasted_code_lets_the_login_finish_saving() {
    let data = TempDir::new();
    let saving = [("GATHR_REPLAY_LOGIN_MS", Path::new("1500"))];
    let service = TestService::with_engine(&data.0, &saving);
    let id = service.add_account("Test");
    service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let cancelled = service.call("DELETE", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let engine = engine_until(&service, &id, "the login to be saved", |e| {
        e["login"] == "signed_in"
    });
    assert!(engine["sign_in"].is_null(), "still signing in: {engine}");
}

#[test]
fn cancel_ends_the_wait_and_sign_in_starts_again() {
    let data = TempDir::new();
    let (service, id) = service(&data);
    service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    let cancelled = service.call("DELETE", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let engine = service.call("GET", &format!("/api/accounts/{id}")).json();
    assert!(engine["sign_in"].is_null(), "still signing in: {engine}");

    let again = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(again.status, 200, "{}", again.body);
}

/// A double click on Sign in, then Cancel, all before a slow launcher prints its page (Vale, 905).
#[test]
fn cancel_before_the_page_stops_both_of_two_starts() {
    let data = TempDir::new();
    let slow = [("GATHR_REPLAY_PAGE_MS", Path::new("3000"))];
    let service = TestService::with_engine(&data.0, &slow);
    let id = service.add_account("Test");

    let started = Instant::now();
    let answers = std::thread::scope(|scope| {
        let start = || service.call("POST", &format!("/api/accounts/{id}/sign-in"));
        let starts = [scope.spawn(start), scope.spawn(start)];
        std::thread::sleep(Duration::from_millis(1000));
        let engine = service.call("GET", &format!("/api/accounts/{id}")).json();
        assert_eq!(
            engine["sign_in"]["state"], "waiting",
            "Cancel isn't shown yet: {engine}"
        );
        let cancelled = service.call("DELETE", &format!("/api/accounts/{id}/sign-in"));
        assert_eq!(cancelled.status, 200, "{}", cancelled.body);
        starts.map(|s| s.join().expect("the start answers"))
    });
    for answer in &answers {
        assert_eq!(
            answer.json(),
            json!({ "cancelled": true }),
            "a start went on"
        );
    }
    assert!(
        started.elapsed() < Duration::from_millis(2500),
        "the starts waited for the page: {:?}",
        started.elapsed()
    );
    let engine = service.call("GET", &format!("/api/accounts/{id}")).json();
    assert!(engine["sign_in"].is_null(), "still signing in: {engine}");
    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    assert_eq!(
        sent.status, 409,
        "a sign-in is still waiting: {}",
        sent.body
    );
}

#[test]
fn a_code_with_no_sign_in_running_fails() {
    let data = TempDir::new();
    let (service, id) = service(&data);
    let code = json!({ "code": "made-up-code#made-up-state" });
    let sent = service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    assert_eq!(sent.status, 409, "{}", sent.body);
    assert_eq!(sent.json()["error"], "Sign in failed", "{}", sent.body);
}

#[test]
fn sign_in_fails_with_why_when_claude_code_isnt_there() {
    let data = TempDir::new();
    let missing = data.0.join("no-such-claude.exe");
    let service = TestService::start_with(&data.0, &[("GATHR_CLAUDE_PATH", &missing)]);
    let id = service.add_account("Test");
    let started = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(started.status, 500, "{}", started.body);
    let answer = started.json();
    assert_eq!(answer["error"], "Sign in failed", "{answer}");
    assert!(
        answer["why"].as_str().unwrap().contains("isn't a file"),
        "{answer}"
    );
}

#[test]
fn shutdown_during_a_sign_out_closes_its_engine_within_the_grace() {
    let data = TempDir::new();
    let stall = [("GATHR_REPLAY_LOGOUT", Path::new("stall"))];
    let service = TestService::with_engine(&data.0, &stall);
    let id = service.signed_in(&data.0);
    let (port, token) = (service.info.port, service.info.token.clone());
    let path = format!("/api/accounts/{id}/sign-out");
    let signing_out = std::thread::spawn(move || gathr::http::call(port, &token, "POST", &path));
    let started = data
        .0
        .join("accounts")
        .join(&id)
        .join("claude/logout-started");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !started.is_file() {
        assert!(Instant::now() < deadline, "auth logout never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let began = Instant::now();
    service.stop();
    let took = began.elapsed();
    let _ = signing_out.join();
    assert!(
        took < Duration::from_secs(15),
        "the service took {took:?} to stop"
    );
    let sql = "SELECT error_type FROM event_log WHERE name = 'gathr.engine.sign_out'";
    let (ok, rows) = gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"][0]["error_type"], "sign_out", "{rows}");
}
