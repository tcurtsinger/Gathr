//! Deleting a project or an agent through a real service (0069): a project goes with its chats and
//! every folder Gathr made for it but Shared, never the person's checkout; an agent goes wherever it
//! is, its open tasks cancelled, its lead passed on, and what it was the only agent of with it; its
//! old messages still name it; the confirm's counts are what the delete then does.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, first_token, gathr, git, held_recording, text, until,
};
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

/// The agent called `name` in the list: its id.
fn id_of(service: &TestService, name: &str) -> String {
    let listed = agents(service);
    let found = listed["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == name);
    let found = found.unwrap_or_else(|| panic!("no agent {name}: {listed}"));
    found["id"].as_str().unwrap().to_string()
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

/// What `path`'s delete would do, as the confirm reads it.
fn counts(service: &TestService, path: &str) -> Value {
    let counted = service.call("GET", &format!("{path}/delete"));
    assert_eq!(counted.status, 200, "{}", counted.body);
    counted.json()
}

/// The tasks of `chat` that `owner` does.
fn tasks_of(service: &TestService, chat: &str, owner: &str) -> Vec<Value> {
    let tasks = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let all = tasks["tasks"].as_array().unwrap().iter();
    all.filter(|t| t["owner"] == owner).cloned().collect()
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

/// Holds `folder` so the disk refuses to remove it until dropped: on Windows a file in it open with
/// no sharing, elsewhere the folder read-only, so nothing in it can go.
struct Held {
    #[cfg(windows)]
    _file: std::fs::File,
    #[cfg(unix)]
    folder: PathBuf,
}

impl Held {
    fn new(folder: &Path) -> Held {
        let file = folder.join("held.bin");
        std::fs::write(&file, "held").unwrap();
        Held::hold(folder, &file)
    }

    #[cfg(windows)]
    fn hold(_folder: &Path, file: &Path) -> Held {
        use std::os::windows::fs::OpenOptionsExt;
        let open = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(file);
        Held {
            _file: open.unwrap(),
        }
    }

    #[cfg(unix)]
    fn hold(folder: &Path, _file: &Path) -> Held {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o555)).unwrap();
        Held {
            folder: folder.to_path_buf(),
        }
    }
}

#[cfg(unix)]
impl Drop for Held {
    fn drop(&mut self) {
        use std::os::unix::fs::PermissionsExt;
        let open = std::fs::Permissions::from_mode(0o755);
        let _ = std::fs::set_permissions(&self.folder, open);
    }
}

#[test]
fn a_folder_that_cant_be_removed_is_named_with_its_reference_and_goes_at_a_later_sweep() {
    let data = TempDir::new();
    let service = TestService::start_with(&data.0, &[("GATHR_WAIT_EVERY_MS", Path::new("100"))]);
    let account = service.add_account("Work");
    let kai = agent(&service, &account, "Kai");
    let (folder, _) = common::project(&data.0.join("repo"));
    let basket = new_project(
        &service,
        "Basket",
        &folder,
        json!([member(&kai, "supervisor")]),
    );
    // Reading its caches names its folder in the agent folder, as a checks run does.
    let size = service.call("GET", &format!("/api/projects/{basket}/caches"));
    assert_eq!(size.status, 200, "{}", size.body);
    let caches = real(&beside(&data.0)).join("Basket").join("caches");
    std::fs::create_dir_all(&caches).unwrap();
    let held = Held::new(&caches);

    let deleted = delete(&service, &format!("/api/projects/{basket}"));
    assert!(
        project_of(&service, &basket).is_none(),
        "the project is listed"
    );
    let left = deleted["left"].as_array().unwrap();
    assert_eq!(left.len(), 1, "{deleted}");
    let why = left[0].as_str().unwrap();
    assert!(why.contains(&caches.display().to_string()), "{why}");
    let reference = why
        .rsplit_once("(ref ")
        .and_then(|(_, r)| r.strip_suffix(')'));
    let reference = reference.unwrap_or_else(|| panic!("no reference: {why}"));
    let traced = || {
        let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
        assert!(ok, "{trace}");
        trace.to_string()
    };
    assert!(traced().contains("gathr.cleanup.skip"), "{}", traced());
    drop(held);
    until(20, "the caches removed at a later sweep", || {
        (!caches.exists()).then_some(())
    });
    until(10, "the removal on the same reference", || {
        traced().contains("gathr.cleanup.remove").then_some(())
    });
}

/// Ann's world, on `common::team` (Lead leads the plain team chat, Ann is its member): Larder (Lead
/// leads, Ann a member), Menus (Ann leads, Lead a member) and Pantry (Ann alone, two chats), each
/// with a chat; two tasks for Ann in the team chat; and Ann's reply in Larder's chat.
/// Fields drop in order: the service, which keeps a failed test's events and data folder, goes
/// before the folder does.
struct Room {
    service: TestService,
    ann: String,
    team: String,
    larder: String,
    larder_chat: String,
    menus: String,
    menus_chat: String,
    pantry: String,
    pantry_chats: [String; 2],
    _data: TempDir,
}

fn room() -> Room {
    let data = TempDir::new();
    let held = held_recording(&data.0, "signed_in");
    let (service, team, lead) = common::team(&data.0, &held, &[]);
    let ann = id_of(&service, "Ann");
    let (folder, _) = common::project(&data.0.join("repo"));
    let (led, joined) = (member(&lead, "supervisor"), member(&ann, "member"));
    let larder = new_project(&service, "Larder", &folder, json!([led, joined]));
    let (led, joined) = (member(&ann, "supervisor"), member(&lead, "member"));
    let menus = new_project(&service, "Menus", &folder, json!([led, joined]));
    let pantry = new_project(
        &service,
        "Pantry",
        &folder,
        json!([member(&ann, "supervisor")]),
    );
    let (larder_chat, menus_chat) = (chat_in(&service, &larder), chat_in(&service, &menus));
    let pantry_chats = [chat_in(&service, &pantry), chat_in(&service, &pantry)];

    // The lead gives Ann two tasks while its first turn holds.
    send(&service, &team, json!({ "text": "Plan it" }));
    let (port, lead_token) = (service.info.port, first_token(&data.0));
    common::fix_it(port, &lead_token);
    let notes = common::briefed(json!({ "to": "Ann", "what": "Notes" }));
    let first = text(&call_tool(port, &lead_token, "give_task", notes));
    assert!(first.starts_with("Task #1 for Ann"), "{first}");
    let after = common::briefed(json!({ "to": "Ann", "what": "Review notes", "after": [1] }));
    text(&call_tool(port, &lead_token, "give_task", after));
    send(
        &service,
        &larder_chat,
        json!({ "text": "Hi", "agent": ann }),
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&larder_chat, 30, "Ann's reply in Larder", |h| {
        let mut turns = h["turns"].as_array().unwrap().iter();
        turns.any(|t| t["agent"] == ann.as_str() && t["status"] == "done")
    });
    Room {
        service,
        ann,
        team,
        larder,
        larder_chat,
        menus,
        menus_chat,
        pantry,
        pantry_chats,
        _data: data,
    }
}

#[test]
fn a_deleted_agent_leaves_every_team_its_open_tasks_are_cancelled_and_its_words_stay() {
    let r = room();
    delete(&r.service, &format!("/api/agents/{}", r.ann));

    for project in [&r.larder, &r.menus] {
        assert_eq!(
            project_seats(&r.service, project),
            ["Lead supervisor"],
            "{project}"
        );
    }
    for chat in [&r.team, &r.larder_chat, &r.menus_chat] {
        assert_eq!(chat_seats(&r.service, chat), ["Lead supervisor"], "{chat}");
    }
    let tasks = tasks_of(&r.service, &r.team, &r.ann);
    assert_eq!(tasks.len(), 2, "{tasks:?}");
    for task in &tasks {
        let ended = (&task["state"], &task["reason"]);
        assert_eq!(
            ended,
            (&json!("cancelled"), &json!("its agent was deleted")),
            "{task}"
        );
    }
    // Ann's reply stays in Larder's chat, by Ann, and the agents' list still names Ann for it.
    let history = r
        .service
        .call("GET", &format!("/api/chats/{}", r.larder_chat))
        .json();
    let mut messages = history["messages"].as_array().unwrap().iter();
    let by_ann = messages.any(|m| m["role"] == "assistant" && m["agent"] == r.ann.as_str());
    assert!(by_ann, "{history}");
    assert_eq!(
        agents(&r.service)["removed"],
        json!([{ "id": r.ann, "name": "Ann" }])
    );
}

/// The lead gave Ann's tasks; once it is deleted, they settle with no turn for it: no failed turn
/// shows in the chat, and the hear-back that went nowhere is recorded.
#[test]
fn a_task_a_deleted_agent_gave_settles_with_no_turn_for_it() {
    let r = room();
    let lead = id_of(&r.service, "Lead");
    delete(&r.service, &format!("/api/agents/{lead}"));
    assert_eq!(chat_seats(&r.service, &r.team), ["Ann supervisor"]);
    let cancelled = r.service.send("POST", "/api/tasks/1/cancel", json!({}));
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);

    // Ann's turn on #1 may end after the delete and block it: that hear-back goes nowhere too, so
    // the cancel's is the newest, not the only one.
    let sql = "SELECT (SELECT json_extract(attrs, '$.why') FROM event_log WHERE name = 'gathr.task.hear_back'
                 ORDER BY ts DESC, id DESC LIMIT 1) AS why,
               (SELECT count(*) FROM tasks WHERE number IN (1, 2) AND heard = 1 AND heard_as LIKE 'cancelled%') AS heard";
    until(10, "the hear-back with nobody to hear it", || {
        let found = &common::rows(&r._data.0, sql)[0];
        (found["why"] == "its giver was deleted" && found["heard"] == 2).then_some(())
    });
    let history = r
        .service
        .call("GET", &format!("/api/chats/{}", r.team))
        .json();
    let turns = history["turns"].as_array().unwrap().iter();
    let failed: Vec<&Value> = turns.filter(|t| t["status"] == "failed").collect();
    assert!(failed.is_empty(), "{failed:?}");
    let left = history["turns"].as_array().unwrap().iter();
    let queued = left.filter(|t| t["agent"] == lead.as_str() && t["status"] == "queued");
    assert_eq!(queued.count(), 0, "{history}");
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
        member(&bo, "reviewer")
    ]);
    let larder = new_project(&service, "Larder", &folder, team);
    let in_larder = chat_in(&service, &larder);
    let pair = json!({ "members": [member(&mira, "supervisor"), member(&bo, "member")] });
    let plain = new_chat(&service, pair);

    delete(&service, &format!("/api/agents/{mira}"));
    let after = ["Ann supervisor", "Bo reviewer"];
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
    assert_eq!(listed["removed"], json!([{ "id": old, "name": "Ann" }]));
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

