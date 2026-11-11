//! A project's checks set by its Supervisor through Gathr's tools (0036, owner's call on task
//! 1455): only the Supervisor, starting from what Gathr finds; a command the project's own files
//! don't name asks the person once per project.

mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use common::{
    TempDir, TestService, call_tool, held_recording, nth, project, sign_in_account, text,
};
use serde_json::{Value, json};

/// Lets the held engines go however the test ends: dropped first, before the service.
struct Release(std::path::PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

struct Room {
    _release: Release,
    service: TestService,
    chat: String,
    project: String,
    lead: String,
    ann: String,
}

/// A project whose checkout names its own checks (Cargo, npm, a workflow), and a chat in it with
/// a Supervisor (Lead) and a member (Ann), each engine held once its message arrives.
fn room(data: &Path, repos: &Path) -> Room {
    let (folder, _) = project(repos);
    std::fs::write(folder.join("Cargo.toml"), "[package]\nname = \"made-up\"\n").unwrap();
    let package = r#"{ "scripts": { "test": "vitest run", "lint": "biome ci" } }"#;
    std::fs::write(folder.join("package.json"), package).unwrap();
    let workflows = folder.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    let workflow =
        "jobs:\n  ui:\n    steps:\n      - run: npx vitest run\n        working-directory: ui\n";
    std::fs::write(workflows.join("ci.yml"), workflow).unwrap();
    let service = TestService::with_engine(
        data,
        &[
            ("GATHR_REPLAY", &held_recording(data)),
            ("GATHR_REPLAY_HOLD", &data.join("go")),
            ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
        ],
    );
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let body = json!({ "name": "Site", "folder": folder, "members": members });
    let project = service.send("POST", "/api/projects", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let chat = service
        .send("POST", "/api/chats", json!({ "project": project }))
        .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    Room {
        _release: Release(data.join("go")),
        service,
        chat,
        project,
        lead,
        ann,
    }
}

/// Sends `agent` a message and answers its engine's token once its turn holds.
fn turn_of(room: &Room, data: &Path, agent: &str) -> String {
    let path = format!("/api/chats/{}/messages", room.chat);
    let sent = room.service.send(
        "POST",
        &path,
        json!({ "text": "Set it up", "agent": agent }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    nth(&data.join("env.jsonl"), 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string()
}

fn saved(room: &Room) -> Value {
    let projects = room.service.call("GET", "/api/projects").json();
    let mut found = projects["projects"].as_array().unwrap().iter();
    found.find(|p| p["id"] == room.project.as_str()).unwrap()["checks"].clone()
}

fn approvals(room: &Room) -> Value {
    room.service
        .call("GET", &format!("/api/chats/{}", room.chat))
        .json()["approvals"]
        .clone()
}

/// `set_checks` on a thread, answering its approval with `allow` if it asks; whether it asked
/// and the tool's result.
fn set(room: &Room, token: &str, checks: Value, allow: bool) -> (bool, Value) {
    let (port, token) = (room.service.info.port, token.to_string());
    let call = std::thread::spawn(move || {
        call_tool(port, &token, "set_checks", json!({ "checks": checks }))
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut asked = false;
    while !call.is_finished() {
        let open = approvals(room);
        if let Some(request) = open[0]["request_id"].as_str().filter(|_| !asked) {
            assert_eq!(open[0]["tool"], "project checks", "{open}");
            let to = format!("/api/chats/{}/approvals/{request}", room.chat);
            let answered = room.service.send("POST", &to, json!({ "allow": allow }));
            assert_eq!(answered.status, 200, "{}", answered.body);
            asked = true;
        }
        assert!(Instant::now() < deadline, "set_checks didn't end in 10 s");
        std::thread::sleep(Duration::from_millis(50));
    }
    (asked, call.join().unwrap())
}

#[test]
fn only_the_supervisor_lists_or_sets_a_projects_checks() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = room(&data.0, &repos.0);
    let token = turn_of(&room, &data.0, &room.ann);
    let port = room.service.info.port;
    for (tool, arguments) in [
        ("checks", json!({})),
        ("set_checks", json!({ "ci": "both" })),
        ("set_checks", json!({ "checks": ["cargo test"] })),
    ] {
        let refused = call_tool(port, &token, tool, arguments);
        let said = text(&refused);
        assert_eq!(refused["isError"], true, "{tool}: {said}");
        assert!(
            said.contains("Supervisor") && said.contains("(ref "),
            "{tool}: {said}"
        );
    }
    assert_eq!(saved(&room), json!([]), "a member's checks were saved");
}

#[test]
fn the_supervisor_starts_from_what_gathr_finds_and_an_own_runner_runs_without_asking() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = room(&data.0, &repos.0);
    let token = turn_of(&room, &data.0, &room.lead);
    let listed = call_tool(room.service.info.port, &token, "checks", json!({}));
    let listed: Value = serde_json::from_str(&text(&listed)).unwrap();
    let found = json!([
        "npm test",
        "npm run lint",
        "cargo test",
        "cargo clippy",
        "cd ui && npx vitest run"
    ]);
    assert_eq!(listed["checks"], json!([]), "{listed}");
    assert_eq!(listed["found"], found, "{listed}");

    let (asked, result) = set(&room, &token, json!(["cargo test", " npm test "]), true);
    assert!(!asked && result["isError"] != true, "{}", text(&result));
    let checks = saved(&room);
    let commands: Vec<&str> = checks
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["command"].as_str().unwrap())
        .collect();
    assert_eq!(commands, ["cargo test", "npm test"]);
    assert!(
        checks
            .as_array()
            .unwrap()
            .iter()
            .all(|c| c["source"] == room.lead.as_str()),
        "{checks}"
    );
}

#[test]
fn another_command_asks_the_person_once_per_project_and_a_no_saves_nothing() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = room(&data.0, &repos.0);
    let token = turn_of(&room, &data.0, &room.lead);

    let (asked, result) = set(&room, &token, json!(["cargo test", "make other"]), false);
    let said = text(&result);
    assert!(
        asked && result["isError"] == true && said.contains("(ref "),
        "{said}"
    );
    assert_eq!(saved(&room), json!([]), "a refused list was saved");

    let (asked, result) = set(&room, &token, json!(["cargo test", "make check"]), true);
    assert!(asked && result["isError"] != true, "{}", text(&result));
    let (asked, result) = set(&room, &token, json!(["make check"]), true);
    assert!(
        !asked && result["isError"] != true,
        "an allowed command asked again: {}",
        text(&result)
    );
    assert_eq!(saved(&room)[0]["command"], "make check");
}

fn settings_of(room: &Room) -> Value {
    let projects = room.service.call("GET", "/api/projects").json();
    let mut found = projects["projects"].as_array().unwrap().iter();
    found
        .find(|p| p["id"] == room.project.as_str())
        .unwrap()
        .clone()
}

/// Owner, 2026-11-11 (Orin's task #88): the Supervisor sets all of Checks & merge, the required
/// checks and the merge method too, each recorded with who set it and when.
#[test]
fn the_supervisor_sets_the_required_checks_and_the_merge_method_too() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = room(&data.0, &repos.0);
    let token = turn_of(&room, &data.0, &room.lead);
    let port = room.service.info.port;
    let set = call_tool(
        port,
        &token,
        "set_checks",
        json!({ "ci": "both", "merge": "squash" }),
    );
    assert_eq!(set["isError"], false, "{}", text(&set));
    let saved = settings_of(&room);
    assert_eq!(
        (&saved["ci"], &saved["merge"]),
        (&json!("both"), &json!("squash")),
        "{saved}"
    );
    for by in [&saved["ci_by"], &saved["merge_by"]] {
        assert_eq!(by["source"], room.lead.as_str(), "{saved}");
        assert!(by["set"].is_string(), "{saved}");
    }
    let listed = call_tool(port, &token, "checks", json!({}));
    let listed: Value = serde_json::from_str(&text(&listed)).unwrap();
    assert_eq!(
        (&listed["ci"], &listed["merge"]),
        (&json!("both"), &json!("squash")),
        "{listed}"
    );
    for (ask, why) in [
        (json!({ "ci": "travis" }), "travis"),
        (json!({}), "nothing"),
    ] {
        let refused = call_tool(port, &token, "set_checks", ask);
        let said = text(&refused);
        assert!(
            refused["isError"] == true && said.contains(why) && said.contains("(ref "),
            "{said}"
        );
    }
    assert_eq!(settings_of(&room)["ci"], "both", "a refusal changed it");
}
