//! Approvals (0029): each agent's mode picks Claude Code's permission mode, and in every mode but
//! Full, Gathr's hook asks before a push or a write outside the engine's folder, admitting only
//! the engine's own token. Full's hook answers only the hold (0063).

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{
    TempDir, TestService, gathr, git, nth, project, push_as_engine, recording, sign_in_account,
};
use serde_json::{Value, json};

/// `signed_in` with the engine held after the message arrives, until `go` exists.
fn held_recording(dir: &Path) -> PathBuf {
    let held = dir.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    held
}

fn started_dir(started: &Value) -> PathBuf {
    PathBuf::from(started["cwd"].as_str().unwrap())
}

/// `push_as_engine`, failing at once if the push asks the person instead of going through.
fn push_unasked(
    service: &TestService,
    chat: &str,
    started: &Value,
    args: &[&str],
) -> (bool, String) {
    let pushing = push_as_engine(started, args);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pushing.is_finished() {
        let open = service
            .send("GET", &format!("/api/chats/{chat}"), json!(null))
            .json();
        let asked = open["approvals"].as_array().is_some_and(|a| !a.is_empty());
        assert!(!asked, "{args:?} asked: {}", open["approvals"]);
        assert!(Instant::now() < deadline, "{args:?} didn't end in 10 s");
        std::thread::sleep(Duration::from_millis(50));
    }
    pushing.join().unwrap()
}

/// The chat's one open approval, once there is one: its request and tool.
fn open_push(service: &TestService, chat: &str) -> (String, String) {
    let history = service.wait_for(chat, 10, "the push's approval", |h| {
        h["approvals"].as_array().is_some_and(|a| !a.is_empty())
    });
    let open = &history["approvals"][0];
    // A window opened after the ask shows the card as the live socket did, with its reason if any.
    assert!(open.get("why").is_some(), "{open}");
    let request = open["request_id"].as_str().unwrap().to_string();
    (request, open["tool"].as_str().unwrap().to_string())
}

#[test]
fn a_push_to_the_default_branch_waits_for_the_persons_answer_however_it_is_run() {
    let data = TempDir::new();
    let (go, env) = (data.0.join("go"), data.0.join("env.jsonl"));
    let (project_dir, _) = project(&data.0);
    let marker = data.0.join("own-hook-ran");
    let own = project_dir.join(".git").join("hooks").join("pre-push");
    let touch = marker.to_string_lossy().replace('\\', "/");
    std::fs::write(&own, format!("#!/bin/sh\ntouch '{touch}'\n")).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&own, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Rook", "account": account }),
    );
    let rook = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": rook, "role": "supervisor" }]);
    let chat = common::project_chat(&service, &project_dir, members);

    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Ship it", "agent": rook }));
    let reference = sent.json()["ref"].as_str().unwrap().to_string();
    let started = nth(&env, 0);
    let origin = data.0.join("origin.git");
    let has = |branch: &str| {
        let refs = git(&origin, &["for-each-ref", "--format=%(refname)"]);
        refs.lines().any(|r| r == format!("refs/heads/{branch}"))
    };
    let answer = |request: &str, allow: bool| {
        let to = format!("/api/chats/{chat}/approvals/{request}");
        let answered = service.send("POST", &to, json!({ "allow": allow }));
        assert_eq!(answered.status, 200, "{}", answered.body);
    };

    let alias = [
        "-c",
        "alias.publish=push",
        "publish",
        "origin",
        "HEAD:refs/heads/main",
    ];
    // Something to push: the worktree starts at the remote's default branch.
    git(
        &started_dir(&started),
        &["commit", "-q", "--allow-empty", "-m", "ship"],
    );
    let before = git(&origin, &["rev-parse", "refs/heads/main"]);
    let pushing = push_as_engine(&started, &alias);
    let (request, tool) = open_push(&service, &chat);
    assert_eq!(tool, "git push");
    answer(&request, false);
    let (pushed, said) = pushing.join().unwrap();
    let after = git(&origin, &["rev-parse", "refs/heads/main"]);
    assert!(
        !pushed && before == after,
        "a declined push went through: {said}"
    );
    assert!(
        !marker.exists(),
        "the project's own hook ran before the person allowed"
    );

    let pushing = push_as_engine(&started, &["push", "origin", "HEAD:refs/heads/main"]);
    let (request, _) = open_push(&service, &chat);
    answer(&request, true);
    let (pushed, said) = pushing.join().unwrap();
    let after = git(&origin, &["rev-parse", "refs/heads/main"]);
    assert!(
        pushed && before != after,
        "an allowed push didn't go through: {said}"
    );
    assert!(
        marker.exists(),
        "the project's own pre-push hook didn't run after Gathr's"
    );

    let mut stranger = started.clone();
    stranger["GATHR_AGENT_TOKEN"] = Value::Null;
    let (pushed, said) = push_as_engine(&stranger, &["push", "origin", "HEAD:refs/heads/no-token"])
        .join()
        .unwrap();
    assert!(
        !pushed && !has("no-token"),
        "a push without the engine's token went through: {said}"
    );
    let shared = common::git_command()
        .args(["config", "--local", "--get", "core.hooksPath"])
        .current_dir(&project_dir)
        .output()
        .unwrap();
    assert!(
        !shared.status.success(),
        "the project's own checkout got Gathr's hooks: {}",
        String::from_utf8_lossy(&shared.stdout)
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    assert!(
        ok && trace.to_string().contains("gathr.approvals.push"),
        "{trace}"
    );
    std::fs::write(&go, "").unwrap();
}

