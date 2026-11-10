//! Chats for a team (0013, 0028): members and roles, who a message goes to, the order replies keep,
//! and each agent's own worktree of the chat's project.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, git, next_live, project, recording, sign_in_account};
use serde_json::{Value, json};

/// A signed-in account's agent: its id.
fn agent(service: &TestService, data: &Path, account: &str, name: &str) -> String {
    sign_in_account(data, account);
    let added = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    added.json()["id"].as_str().unwrap().to_string()
}

fn agents(service: &TestService, data: &Path, names: &[&str]) -> Vec<String> {
    let account = service.add_account("Team");
    names
        .iter()
        .map(|name| agent(service, data, &account, name))
        .collect()
}

fn member(agent: &str, role: &str) -> Value {
    json!({ "agent": agent, "role": role })
}

/// A new chat with `members` (in a project on that folder): its id.
fn team_chat(service: &TestService, members: Value, project: Option<&Path>) -> String {
    if let Some(folder) = project {
        return common::project_chat(service, folder, members);
    }
    let body = json!({ "members": members });
    let made = service.send("POST", "/api/chats", body);
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

fn send(service: &TestService, chat: &str, body: Value) -> Value {
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body.clone());
    assert_eq!(sent.status, 200, "{body}: {}", sent.body);
    sent.json()
}

fn turn<'a>(history: &'a Value, id: &Value) -> &'a Value {
    let turns = history["turns"].as_array().unwrap();
    turns
        .iter()
        .find(|t| t["id"] == *id)
        .unwrap_or(&Value::Null)
}

fn ended(history: &Value, n: usize) -> bool {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.iter().filter(|t| t["ended"].is_string()).count() == n
}

#[test]
fn role_rules_are_refused_through_the_api() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let ids = agents(
        &service,
        &data.0,
        &["Orin", "Vale", "Pia", "Juno", "Cass", "Rook"],
    );
    let [a, b, c, d, e, f] = [0, 1, 2, 3, 4, 5].map(|i| ids[i].as_str());
    let refused = [
        (
            "six agents",
            json!({ "members": [member(a, "supervisor"), member(b, "member"),
            member(c, "member"), member(d, "member"), member(e, "member"), member(f, "member")] }),
        ),
        (
            "two agents and no Supervisor",
            json!({ "members": [member(a, "member"), member(b, "member")] }),
        ),
        (
            "two Supervisors",
            json!({ "members": [member(a, "supervisor"), member(b, "supervisor")] }),
        ),
        (
            "two Reviewers",
            json!({ "members": [member(a, "supervisor"), member(b, "reviewer"),
            member(c, "reviewer")] }),
        ),
        (
            "a Reviewer alone",
            json!({ "members": [member(a, "reviewer")] }),
        ),
        (
            "one agent twice",
            json!({ "members": [member(a, "supervisor"), member(a, "member")] }),
        ),
        (
            "no such agent",
            json!({ "members": [member(a, "supervisor"), member("nope", "member")] }),
        ),
        (
            "no such role",
            json!({ "members": [member(a, "supervisor"), member(b, "lead")] }),
        ),
        ("no members", json!({ "members": [] })),
    ];
    for (what, body) in refused {
        let answer = service.send("POST", "/api/chats", body);
        let error = answer.json();
        assert_eq!(answer.status, 400, "{what} wasn't refused: {}", answer.body);
        assert!(
            error["ref"].is_string() && error["why"].is_string(),
            "{what}: {error}"
        );
    }

    let team = json!([
        member(a, "supervisor"),
        member(b, "reviewer"),
        member(c, "member")
    ]);
    let chat = team_chat(&service, team, None);
    let path = format!("/api/chats/{chat}");
    let history = service.call("GET", &path).json();
    let roles: Vec<(&str, &str)> = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| (m["name"].as_str().unwrap(), m["role"].as_str().unwrap()))
        .collect();
    assert_eq!(
        roles,
        [
            ("Orin", "supervisor"),
            ("Vale", "reviewer"),
            ("Pia", "member")
        ]
    );

    let no_lead = json!({ "members": [member(b, "reviewer"), member(c, "member")] });
    let answer = service.send("PATCH", &path, no_lead);
    assert_eq!(
        answer.status, 400,
        "a team without a Supervisor: {}",
        answer.body
    );
    let moved = json!({ "members": [member(b, "supervisor"), member(c, "member")] });
    let answer = service.send("PATCH", &path, moved);
    assert_eq!(answer.status, 200, "{}", answer.body);
    let changes = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.members' AND error_type IS NULL";
    let (_, rows) = gathr(&data.0, &["diag", "query", changes]);
    assert_eq!(rows["rows"][0]["n"], 2, "each change is recorded: {rows}");

    let answer = service.call("DELETE", &format!("/api/agents/{b}"));
    assert_eq!(
        answer.status, 409,
        "a chat's agent was deleted: {}",
        answer.body
    );
    let answer = service.call("DELETE", &format!("/api/agents/{a}"));
    assert_eq!(answer.status, 200, "an agent no chat has: {}", answer.body);
}

