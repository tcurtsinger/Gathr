//! What the update tests share (0032): copies of the real `gathrd` in a test data folder, a stand-in
//! release feed, and the service that takes over.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gathr::data::{DataDir, ServiceInfo};
use gathr::http;
use serde_json::{Value, json};

use super::feed::Feed;
use super::{TestService, gathr, until};

pub fn program_name() -> String {
    format!("gathrd{}", std::env::consts::EXE_SUFFIX)
}

/// The built `gathrd` as the folder's copy of `version`, named current: what an older install left.
pub fn seed_copy(data: &Path, version: &str) -> PathBuf {
    let folder = data.join("app").join(version);
    std::fs::create_dir_all(&folder).unwrap();
    let program = folder.join(program_name());
    std::fs::copy(env!("CARGO_BIN_EXE_gathrd"), &program).unwrap();
    let current = json!({ "version": version }).to_string();
    std::fs::write(data.join("app/current.json"), current).unwrap();
    program
}

/// Publishes `version` as the newest release: `program` as its `gathrd`, with a UI saying `ui`.
pub fn publish(feed: &Feed, dir: &Path, version: &str, program: &str, ui: &str) {
    let stage = dir.join(format!("stage-{}", gathr::ids::span_id()));
    std::fs::create_dir_all(stage.join("ui")).unwrap();
    std::fs::copy(program, stage.join(program_name())).unwrap();
    std::fs::write(stage.join("ui/index.html"), ui).unwrap();
    let archive = stage.with_extension("tar.gz");
    let made = std::process::Command::new(gathr::platform::tar())
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(&stage)
        .args([program_name().as_str(), "ui"])
        .status()
        .unwrap();
    assert!(made.success(), "tar: {made}");
    let bytes = std::fs::read(&archive).unwrap();
    let target = gathr::platform::release_target().expect("a platform Gathr ships for");
    let file = format!("gathrd-{version}-{target}.tar.gz");
    let manifest = json!({ "version": version, "platforms": { target: {
        "file": file, "sha256": gathr::fetch::sha256(&bytes), "size": bytes.len() } } });
    feed.put(&file, bytes);
    feed.put("gathrd.json", manifest.to_string().into_bytes());
}

/// The service that took over from `pid` on the folder, once it answers.
pub fn successor(data: &Path, pid: u32) -> ServiceInfo {
    until(90, "a new service on the folder", || {
        let info = ServiceInfo::read(&DataDir::new(data)).ok()?;
        let health = http::call(info.port, &info.token, "GET", "/health").ok()?;
        (info.pid != pid && health.status == 200).then_some(info)
    })
}

pub fn get(info: &ServiceInfo, path: &str) -> Value {
    http::call(info.port, &info.token, "GET", path)
        .unwrap()
        .json()
}

/// Stops whatever service is on the folder when the test ends, as `TestService` stops its own.
pub struct Successor(pub PathBuf);

impl Drop for Successor {
    fn drop(&mut self) {
        let Ok(info) = ServiceInfo::read(&DataDir::new(&self.0)) else {
            return;
        };
        let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
        let deadline = Instant::now() + Duration::from_secs(20);
        while http::call(info.port, &info.token, "GET", "/health").is_ok()
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

pub fn events(data: &Path, name: &str) -> Vec<Value> {
    let sql = format!(
        "SELECT status, error_type, attrs, ref FROM event_log WHERE name = '{name}' ORDER BY id"
    );
    let (ok, rows) = gathr(data, &["diag", "query", &sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

/// Checks for a release now; `GET /api/app` says when it's ready.
pub fn check_now(service: &TestService) {
    let asked = service.call("POST", "/api/app/update");
    assert_eq!(asked.status, 200, "{}", asked.body);
}

/// Once a release is ready, waits for nothing (manual updates): never switches until asked.
pub fn ready(service: &TestService) -> Value {
    until(30, "a ready update", || {
        let app = service.call("GET", "/api/app").json();
        (app["state"] == "ready").then_some(app)
    })
}

/// Update in the sidebar's foot: installs the ready release now.
pub fn install(service: &TestService) -> Value {
    let asked = service.send("POST", "/api/app/update", json!({ "install": true }));
    assert_eq!(asked.status, 200, "{}", asked.body);
    asked.json()
}

/// What the person does: check, then Update once it's ready.
pub fn update_now(service: &TestService) {
    check_now(service);
    ready(service);
    install(service);
}

pub fn say(service: &TestService, chat: &str, text: &str) -> Value {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

/// An agent on the signed-in account, and a chat it said `text` in.
pub fn agent_says(service: &TestService, data: &Path, text: &str) -> (String, String) {
    let account = service.signed_in(data);
    let made = json!({ "name": "Made-up", "account": account });
    let made = service.send("POST", "/api/agents", made);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    (agent, chat)
}

/// The carry-on Update leaves the agent, run on the new service: its words.
pub fn carried_on(next: &ServiceInfo, chat: &str) -> Value {
    until(30, "the carry-on to run on the new service", || {
        let history = get(next, &format!("/api/chats/{chat}"));
        let turns = history["turns"].as_array()?;
        (turns.len() == 2 && turns[1]["status"] == "done").then_some(history)
    })
}
