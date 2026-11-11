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

/// 0036: a project's name, folder, team, checks, CI choice and merge method are each changed on
/// their own, each window hears of it, and it's recorded.
#[test]
fn each_project_setting_is_changed_on_its_own_and_heard_live() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin", "Vale"]);
    let team = json!([member(&ids[0], "supervisor")]);
    let made = service.send(
        "POST",
        "/api/projects",
        json!({ "name": "Gathr", "folder": folder, "members": team }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let made = made.json();
    let id = made["id"].as_str().unwrap().to_string();
    // A repo with no GitHub Actions workflows has only Gathr's checks to go by.
    let defaults = (&made["checks"], &made["ci"], &made["merge"]);
    assert_eq!(
        defaults,
        (&json!([]), &json!("gathr"), &json!("fast-forward")),
        "{made}"
    );

    let other = repos.0.join("other");
    let changes = [
        json!({ "name": "Gathr app" }),
        json!({ "folder": other }),
        json!({ "members": [member(&ids[0], "supervisor"), member(&ids[1], "reviewer")] }),
        json!({ "checks": ["cargo test --workspace", "npm test"] }),
        json!({ "ci": "both" }),
        json!({ "merge": "merge" }),
    ];
    let path = format!("/api/projects/{id}");
    let mut live = service.live();
    for change in &changes {
        let answer = service.send("PATCH", &path, change.clone());
        assert_eq!(answer.status, 200, "{change}: {}", answer.body);
        let heard = common::next_live(&mut live);
        let expected = json!({ "type": "project", "project": id, "change": "changed" });
        assert_eq!(heard, expected, "{change}");
    }
    let listed = service.call("GET", "/api/projects").json()["projects"][0].clone();
    assert_eq!(listed["name"], "Gathr app", "{listed}");
    let saved = std::path::PathBuf::from(listed["folder"].as_str().unwrap());
    assert_eq!(saved.file_name().unwrap(), "other", "{listed}");
    assert_eq!(
        roles(&listed["members"]),
        [pair(&ids[0], "supervisor"), pair(&ids[1], "reviewer")]
    );
    assert_eq!(
        listed["checks"],
        json!(["cargo test --workspace", "npm test"])
    );
    assert_eq!(
        (&listed["ci"], &listed["merge"]),
        (&json!("both"), &json!("merge"))
    );
    // Its own name in another case is still its own.
    let recased = service.send("PATCH", &path, json!({ "name": "GATHR APP" }));
    assert_eq!(recased.status, 200, "{}", recased.body);

    let query = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.project.change'";
    let (_, rows) = gathr(&data.0, &["diag", "query", query]);
    assert_eq!(rows["rows"][0]["n"], 7, "one event per change: {rows}");
}

/// A repo whose checkout has GitHub Actions workflows uses their CI unless the person picks.
#[test]
fn a_project_with_github_actions_uses_its_ci() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let workflows = folder.join(".github").join("workflows");
    std::fs::create_dir_all(&workflows).unwrap();
    std::fs::write(workflows.join("ci.yml"), "on: push\n").unwrap();
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin"]);
    let body =
        json!({ "name": "Gathr", "folder": folder, "members": [member(&ids[0], "supervisor")] });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    assert_eq!(made.json()["ci"], "ci", "{}", made.body);
}

#[test]
fn a_bad_project_setting_is_refused_with_a_reference_and_changes_nothing() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let service = TestService::start(&data.0);
    let ids = agents(&service, &data.0, &["Orin", "Vale"]);
    let team = json!([member(&ids[0], "supervisor")]);
    let first = service.send(
        "POST",
        "/api/projects",
        json!({ "name": "Gathr", "folder": folder, "members": team }),
    );
    let id = first.json()["id"].as_str().unwrap().to_string();
    let second = json!({ "name": "Site", "folder": repos.0.join("other"), "members": team });
    assert_eq!(service.send("POST", "/api/projects", second).status, 200);
    let before = service.call("GET", "/api/projects").json();

    let path = format!("/api/projects/{id}");
    let refused = [
        (
            json!({ "name": "site" }),
            409,
            "another project's name, in another case",
        ),
        (json!({ "name": " " }), 400, "no name"),
        (
            json!({ "folder": "relative/path" }),
            400,
            "a folder that isn't a full path",
        ),
        (
            json!({ "folder": repos.0.join("nope") }),
            400,
            "not a git repository",
        ),
        (
            json!({ "members": [member(&ids[1], "reviewer")] }),
            400,
            "a Reviewer alone",
        ),
        (
            json!({ "members": [member("nope", "member")] }),
            400,
            "no such agent",
        ),
        (json!({ "members": [] }), 400, "no agents"),
        (
            json!({ "checks": ["cargo test", "  "] }),
            400,
            "an empty check",
        ),
        (json!({ "ci": "travis" }), 400, "an unknown CI choice"),
        (json!({ "merge": "rebase" }), 400, "an unknown merge method"),
        (
            json!({ "ci": "both", "merge": "rebase" }),
            400,
            "one bad field among good ones",
        ),
    ];
    for (body, status, what) in refused {
        let answer = service.send("PATCH", &path, body);
        assert_eq!(answer.status, status, "{what}: {}", answer.body);
        let error = answer.json();
        assert_eq!(error["error"], "Project settings failed", "{what}: {error}");
        let reference = error["ref"]
            .as_str()
            .unwrap_or_else(|| panic!("{what}: {error}"));
        let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
        assert!(
            ok && trace.to_string().contains("Project settings failed"),
            "{what}: {trace}"
        );
    }
    let missing = service.send("PATCH", "/api/projects/nope", json!({ "ci": "ci" }));
    assert_eq!(missing.status, 404, "{}", missing.body);
    let after = service.call("GET", "/api/projects").json();
    assert_eq!(before, after, "a refused change changed the project");
}
