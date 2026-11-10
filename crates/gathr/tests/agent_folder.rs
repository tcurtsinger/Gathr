//! The agent folder (0034) through a real service and the replay engine: the setting, where an
//! agent's work lands, where it writes without asking, and what Gathr removes for it: never a
//! folder a running engine uses, which is skipped with a reference and removed at a later sweep.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, git, held_recording, hook, nth, sign_in_account, until};
use gathr::folders::beside;
use serde_json::{Value, json};

/// A signed-in service whose engines hold after the message arrives until `go` exists, their
/// starts in `env.jsonl` and arguments in `args.jsonl`; looks every 100 ms, idle engines close
/// after 300 ms. And its account.
fn service(data: &Path) -> (TestService, String) {
    let service = TestService::with_engine(
        data,
        &[
            ("GATHR_REPLAY", &held_recording(data)),
            ("GATHR_REPLAY_HOLD", &data.join("go")),
            ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
            ("GATHR_REPLAY_ARGS", &data.join("args.jsonl")),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
            ("GATHR_IDLE_CLOSE_MS", Path::new("300")),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(data, &account);
    (service, account)
}

fn agent(service: &TestService, account: &str, name: &str) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// A project named `name` on a new repository under `data`, with `agent` as its Supervisor: its id
/// and folder.
fn project(service: &TestService, data: &Path, name: &str, agent: &str) -> (String, PathBuf) {
    let (folder, _) = common::project(&data.join("repo"));
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let body = json!({ "name": name, "folder": folder, "members": members });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    (made.json()["id"].as_str().unwrap().to_string(), folder)
}

fn chat_in(service: &TestService, project: &str) -> String {
    let made = service.send("POST", "/api/chats", json!({ "project": project }));
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

fn send(service: &TestService, chat: &str, agent: &str) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
}

/// The reference of the first skipped removal of a `kind` folder for `why`, once there is one.
fn skipped(data: &Path, kind: &str, why: &str) -> String {
    let sql = format!(
        "SELECT ref FROM event_log WHERE name = 'gathr.cleanup.skip' AND error_type = '{why}'
         AND json_extract(attrs, '$.kind') = '{kind}' ORDER BY ts"
    );
    until(20, &format!("a {kind} skipped ({why})"), || {
        let (ok, found) = gathr(data, &["diag", "query", &sql]);
        assert!(ok, "{found}");
        found["rows"][0]["ref"].as_str().map(String::from)
    })
}

fn traced(data: &Path, reference: &str) -> String {
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    trace.to_string()
}

/// Waits until the trace behind `reference` records the folder's removal.
fn removal_traced(data: &Path, reference: &str) {
    until(10, "the removal on the skip's trace", || {
        traced(data, reference)
            .contains("gathr.cleanup.remove")
            .then_some(())
    });
}

#[test]
fn the_agent_folder_is_shown_saved_and_refused_when_it_cant_be_used() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let shown = service.call("GET", "/api/settings").json();
    let default = beside(&data.0).display().to_string();
    assert_eq!(
        shown["agent_folder"], default,
        "the default, as a full path: {shown}"
    );
    let file = data.0.join("a-file");
    std::fs::write(&file, "").unwrap();
    for unusable in [
        PathBuf::new(),
        PathBuf::from("relative").join("folder"),
        data.0.join("inside-the-data-folder"),
        file.join("under-a-file"),
    ] {
        let body = json!({ "agent_folder": unusable });
        let refused = service.send("PATCH", "/api/settings", body);
        assert_eq!(
            refused.status,
            400,
            "{}: {}",
            unusable.display(),
            refused.body
        );
        let answer = refused.json();
        assert_eq!(answer["error"], "Save failed", "{answer}");
        let why = answer["why"].as_str().unwrap();
        let trace = traced(&data.0, answer["ref"].as_str().unwrap());
        assert!(trace.contains("gathr.settings.change"), "{trace}");
        assert!(
            trace.contains(&why.replace('\\', "\\\\")),
            "{why} in {trace}"
        );
    }
    let elsewhere = TempDir::new();
    let chosen = elsewhere.0.join("Agents");
    let saved = service.send("PATCH", "/api/settings", json!({ "agent_folder": chosen }));
    assert_eq!(saved.status, 200, "{}", saved.body);
    let shown = service.call("GET", "/api/settings").json();
    assert_eq!(
        shown["agent_folder"],
        chosen.display().to_string(),
        "{shown}"
    );
    assert_eq!(shown["stale_days"], 3, "the days stay: {shown}");
    assert!(chosen.is_dir(), "it was made");
}

/// A project chat's first turn for an agent whose names no disk takes as they are: the engine's
/// start (`env.jsonl`), its arguments, and the agent's own folder and Shared.
struct Started {
    service: TestService,
    data: TempDir,
    chat: String,
    agent: String,
    project: String,
    started: Value,
    args: Vec<Value>,
    own: PathBuf,
    shared: PathBuf,
}

fn started() -> Started {
    let data = TempDir::new();
    let (service, account) = service(&data.0);
    let agent = agent(&service, &account, "Pia: UI");
    let (project, _) = project(&service, &data.0, "CON", &agent);
    let chat = chat_in(&service, &project);
    send(&service, &chat, &agent);
    let started = nth(&data.0.join("env.jsonl"), 0);
    let args = serde_json::from_value(nth(&data.0.join("args.jsonl"), 0)).unwrap();
    let project_folder = beside(&data.0).join("CON_");
    Started {
        own: project_folder.join("Pia- UI"),
        shared: project_folder.join("Shared"),
        service,
        data,
        chat,
        agent,
        project,
        started,
        args,
    }
}

#[test]
fn a_project_chats_worktree_and_temp_files_land_in_the_agents_own_folder() {
    let s = started();
    let part = s.chat[s.chat.len() - 6..].to_lowercase();
    let worktree = s.own.join(&part);
    let cwd = PathBuf::from(s.started["cwd"].as_str().unwrap());
    assert_eq!(cwd, worktree, "{}", s.started);
    assert!(worktree.join(".git").exists(), "a worktree");
    let temp = s.own.join("temp").join(format!("chat-{part}"));
    let temp = temp.display().to_string();
    assert_eq!(
        s.started["TEMP"],
        json!([temp, temp, temp]),
        "{}",
        s.started
    );
    for folder in [&s.own, &s.shared] {
        let add = format!("--add-dir={}", folder.display());
        assert!(s.args.iter().any(|a| a == &add), "{add} in {:?}", s.args);
    }
    let history = s
        .service
        .call("GET", &format!("/api/chats/{}", s.chat))
        .json();
    let shown = &history["members"][0]["folder"];
    assert_eq!(shown, &worktree.display().to_string(), "{history}");
}

#[test]
fn an_agent_writes_in_its_own_folder_and_shared_without_asking_and_is_asked_elsewhere() {
    let s = started();
    let token = s.started["GATHR_AGENT_TOKEN"].as_str().unwrap();
    let port = s.service.info.port;
    let write = |path: PathBuf| {
        let answer = hook(port, token, "Write", json!({ "file_path": path }));
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()["hookSpecificOutput"]["permissionDecision"].clone()
    };
    assert_eq!(write(s.own.join("notes.md")), Value::Null, "its own folder");
    assert_eq!(write(s.shared.join("plan.md")), Value::Null, "Shared");
    let root = s.own.parent().unwrap().parent().unwrap();
    assert_eq!(write(root.join("other.md")), "ask", "beside the project");
    assert_eq!(
        write(s.data.0.join("gathr.db")),
        "ask",
        "Gathr's data folder"
    );
}

#[test]
fn a_renamed_agent_keeps_its_folder() {
    let s = started();
    std::fs::write(s.data.0.join("go"), "").unwrap();
    s.service.wait_for(&s.chat, 20, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let path = format!("/api/agents/{}", s.agent);
    let renamed = s.service.send("PATCH", &path, json!({ "name": "Juniper" }));
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let next = chat_in(&s.service, &s.project);
    send(&s.service, &next, &s.agent);
    let started = nth(&s.data.0.join("env.jsonl"), 1);
    let cwd = PathBuf::from(started["cwd"].as_str().unwrap());
    assert_eq!(cwd.parent(), Some(s.own.as_path()), "{started}");
}

#[test]
fn an_engines_temp_folder_lasts_as_long_as_the_engine() {
    let data = TempDir::new();
    let (service, account) = service(&data.0);
    let ann = agent(&service, &account, "Ann");
    let chat = service.new_chat();
    send(&service, &chat, &ann);
    let started = nth(&data.0.join("env.jsonl"), 0);
    let temp = PathBuf::from(started["TEMP"][0].as_str().unwrap());
    let part = chat[chat.len() - 6..].to_lowercase();
    let own = beside(&data.0).join("Chats").join("Ann");
    assert_eq!(
        temp,
        own.join("temp").join(format!("chat-{part}")),
        "{started}"
    );
    std::fs::write(temp.join("build.log"), "made by a test run").unwrap();
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 20, "the turn", |h| h["turns"][0]["status"] == "done");
    until(20, "the temp folder removed once the engine closed", || {
        (!temp.exists()).then_some(())
    });
    assert!(own.join(&part).is_dir(), "the chat's own folder stays");
}

#[test]
fn archiving_removes_a_worktree_once_nothing_runs_in_it_and_nothing_is_left_uncommitted() {
    let s = started();
    let worktree = PathBuf::from(s.started["cwd"].as_str().unwrap());
    let notes = worktree.join("notes.md");
    std::fs::write(&notes, "not committed").unwrap();
    let archived = s
        .service
        .call("POST", &format!("/api/chats/{}/archive", s.chat));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let skip = skipped(&s.data.0, "worktree", "in_use");
    std::fs::write(s.data.0.join("go"), "").unwrap();
    skipped(&s.data.0, "worktree", "changes");
    assert!(notes.is_file(), "what isn't committed stays");
    std::fs::remove_file(&notes).unwrap();
    until(20, "the worktree removed", || {
        (!worktree.exists()).then_some(())
    });
    let repo = s.data.0.join("repo").join("project");
    let branches = git(&repo, &["branch", "--list", "gathr/*"]);
    assert!(!branches.is_empty(), "its branch stays");
    removal_traced(&s.data.0, &skip);
}
