//! What the update tests share (0032): copies of the real `gathrd` in a test data folder, a stand-in
//! release feed, the service that takes over, and a team chat whose engines hold on a test's word.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gathr::data::{DataDir, ServiceInfo};
use gathr::{VERSION, http};
use serde_json::{Value, json};

use super::feed::Feed;
use super::{TempDir, TestService, call_tool, gathr, recording, replay_engine, text, until};

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

/// A team's update: the old copy's service, its folder, and a chat with a Supervisor (Lead) and a
/// member (Ann) whose engines each hold once, after their message arrives, until `go` is made.
pub struct Room {
    pub dir: TempDir,
    pub data: PathBuf,
    pub feed: Feed,
    pub service: TestService,
    pub chat: String,
    pub lead: String,
    pub ann: String,
}

impl Room {
    pub fn start() -> Room {
        Room::start_with(&[])
    }

    /// The same, with more in the old copy's environment.
    pub fn start_with(more: &[(&str, &Path)]) -> Room {
        let dir = TempDir::new();
        let data = dir.0.join("data");
        let old = seed_copy(&data, "0.0.1");
        let feed = Feed::start();
        // Each engine holds once and uses `go` up, so the test lets one held turn go on at a time.
        let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
        let (first, rest) = frames.split_once('\n').unwrap();
        let held = dir.0.join("held.jsonl");
        let take = r#"{"dir":"hold","line":"take"}"#;
        // Three turns to an engine: a carry-on, then what was sent meanwhile.
        let round = format!("{first}\n{take}\n{}\n", rest.trim_end());
        std::fs::write(&held, round.repeat(3)).unwrap();
        // GitHub's `gh` stands in with no answers, so a task's look at its PRs fails at once.
        let answers = dir.0.join("gh.json");
        std::fs::write(&answers, "[]").unwrap();
        let (url, engine, gh) = (feed.url(), replay_engine(), super::fake_gh());
        let (go, tokens) = (dir.0.join("go"), dir.0.join("env.jsonl"));
        let mut env: Vec<(&str, &Path)> = vec![
            ("GATHR_APP_RELEASES", &url),
            ("GATHR_CLAUDE_PATH", &engine),
            ("GATHR_REPLAY", &held),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &tokens),
            ("GATHR_GH_PATH", &gh),
            ("GATHR_FAKE_GH", &answers),
        ];
        env.extend_from_slice(more);
        let service = TestService::start_program(&old, &data, &env);
        let account = service.signed_in(&data);
        let agent = |name: &str| {
            let made = json!({ "name": name, "account": account });
            let made = service.send("POST", "/api/agents", made);
            assert_eq!(made.status, 200, "{}", made.body);
            made.json()["id"].as_str().unwrap().to_string()
        };
        let (lead, ann) = (agent("Lead"), agent("Ann"));
        let members =
            json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
        let made = service.send("POST", "/api/chats", json!({ "members": members }));
        assert_eq!(made.status, 200, "{}", made.body);
        let chat = made.json()["id"].as_str().unwrap().to_string();
        Room {
            dir,
            data,
            feed,
            service,
            chat,
            lead,
            ann,
        }
    }

    /// Lets the one engine that holds now go on.
    pub fn go(&self) {
        std::fs::write(self.dir.0.join("go"), "").unwrap();
    }

    /// The token of the `n`th engine started.
    pub fn token(&self, n: usize) -> String {
        let line = super::nth(&self.dir.0.join("env.jsonl"), n);
        line["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
    }

    /// The lead gives Ann task #1 and ends its turn; Ann's turn on the task runs, reports the task
    /// done and holds: its id. A task nobody reports is never done (0047).
    pub fn ann_works(&self) -> String {
        say(&self.service, &self.chat, "Plan it");
        let ask = json!({ "to": "ann", "what": "Write the notes", "why": "for the review" });
        let given = text(&call_tool(
            self.service.info.port,
            &self.token(0),
            "give_task",
            ask,
        ));
        assert!(given.starts_with("Task #1 for ann: ready."), "{given}");
        self.go();
        let history = self
            .service
            .wait_for(&self.chat, 20, "Ann's turn on the task", |h| {
                let turns = h["turns"].as_array().unwrap();
                turns.len() == 2 && turns[0]["status"] == "done" && turns[1]["status"] == "running"
            });
        assert_eq!(history["turns"][1]["agent"], self.ann.as_str(), "{history}");
        assert_eq!(history["turns"][1]["task"], 1, "{history}");
        let report = json!({ "how": "done", "note": "the notes are written" });
        let port = self.service.info.port;
        let said = text(&call_tool(port, &self.token(1), "finish_task", report));
        assert!(said.starts_with("Recorded: done."), "{said}");
        history["turns"][1]["id"].as_str().unwrap().to_string()
    }

    /// A release is published and Update pressed.
    pub fn update(&self) {
        self.update_to(env!("CARGO_BIN_EXE_gathrd"));
    }

    /// The same with `program` as the release's service: one that isn't `gathrd` fails to start.
    pub fn update_to(&self, program: &str) {
        publish(&self.feed, &self.dir.0, VERSION, program, "new ui");
        update_now(&self.service);
    }

    /// The chat as the service at `next` has it.
    pub fn history(&self, next: &ServiceInfo) -> Value {
        get(next, &format!("/api/chats/{}", self.chat))
    }

    /// Lets held engines go on, one at a time, until the chat on `next` is as `done` wants it.
    pub fn go_until(&self, next: &ServiceInfo, what: &str, done: impl Fn(&Value) -> bool) -> Value {
        until(60, what, || {
            let history = self.history(next);
            if done(&history) {
                return Some(history);
            }
            if !self.dir.0.join("go").exists() {
                self.go();
            }
            None
        })
    }

    /// Pauses (`/api/pauses`), stops (`/api/pauses/stop`) or resumes (`/api/pauses/resume`) the chat on `on`.
    pub fn pause(&self, on: &ServiceInfo, path: &str) -> Value {
        let body = json!({ "scope": "chat", "target": self.chat });
        let paused = http::send(on.port, &on.token, "POST", path, &body).unwrap();
        assert_eq!(paused.status, 200, "{}", paused.body);
        paused.json()
    }

    /// One column of a read-only query on the folder's database.
    pub fn column(&self, sql: &str, column: &str) -> Vec<Value> {
        let (ok, rows) = gathr(&self.data, &["diag", "query", sql]);
        assert!(ok, "{rows}");
        let rows = rows["rows"].as_array().cloned().unwrap_or_default();
        rows.iter().map(|row| row[column].clone()).collect()
    }

    /// Every state task #1 has had, in order.
    pub fn states(&self) -> Vec<Value> {
        let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.task.state' ORDER BY id";
        let attrs = self.column(sql, "attrs");
        attrs
            .iter()
            .map(|a| a["gathr.task.state"].clone())
            .collect()
    }

    /// How many turns the lead has had, and how many hear-backs were ever queued.
    pub fn lead_heard(&self) -> (usize, usize) {
        let turns = format!("SELECT id FROM turns WHERE agent_id = '{}'", self.lead);
        let heard = "SELECT name FROM event_log WHERE name = 'gathr.task.hear_back'";
        (
            self.column(&turns, "id").len(),
            self.column(heard, "name").len(),
        )
    }

    /// The turns saved on task #1 that answer `turn`: their ids.
    pub fn carry_ons(&self, turn: &str) -> Vec<Value> {
        let sql = format!("SELECT id FROM turns WHERE task = 1 AND answers = '{turn}'");
        self.column(&sql, "id")
    }
}

/// The carry-on of `stopped` on the service at `next`, running and held: the chat's history.
pub fn carry_on_held(next: &ServiceInfo, chat: &str, stopped: &str) -> Value {
    until(
        30,
        "the carry-on to run and hold on the new service",
        || {
            let history = get(next, &format!("/api/chats/{chat}"));
            let turns = history["turns"].as_array()?;
            let carried = turns.iter().find(|t| t["answers"] == stopped)?;
            (carried["status"] == "running").then_some(history.clone())
        },
    )
}
