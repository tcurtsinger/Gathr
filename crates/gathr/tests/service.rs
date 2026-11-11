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
    assert!(
        lines.contains("Gathr is already using this data folder"),
        "{health}"
    );
}

/// The folder's lock held for a moment with no service behind it (on macOS, a process another
/// thread is starting keeps a copy of the lock file open until its program runs): the service
/// waits for it, and starts once it's let go.
#[test]
fn a_service_waits_for_a_lock_held_by_no_service() {
    let dir = TempDir::new();
    let data = DataDir::new(&dir.0);
    let held = data.take_lock().unwrap();
    let mut service = gathrd(&dir.0).spawn().unwrap();
    std::thread::sleep(Duration::from_secs(1));
    let exited = service.try_wait().unwrap();
    drop(held);
    let up = common::until(20, "the service to answer", || {
        let info = ServiceInfo::read(&data).ok()?;
        let health = http::call(info.port, &info.token, "GET", "/health").ok()?;
        (health.status == 200).then_some(info)
    });
    let _ = http::call(up.port, &up.token, "POST", "/api/shutdown");
    service.wait().unwrap();
    assert_eq!(exited, None, "gathrd exited while the folder was held");
    assert_eq!(up.pid, service.id());
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
    let (data, said) = dev_window_without_app(&dir, 0);
    let info = ServiceInfo::read(&DataDir::new(&data)).expect("gathr dev started no service");
    if http::call(info.port, &info.token, "GET", "/health").is_ok() {
        let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
        panic!(
            "gathrd pid {} still ran after gathr dev failed ({said})",
            info.pid
        );
    }
}

/// CI's Mac build is started beside a `ui` folder, named relative to where it starts (the owner's
/// first Mac run looked for `ui/ui/node_modules/…`).
#[test]
fn dev_window_runs_vite_from_the_ui_folder_it_starts_in() {
    let dir = TempDir::new();
    let vite = dir.0.join("ui/node_modules/vite/bin");
    std::fs::create_dir_all(&vite).unwrap();
    std::fs::write(
        vite.join("vite.js"),
        "require('fs').writeFileSync('../vite-ran', process.cwd());\n\
         require('net').createServer().listen(+process.env.GATHR_UI_PORT, '127.0.0.1');\n",
    )
    .unwrap();
    // Not from port 0: macOS hands those out in order, so another test's service can hold port + 10,
    // and the UI's port would answer before this Vite ran. Below every system's ephemeral range,
    // with both ports free when picked.
    let free = |port: u16| std::net::TcpListener::bind(("127.0.0.1", port)).is_ok();
    let start = 20_000 + (std::process::id() % 10_000) as u16;
    let port = (start..30_000)
        .find(|&p| free(p) && free(p + 10))
        .expect("no free pair of ports");
    let (_, said) = dev_window_without_app(&dir, port);
    let ran = std::fs::read_to_string(dir.0.join("vite-ran"))
        .unwrap_or_else(|_| panic!("the UI's dev server never ran: {said}"));
    // Canonical on both sides: macOS's temp folder is behind a link.
    assert_eq!(
        std::fs::canonicalize(&ran).unwrap(),
        std::fs::canonicalize(dir.0.join("ui")).unwrap(),
        "Vite ran in the wrong folder"
    );
}

/// Runs a copy of `gathr dev --window` from `dir`, where no `gathr-app` sits beside it, so it
/// fails once the UI's dev server is up. Answers the data folder and what it wrote to stderr.
fn dev_window_without_app(dir: &TempDir, port: u16) -> (std::path::PathBuf, String) {
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
                &port.to_string(),
                "--window",
            ])
            .env_remove("CARGO")
            .current_dir(&dir.0)
            .stdout(std::fs::File::create(&log).unwrap())
            .stderr(std::fs::File::create(dir.0.join("dev.err")).unwrap())
            .status()
            .unwrap();
    let said = std::fs::read_to_string(dir.0.join("dev.err")).unwrap();
    assert!(
        !status.success(),
        "gathr dev started a window with no gathr-app"
    );
    (data, said)
}

/// With all its runtime threads on one CPU, the default HTTP/1 connection's watch for a client
/// hanging up made Windows flood the I/O driver, and a request sat for up to 30 s (1209). A runner
/// short of CPU is the same: every request still answers at once.
#[cfg(windows)]
#[test]
fn requests_answer_at_once_with_the_service_on_one_cpu() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let pin = format!(
        "(Get-Process -Id {}).ProcessorAffinity = 1",
        service.child.id()
    );
    let pinned = std::process::Command::new("powershell")
        .args(["-NoProfile", "-Command", &pin])
        .status()
        .unwrap();
    assert!(pinned.success(), "the service wasn't pinned to one CPU");
    for n in 1..=30 {
        let started = Instant::now();
        let listed = service.call("GET", "/api/chats");
        assert_eq!(listed.status, 200, "{}", listed.body);
        let took = started.elapsed();
        assert!(took < Duration::from_secs(2), "request {n} took {took:?}");
    }
}

/// Whatever a connection still holds, a stop waits for it only a few seconds, and says so (G162).
#[test]
fn a_stop_waits_only_a_little_for_a_connection_that_never_ends() {
    use std::io::Write;

    let dir = TempDir::new();
    let mut service = TestService::start(&dir.0);
    let port = service.info.port;
    let mut held = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let part = format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n");
    held.write_all(part.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    let asked = std::time::Instant::now();
    let _ = service.call("POST", "/api/shutdown");
    while asked.elapsed() < Duration::from_secs(30) && service.child.try_wait().unwrap().is_none() {
        std::thread::sleep(Duration::from_millis(50));
    }
    let took = asked.elapsed();
    assert!(took < Duration::from_secs(20), "the stop took {took:?}");
    drop(held);
    let sql = "SELECT ref, json_extract(attrs, '$.open') AS open FROM event_log
               WHERE name = 'gathr.service.connections' AND status = 'error'";
    let (ok, answer) = gathr(&dir.0, &["diag", "query", sql]);
    assert!(ok, "{answer}");
    assert_eq!(answer["rows"][0]["open"], 1, "{answer}");
    assert!(answer["rows"][0]["ref"].is_string(), "{answer}");
}
