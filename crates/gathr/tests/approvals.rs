//! Approvals (0029): each agent's mode picks Claude Code's permission mode, and in every mode but
//! Full, Gathr's hook asks before a push or a write outside the engine's folder, admitting only
//! the engine's own token.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, git, project, recording, sign_in_account};
use gathr::http;
use serde_json::{Value, json};

/// `signed_in` with the engine held after the message arrives, until `go` exists.
fn held_recording(dir: &Path) -> PathBuf {
    let held = dir.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    held
}

/// The `n`th line of a file the replay engine appends to, once it's there and whole.
fn nth(file: &Path, n: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        let whole = text.rsplit_once('\n').map_or("", |(lines, _)| lines);
        if let Some(line) = whole.lines().nth(n) {
            return serde_json::from_str(line).unwrap();
        }
        assert!(
            Instant::now() < deadline,
            "no line {n} in {}",
            file.display()
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn after<'a>(args: &'a [Value], switch: &str) -> &'a str {
    let at = args.iter().position(|a| a == switch);
    let at = at.unwrap_or_else(|| panic!("no {switch} in {args:?}"));
    args[at + 1].as_str().unwrap()
}

fn mode(args: &[Value]) -> &str {
    let found = args.iter().filter_map(Value::as_str);
    let mut modes = found.filter_map(|a| a.strip_prefix("--permission-mode="));
    modes
        .next()
        .unwrap_or_else(|| panic!("no permission mode in {args:?}"))
}

fn hook(port: u16, token: &str, tool: &str, input: Value) -> http::Response {
    let body = json!({ "hook_event_name": "PreToolUse", "tool_name": tool, "tool_input": input });
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
    ];
    http::post(port, "/hook", &headers, &body.to_string()).unwrap()
}

#[test]
fn gathr_asks_before_a_push_or_a_write_outside_and_full_turns_its_hook_off() {
    let data = TempDir::new();
    let (go, args, env) = (
        data.0.join("go"),
        data.0.join("args.jsonl"),
        data.0.join("env.jsonl"),
    );
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Work");
    sign_in_account(&data.0, &account);
    let orin = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Orin", "account": account }),
    );
    assert_eq!(
        orin.json()["approvals"],
        "auto",
        "a new agent isn't on auto: {}",
        orin.body
    );
    let orin = orin.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": orin }));
    let reference = sent.json()["ref"].as_str().unwrap().to_string();

    let first: Vec<Value> = serde_json::from_value(nth(&args, 0)).unwrap();
    assert_eq!(mode(&first), "auto");
    let settings: Value = serde_json::from_str(after(&first, "--settings")).unwrap();
    let given = &settings["hooks"]["PreToolUse"][0]["hooks"][0];
    let port = service.info.port;
    assert_eq!(
        given["url"],
        format!("http://127.0.0.1:{port}/hook").as_str(),
        "{settings}"
    );
    assert_eq!(
        given["headers"]["Authorization"], "Bearer $GATHR_AGENT_TOKEN",
        "{settings}"
    );
    assert_eq!(
        given["onFailure"], "block",
        "a hook that fails would let a push through"
    );

    let started = nth(&env, 0);
    let (token, cwd) = (
        started["GATHR_AGENT_TOKEN"].as_str().unwrap(),
        started["cwd"].as_str(),
    );
    let cwd = PathBuf::from(cwd.unwrap());
    let inside = hook(
        port,
        token,
        "Write",
        json!({ "file_path": cwd.join("a.txt") }),
    );
    assert_eq!(inside.status, 200, "{}", inside.body);
    assert_eq!(inside.json(), json!({}), "a write inside asked");
    let outside = cwd.parent().unwrap().join("elsewhere.txt");
    let asked = hook(port, token, "Write", json!({ "file_path": outside })).json();
    assert_eq!(
        asked["hookSpecificOutput"]["permissionDecision"], "ask",
        "{asked}"
    );
    let pushed = hook(
        port,
        token,
        "Bash",
        json!({ "command": "git push origin main" }),
    )
    .json();
    assert_eq!(
        pushed["hookSpecificOutput"]["permissionDecision"], "ask",
        "{pushed}"
    );
    let refused = hook(
        port,
        &service.info.token,
        "Bash",
        json!({ "command": "git push" }),
    );
    assert_eq!(
        refused.status, 401,
        "the service's own token reached the hook"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    assert!(
        ok && trace.to_string().contains("gathr.approvals.ask"),
        "{trace}"
    );

    let full = service.send(
        "PATCH",
        &format!("/api/agents/{orin}"),
        json!({ "approvals": "full" }),
    );
    assert_eq!(full.json()["approvals"], "full", "{}", full.body);
    std::fs::write(&go, "").unwrap();
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    service.send("POST", &path, json!({ "text": "Again", "agent": orin }));
    let next: Vec<Value> = serde_json::from_value(nth(&args, 1)).unwrap();
    assert_eq!(mode(&next), "bypassPermissions");
    let settings: Value = serde_json::from_str(after(&next, "--settings")).unwrap();
    assert!(
        settings.get("hooks").is_none(),
        "Full still asks: {settings}"
    );
    let unknown = service.send(
        "PATCH",
        &format!("/api/agents/{orin}"),
        json!({ "approvals": "never" }),
    );
    assert_ne!(unknown.status, 200, "an unknown mode was saved");
}

