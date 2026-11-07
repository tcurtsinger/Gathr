//! A real `gathrd` on a temp data folder and a free port, and the `gathr` command against it.

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
        let mut child = gathrd(data).spawn().unwrap();
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

    pub fn get(&self, path: &str, headers: &[(&str, &str)]) -> Response {
        http::request(self.info.port, "GET", path, headers).unwrap()
    }

    pub fn call(&self, method: &str, path: &str) -> Response {
        http::call(self.info.port, &self.info.token, method, path).unwrap()
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