#[test]
fn a_message_goes_to_the_supervisor_unless_it_names_a_member() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let ids = agents(&service, &data.0, &["Orin", "Pia", "Rook"]);
    let (orin, pia, rook) = (&ids[0], &ids[1], &ids[2]);
    let chat = team_chat(
        &service,
        json!([member(orin, "supervisor"), member(pia, "member")]),
        None,
    );

    let unnamed = send(&service, &chat, json!({ "text": "Who's free?" }));
    service.wait_for(&chat, 10, "the first turn", |h| ended(h, 1));
    let named = send(
        &service,
        &chat,
        json!({ "text": "@pia, look at this with @Orin" }),
    );
    let history = service.wait_for(&chat, 10, "the named turn", |h| ended(h, 2));
    assert_eq!(
        turn(&history, &unnamed["turn"])["agent"],
        orin.as_str(),
        "{history}"
    );
    assert_eq!(
        turn(&history, &named["turn"])["agent"],
        pia.as_str(),
        "{history}"
    );
    let authors: Vec<(&str, &str)> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| {
            (
                m["role"].as_str().unwrap(),
                m["agent"].as_str().unwrap_or("none"),
            )
        })
        .collect();
    assert_eq!(
        authors,
        [
            ("user", orin.as_str()),
            ("assistant", orin),
            ("user", pia),
            ("assistant", pia)
        ],
        "each message says its agent: {history}"
    );

    // Picking an agent outside a chat with no project adds it (tests/projects.rs has the rules).
    let outsider = json!({ "text": "Hello", "agent": rook });
    let answer = service.send("POST", &format!("/api/chats/{chat}/messages"), outsider);
    assert_eq!(answer.status, 200, "{}", answer.body);
    let history = service.wait_for(&chat, 10, "Rook's turn", |h| ended(h, 3));
    assert_eq!(history["members"][2]["agent"], rook.as_str(), "{history}");
}

