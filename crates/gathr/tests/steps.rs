//! What a step ran (0041), through the real service: the chat's history and the live socket both
//! carry it once the tool ends, with secrets hidden, for Claude Code and Codex. The engines are
//! played from `steps_*.jsonl`, hand-written in the shape of real frames (their first row says so).

mod common;

use std::path::Path;

use common::{TempDir, TestService, gathr, next_live, recording};
use serde_json::{Value, json};

const KEY: &str = "sk-made-up-0123456789abcdef";

/// A service whose engines replay `name`, with a signed-in account for `engine`.
fn service(data: &Path, engine: &str, name: &str) -> (TestService, String) {
    let (replay, played) = (common::replay_engine(), recording(name));
    let env = [
        ("GATHR_CLAUDE_PATH", replay.as_path()),
        ("GATHR_CODEX_PATH", replay.as_path()),
        ("GATHR_REPLAY", played.as_path()),
    ];
    let service = TestService::start_with(data, &env);
    let body = json!({ "engine": engine, "name": "Steps test" });
    let account = service.send("POST", "/api/accounts", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    if engine == "codex" {
        let folder = data.join("accounts").join(&account).join("codex");
        std::fs::create_dir_all(&folder).unwrap();
        let login = r#"{"tokens":{"refresh_token":"made-up"}}"#;
        std::fs::write(folder.join("auth.json"), login).unwrap();
    } else {
        common::sign_in_account(data, &account);
    }
    (service, account)
}

/// One turn by an agent on `account`: the chat's history once it's done, and every live event
/// up to the turn's end.
fn one_turn(service: &TestService, account: &str) -> (Value, Vec<Value>) {
    let body = json!({ "name": "Made-up", "account": account });
    let made = service.send("POST", "/api/agents", body);
    assert_eq!(made.status, 200, "{}", made.body);
    let agent = made.json()["id"].clone();
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let mut live = service.live();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send(
        "POST",
        &path,
        json!({ "text": "Run the steps", "agent": agent }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let mut heard = Vec::new();
    loop {
        let event = next_live(&mut live);
        let ended = event["type"] == "turn" && event["status"] != "running";
        heard.push(event);
        if ended {
            break;
        }
    }
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    (history, heard)
}

/// Each tool message in the history: its id, its tool and what its step kept.
fn steps(history: &Value) -> Vec<(String, String, Value)> {
    let messages = history["messages"].as_array().unwrap().iter();
    let tools = messages.filter(|m| m["kind"] == "tool");
    tools
        .map(|m| {
            let (id, tool) = (
                m["id"].as_str().unwrap(),
                m["body"]["tool"].as_str().unwrap(),
            );
            (id.to_string(), tool.to_string(), m["body"]["input"].clone())
        })
        .collect()
}

/// The history's steps are `expected`, the live socket sent each tool's last body as the history
/// has it, and the made-up key is in neither.
fn carries(history: &Value, heard: &[Value], expected: &[(&str, Value)]) {
    let kept = steps(history);
    let shown: Vec<(&str, Value)> = kept
        .iter()
        .map(|(_, tool, input)| (tool.as_str(), input.clone()))
        .collect();
    assert_eq!(shown, expected, "{history}");
    for (id, tool, input) in &kept {
        let ends = heard.iter().filter(|e| e["type"] == "tool");
        let end = ends.rev().find(|e| e["message"] == id.as_str());
        let live = end.map(|e| e["body"]["input"].clone());
        assert_eq!(live.as_ref(), Some(input), "{tool}'s end, live: {end:?}");
    }
    for said in [history.to_string(), json!(heard).to_string()] {
        assert!(!said.contains(KEY), "the key was kept: {said}");
    }
}

fn text(text: &str) -> Value {
    json!({ "text": text })
}

#[test]
fn a_claude_turns_steps_carry_what_they_ran_in_history_and_live_with_secrets_hidden() {
    let data = TempDir::new();
    let (service, account) = service(&data.0, "claude", "steps_claude");
    let (history, heard) = one_turn(&service, &account);
    let curl = "curl --token=[redacted:flag] -H \"Authorization: Bearer [redacted:bearer]\" \
                https://example.test/api";
    let expected = [
        (
            "Bash",
            json!({ "text": "git status --short", "title": "Show what changed" }),
        ),
        (
            "PowerShell",
            json!({ "text": "cargo fmt --all --check…",
                "request": "cargo fmt --all --check
cargo clippy --workspace" }),
        ),
        ("Read", text("src/lib.rs")),
        ("Edit", text("src/lib.rs")),
        ("Write", text("notes/plan.md")),
        ("Grep", text("fn main")),
        ("WebFetch", text("https://example.test/docs/steps")),
        ("Bash", text(curl)),
        ("mcp__gathr__list_tasks", Value::Null),
        ("Agent", text("Find the tests")),
        ("Monitor", Value::Null),
    ];
    carries(&history, &heard, &expected);

    // The step shows what it runs while it runs: the tool's start carries it too.
    let starts = heard.iter().filter(|e| e["type"] == "message");
    let started: Vec<&Value> = starts
        .filter(|e| e["message"]["kind"] == "tool")
        .map(|e| &e["message"]["body"])
        .collect();
    assert_eq!(started[0]["status"], "running", "{started:?}");
    assert_eq!(started[0]["input"], expected[0].1, "{started:?}");

    // 0011: the event record never copies content. A tool's event says its name, never what it
    // ran, so nothing of a step reaches a bundle, which is made to be shared.
    let query = "SELECT attrs FROM event_log WHERE name LIKE 'execute_tool %' ORDER BY ts, id";
    let (ok, rows) = gathr(&data.0, &["diag", "query", query]);
    assert!(ok, "{rows}");
    let events = rows["rows"].as_array().unwrap();
    assert_eq!(events.len(), expected.len(), "{rows}");
    for event in events {
        let attrs = event["attrs"].as_object().unwrap();
        let keys: Vec<&String> = attrs.keys().collect();
        assert_eq!(keys, ["gen_ai.tool.name"], "{event}");
    }
    let (ok, made) = gathr(&data.0, &["diag", "bundle"]);
    assert!(ok, "{made}");
    let bundle = std::fs::read_to_string(made["written"].as_str().unwrap()).unwrap();
    assert!(
        bundle.contains("execute_tool Bash"),
        "no tool in the bundle"
    );
    for ran in [
        "git status",
        "Show what changed",
        "cargo fmt",
        "src/lib.rs",
        "notes/plan.md",
        "fn main",
        "example.test",
        "Find the tests",
    ] {
        assert!(!bundle.contains(ran), "the bundle holds a step's {ran:?}");
    }
}

/// What a step showed while it ran stays when its turn is cut off: the engine exits with a Read
/// still open, and the stopped step keeps its path.
#[test]
fn a_tool_stopped_mid_run_keeps_what_it_was_running() {
    let data = TempDir::new();
    let (service, account) = service(&data.0, "claude", "exits_mid_reply");
    let body = json!({ "name": "Made-up", "account": account });
    let agent = service.send("POST", "/api/agents", body).json()["id"].clone();
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let history = service.wait_for(&chat, 10, "the turn to fail", |h| {
        h["turns"][0]["status"] == "failed"
    });
    let kept = steps(&history);
    assert_eq!(kept.len(), 1, "{history}");
    let stopped = history["messages"].as_array().unwrap().iter();
    let stopped = stopped.filter(|m| m["kind"] == "tool").collect::<Vec<_>>();
    assert_eq!(stopped[0]["body"]["status"], "stopped", "{history}");
    assert_eq!(kept[0].2, text("note.txt"), "{history}");
}

#[test]
fn a_codex_turns_steps_carry_the_command_out_of_its_shell_the_files_and_the_search() {
    let data = TempDir::new();
    let (service, account) = service(&data.0, "codex", "steps_codex");
    let (history, heard) = one_turn(&service, &account);
    let expected = [
        ("Shell", text("git status --short")),
        (
            "Shell",
            json!({ "text": "Get-Content 'src\\lib.rs' | Select-Object -First 5…",
                "request": "Get-Content 'src\\lib.rs' | Select-Object -First 5\ncargo clippy --workspace" }),
        ),
        (
            "Shell",
            text("curl --token=[redacted:flag] https://example.test/api"),
        ),
        ("Edit", text("C:\\work\\demo\\src\\lib.rs +1 more")),
        // Codex names the search only as it ends: the end's body carries what the start lacked.
        ("WebSearch", text("posix shell quoting rules")),
        ("mcp__gathr__list_tasks", Value::Null),
        ("ViewImage", Value::Null),
    ];
    carries(&history, &heard, &expected);
}

/// 0019's forced failure: an Edit whose `file_path` an engine renamed, and a sub-agent's Grep
/// whose `pattern` it renamed. The Edit shows its label alone, the sub-agent's step isn't shown
/// at all, the turn still ends well, and its reference finds both changes.
#[test]
fn a_tool_without_its_field_is_found_from_the_turns_reference() {
    let data = TempDir::new();
    let (service, account) = service(&data.0, "claude", "steps_shape_changed");
    let (history, heard) = one_turn(&service, &account);
    let expected = [("Edit", Value::Null), ("Agent", text("Find the tests"))];
    carries(&history, &heard, &expected);

    let reference = history["turns"][0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let events = trace["events"].as_array().unwrap().iter();
    let changed: Vec<&Value> = events
        .filter(|e| e["name"] == "gathr.engine.tool_input")
        .collect();
    for event in &changed {
        assert_eq!(event["status"], "error", "{trace}");
        assert_eq!(event["error_type"], "shape_changed", "{trace}");
    }
    let found: Vec<&Value> = changed.iter().map(|e| &e["attrs"]).collect();
    let edit = json!({ "gen_ai.tool.name": "Edit", "field": "file_path" });
    let grep = json!({ "gen_ai.tool.name": "Grep", "field": "pattern" });
    assert_eq!(found, [&edit, &grep], "{trace}");
}

/// What each step returned (0044), through the service, from a live turn (`results_claude`): in
/// history and live alike, a read only as its count, and nothing of it in the event record.
#[test]
fn each_step_keeps_what_it_returned_in_history_and_live_and_never_in_events() {
    let data = TempDir::new();
    let (service, account) = service(&data.0, "claude", "results_claude");
    let (history, heard) = one_turn(&service, &account);
    let messages = history["messages"].as_array().unwrap();
    let body = |tool: &str| {
        let found = messages.iter().find(|m| m["body"]["tool"] == tool);
        found.map(|m| (m["id"].clone(), m["body"].clone())).unwrap()
    };
    for (tool, said) in [
        ("PowerShell", "git version 2.55.0.windows.5"),
        ("Read", "4 lines"),
        ("Edit", "+1 −1"),
        ("Glob", "1 file\nnotes.txt"),
        ("mcp__gathr__list_tasks", "No open tasks."),
    ] {
        let (id, kept) = body(tool);
        assert_eq!(kept["result"]["text"], said, "{tool}: {kept}");
        let end = heard
            .iter()
            .rev()
            .find(|e| e["type"] == "tool" && e["message"] == id);
        let live = end.map(|e| e["body"]["result"].clone());
        assert_eq!(live.as_ref(), Some(&kept["result"]), "{tool}'s end, live");
    }
    // The file's own lines were read, and never kept: only how many.
    assert!(!history.to_string().contains("beta line\n"), "{history}");
    let query =
        "SELECT attrs FROM event_log WHERE attrs LIKE '%git version%' OR attrs LIKE '%4 lines%'";
    let (ok, rows) = gathr(&data.0, &["diag", "query", query]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"], json!([]), "a result reached the event record");
}

/// #161's review: a frame that ends two calls has no `tool_use_result` to say which is which, so
/// a read is never told apart by its text. Its file's lines, here holding a secret `redact` can't
/// know, are kept nowhere; an MCP answer in the same frame still is.
#[test]
fn a_read_whose_kind_isnt_said_keeps_nothing_of_its_file() {
    let data = TempDir::new();
    let secret = "hand-made-secret-QX7ZPV";
    let real = std::fs::read_to_string(recording("results_claude")).unwrap();
    let rows: Vec<&str> = real.lines().collect();
    let first_in = rows.iter().find(|r| r.contains(r#""dir":"in""#)).unwrap();
    let init = rows
        .iter()
        .find(|r| r.contains(r#"\"subtype\":\"init\""#))
        .unwrap();
    let out = |frame: Value| json!({ "dir": "out", "line": frame.to_string() }).to_string();
    let session = "6c0d7e1b-0000-4000-8000-000000000000";
    let calls = json!({ "type": "assistant", "session_id": session, "parent_tool_use_id": null,
        "message": { "role": "assistant", "content": [
            { "type": "tool_use", "id": "toolu_read", "name": "Read", "input": { "file_path": "notes.txt" } },
            { "type": "tool_use", "id": "toolu_tasks", "name": "mcp__gathr__list_tasks", "input": {} } ] } });
    let ended = json!({ "type": "user", "session_id": session, "parent_tool_use_id": null,
        "message": { "role": "user", "content": [
            { "type": "tool_result", "tool_use_id": "toolu_read", "content": format!("1\t{secret}\n2\tmore") },
            { "type": "tool_result", "tool_use_id": "toolu_tasks",
              "content": [{ "type": "text", "text": "No open tasks." }] } ] } });
    let done = json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
        "result": "Done.", "session_id": session, "usage": { "input_tokens": 3, "output_tokens": 2 } });
    let played = [
        first_in.to_string(),
        init.to_string(),
        out(calls),
        out(ended),
        out(done),
    ];
    // `recording` joins a whole path as it is: this one is played from the test's folder.
    let path = data.0.join("two_calls");
    std::fs::write(path.with_extension("jsonl"), played.join("\n") + "\n").unwrap();
    let (service, account) = service(&data.0, "claude", path.to_str().unwrap());
    let (history, heard) = one_turn(&service, &account);

    for said in [history.to_string(), json!(heard).to_string()] {
        assert!(!said.contains(secret), "the file's text was kept: {said}");
    }
    let messages = history["messages"].as_array().unwrap();
    let body =
        |tool: &str| messages.iter().find(|m| m["body"]["tool"] == tool).unwrap()["body"].clone();
    assert_eq!(body("Read")["result"], Value::Null, "{history}");
    assert_eq!(
        body("mcp__gathr__list_tasks")["result"]["text"],
        "No open tasks.",
        "{history}"
    );
}

/// #161's second look: a single read whose `tool_use_result` is empty, or holds no count Gathr
/// knows, is no more told than one with none at all. Its file's text is kept nowhere.
#[test]
fn a_read_with_metadata_gathr_doesnt_know_keeps_nothing_of_its_file() {
    let secret = "hand-made-secret-LM4WQT";
    let real = std::fs::read_to_string(recording("results_claude")).unwrap();
    let rows: Vec<&str> = real.lines().collect();
    let first_in = rows.iter().find(|r| r.contains(r#""dir":"in""#)).unwrap();
    let init = rows
        .iter()
        .find(|r| r.contains(r#"\"subtype\":\"init\""#))
        .unwrap();
    let out = |frame: Value| json!({ "dir": "out", "line": frame.to_string() }).to_string();
    let session = "6c0d7e1b-0000-4000-8000-000000000001";
    for metadata in [
        json!({}),
        json!({ "type": "text", "file": { "filePath": "notes.txt" } }),
    ] {
        let data = TempDir::new();
        let call = json!({ "type": "assistant", "session_id": session, "parent_tool_use_id": null,
            "message": { "role": "assistant", "content": [
                { "type": "tool_use", "id": "toolu_read", "name": "Read", "input": { "file_path": "notes.txt" } } ] } });
        let ended = json!({ "type": "user", "session_id": session, "parent_tool_use_id": null,
            "tool_use_result": metadata,
            "message": { "role": "user", "content": [
                { "type": "tool_result", "tool_use_id": "toolu_read", "content": format!("1\t{secret}") } ] } });
        let done = json!({ "type": "result", "subtype": "success", "is_error": false, "num_turns": 1,
            "result": "Done.", "session_id": session, "usage": { "input_tokens": 3, "output_tokens": 2 } });
        let played = [
            first_in.to_string(),
            init.to_string(),
            out(call),
            out(ended),
            out(done),
        ];
        let path = data.0.join("one_read");
        std::fs::write(path.with_extension("jsonl"), played.join("\n") + "\n").unwrap();
        let (service, account) = service(&data.0, "claude", path.to_str().unwrap());
        let (history, heard) = one_turn(&service, &account);
        for said in [history.to_string(), json!(heard).to_string()] {
            assert!(
                !said.contains(secret),
                "{metadata}: the file's text was kept: {said}"
            );
        }
    }
}
