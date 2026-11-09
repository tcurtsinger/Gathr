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
fn a_new_chat_in_a_project_copies_its_folder_and_team_and_nothing_else() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let ids = agents(&service, &data.0, &["Orin", "Vale", "Pia"]);
    let (orin, vale, pia) = (&ids[0], &ids[1], &ids[2]);
    let team = json!([member(orin, "supervisor"), member(vale, "reviewer")]);
    let body = json!({ "name": "Gathr", "folder": folder, "members": team });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let id = made.json()["id"].as_str().unwrap().to_string();

    let refused = [
        (
            json!({ "name": "gathr", "folder": folder, "members": team }),
            409,
            "a name used, in another case",
        ),
        (
            json!({ "name": " ", "folder": folder, "members": team }),
            400,
            "no name",
        ),
        (
            json!({ "name": "Other", "folder": repos.0.join("nope"), "members": team }),
            400,
            "not a git repository",
        ),
        (
            json!({ "name": "Other", "folder": folder, "members": [member(vale, "reviewer")] }),
            400,
            "a Reviewer alone",
        ),
        (
            json!({ "name": "Other", "folder": folder, "members": [member("nope", "member")] }),
            400,
            "no such agent",
        ),
    ];
    for (body, status, what) in refused {
        let answer = service.send("POST", "/api/projects", body);
        assert_eq!(answer.status, status, "{what}: {}", answer.body);
        assert!(answer.json()["ref"].is_string(), "{what}: {}", answer.body);
    }
    let listed = service.call("GET", "/api/projects").json()["projects"].clone();
    assert_eq!(listed.as_array().unwrap().len(), 1, "{listed}");
    assert_eq!(listed[0]["name"], "Gathr");
    assert_eq!(
        roles(&listed[0]["members"]),
        [pair(orin, "supervisor"), pair(vale, "reviewer")]
    );

    let first = service
        .send("POST", "/api/chats", json!({ "project": id }))
        .json();
    let chat = first["id"].as_str().unwrap().to_string();
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Start" }),
    );
    service.wait_for(&chat, 20, "the first chat's turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    // The chat's own team is what counts after it's made: a change stays in the chat.
    let grown = json!({ "members": [member(orin, "supervisor"), member(vale, "reviewer"), member(pia, "member")] });
    let answer = service.send("PATCH", &format!("/api/chats/{chat}"), grown);
    assert_eq!(answer.status, 200, "{}", answer.body);
    let listed = service.call("GET", "/api/projects").json()["projects"].clone();
    assert_eq!(
        listed[0]["members"].as_array().unwrap().len(),
        2,
        "the chat changed its project: {listed}"
    );

    let second = service
        .send("POST", "/api/chats", json!({ "project": id }))
        .json();
    let next = second["id"].as_str().unwrap().to_string();
    let history = service.call("GET", &format!("/api/chats/{next}")).json();
    assert_eq!(history["project"], id.as_str(), "{history}");
    assert!(history["folder"].is_string(), "{history}");
    assert_eq!(
        roles(&history["members"]),
        [pair(orin, "supervisor"), pair(vale, "reviewer")]
    );
    assert_eq!(
        history["messages"],
        json!([]),
        "a new chat inherited messages: {history}"
    );
    assert_eq!(history["turns"], json!([]), "{history}");
    let tasks = service
        .call("GET", &format!("/api/chats/{next}/tasks"))
        .json();
    assert_eq!(
        tasks,
        json!({ "tasks": [], "waits": [] }),
        "a new chat inherited work"
    );
    let chats = service.call("GET", "/api/chats").json();
    let grouped = chats
        .as_array()
        .unwrap()
        .iter()
        .filter(|c| c["project"] == id.as_str())
        .count();
    assert_eq!(
        grouped, 2,
        "the sidebar groups both chats under the project: {chats}"
    );

    // "New" makes the chat at once, so a project is picked for an empty chat after it's made.
    let empty = service.new_chat();
    let path = format!("/api/chats/{empty}");
    let picked = service.send("PATCH", &path, json!({ "project": id }));
    assert_eq!(picked.status, 200, "{}", picked.body);
    let history = service.call("GET", &path).json();
    assert_eq!(history["project"], id.as_str(), "{history}");
    assert!(history["folder"].is_string(), "{history}");
    assert_eq!(
        roles(&history["members"]),
        [pair(orin, "supervisor"), pair(vale, "reviewer")]
    );
    let talked = service.send(
        "PATCH",
        &format!("/api/chats/{chat}"),
        json!({ "project": id }),
    );
    assert_eq!(talked.status, 409, "a chat with messages: {}", talked.body);
    let nowhere = service.send("PATCH", &path, json!({ "project": "nope" }));
    assert_eq!(nowhere.status, 404, "{}", nowhere.body);

    // In a project the team is the project's: an agent outside it isn't added by a send.
    let outsider = json!({ "text": "Hello", "agent": pia });
    let answer = service.send("POST", &format!("/api/chats/{next}/messages"), outsider);
    assert_eq!(answer.status, 400, "{}", answer.body);
    let unknown = service.send("POST", "/api/chats", json!({ "project": "nope" }));
    assert_eq!(unknown.status, 404, "{}", unknown.body);
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
        one["members"],
        json!([]),
        "one agent is a plain chat still: {one}"
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
    assert_eq!(rows["rows"][0]["n"], 4, "each join is recorded: {rows}");
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