/// Lets the held engine go however the test ends, so a failure ends the run instead of holding it.
struct Release(PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

/// Owner's call (task 1421): an agent's push to a branch of its own goes through; one to the
/// remote's default branch, one that rewrites or deletes a branch, or one to anywhere but the
/// worktree's origin asks, each saying why.
#[test]
fn only_a_risky_push_asks() {
    let data = TempDir::new();
    let (go, env) = (data.0.join("go"), data.0.join("env.jsonl"));
    let _release = Release(go.clone());
    let (project_dir, _) = project(&data.0);
    // Someone else's branch, with a commit the agent's doesn't have: known locally once fetched.
    let other = data.0.join("other");
    git(&other, &["checkout", "-q", "-b", "room/theirs", "HEAD~1"]);
    git(&other, &["commit", "-q", "--allow-empty", "-m", "theirs"]);
    git(&other, &["push", "-q", "origin", "room/theirs"]);
    git(&project_dir, &["fetch", "-q", "origin"]);
    git(&data.0, &["init", "-q", "--bare", "elsewhere.git"]);
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Rook", "account": account }),
    );
    let rook = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": rook, "role": "supervisor" }]);
    let chat = common::project_chat(&service, &project_dir, members);
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Ship it", "agent": rook }));
    let reference = sent.json()["ref"].as_str().unwrap().to_string();
    let started = nth(&env, 0);
    git(
        &started_dir(&started),
        &["commit", "-q", "--allow-empty", "-m", "ship"],
    );
    let origin = data.0.join("origin.git");
    let has = |branch: &str| {
        let refs = git(&origin, &["for-each-ref", "--format=%(refname)"]);
        refs.lines().any(|r| r == format!("refs/heads/{branch}"))
    };

    let own = ["push", "origin", "HEAD:refs/heads/room/x"];
    let (pushed, said) = push_unasked(&service, &chat, &started, &own);
    assert!(
        pushed && has("room/x"),
        "a push to its own branch didn't go through: {said}"
    );

    let elsewhere = data.0.join("elsewhere.git");
    let elsewhere = elsewhere.to_string_lossy();
    let asks: [(&[&str], &str); 4] = [
        (
            &["push", "origin", "HEAD:refs/heads/main"],
            "default branch",
        ),
        (
            &["push", "origin", "+HEAD:refs/heads/room/theirs"],
            "rewrites",
        ),
        (&["push", "origin", ":refs/heads/room/x"], "deletes"),
        (
            &["push", &elsewhere, "HEAD:refs/heads/room/x"],
            "not the worktree's origin",
        ),
    ];
    for (args, why) in asks {
        let pushing = push_as_engine(&started, args);
        let history = service.wait_for(&chat, 10, "the push's approval", |h| {
            h["approvals"].as_array().is_some_and(|a| !a.is_empty())
        });
        let open = &history["approvals"][0];
        let said_why = open["why"].as_str().unwrap_or_default();
        assert!(said_why.contains(why), "{args:?} asked with {open}");
        let request = open["request_id"].as_str().unwrap();
        let to = format!("/api/chats/{chat}/approvals/{request}");
        let answered = service.send("POST", &to, json!({ "allow": false }));
        assert_eq!(answered.status, 200, "{}", answered.body);
        let (pushed, said) = pushing.join().unwrap();
        assert!(!pushed, "{args:?} went through when declined: {said}");
    }
    assert!(has("room/x"), "a declined deletion deleted");
    let theirs = git(
        &origin,
        &["log", "-1", "--format=%s", "refs/heads/room/theirs"],
    );
    assert_eq!(
        theirs.trim(),
        "theirs",
        "a declined force push rewrote the branch"
    );

    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    let trace = trace.to_string();
    assert!(
        ok && trace.contains("an ordinary push"),
        "the plain push left no event: {trace}"
    );
}

