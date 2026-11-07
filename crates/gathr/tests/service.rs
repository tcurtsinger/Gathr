//! The service API seam: a real gathrd, traced with `gathr diag`.

mod common;

use common::{TempDir, TestService, gathr, gathrd};

#[test]
fn health_answers_with_the_token() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let response = service.call("GET", "/health");
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(response.json()["schema_version"], 1);
}

#[test]
fn a_refused_request_has_a_reference_that_diag_traces() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let response = service.get("/health", &[]);
    assert_eq!(response.status, 401, "{}", response.body);
    let reference = response.json()["ref"]
        .as_str()
        .expect("a ref in the answer")
        .to_string();

    let (ok, trace) = gathr(&dir.0, &["diag", "trace", &reference]);
    assert!(ok, "{trace}");
    let event = &trace["events"][0];
    assert_eq!(event["name"], "gathr.api.request", "{trace}");
    assert_eq!(event["attrs"]["reason"], "token missing", "{trace}");
    assert_eq!(event["attrs"]["http.response.status_code"], 401, "{trace}");
}

#[test]
fn traceparent_sets_the_reference() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let response = service.get("/health", &[("traceparent", traceparent)]);
    assert_eq!(response.json()["ref"], "4bf92f35", "{}", response.body);
}

#[test]
fn requests_from_other_sites_are_refused() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let auth = format!("Bearer {}", service.info.token);
    let cases = [
        ("Host", "evil.example".to_string(), 403),
        ("Host", format!("evil.example:{}", service.info.port), 403),
        ("Origin", "http://evil.example".to_string(), 403),
        ("Origin", "http://localhost:1".to_string(), 403),
        ("Origin", "http://tauri.localhost".to_string(), 200),
        ("Origin", "tauri://localhost".to_string(), 200),
        (
            "Origin",
            format!("http://localhost:{}", service.info.port),
            200,
        ),
    ];
    for (header, value, expected) in cases {
        let response = service.get("/health", &[("Authorization", &auth), (header, &value)]);
        assert_eq!(
            response.status, expected,
            "{header}: {value} -> {}",
            response.body
        );
    }
    let wrong = service.get("/health", &[("Authorization", "Bearer 0000")]);
    assert_eq!(wrong.json()["why"], "token wrong", "{}", wrong.body);
}

#[test]
fn a_second_service_on_the_same_folder_exits() {
    let dir = TempDir::new();
    let _first = TestService::start(&dir.0);
    let second = gathrd(&dir.0).status().unwrap();
    assert!(!second.success());
    let (_, health) = gathr(&dir.0, &["diag", "health"]);
    let lines = health["crash_log"]["last_lines"].to_string();
    assert!(lines.contains("another gathrd is running"), "{health}");
}

/// So a copy of a stopped data folder is whole in `gathr.db` (hand check 10 replaces that file).
#[test]
fn a_stopped_service_leaves_everything_in_the_database_file() {
    let dir = TempDir::new();
    TestService::start(&dir.0).stop();
    let wal = std::fs::metadata(dir.0.join("gathr.db-wal")).map_or(0, |m| m.len());
    assert_eq!(wal, 0, "the WAL still holds {wal} bytes after a stop");
}

#[test]
fn stopping_records_why() {
    let dir = TempDir::new();
    let status = TestService::start(&dir.0).stop();
    assert!(status.success());
    let sql = "SELECT json_extract(attrs, '$.reason') AS reason FROM event_log WHERE name = 'gathr.service.stop'";
    let (ok, answer) = gathr(&dir.0, &["diag", "query", sql]);
    assert!(ok, "{answer}");
    assert_eq!(
        answer["rows"][0]["reason"], "requested over the API",
        "{answer}"
    );
}
