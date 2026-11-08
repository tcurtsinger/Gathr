//! The service API seam: a real gathrd, traced with `gathr diag`.

mod common;

use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, gathrd};
use gathr::data::{DataDir, ServiceInfo};
use gathr::http;

#[test]
fn health_answers_with_the_token() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let response = service.call("GET", "/health");
    assert_eq!(response.status, 200, "{}", response.body);
    assert_eq!(response.json()["schema_version"], gathr::db::SCHEMA_VERSION);
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

/// `gathr dev` stopped through `/api/shutdown` is a stop someone asked for, not a failure.
#[test]
fn dev_stopped_over_the_api_ends_cleanly() {
    let dir = TempDir::new();
    let dev = std::process::Command::new(env!("CARGO_BIN_EXE_gathr"))
        .args(["--data", dir.0.to_str().unwrap(), "dev", "--port", "0"])
        .env_remove("CARGO") // Not under `cargo run`, so it builds nothing.
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let info = loop {
        if let Ok(info) = ServiceInfo::read(&DataDir::new(&dir.0)) {
            break info;
        }
        assert!(
            Instant::now() < deadline,
            "gathr dev wrote no service.json in 20 s"
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    http::call(info.port, &info.token, "POST", "/api/shutdown").unwrap();
    let out = dev.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "gathr dev exited {}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
}

/// A window that can't start (here, no `gathr-app` beside `gathr`) fails `gathr dev`, and the
/// service it started stops with it rather than running on unseen.
#[test]
fn dev_window_failing_stops_the_service() {
    let dir = TempDir::new();
    let bin = dir.0.join("bin");
    std::fs::create_dir(&bin).unwrap();
    for exe in [env!("CARGO_BIN_EXE_gathr"), env!("CARGO_BIN_EXE_gathrd")] {
        let exe = std::path::Path::new(exe);
        std::fs::copy(exe, bin.join(exe.file_name().unwrap())).unwrap();
    }
    let data = dir.0.join("data");
    // To a file, not a pipe: a service left running would hold a pipe open, and the test hang.
    let log = dir.0.join("dev.log");
    let status =
        std::process::Command::new(bin.join(format!("gathr{}", std::env::consts::EXE_SUFFIX)))
            .args([
                "--data",
                data.to_str().unwrap(),
                "dev",
                "--port",
                "0",
                "--window",
            ])
            .env_remove("CARGO")
            .stdout(std::fs::File::create(&log).unwrap())
            .stderr(std::fs::File::create(dir.0.join("dev.err")).unwrap())
            .status()
            .unwrap();
    let said = std::fs::read_to_string(dir.0.join("dev.err")).unwrap();
    assert!(
        !status.success(),
        "gathr dev started a window with no gathr-app"
    );
    let info = ServiceInfo::read(&DataDir::new(&data)).expect("gathr dev started no service");
    if http::call(info.port, &info.token, "GET", "/health").is_ok() {
        let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
        panic!(
            "gathrd pid {} still ran after gathr dev failed ({said})",
            info.pid
        );
    }
}
