//! Gathr's own copies of Claude Code (docs/decisions/0025-engine-copies.md), through the real
//! service and a stand-in release feed on 127.0.0.1 that serves the replay engine as each release.

mod common;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, recording, replay_engine, sign_in};
use serde_json::{Value, json};

/// Claude Code's release feed as its installer reads it: `latest`, then each release's
/// `manifest.json` and its file. Counts what was asked for.
struct Feed {
    port: u16,
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    asked: Arc<Mutex<Vec<String>>>,
}

impl Feed {
    fn start() -> Feed {
        Self::serve(false)
    }

    /// A feed that takes each request and never answers it.
    fn silent() -> Feed {
        Self::serve(true)
    }

    fn serve(silent: bool) -> Feed {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let files: Arc<Mutex<HashMap<String, Vec<u8>>>> = Arc::default();
        let asked: Arc<Mutex<Vec<String>>> = Arc::default();
        let (f, a) = (files.clone(), asked.clone());
        std::thread::spawn(move || {
            let mut held = Vec::new();
            for stream in listener.incoming().flatten() {
                if silent {
                    a.lock().unwrap().push(String::new());
                    held.push(stream);
                    continue;
                }
                let mut line = String::new();
                let mut reader = BufReader::new(&stream);
                let _ = reader.read_line(&mut line);
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {}
                a.lock().unwrap().push(path.clone());
                let body = f.lock().unwrap().get(&path).cloned();
                let mut out = &stream;
                let _ = match body {
                    Some(body) => out
                        .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len()).as_bytes())
                        .and_then(|()| out.write_all(&body)),
                    None => out.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"),
                };
            }
        });
        Feed { port, files, asked }
    }

    fn url(&self) -> PathBuf {
        PathBuf::from(format!("http://127.0.0.1:{}/releases", self.port))
    }

    /// Publishes `version` as the latest release, its file `binary`, under `checksum` (its own
    /// SHA-256 when none is given).
    fn publish(&self, version: &str, binary: &[u8], checksum: Option<&str>) {
        let target = gathr::platform::release_target().expect("a platform Claude Code ships for");
        let name = if cfg!(windows) {
            "claude.exe"
        } else {
            "claude"
        };
        let sha = checksum.map_or_else(|| gathr::claude::copies::sha256(binary), String::from);
        let manifest = json!({ "version": version, "platforms": {
            target: { "binary": name, "checksum": sha, "size": binary.len() } } });
        let mut files = self.files.lock().unwrap();
        files.insert(
            "/releases/latest".into(),
            format!("{version}\n").into_bytes(),
        );
        files.insert(
            format!("/releases/{version}/manifest.json"),
            manifest.to_string().into_bytes(),
        );
        files.insert(
            format!("/releases/{version}/{target}/{name}"),
            binary.to_vec(),
        );
    }

    fn downloads(&self) -> usize {
        let asked = self.asked.lock().unwrap();
        asked
            .iter()
            .filter(|p| p.ends_with("/claude") || p.ends_with("/claude.exe"))
            .count()
    }
}

/// A service on the feed whose replay engine names the release folder it runs from as its version.
fn service_on(data: &Path, feed: &Feed) -> TestService {
    sign_in(data);
    TestService::start_with(
        data,
        &[
            ("GATHR_CLAUDE_RELEASES", &feed.url()),
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_VERSION", Path::new("folder")),
        ],
    )
}

/// The engine's state once the check under way has ended, waiting up to 60 s.
fn checked(service: &TestService) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let engine = service.call("GET", "/api/engines/claude").json();
        let copy = &engine["copy"];
        if copy["state"] == "idle" && !copy["checked"].is_null() {
            return copy.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the check didn't end in 60 s: {engine}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn check_now(service: &TestService) -> Value {
    let asked = service.call("POST", "/api/engines/claude/update");
    assert_eq!(asked.status, 200, "{}", asked.body);
    std::thread::sleep(Duration::from_millis(100));
    checked(service)
}