/// The live `approval` recording with its question made a Bash push, held once it's answered,
/// before the command ends.
fn allowed_push_recording(dir: &Path) -> PathBuf {
    let command = "git push origin HEAD:refs/heads/main";
    let mut rows = Vec::new();
    for row in std::fs::read_to_string(recording("approval"))
        .unwrap()
        .lines()
    {
        let mut row: Value = serde_json::from_str(row).unwrap();
        let mut frame: Value = serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
        let answered = row["dir"] == "out" && frame["type"] == "control_response";
        if frame["request"]["subtype"] == "can_use_tool" {
            frame["request"]["tool_name"] = json!("Bash");
            frame["request"]["input"] = json!({ "command": command });
            row["line"] = json!(frame.to_string());
        }
        rows.push(row.to_string());
        if answered {
            rows.push(json!({ "dir": "hold" }).to_string());
        }
    }
    let file = dir.join("allowed-push.jsonl");
    std::fs::write(&file, rows.join("\n")).unwrap();
    file
}

#[test]
fn a_push_the_person_allowed_as_a_command_asks_no_twice() {
    let data = TempDir::new();
    let (go, env) = (data.0.join("go"), data.0.join("env.jsonl"));
    let (project_dir, _) = project(&data.0);
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &allowed_push_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Rook", "account": account }),
    );
    let rook = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": rook, "role": "supervisor" }]);
    let chat = common::project_chat(&service, &project_dir, members);

    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Ship it", "agent": rook }));
    let (request, tool) = open_push(&service, &chat);
    assert_eq!(tool, "Bash");
    let to = format!("/api/chats/{chat}/approvals/{request}");
    let answered = service.send("POST", &to, json!({ "allow": true }));
    assert_eq!(answered.status, 200, "{}", answered.body);

    let started = nth(&env, 0);
    git(
        &started_dir(&started),
        &["commit", "-q", "--allow-empty", "-m", "ship"],
    );
    let push = ["push", "origin", "HEAD:refs/heads/main"];
    let (pushed, said) = push_unasked(&service, &chat, &started, &push);
    assert!(pushed, "the allowed push didn't go through: {said}");
    std::fs::write(&go, "").unwrap();
}