/// `signed_in` with a second text block after `go` exists: the reply is half written until then.
fn two_blocks(dir: &Path) -> PathBuf {
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let block = lines
        .iter()
        .position(|l| l.contains(r#"\"type\":\"assistant\""#))
        .unwrap();
    let again = lines[block].replace("Hi!", "Again");
    let mut out: Vec<String> = lines[..=block].iter().map(|l| l.to_string()).collect();
    out.push(r#"{"dir":"hold"}"#.into());
    out.push(again);
    out.extend(lines[block + 1..].iter().map(|l| l.to_string()));
    let path = dir.join("two-blocks.jsonl");
    std::fs::write(&path, out.join("\n")).unwrap();
    path
}

#[test]
fn replies_keep_their_place_by_when_their_turn_started() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let replay = two_blocks(&data.0);
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_REPLAY", &replay), ("GATHR_REPLAY_HOLD", &go)],
    );
    let ids = agents(&service, &data.0, &["Orin", "Pia"]);
    let chat = team_chat(
        &service,
        json!([member(&ids[0], "supervisor"), member(&ids[1], "member")]),
        None,
    );
    let first_blocks = |n: usize| {
        move |h: &Value| {
            let messages = h["messages"].as_array().cloned().unwrap_or_default();
            messages.iter().filter(|m| m["body"] == "Hi!").count() == n
        }
    };

    let mut live = service.live();
    send(&service, &chat, json!({ "text": "Orin first" }));
    service.wait_for(&chat, 10, "Orin's first block", first_blocks(1));
    send(&service, &chat, json!({ "text": "@Pia second" }));
    service.wait_for(&chat, 10, "Pia's first block", first_blocks(2));
    std::fs::write(&go, "").unwrap();
    let history = service.wait_for(&chat, 10, "both turns", |h| ended(h, 2));

    let shown: Vec<String> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["body"].as_str().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        shown,
        ["Orin first", "Hi!", "Again", "@Pia second", "Hi!", "Again"],
        "replies interleaved: {history}"
    );
    // A window following live places each turn as a reload would, from the running update alone.
    let mut placed = Vec::new();
    while placed.len() < 2 {
        let update = next_live(&mut live);
        if update["type"] == "turn" && update["status"] == "running" {
            placed.push((update["turn"].clone(), update["position"].clone()));
        }
    }
    for (id, position) in placed {
        let kept = &turn(&history, &id)["position"];
        assert!(kept.is_i64(), "history has no place for {id}: {history}");
        assert_eq!(
            &position, kept,
            "turn {id} started live at {position}, history says {kept}"
        );
    }
}

/// The briefing the engine's last start was given, read while that engine still runs.
fn last_briefing(args: &Path) -> String {
    let starts = std::fs::read_to_string(args).unwrap();
    let last: Vec<String> = serde_json::from_str(starts.lines().last().unwrap()).unwrap();
    let at = last
        .iter()
        .position(|a| a == "--append-system-prompt-file")
        .unwrap();
    std::fs::read_to_string(&last[at + 1]).unwrap()
}

#[test]
fn a_new_supervisor_reaches_the_next_turns_briefing() {
    let data = TempDir::new();
    let args = data.0.join("engine-args.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
        ],
    );
    let ids = agents(&service, &data.0, &["Orin", "Pia"]);
    let chat = team_chat(
        &service,
        json!([member(&ids[0], "supervisor"), member(&ids[1], "member")]),
        None,
    );
    send(&service, &chat, json!({ "text": "Start" }));
    service.wait_for(&chat, 10, "Orin's first turn", |h| ended(h, 1));
    let before = last_briefing(&args);
    assert!(before.contains("as its Supervisor"), "{before}");

    let swapped = json!({ "members": [member(&ids[0], "member"), member(&ids[1], "supervisor")] });
    let answer = service.send("PATCH", &format!("/api/chats/{chat}"), swapped);
    assert_eq!(answer.status, 200, "{}", answer.body);
    send(&service, &chat, json!({ "text": "@Orin again" }));
    service.wait_for(&chat, 10, "Orin's turn after the swap", |h| ended(h, 2));
    let after = last_briefing(&args);
    assert!(
        !after.contains("as its Supervisor") && after.contains("Pia (Supervisor)"),
        "the engine kept the old roles: {after}"
    );
}

