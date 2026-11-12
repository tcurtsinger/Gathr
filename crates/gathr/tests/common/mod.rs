//! A real `gathrd` on a temp data folder and a free port, and the `gathr` command against it.
// Each test file uses part of this module.
#![allow(dead_code)]

pub mod feed;
pub mod update;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output};
use std::time::{Duration, Instant};

use gathr::data::{DataDir, ServiceInfo};
use gathr::http::{self, Response};
use serde_json::{Value, json};

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
        // A service on a data folder here keeps its agent folder beside it (0034).
        let _ = std::fs::remove_dir_all(gathr::folders::beside(&self.0));
    }
}

pub struct TestService {
    pub child: Child,
    pub info: ServiceInfo,
    data: PathBuf,
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
        Self::spawn(command, data)
    }

    /// Starts `program`, a copy of `gathrd`, with extra environment variables.
    pub fn start_program(program: &Path, data: &Path, env: &[(&str, &Path)]) -> TestService {
        let mut command = Command::new(program);
        command.arg("--data").arg(data).arg("--port").arg("0");
        for (name, value) in env {
            command.env(name, value);
        }
        Self::spawn(command, data)
    }

    /// Starts it with extra arguments, such as `--ui <folder>`.
    pub fn start_args(data: &Path, args: &[(&str, &Path)]) -> TestService {
        let mut command = gathrd(data);
        for (name, value) in args {
            command.arg(name).arg(value);
        }
        Self::spawn(command, data)
    }

    fn spawn(mut command: Command, data: &Path) -> TestService {
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(info) = ServiceInfo::read(&DataDir::new(data))
                && info.pid == child.id()
            {
                let data = data.to_path_buf();
                return TestService { child, info, data };
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

    /// The live socket, past its handshake, reading with a 3 s timeout.
    pub fn live(&self) -> std::net::TcpStream {
        use std::io::{Read, Write};
        let mut socket = std::net::TcpStream::connect(("127.0.0.1", self.info.port)).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let handshake = format!(
            "GET /api/live?token={} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nUpgrade: websocket\r\n\
             Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
             Sec-WebSocket-Version: 13\r\n\r\n",
            self.info.token, self.info.port
        );
        socket.write_all(handshake.as_bytes()).unwrap();
        let mut head = Vec::new();
        let mut byte = [0u8];
        while !head.ends_with(b"\r\n\r\n") {
            socket.read_exact(&mut byte).unwrap();
            head.push(byte[0]);
        }
        let head = String::from_utf8_lossy(&head);
        assert!(head.starts_with("HTTP/1.1 101"), "{head}");
        socket
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
            if std::thread::panicking() {
                // A failing test isn't held up by a service that won't stop.
                let deadline = Instant::now() + Duration::from_secs(10);
                while matches!(self.child.try_wait(), Ok(None)) && Instant::now() < deadline {
                    std::thread::sleep(Duration::from_millis(50));
                }
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
        if std::thread::panicking() {
            keep_failure(&self.data);
        }
    }
}

/// A failed test's evidence: the service's last 50 events on stderr, and its data folder copied to
/// `GATHR_TEST_KEEP` (CI keeps that folder) under the test's name.
fn keep_failure(data: &Path) {
    let last = "SELECT time, name, phase, ref, duration_us, status, attrs FROM event_log \
                ORDER BY ts DESC, id DESC LIMIT 50";
    let events = Command::new(env!("CARGO_BIN_EXE_gathr"))
        .arg("--data")
        .arg(data)
        .args(["diag", "query", last])
        .output();
    if let Ok(out) = events {
        eprintln!("the service's last 50 events, newest first:");
        eprintln!("{}", String::from_utf8_lossy(&out.stdout));
    }
    let Some(keep) = std::env::var_os("GATHR_TEST_KEEP") else {
        return;
    };
    let test = std::thread::current()
        .name()
        .unwrap_or("test")
        .replace("::", "-");
    let to = Path::new(&keep).join(format!("{test}-{}", gathr::ids::span_id()));
    match copy_dir(data, &to) {
        Ok(()) => eprintln!("its data folder is kept in {}", to.display()),
        Err(e) => eprintln!("keeping its data folder failed: {e}"),
    }
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
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

/// The stand-in for GitHub's `gh` (`examples/fake_gh.rs`), which `cargo test` builds too.
pub fn fake_gh() -> PathBuf {
    let path = replay_engine().with_file_name(format!("fake_gh{}", std::env::consts::EXE_SUFFIX));
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

/// The next JSON sent on the live socket (the service's frames are unmasked and unfragmented).
pub fn next_live(socket: &mut std::net::TcpStream) -> Value {
    use std::io::Read;
    loop {
        let mut head = [0u8; 2];
        socket
            .read_exact(&mut head)
            .expect("the live socket went quiet");
        let len = match head[1] & 0x7F {
            126 => {
                let mut n = [0u8; 2];
                socket.read_exact(&mut n).unwrap();
                u16::from_be_bytes(n) as usize
            }
            127 => {
                let mut n = [0u8; 8];
                socket.read_exact(&mut n).unwrap();
                u64::from_be_bytes(n) as usize
            }
            n => n as usize,
        };
        let mut body = vec![0u8; len];
        socket.read_exact(&mut body).unwrap();
        if head[0] & 0x0F == 1 {
            return serde_json::from_slice(&body).unwrap();
        }
    }
}

/// `git` without the repository the tests themselves may run inside (a hook, `git bisect run`):
/// with its `GIT_DIR`, a test's `git init` would act on the real repository (S37).
pub fn git_command() -> Command {
    let mut command = Command::new("git");
    for var in gathr::platform::env::LOCAL_GIT_VARS {
        command.env_remove(var);
    }
    command
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = git_command()
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A project cloned from a remote that has since moved on: the project, and the remote's newest
/// commit.
pub fn project(root: &Path) -> (PathBuf, String) {
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    git(&seed, &["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("README.md"), "made up\n").unwrap();
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-q", "-m", "first"]);
    git(root, &["clone", "-q", "--bare", "seed", "origin.git"]);
    git(root, &["clone", "-q", "origin.git", "project"]);
    git(root, &["clone", "-q", "origin.git", "other"]);
    let other = root.join("other");
    std::fs::write(other.join("NEXT.md"), "later\n").unwrap();
    git(&other, &["add", "."]);
    git(&other, &["commit", "-q", "-m", "second"]);
    git(&other, &["push", "-q", "origin", "main"]);
    (root.join("project"), git(&other, &["rev-parse", "HEAD"]))
}

/// `signed_in` with the engine held after the message arrives, until `go` exists.
pub fn held_recording(dir: &Path) -> PathBuf {
    let held = dir.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    held
}

/// Ends a process outright, as a crash would.
pub fn kill(pid: &str) {
    let _ = if cfg!(windows) {
        Command::new("taskkill").args(["/F", "/PID", pid]).output()
    } else {
        Command::new("kill").args(["-KILL", pid]).output()
    };
}

pub fn until<T>(secs: u64, what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "waited {secs} s for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// One JSON-RPC request to `/mcp` as an engine sends it under the 2026-07-28 revision.
pub fn mcp(port: u16, token: &str, method: &str, params: Value, name: Option<&str>) -> Response {
    let mut params = params;
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let auth = format!("Bearer {token}");
    let mut headers = vec![
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("MCP-Protocol-Version", "2026-07-28"),
        ("Mcp-Method", method),
    ];
    if let Some(name) = name {
        headers.push(("Mcp-Name", name));
    }
    http::post(port, "/mcp", &headers, &body.to_string())
        .unwrap_or_else(|e| panic!("{method} {name:?}: {e}"))
}

pub fn call_tool(port: u16, token: &str, tool: &str, arguments: Value) -> Value {
    let params = json!({ "name": tool, "arguments": arguments });
    let answer = mcp(port, token, "tools/call", params, Some(tool));
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["result"].clone()
}

pub fn text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

/// A chat in a new project on `folder` with `members`: its id. Each call names its own project.
pub fn project_chat(service: &TestService, folder: &Path, members: Value) -> String {
    let name = format!("Project {}", gathr::ids::uuid());
    let body = serde_json::json!({ "name": name, "folder": folder, "members": members });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let project = made.json()["id"].as_str().unwrap().to_string();
    let chat = service.send(
        "POST",
        "/api/chats",
        serde_json::json!({ "project": project }),
    );
    assert_eq!(chat.status, 200, "{}", chat.body);
    chat.json()["id"].as_str().unwrap().to_string()
}

/// A signed-in chat with a Supervisor (Lead) and a member (Ann): the service, the chat, the lead.
pub fn team(data: &Path, replay: &Path, more: &[(&str, &Path)]) -> (TestService, String, String) {
    let mut env = vec![
        ("GATHR_REPLAY", replay),
        ("GATHR_REPLAY_HOLD", Path::new("")),
        ("GATHR_REPLAY_ENV", Path::new("")),
    ];
    let (go, tokens) = (data.join("go"), data.join("env.jsonl"));
    env[1].1 = &go;
    env[2].1 = &tokens;
    env.extend_from_slice(more);
    let service = TestService::with_engine(data, &env);
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    (service, chat, lead)
}

/// The token of the first engine the replay engine started, from `GATHR_REPLAY_ENV`.
pub fn first_token(data: &Path) -> String {
    until(10, "the engine's token", || {
        let text = std::fs::read_to_string(data.join("env.jsonl")).ok()?;
        let line: Value = serde_json::from_str(text.lines().next()?).ok()?;
        line["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

/// The `n`th line of a file the replay engine appends to, once it's there and whole.
pub fn nth(file: &Path, n: usize) -> Value {
    // Project engines create a Git worktree first; parallel nextest setup can exceed 10 s.
    until(30, &format!("line {n} in {}", file.display()), || {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let whole = text.rsplit_once('\n').map_or("", |(lines, _)| lines);
        serde_json::from_str(whole.lines().nth(n)?).ok()
    })
}

/// Gathr's hook (0029), asked as Claude Code asks it before `tool`, with the engine's `token`.
pub fn hook(port: u16, token: &str, tool: &str, input: Value) -> Response {
    hook_at(port, "/hook", token, tool, input)
}

/// The same at `path`: `/hook?auto` is how an engine in auto mode asks.
pub fn hook_at(port: u16, path: &str, token: &str, tool: &str, input: Value) -> Response {
    let body = json!({ "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input });
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
    ];
    http::post(port, path, &headers, &body.to_string()).unwrap()
}