/// The live `approval` recording with its question a Write of `file`, held once it's answered.
fn write_recording(dir: &Path, file: &Path) -> PathBuf {
    let mut rows = Vec::new();
    for row in std::fs::read_to_string(recording("approval"))
        .unwrap()
        .lines()
    {
        let mut row: Value = serde_json::from_str(row).unwrap();
        let mut frame: Value = serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
        let answered = row["dir"] == "out" && frame["type"] == "control_response";
        if frame["request"]["subtype"] == "can_use_tool" {
            frame["request"]["input"] = json!({ "file_path": file, "content": "hi" });
            row["line"] = json!(frame.to_string());
        }
        rows.push(row.to_string());
        if answered {
            rows.push(json!({ "dir": "hold" }).to_string());
        }
    }
    let path = dir.join("write-outside.jsonl");
    std::fs::write(&path, rows.join("\n")).unwrap();
    path
}

/// Owner's pick: one Allow of a write outside the agent's folders covers the folder for that
/// agent in that chat; a Deny remembers nothing.
#[test]
fn one_allow_covers_the_checkout_for_the_rest_of_the_chat() {
    let (data, elsewhere) = (TempDir::new(), TempDir::new());
    let (go, env) = (data.0.join("go"), data.0.join("env.jsonl"));
    let _release = Release(go.clone());
    let repo = elsewhere.0.join("notes-mcp");
    std::fs::create_dir_all(repo.join(".git")).unwrap();
    let first = repo.join("server.ts");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &write_recording(&data.0, &first)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Rook", "account": account }),
    );
    let rook = made.json()["id"].as_str().unwrap().to_string();
    let port = service.info.port;
    let write = |token: &str, path: &Path| {
        let answer = common::hook(port, token, "Write", json!({ "file_path": path }));
        assert_eq!(answer.status, 200, "{}", answer.body);
        answer.json()["hookSpecificOutput"].clone()
    };
    let start = |n: usize| {
        let chat = service.new_chat();
        let path = format!("/api/chats/{chat}/messages");
        service.send("POST", &path, json!({ "text": "Write it", "agent": rook }));
        let token = nth(&env, n)["GATHR_AGENT_TOKEN"]
            .as_str()
            .unwrap()
            .to_string();
        (chat, token)
    };
    let answer = |chat: &str, allow: bool| {
        let (request, tool) = open_push(&service, chat);
        assert_eq!(tool, "Write");
        let to = format!("/api/chats/{chat}/approvals/{request}");
        let answered = service.send("POST", &to, json!({ "allow": allow }));
        assert_eq!(answered.status, 200, "{}", answered.body);
    };

    let (chat, token) = start(0);
    let asked = write(&token, &first);
    assert_eq!(asked["permissionDecision"], "ask", "{asked}");
    let top = gathr::folders::real(&repo);
    let said = format!(
        "Gathr asks because it writes outside the agent's folders. Allow covers {} in this chat.",
        top.display()
    );
    assert_eq!(asked["permissionDecisionReason"], said.as_str());
    answer(&chat, true);
    let again = repo.join("src").join("tools.ts");
    assert_eq!(write(&token, &again), Value::Null, "the same checkout");
    let other = elsewhere.0.join("other").join("a.md");
    assert_eq!(
        write(&token, &other)["permissionDecision"],
        "ask",
        "elsewhere"
    );
    let sql = "SELECT json_extract(attrs, '$.folder') AS folder FROM event_log
               WHERE name = 'gathr.approvals.covered'";
    let (ok, found) = gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{found}");
    assert_eq!(found["rows"][0]["folder"], top.display().to_string());
    std::fs::write(&go, "").unwrap();
    service.wait_for(&chat, 20, "the turn", |h| h["turns"][0]["status"] == "done");

    // The same agent in another chat is asked again, and a Deny remembers nothing.
    let (next, token) = start(1);
    assert_eq!(write(&token, &again)["permissionDecision"], "ask");
    answer(&next, false);
    assert_eq!(write(&token, &again)["permissionDecision"], "ask");
}
