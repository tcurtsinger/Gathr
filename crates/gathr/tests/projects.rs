//! Projects as saved records that a new chat copies, and picking agents in a plain chat (0028).

mod common;

use std::path::Path;

use common::{TempDir, TestService, gathr, project, recording, sign_in_account};
use serde_json::{Value, json};

fn agents(service: &TestService, data: &Path, names: &[&str]) -> Vec<String> {
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let made = |name: &&str| {
        let added = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(added.status, 200, "{}", added.body);
        added.json()["id"].as_str().unwrap().to_string()
    };
    names.iter().map(made).collect()
}

fn member(agent: &str, role: &str) -> Value {
    json!({ "agent": agent, "role": role })
}

fn roles(members: &Value) -> Vec<(String, String)> {
    let members = members.as_array().unwrap();
    let pair = |m: &Value| {
        (
            m["agent"].as_str().unwrap().into(),
            m["role"].as_str().unwrap().into(),
        )
    };
    members.iter().map(pair).collect()
}

fn pair(agent: &str, role: &str) -> (String, String) {
    (agent.to_string(), role.to_string())
}

#[test]
fn picking_another_agent_in_a_plain_chat_adds_it_under_the_first_as_supervisor() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let ids = agents(
        &service,
        &data.0,
        &["Orin", "Pia", "Vale", "Juno", "Cass", "Rook"],
    );
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let send = |agent: &str, n: usize| {
        let answer = service.send("POST", &path, json!({ "text": "Hi", "agent": agent }));
        assert_eq!(answer.status, 200, "{}", answer.body);
        service.wait_for(&chat, 20, "the turn", |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns.len() == n && turns.iter().all(|t| t["ended"].is_string())
        })
    };

    let one = send(&ids[0], 1);
    assert_eq!(
        roles(&one["members"]),
        [pair(&ids[0], "supervisor")],
        "a chat of one keeps its agent as its lead: {one}"
    );
    let two = send(&ids[1], 2);
    assert_eq!(
        roles(&two["members"]),
        [pair(&ids[0], "supervisor"), pair(&ids[1], "member")]
    );
    let unnamed = service
        .send("POST", &path, json!({ "text": "Who's next?" }))
        .json();
    let three = service.wait_for(&chat, 20, "the unnamed turn", |h| {
        h["turns"]
            .as_array()
            .is_some_and(|t| t.len() == 3 && t[2]["ended"].is_string())
    });
    assert_eq!(three["turns"][2]["id"], unnamed["turn"]);
    assert_eq!(
        three["turns"][2]["agent"],
        ids[0].as_str(),
        "an unnamed message goes to the Supervisor"
    );
    for (n, agent) in ids[2..5].iter().enumerate() {
        send(agent, 4 + n);
    }
    let full = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(full["members"].as_array().unwrap().len(), 5, "{full}");
    let sixth = service.send("POST", &path, json!({ "text": "Hi", "agent": ids[5] }));
    assert_eq!(sixth.status, 400, "a sixth agent: {}", sixth.body);
    let joins = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.members'";
    let (_, rows) = gathr(&data.0, &["diag", "query", joins]);
    assert_eq!(
        rows["rows"][0]["n"], 5,
        "the first agent's seat and each join are recorded: {rows}"
    );
}

/// A window open before them hears of both, so its sidebar shows them (the round's "No chats yet").
#[test]
fn a_project_and_a_chat_made_elsewhere_reach_every_open_window() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin"]);
    let mut live = service.live();

    let body =
        json!({ "name": "Gathr", "folder": folder, "members": [member(&ids[0], "supervisor")] });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let project = made.json()["id"].clone();
    let chat = service.send("POST", "/api/chats", json!({ "project": project }));
    assert_eq!(chat.status, 200, "{}", chat.body);

    let heard = [common::next_live(&mut live), common::next_live(&mut live)];
    let expected = [
        json!({ "type": "project", "project": project, "change": "created" }),
        json!({ "type": "chat", "chat": chat.json()["id"], "change": "created" }),
    ];
    assert_eq!(heard, expected);
}

#[test]
fn a_project_that_fails_to_save_is_found_from_its_reference() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin"]);
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    let trigger = "CREATE TRIGGER no_projects BEFORE INSERT ON projects BEGIN SELECT RAISE(ABORT, 'forced'); END";
    db.execute(trigger, []).unwrap();

    let body = json!({ "name": "Gathr", "folder": folder, "members": [member(&ids[0], "member")] });
    let failed = service.send("POST", "/api/projects", body);
    assert_eq!(failed.status, 500, "{}", failed.body);
    let error = failed.json();
    assert_eq!(error["error"], "New project failed", "{error}");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", error["ref"].as_str().unwrap()]);
    assert!(ok && trace.to_string().contains("forced"), "{trace}");
}

#[test]
fn a_folder_outside_git_is_made_a_repository_when_new_project_asks() {
    let data = TempDir::new();
    let folders = TempDir::new();
    let folder = folders.0.join("Game");
    std::fs::create_dir_all(folder.join("art")).unwrap();
    std::fs::write(folder.join("art").join("map.txt"), "hills").unwrap();
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin"]);

    let checked = service.send("POST", "/api/projects/check", json!({ "folder": folder }));
    assert_eq!(checked.json(), json!({ "folder": true, "repo": false }));
    let missing = folders.0.join("Gone");
    let checked = service.send("POST", "/api/projects/check", json!({ "folder": missing }));
    assert_eq!(checked.json(), json!({ "folder": false, "repo": false }));

    let body =
        json!({ "name": "Game", "folder": folder, "members": [member(&ids[0], "supervisor")] });
    let refused = service.send("POST", "/api/projects", body.clone());
    assert_eq!(refused.status, 400, "{}", refused.body);
    assert!(!folder.join(".git").exists());

    let mut asked = body;
    asked["init"] = json!(true);
    let made = service.send("POST", "/api/projects", asked);
    assert_eq!(made.status, 200, "{}", made.body);
    let checked = service.send("POST", "/api/projects/check", json!({ "folder": folder }));
    assert_eq!(checked.json(), json!({ "folder": true, "repo": true }));
    let files = std::process::Command::new("git")
        .args(["ls-tree", "-r", "--name-only", "HEAD"])
        .current_dir(&folder)
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&files.stdout).trim(), "art/map.txt");
}