/// Each engine start's working folder and PATH, as the replay engine wrote them.
fn starts(env: &Path) -> Vec<Value> {
    let text = std::fs::read_to_string(env).unwrap_or_default();
    text.lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn same_folder(a: &str, b: &Path) -> bool {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    canon(Path::new(a)) == canon(b)
}

#[test]
fn each_agent_works_in_its_own_worktree_from_the_fetched_default_branch() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (project, newest) = project(&repos.0);
    let local = git(&project, &["rev-parse", "HEAD"]);
    let env = data.0.join("engine-env.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    // Two names that differ only in what a branch name can't hold (Vale's review of #49).
    let ids = agents(&service, &data.0, &["Build A", "Build-A"]);
    let team = json!([member(&ids[0], "supervisor"), member(&ids[1], "member")]);
    let chat = team_chat(&service, team, Some(&project));

    send(&service, &chat, json!({ "text": "Start" }));
    service.wait_for(&chat, 20, "Build A's turn", |h| ended(h, 1));
    send(&service, &chat, json!({ "text": "@Build-A go" }));
    let history = service.wait_for(&chat, 20, "Build-A's turn", |h| ended(h, 2));
    let failed: Vec<&Value> = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["status"] != "done")
        .collect();
    assert!(failed.is_empty(), "{failed:?}");

    let folders: Vec<String> = starts(&env)
        .iter()
        .map(|s| s["cwd"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(folders.len(), 2, "{folders:?}");
    assert_ne!(folders[0], folders[1], "two agents share a folder");
    let branches: Vec<String> = folders
        .iter()
        .map(|f| git(Path::new(f), &["branch", "--show-current"]))
        .collect();
    assert_ne!(branches[0], branches[1], "two agents share a branch");
    for folder in &folders {
        assert!(
            !same_folder(folder, &project),
            "an agent works in the project folder"
        );
        let base = git(Path::new(folder), &["rev-parse", "HEAD"]);
        assert_eq!(
            base, newest,
            "{folder} didn't start from the fetched default branch"
        );
    }
    assert_eq!(
        git(&project, &["rev-parse", "HEAD"]),
        local,
        "the project folder moved"
    );
    let shown: Vec<&str> = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["folder"].as_str().unwrap_or(""))
        .collect();
    assert!(
        shown
            .iter()
            .zip(&folders)
            .all(|(s, f)| same_folder(f, Path::new(s))),
        "the chat shows each agent's folder: {shown:?} {folders:?}"
    );
}

#[test]
fn a_worktree_that_cant_be_made_fails_the_turn_with_a_reference_that_finds_why() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (project, _) = project(&repos.0);
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let ids = agents(&service, &data.0, &["Orin"]);
    let chat = team_chat(&service, json!([member(&ids[0], "member")]), Some(&project));
    std::fs::remove_dir_all(&project).unwrap();

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Start" }),
    );
    let deadline = Instant::now() + Duration::from_secs(20);
    let failed = loop {
        let history = service.call("GET", &format!("/api/chats/{chat}")).json();
        if let Some(t) = history["turns"]
            .as_array()
            .and_then(|t| t.first())
            .filter(|t| t["status"] == "failed")
        {
            break t.clone();
        }
        assert!(
            Instant::now() < deadline,
            "the turn didn't fail: {} {history}",
            sent.body
        );
        std::thread::sleep(Duration::from_millis(50));
    };
    assert!(
        failed["why"].as_str().unwrap().contains("worktree"),
        "{failed}"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", failed["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let events = trace.to_string();
    assert!(
        events.contains("gathr.worktree.add"),
        "the trace doesn't show the worktree: {trace}"
    );
}

/// macOS: an app started from the Dock gets a short PATH, so the engine gets the login shell's.
#[cfg(target_os = "macos")]
#[test]
fn an_engine_gets_the_login_shells_path() {
    use std::os::unix::fs::PermissionsExt;
    let data = TempDir::new();
    let shell = data.0.join("shell.sh");
    let script = "#!/bin/sh\nPATH=\"/gathr-test-tools/bin:$PATH\"\nexport PATH\nshift\nexec /bin/sh -c \"$1\"\n";
    std::fs::write(&shell, script).unwrap();
    std::fs::set_permissions(&shell, std::fs::Permissions::from_mode(0o755)).unwrap();
    let env = data.0.join("engine-env.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ENV", &env),
            ("SHELL", &shell),
        ],
    );
    let account = service.add_account("Team");
    let orin = agent(&service, &data.0, &account, "Orin");
    let chat = team_chat(&service, json!([member(&orin, "member")]), None);
    send(&service, &chat, json!({ "text": "Start" }));
    service.wait_for(&chat, 10, "the turn", |h| ended(h, 1));
    let path = starts(&env)[0]["PATH"].as_str().unwrap().to_string();
    assert!(
        path.split(':').any(|p| p == "/gathr-test-tools/bin"),
        "{path}"
    );
}
