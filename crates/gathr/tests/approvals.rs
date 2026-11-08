//! Approvals (0029): each agent's mode picks Claude Code's permission mode, and in every mode but
//! Full, Gathr's hook asks before a push or a write outside the engine's folder, admitting only
//! the engine's own token.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, recording, sign_in_account};
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

/// The `n`th line of a file the replay engine appends to, once it's there.
fn nth(file: &Path, n: usize) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let text = std::fs::read_to_string(file).unwrap_or_default();
        if let Some(line) = text.lines().nth(n) {
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
