//! Deleting a project or an agent through a real service (0069): a project goes with its chats and
//! every folder Gathr made for it but Shared, never the person's checkout; an agent goes wherever it
//! is, its open tasks cancelled, its lead passed on, and what it was the only agent of with it; its
//! old messages still name it; the confirm's counts are what the delete then does.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, git, held_recording, until};
use gathr::folders::{beside, real};
use serde_json::{Value, json};

fn member(agent: &str, role: &str) -> Value {
    json!({ "agent": agent, "role": role })
}

/// A new agent on `account`: its id.
fn agent(service: &TestService, account: &str, name: &str) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

fn agents(service: &TestService) -> Value {
    let listed = service.call("GET", "/api/agents");
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

/// A new project named `name` on `folder` with `members`: its id.
fn new_project(service: &TestService, name: &str, folder: &Path, members: Value) -> String {
    let body = json!({ "name": name, "folder": folder, "members": members });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// The project as listed, or none once it's gone.
fn project_of(service: &TestService, id: &str) -> Option<Value> {
    let listed = service.call("GET", "/api/projects").json();
    let mut found = listed["projects"].as_array().unwrap().iter();
    found.find(|p| p["id"] == id).cloned()
}

/// A new chat with `body` (`{"project"}` or `{"members"}`): its id.
fn new_chat(service: &TestService, body: Value) -> String {
    let made = service.send("POST", "/api/chats", body);
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

fn chat_in(service: &TestService, project: &str) -> String {
    new_chat(service, json!({ "project": project }))
}

fn send(service: &TestService, chat: &str, body: Value) {
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    assert_eq!(sent.status, 200, "{}", sent.body);
}

/// A team as "Name role", lead first.
fn seats(members: &Value) -> Vec<String> {
    let members = members.as_array().unwrap().iter();
    members
        .map(|m| {
            format!(
                "{} {}",
                m["name"].as_str().unwrap(),
                m["role"].as_str().unwrap()
            )
        })
        .collect()
}

fn chat_seats(service: &TestService, chat: &str) -> Vec<String> {
    let history = service.call("GET", &format!("/api/chats/{chat}"));
    assert_eq!(history.status, 200, "{}", history.body);
    seats(&history.json()["members"])
}

fn project_seats(service: &TestService, project: &str) -> Vec<String> {
    let found = project_of(service, project);
    seats(&found.unwrap_or_else(|| panic!("no project {project}"))["members"])
}

fn gone(service: &TestService, chat: &str) -> bool {
    service.call("GET", &format!("/api/chats/{chat}")).status == 404
}

fn archived(service: &TestService) -> Vec<String> {
    let chats = service.call("GET", "/api/chats?archived=true").json();
    let ids = chats.as_array().unwrap().iter();
    ids.map(|c| c["id"].as_str().unwrap().to_string()).collect()
}

/// Deletes at `path`, which must answer 200: the answer.
fn delete(service: &TestService, path: &str) -> Value {
    let deleted = service.call("DELETE", path);
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    deleted.json()
}

#[test]
fn a_project_goes_with_its_chats_and_folders_and_shared_and_the_persons_checkout_stay() {
    let data = TempDir::new();
    let (held, go, env) = (
        held_recording(&data.0, "signed_in"),
        data.0.join("go"),
        data.0.join("env.jsonl"),
    );
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.signed_in(&data.0);
    let ann = agent(&service, &account, "Ann");
    let (folder, _) = common::project(&data.0.join("repo"));
    let larder = new_project(
        &service,
        "Larder",
        &folder,
        json!([member(&ann, "supervisor")]),
    );
    let (working, open, put_away) = (
        chat_in(&service, &larder),
        chat_in(&service, &larder),
        chat_in(&service, &larder),
    );
    let archive = service.call("POST", &format!("/api/chats/{put_away}/archive"));
    assert_eq!(archive.status, 200, "{}", archive.body);
    // Ann's turn runs, held, in its worktree in the agent folder.
    send(&service, &working, json!({ "text": "Hi", "agent": ann }));
    let started = common::nth(&env, 0);
    let place = real(&beside(&data.0)).join("Larder");
    let worktree = PathBuf::from(started["cwd"].as_str().unwrap());
    assert_eq!(
        worktree.parent(),
        Some(place.join("Ann").as_path()),
        "{started}"
    );
    let shared = place.join("Shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(shared.join("plan.md"), "the team's plan").unwrap();
    // Its checks checkout and caches, as a checks run leaves them.
    let checks = place.join("checks");
    git(
        &folder,
        &[
            "worktree",
            "add",
            "--detach",
            &checks.display().to_string(),
            "HEAD",
        ],
    );
    std::fs::create_dir_all(place.join("caches").join("npm")).unwrap();
    std::fs::write(place.join("caches").join("npm").join("index"), "cached").unwrap();
    // The person's own checkout, with work not committed.
    let head = git(&folder, &["rev-parse", "HEAD"]);
    std::fs::write(folder.join("mine.md"), "not committed").unwrap();

    let (port, token) = (service.info.port, service.info.token.clone());
    let path = format!("/api/projects/{larder}");
    let deleting =
        std::thread::spawn(move || gathr::http::call(port, &token, "DELETE", &path).unwrap());
    // The held stand-in reads its stop once it streams: let it go once the delete has stopped it.
    until(30, "the running chat archived by the delete", || {
        archived(&service).contains(&working).then_some(())
    });
    std::fs::write(&go, "").unwrap();
    let deleted = deleting.join().unwrap();
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    let answer = deleted.json();
    assert_eq!(answer["chats"], 3, "{answer}");
    assert_eq!(answer["left"], json!([]), "{answer}");

    assert!(
        project_of(&service, &larder).is_none(),
        "the project is listed"
    );
    for chat in [&working, &open, &put_away] {
        assert!(gone(&service, chat), "{chat} is still there");
    }
    for part in ["Ann", "checks", "caches"] {
        assert!(!place.join(part).exists(), "{part} is still there");
    }
    let plan = std::fs::read_to_string(shared.join("plan.md")).unwrap();
    assert_eq!(plan, "the team's plan", "Shared stays");
    assert_eq!(git(&folder, &["rev-parse", "HEAD"]), head);
    let mine = std::fs::read_to_string(folder.join("mine.md")).unwrap();
    assert_eq!(mine, "not committed", "the person's work stays");
    let branches = git(&folder, &["branch", "--list", "gathr/*"]);
    assert!(!branches.is_empty(), "Ann's branch stays");
    let worktrees = git(&folder, &["worktree", "list", "--porcelain"]);
    let listed = worktrees.lines().filter(|l| l.starts_with("worktree "));
    assert_eq!(listed.count(), 1, "only the checkout itself: {worktrees}");
    let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.project.delete'";
    assert_eq!(common::rows(&data.0, sql)[0]["n"], 1);
}

#[test]
fn deleting_a_lead_makes_the_next_agent_lead_in_the_project_and_in_each_chat() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let [mira, ann, bo] = ["Mira", "Ann", "Bo"].map(|name| agent(&service, &account, name));
    let (folder, _) = common::project(&data.0.join("repo"));
    let team = json!([
        member(&mira, "supervisor"),
        member(&ann, "member"),
        member(&bo, "member")
    ]);
    let larder = new_project(&service, "Larder", &folder, team);
    let in_larder = chat_in(&service, &larder);
    let pair = json!({ "members": [member(&mira, "supervisor"), member(&bo, "member")] });
    let plain = new_chat(&service, pair);

    delete(&service, &format!("/api/agents/{mira}"));
    let after = ["Ann supervisor", "Bo member"];
    assert_eq!(project_seats(&service, &larder), after);
    assert_eq!(chat_seats(&service, &in_larder), after);
    assert_eq!(chat_seats(&service, &plain), ["Bo supervisor"]);
}

#[test]
fn deleting_the_only_agent_of_a_project_or_a_chat_deletes_it_with_its_chats() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let (kai, bo) = (
        agent(&service, &account, "Kai"),
        agent(&service, &account, "Bo"),
    );
    let (folder, _) = common::project(&data.0.join("repo"));
    let pantry = new_project(
        &service,
        "Pantry",
        &folder,
        json!([member(&kai, "supervisor")]),
    );
    let in_pantry = [chat_in(&service, &pantry), chat_in(&service, &pantry)];
    let alone = new_chat(&service, json!({ "members": [member(&kai, "supervisor")] }));
    let pair = json!({ "members": [member(&kai, "supervisor"), member(&bo, "member")] });
    let shared = new_chat(&service, pair);

    let deleted = delete(&service, &format!("/api/agents/{kai}"));
    assert_eq!(
        (&deleted["projects"], &deleted["chats"]),
        (&json!(1), &json!(3)),
        "{deleted}"
    );
    assert!(project_of(&service, &pantry).is_none(), "Pantry is listed");
    for chat in in_pantry.iter().chain([&alone]) {
        assert!(gone(&service, chat), "{chat} is still there");
    }
    assert_eq!(chat_seats(&service, &shared), ["Bo supervisor"]);
}

#[test]
fn a_removed_agents_name_is_free_and_it_is_in_no_list() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let old = agent(&service, &account, "Ann");
    delete(&service, &format!("/api/agents/{old}"));

    // Names are unique ignoring case, among the agents that aren't removed.
    let new = agent(&service, &account, "ann");
    assert_ne!(new, old);
    let listed = agents(&service);
    let shown = listed["agents"].as_array().unwrap().iter();
    let shown: Vec<(&Value, &Value)> = shown.map(|a| (&a["id"], &a["name"])).collect();
    assert_eq!(shown, [(&json!(new), &json!("ann"))], "{listed}");
    assert_eq!(
        listed["removed"],
        json!([{ "id": old, "name": "Ann", "shape": null, "eyes": null, "colour": null }])
    );
    let path = format!("/api/agents/{old}");
    assert_eq!(
        service.call("GET", &path).status,
        404,
        "a removed agent is shown"
    );
    let renamed = service.send("PATCH", &path, json!({ "name": "Old" }));
    assert_eq!(
        renamed.status, 404,
        "a removed agent changed: {}",
        renamed.body
    );
    let seated = service.send(
        "POST",
        "/api/chats",
        json!({ "members": [member(&old, "supervisor")] }),
    );
    assert_eq!(
        seated.status, 400,
        "a removed agent seated: {}",
        seated.body
    );
}