fn events(data: &Path, name: &str) -> Vec<Value> {
    let sql = format!(
        "SELECT status, error_type, attrs, ref FROM event_log WHERE name = '{name}' ORDER BY id"
    );
    let (ok, rows) = gathr(data, &["diag", "query", &sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

fn turn_done(service: &TestService, chat: &str) {
    service.wait_for(chat, 20, "the turn to finish", |h| {
        h["turns"]
            .as_array()
            .and_then(|t| t.last())
            .map(|t| t["status"] == "done")
            == Some(true)
    });
}

fn send(service: &TestService, chat: &str) {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Say hi" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
}

fn engine_bytes() -> Vec<u8> {
    std::fs::read(replay_engine()).unwrap()
}

#[test]
fn the_service_installs_the_latest_claude_code_and_chats_run_that_copy() {
    let (data, feed) = (TempDir::new(), Feed::start());
    feed.publish("2.1.400", &engine_bytes(), None);
    let service = service_on(&data.0, &feed);

    let copy = checked(&service);
    assert_eq!(copy["version"], "2.1.400", "{copy}");
    let chat = service.new_chat();
    send(&service, &chat);
    turn_done(&service, &chat);
    let started = events(&data.0, "gathr.engine.start");
    let path = started[0]["attrs"]["path"].as_str().unwrap_or_default();
    assert!(
        path.contains("2.1.400"),
        "the chat didn't run Gathr's copy: {started:?}"
    );

    check_now(&service);
    assert_eq!(
        feed.downloads(),
        1,
        "a release already installed was downloaded again"
    );
}

#[test]
fn a_download_that_doesnt_match_its_checksum_is_never_used_and_its_reference_traces() {
    let (data, feed) = (TempDir::new(), Feed::start());
    feed.publish("2.1.400", &engine_bytes(), Some(&"0".repeat(64)));
    let service = service_on(&data.0, &feed);

    let copy = checked(&service);
    assert!(copy["version"].is_null(), "{copy}");
    let update = events(&data.0, "gathr.engine.update");
    let failed = update.last().unwrap();
    assert_eq!(failed["error_type"], "checksum", "{update:?}");
    let reference = failed["ref"].as_str().unwrap();
    assert_eq!(copy["failed"]["ref"], reference, "{copy}");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok && trace.to_string().contains("SHA-256"), "{trace}");
    let left: Vec<_> = std::fs::read_dir(data.0.join("engines/claude"))
        .map(|d| d.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(
        left.is_empty(),
        "the refused download was left behind: {left:?}"
    );
}

#[test]
fn a_release_that_fails_its_check_leaves_chats_on_the_copy_before() {
    let (data, feed) = (TempDir::new(), Feed::start());
    feed.publish("2.1.400", &engine_bytes(), None);
    let service = service_on(&data.0, &feed);
    assert_eq!(checked(&service)["version"], "2.1.400");

    feed.publish("2.1.401", b"not a program", None);
    let copy = check_now(&service);
    assert_eq!(copy["version"], "2.1.400", "{copy}");
    let update = events(&data.0, "gathr.engine.update");
    assert_eq!(update.last().unwrap()["error_type"], "probe", "{update:?}");
    assert!(
        !data.0.join("engines/claude/2.1.401").exists(),
        "the failed release was kept"
    );
}

#[test]
fn a_failed_repair_keeps_the_copy_that_was_already_there() {
    let (data, feed) = (TempDir::new(), Feed::start());
    feed.publish("2.1.400", &engine_bytes(), None);
    let service = service_on(&data.0, &feed);
    checked(&service);
    feed.publish("2.1.401", &engine_bytes(), None);
    assert_eq!(check_now(&service)["previous"], "2.1.400");

    let root = data.0.join("engines/claude");
    std::fs::remove_dir_all(root.join("2.1.401")).unwrap();
    feed.publish("2.1.400", &engine_bytes(), Some(&"0".repeat(64)));
    check_now(&service);
    let update = events(&data.0, "gathr.engine.update");
    assert_eq!(
        update.last().unwrap()["error_type"],
        "checksum",
        "{update:?}"
    );
    let binary = if cfg!(windows) {
        "claude.exe"
    } else {
        "claude"
    };
    assert!(
        root.join("2.1.400").join(binary).is_file(),
        "the refused download deleted the copy that was there"
    );
}

#[test]
fn shutdown_ends_a_check_waiting_on_the_feed() {
    let (data, feed) = (TempDir::new(), Feed::silent());
    let service = service_on(&data.0, &feed);
    let deadline = Instant::now() + Duration::from_secs(10);
    while feed.asked.lock().unwrap().is_empty() {
        assert!(
            Instant::now() < deadline,
            "the service never asked the feed"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let began = Instant::now();
    service.stop();
    let took = began.elapsed();
    assert!(
        took < Duration::from_secs(5),
        "the service took {took:?} to stop"
    );
}

#[test]
fn a_chat_moves_to_a_new_copy_at_its_next_turn() {
    let (data, feed) = (TempDir::new(), Feed::start());
    feed.publish("2.1.400", &engine_bytes(), None);
    let service = service_on(&data.0, &feed);
    checked(&service);
    let chat = service.new_chat();
    send(&service, &chat);
    turn_done(&service, &chat);

    feed.publish("2.1.401", &engine_bytes(), None);
    let copy = check_now(&service);
    assert_eq!(
        (&copy["version"], &copy["previous"]),
        (&json!("2.1.401"), &json!("2.1.400"))
    );
    send(&service, &chat);
    turn_done(&service, &chat);

    let paths: Vec<String> = events(&data.0, "gathr.engine.start")
        .iter()
        .map(|e| e["attrs"]["path"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(paths.len(), 2, "{paths:?}");
    assert!(
        paths[0].contains("2.1.400") && paths[1].contains("2.1.401"),
        "{paths:?}"
    );
    let closed = events(&data.0, "gathr.engine.close");
    assert!(
        closed
            .iter()
            .any(|e| e["attrs"]["why"].as_str().unwrap_or("").contains("2.1.401")),
        "{closed:?}"
    );
}