#[test]
fn the_counts_for_deleting_an_agent_are_what_its_delete_does() {
    let r = room();
    let path = format!("/api/agents/{}", r.ann);
    // Pantry and its two chats go; two tasks are cancelled; Lead leads Menus and its chat.
    let expected = json!({ "chats": 2, "tasks": 2, "projects": 1, "leads": ["Lead"] });
    assert_eq!(counts(&r.service, &path), expected);

    let deleted = delete(&r.service, &path);
    for field in ["chats", "tasks", "projects", "leads"] {
        assert_eq!(deleted[field], expected[field], "{field}: {deleted}");
    }
    assert!(
        project_of(&r.service, &r.pantry).is_none(),
        "Pantry is listed"
    );
    for chat in &r.pantry_chats {
        assert!(gone(&r.service, chat), "{chat} is still there");
    }
    let tasks = tasks_of(&r.service, &r.team, &r.ann);
    let cancelled = tasks.iter().filter(|t| t["state"] == "cancelled");
    assert_eq!(cancelled.count(), 2, "{tasks:?}");
    assert_eq!(project_seats(&r.service, &r.menus), ["Lead supervisor"]);
    assert_eq!(chat_seats(&r.service, &r.menus_chat), ["Lead supervisor"]);
}

#[test]
fn the_counts_for_deleting_a_project_are_what_its_delete_does() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let kai = agent(&service, &account, "Kai");
    let (folder, _) = common::project(&data.0.join("repo"));
    let larder = new_project(
        &service,
        "Larder",
        &folder,
        json!([member(&kai, "supervisor")]),
    );
    let chats: Vec<String> = (0..4).map(|_| chat_in(&service, &larder)).collect();
    let archive = service.call("POST", &format!("/api/chats/{}/archive", chats[3]));
    assert_eq!(archive.status, 200, "{}", archive.body);
    let path = format!("/api/projects/{larder}");
    // "4 chats will be deleted. Files are kept.": the archived one counts.
    let expected = json!({ "chats": 4, "tasks": 0, "projects": 1, "leads": [] });
    assert_eq!(counts(&service, &path), expected);

    let deleted = delete(&service, &path);
    for field in ["chats", "tasks", "projects", "leads"] {
        assert_eq!(deleted[field], expected[field], "{field}: {deleted}");
    }
    for chat in &chats {
        assert!(gone(&service, chat), "{chat} is still there");
    }
    assert!(
        project_of(&service, &larder).is_none(),
        "the project is listed"
    );
}
