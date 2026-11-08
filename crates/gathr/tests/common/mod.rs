//! A real `gathrd` on a temp data folder and a free port, and the `gathr` command against it.
// Each test file uses part of this module.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

use gathr::data::{DataDir, ServiceInfo};
use gathr::http::{self, Response};
use serde_json::Value;

/// A temp folder, removed when dropped.
pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("gathr-test-{}", gathr::ids::span_id()));
        std::fs::create_dir_all(&path).unwrap();
        TempDir(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub struct TestService {
    pub child: Child,
    pub info: ServiceInfo,
}

impl TestService {
    pub fn start(data: &Path) -> TestService {
        Self::start_with(data, &[])
    }

    /// Starts it with extra environment variables, such as the replay engine's.
    pub fn start_with(data: &Path, env: &[(&str, &Path)]) -> TestService {
        let mut command = gathrd(data);
        for (name, value) in env {
            command.env(name, value);
        }
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(info) = ServiceInfo::read(&DataDir::new(data))
                && info.pid == child.id()
            {
                return TestService { child, info };
            }
            if Instant::now() > deadline || matches!(child.try_wait(), Ok(Some(_))) {
                let _ = child.kill();
                let status = child.wait();
                panic!("gathrd didn't write service.json within 20 s ({status:?})");
            }
            std::thread::yield_now();
        }
    }

    /// Starts it with the replay engine as Claude Code (`GATHR_CLAUDE_PATH`), plus `env`.
    pub fn with_engine(data: &Path, env: &[(&str, &Path)]) -> TestService {
        let engine = replay_engine();
        let mut all = vec![("GATHR_CLAUDE_PATH", engine.as_path())];
        all.extend_from_slice(env);
        Self::start_with(data, &all)
    }

    pub fn get(&self, path: &str, headers: &[(&str, &str)]) -> Response {
        http::request(self.info.port, "GET", path, headers).unwrap()
    }

    pub fn call(&self, method: &str, path: &str) -> Response {
        http::call(self.info.port, &self.info.token, method, path).unwrap()
    }

    pub fn send(&self, method: &str, path: &str, body: Value) -> Response {
        http::send(self.info.port, &self.info.token, method, path, &body).unwrap()
    }

    /// Adds an account through the API: its id.
    pub fn add_account(&self, name: &str) -> String {
        let body = serde_json::json!({ "engine": "claude", "name": name });
        let added = self.send("POST", "/api/accounts", body);
        assert_eq!(added.status, 200, "{}", added.body);
        added.json()["id"].as_str().unwrap().to_string()
    }

    /// Adds an account with a made-up login, where chats then run: its id.
    pub fn signed_in(&self, data: &Path) -> String {
        let id = self.add_account("Test");
        sign_in_account(data, &id);
        id
    }

    /// A new chat's id.
    pub fn new_chat(&self) -> String {
        let chat = self.send("POST", "/api/chats", serde_json::json!({}));
        assert_eq!(chat.status, 200, "{}", chat.body);
        chat.json()["id"].as_str().unwrap().to_string()
    }

    /// Polls the chat until `done` holds for its history, or fails after `secs`.
    pub fn wait_for(
        &self,
        chat: &str,
        secs: u64,
        what: &str,
        done: impl Fn(&Value) -> bool,
    ) -> Value {
        let deadline = Instant::now() + Duration::from_secs(secs);
        loop {
            let history = self.call("GET", &format!("/api/chats/{chat}")).json();
            if done(&history) {
                return history;
            }
            assert!(
                Instant::now() < deadline,
                "waited {secs} s for {what}; the chat: {history}"
            );
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// Asks the service to stop and waits for it to exit.
    pub fn stop(mut self) -> std::process::ExitStatus {
        let _ = self.call("POST", "/api/shutdown");
        self.child.wait().unwrap()
    }
}

impl Drop for TestService {
    fn drop(&mut self) {
        if let Ok(None) = self.child.try_wait() {
            let _ = http::call(self.info.port, &self.info.token, "POST", "/api/shutdown");
            let _ = self.child.wait();
        }
    }
}

pub fn gathrd(data: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gathrd"));
    command.arg("--data").arg(data).arg("--port").arg("0");
    command
}

/// The replay engine (`examples/replay_engine.rs`), which `cargo test` builds next to the tests.
pub fn replay_engine() -> PathBuf {
    let deps = std::env::current_exe().unwrap();
    let examples = deps.parent().unwrap().parent().unwrap().join("examples");
    let path = examples.join(format!("replay_engine{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "{} isn't built: run the tests with cargo test",
        path.display()
    );
    path
}

/// A recorded engine session in `tests/frames`.
pub fn recording(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/frames")
        .join(format!("{name}.jsonl"))
}

/// A made-up login in the test account's folder, current for an hour, so a turn may start.
/// A made-up login in the account's folder, as `claude auth login` leaves one.
pub fn sign_in_account(data: &Path, account: &str) {
    let folder = data.join("accounts").join(account).join("claude");
    std::fs::create_dir_all(&folder).unwrap();
    let expires = gathr::ids::now_us() / 1000 + 3_600_000;
    let login = serde_json::json!({ "claudeAiOauth": {
        "accessToken": "made-up", "refreshToken": "made-up", "expiresAt": expires } });
    std::fs::write(folder.join(".credentials.json"), login.to_string()).unwrap();
}

/// Polls the account until `done` holds for it, or fails after 10 s.
pub fn account_until(
    service: &TestService,
    account: &str,
    what: &str,
    done: impl Fn(&Value) -> bool,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let found = service
            .call("GET", &format!("/api/accounts/{account}"))
            .json();
        if done(&found) {
            return found;
        }
        assert!(Instant::now() < deadline, "waited 10 s for {what}: {found}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Runs `gathr --data <data> <args>`; returns its JSON answer (stdout, or stderr on failure).
pub fn gathr(data: &Path, args: &[&str]) -> (bool, Value) {
    let Output {
        status,
        stdout,
        stderr,
    } = Command::new(env!("CARGO_BIN_EXE_gathr"))
        .arg("--data")
        .arg(data)
        .args(args)
        .output()
        .unwrap();
    let text = if status.success() { stdout } else { stderr };
    let text = String::from_utf8_lossy(&text);
    let value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("gathr {args:?} didn't answer JSON ({e}): {text}"));
    (status.success(), value)
}