/// A `git push` run in the worktree as the agent's engine would: with its environment, through
/// `args` (an alias, say), on a thread: whether git said it pushed, and what it said.
fn push_as_engine(started: &Value, args: &[&str]) -> std::thread::JoinHandle<(bool, String)> {
    let mut command = std::process::Command::new("git");
    command
        .args(args)
        .current_dir(started["cwd"].as_str().unwrap());
    for var in ["GATHR_AGENT_TOKEN", "GATHR_PUSH_URL"] {
        match started[var].as_str() {
            Some(value) => command.env(var, value),
            None => command.env_remove(var),
        };
    }
    std::thread::spawn(move || {
        let out = command.output().unwrap();
        (
            out.status.success(),
            String::from_utf8_lossy(&out.stderr).into(),
        )
    })
}

/// The chat's one open approval, once there is one: its request and tool.
fn open_push(service: &TestService, chat: &str) -> (String, String) {
    let history = service.wait_for(chat, 10, "the push's approval", |h| {
        h["approvals"].as_array().is_some_and(|a| !a.is_empty())
    });
    let open = &history["approvals"][0];
    let request = open["request_id"].as_str().unwrap().to_string();
    (request, open["tool"].as_str().unwrap().to_string())
}

#[test]
fn every_push_from_a_worktree_waits_for_the_persons_answer_however_it_is_run() {
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
    let body = json!({ "members": members, "project": project_dir });
    let chat = service.send("POST", "/api/chats", body);
    assert_eq!(chat.status, 200, "{}", chat.body);
    let chat = chat.json()["id"].as_str().unwrap().to_string();
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
        "HEAD:refs/heads/by-alias",
    ];
    let pushing = push_as_engine(&started, &alias);
    let (request, tool) = open_push(&service, &chat);
    assert_eq!(tool, "git push");
    answer(&request, false);
    let (pushed, said) = pushing.join().unwrap();
    assert!(
        !pushed && !has("by-alias"),
        "a declined push went through: {said}"
    );
    assert!(
        !marker.exists(),
        "the project's own hook ran before the person allowed"
    );

    let pushing = push_as_engine(&started, &["push", "origin", "HEAD:refs/heads/allowed"]);
    let (request, _) = open_push(&service, &chat);
    answer(&request, true);
    let (pushed, said) = pushing.join().unwrap();
    assert!(
        pushed && has("allowed"),
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
    let shared = std::process::Command::new("git")
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

/// The live `approval` recording with its question made a Bash push, held once it's answered,
/// before the command ends.
fn allowed_push_recording(dir: &Path) -> PathBuf {
    let command = "git push origin HEAD:refs/heads/approved";
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
    let body = json!({ "members": members, "project": project_dir });
    let chat = service.send("POST", "/api/chats", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Ship it", "agent": rook }));
    let (request, tool) = open_push(&service, &chat);
    assert_eq!(tool, "Bash");
    let to = format!("/api/chats/{chat}/approvals/{request}");
    let answered = service.send("POST", &to, json!({ "allow": true }));
    assert_eq!(answered.status, 200, "{}", answered.body);

    let push = ["push", "origin", "HEAD:refs/heads/approved"];
    let pushing = push_as_engine(&nth(&env, 0), &push);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !pushing.is_finished() {
        let open = service
            .send("GET", &format!("/api/chats/{chat}"), json!(null))
            .json();
        let asked = open["approvals"].as_array().is_some_and(|a| !a.is_empty());
        assert!(
            !asked && Instant::now() < deadline,
            "the allowed push asked again: {open}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
    let (pushed, said) = pushing.join().unwrap();
    assert!(pushed, "the allowed push didn't go through: {said}");
    std::fs::write(&go, "").unwrap();
}
