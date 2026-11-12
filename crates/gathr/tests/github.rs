//! Gathr's GitHub tools for agents (0046, G185): `pr`, `ci_failures` and `update_pr`, made host-side
//! with Gathr's own `gh`, here the stand-in (`examples/fake_gh.rs`), answering from a file.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, held_recording, nth, project, text};
use serde_json::{Value, json};

const HEAD: &str = "5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a5a";
const NEW: &str = "6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b6b";

/// Lets the held engines go when the test ends.
struct Release(PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "go");
    }
}

struct Room {
    _release: Release,
    service: TestService,
    answers: PathBuf,
    lead: String,
    vale: String,
}

/// GitHub's view of PR #5, as `gh pr view` answers it.
fn view(head: &str, base: &str, state: &str, checks: Value) -> String {
    json!({ "number": 5, "title": "Add notes", "state": state, "headRefOid": head,
        "headRefName": "room/notes", "baseRefName": base, "mergeable": "MERGEABLE",
        "reviewDecision": "", "statusCheckRollup": checks })
    .to_string()
}

/// A project chat with Lead (Supervisor) and Vale (Reviewer); Vale has the review of #5. Each held
/// in a turn, with its engine's token.
fn room(data: &Path, repos: &Path, gh: Value) -> Room {
    let (folder, _) = project(repos);
    let answers = data.join("gh.json");
    std::fs::write(&answers, gh.to_string()).unwrap();
    let service = TestService::with_engine(
        data,
        &[
            ("GATHR_REPLAY", &held_recording(data)),
            ("GATHR_REPLAY_HOLD", &data.join("go")),
            ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
            ("GATHR_GH_PATH", &common::fake_gh()),
            ("GATHR_FAKE_GH", &answers),
            ("GATHR_GITHUB_UPDATE_WAIT_MS", Path::new("10000")),
        ],
    );
    let account = service.add_account("Team");
    common::sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, vale) = (agent("Lead"), agent("Vale"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": vale, "role": "reviewer" }]);
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
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Ship it", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let env = data.join("env.jsonl");
    let token = |n: usize| {
        nth(&env, n)["GATHR_AGENT_TOKEN"]
            .as_str()
            .unwrap()
            .to_string()
    };
    let lead = token(0);
    let review = json!({ "to": "Vale", "what": "Review #5", "kind": "review", "pr": 5 });
    let given = text(&call_tool(service.info.port, &lead, "give_task", review));
    assert!(given.starts_with("Task #"), "{given}");
    Room {
        _release: Release(data.join("go")),
        service,
        answers,
        lead,
        vale: token(1),
    }
}

impl Room {
    fn call(&self, token: &str, tool: &str, arguments: Value) -> (bool, String) {
        let answer = call_tool(self.service.info.port, token, tool, arguments);
        (answer["isError"] != true, text(&answer))
    }

    /// Every `gh` call so far, each its arguments.
    fn calls(&self) -> Vec<Vec<String>> {
        let calls =
            std::fs::read_to_string(self.answers.with_extension("json.calls")).unwrap_or_default();
        calls
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}

fn refused(said: &(bool, String), why: &str) {
    assert!(!said.0, "it went through: {}", said.1);
    assert!(said.1.contains(why), "{}", said.1);
    assert!(said.1.contains("(ref "), "no reference: {}", said.1);
}

fn checks() -> Value {
    json!([
        { "name": "Rust (Windows)", "status": "COMPLETED", "conclusion": "FAILURE",
          "detailsUrl": "https://github.com/o/r/actions/runs/77/job/88" },
        { "name": "UI (macOS)", "status": "COMPLETED", "conclusion": "SUCCESS",
          "detailsUrl": "https://github.com/o/r/actions/runs/77/job/89" },
        { "name": "Rust (macOS)", "status": "IN_PROGRESS", "conclusion": "" }
    ])
}

#[test]
fn an_agent_reads_a_pr_and_why_its_ci_failed_bounded_and_with_secrets_hidden() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let log: String = (1..=100).map(|n| format!("step line {n}\n")).collect();
    let log = format!("{log}error: token ghp_madeup0123456789abcdef0123456789 refused\n");
    let notes = json!([{ "path": "src/lib.rs", "start_line": 12, "message": "expected `;`" }]);
    let gh = json!([
        { "args": ["pr", "view", "5"], "out": view(HEAD, "main", "OPEN", checks()) },
        { "args": ["run", "view", "--job", "88", "--log-failed"], "out": log },
        { "args": ["api", "repos/{owner}/{repo}/check-runs/88/annotations"], "out": notes.to_string() }
    ]);
    let room = room(&data.0, &repos.0, gh);

    let (ok, read) = room.call(&room.lead, "pr", json!({ "pr": 5 }));
    assert!(ok, "{read}");
    assert!(
        read.contains("head 5a5a5a5 (room/notes) into main"),
        "{read}"
    );
    assert!(read.contains("- Rust (Windows): failure"), "{read}");
    assert!(read.contains("- Rust (macOS): in_progress"), "{read}");

    let (ok, failed) = room.call(&room.vale, "ci_failures", json!({ "pr": 5 }));
    assert!(ok, "{failed}");
    assert!(failed.contains("## Rust (Windows) (failure)"), "{failed}");
    assert!(
        !failed.contains("UI (macOS)"),
        "a passing check was read: {failed}"
    );
    assert!(failed.contains("(the last 40 of 101 lines)"), "{failed}");
    assert!(
        !failed.contains("step line 61\n"),
        "more than its last lines: {failed}"
    );
    assert!(!failed.contains("ghp_madeup"), "a key was shown: {failed}");
    assert!(
        failed.contains("annotation src/lib.rs:12: expected `;`"),
        "{failed}"
    );
    // Fixed arguments, in the project's folder: no command an agent wrote reaches `gh`.
    for call in room.calls() {
        assert!(["pr", "run", "api"].contains(&call[0].as_str()), "{call:?}");
    }
}

#[test]
fn only_the_reviewer_updates_a_pr_and_never_one_that_moved_closed_or_was_retargeted() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let gh = json!([
        // Read once for the moved head, once before the update, then the update's first look sees it.
        { "args": ["pr", "view", "5"], "outs": [view(HEAD, "main", "OPEN", json!([])), view(HEAD, "main", "OPEN", json!([])),
            view(NEW, "main", "OPEN", json!([]))] },
        { "args": ["repo", "view"], "out": r#"{"defaultBranchRef":{"name":"main"}}"# },
        { "args": ["api", "-X", "PUT"], "out": r#"{"message":"Updating pull request branch."}"# }
    ]);
    let room = room(&data.0, &repos.0, gh);
    let update = |head: &str| json!({ "pr": 5, "head": head });

    refused(
        &room.call(&room.lead, "update_pr", update(HEAD)),
        "only #5's reviewer",
    );
    refused(
        &room.call(&room.vale, "update_pr", update("1234567")),
        "not 1234567",
    );
    let (ok, said) = room.call(&room.vale, "update_pr", update(&HEAD[..7]));
    assert!(ok, "{said}");
    assert_eq!(said, "Updated #5 with main: head 5a5a5a5 is now 6b6b6b6.");
    let put: Vec<Vec<String>> = room.calls().into_iter().filter(|c| c[0] == "api").collect();
    assert_eq!(put.len(), 1, "{put:?}");
    assert!(
        put[0].contains(&format!("expected_head_sha={HEAD}")),
        "{put:?}"
    );
    let sql =
        "SELECT json_extract(attrs, '$.requested_head') r, json_extract(attrs, '$.returned_head') n
               FROM event_log WHERE name = 'gathr.github.update'";
    let (ok, rows) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"][0]["r"], HEAD, "{rows}");
    assert_eq!(rows["rows"][0]["n"], NEW, "{rows}");

    std::fs::write(
        &room.answers,
        json!([
            { "args": ["pr", "view", "5"], "out": view(NEW, "release", "OPEN", json!([])) },
            { "args": ["repo", "view"], "out": r#"{"defaultBranchRef":{"name":"main"}}"# }
        ])
        .to_string(),
    )
    .unwrap();
    refused(
        &room.call(&room.vale, "update_pr", update(NEW)),
        "not the default branch main",
    );
    std::fs::write(
        &room.answers,
        json!([{ "args": ["pr", "view", "5"], "out": view(NEW, "main", "CLOSED", json!([])) }])
            .to_string(),
    )
    .unwrap();
    refused(
        &room.call(&room.vale, "update_pr", update(NEW)),
        "#5 is closed",
    );
}

/// GitHub sign-in missing on the computer is told apart from an account that can't reach the repo.
#[test]
fn a_failure_says_whether_gathr_has_no_github_sign_in_or_no_reach() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let signed_out = json!([{ "args": ["pr"], "code": 1,
        "err": "To get started with GitHub CLI, please run:  gh auth login" }]);
    let room = room(&data.0, &repos.0, signed_out);
    refused(
        &room.call(&room.vale, "pr", json!({ "pr": 5 })),
        "GitHub sign-in isn't available to Gathr",
    );
    let no_reach = json!([{ "args": ["pr"], "code": 1,
        "err": "GraphQL: Could not resolve to a Repository with the name 'o/r'." }]);
    std::fs::write(&room.answers, no_reach.to_string()).unwrap();
    refused(
        &room.call(&room.vale, "pr", json!({ "pr": 5 })),
        "this GitHub account can't reach the project's repository",
    );
}

#[test]
fn a_chat_outside_a_project_has_no_repository_to_ask_about() {
    let data = TempDir::new();
    let answers = data.0.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let (service, chat, lead) = common::team(
        &data.0,
        &held_recording(&data.0),
        &[
            ("GATHR_GH_PATH", &common::fake_gh()),
            ("GATHR_FAKE_GH", &answers),
        ],
    );
    let _release = Release(data.0.join("go"));
    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Look", "agent": lead }));
    let token = common::first_token(&data.0);
    let answer = call_tool(service.info.port, &token, "pr", json!({ "pr": 5 }));
    let said = (answer["isError"] != true, text(&answer));
    refused(&said, "isn't in a project");
    assert!(
        !answers.with_extension("json.calls").exists(),
        "gh was asked"
    );
}

/// Live, read-only: `pr` and `ci_failures` on a real PR of this repository, through the real `gh` and
/// its sign-in on this computer. Nothing is changed. Run with
/// `GATHR_LIVE_PR=<number> cargo test -p gathr --test github -- --ignored`.
#[test]
#[ignore = "reads GitHub through this computer's gh sign-in"]
fn live_reads_of_a_real_pr() {
    let pr: i64 = std::env::var("GATHR_LIVE_PR")
        .expect("GATHR_LIVE_PR")
        .parse()
        .unwrap();
    let data = TempDir::new();
    let checkout = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let checkout = std::fs::canonicalize(checkout).unwrap();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &data.0.join("go")),
            ("GATHR_REPLAY_ENV", &data.0.join("env.jsonl")),
        ],
    );
    let _release = Release(data.0.join("go"));
    let account = service.add_account("Team");
    common::sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Lead", "account": account }),
    );
    let lead = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": lead, "role": "supervisor" }]);
    let body = json!({ "name": "Gathr", "folder": checkout, "members": members });
    let made = service.send("POST", "/api/projects", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let project = made.json()["id"].as_str().unwrap().to_string();
    let chat = service
        .send("POST", "/api/chats", json!({ "project": project }))
        .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Look", "agent": lead }));
    let token = nth(&data.0.join("env.jsonl"), 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    for tool in ["pr", "ci_failures"] {
        let answer = call_tool(service.info.port, &token, tool, json!({ "pr": pr }));
        let said = text(&answer);
        println!("--- {tool} ---\n{said}");
        assert_ne!(answer["isError"], true, "{said}");
    }
}
