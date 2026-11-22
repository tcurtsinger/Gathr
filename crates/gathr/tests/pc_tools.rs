//! A tool on this PC, through a real service and the replay engine: Gathr starts the program, relays an
//! engine's list and call to it, and its variable reaches the program and nothing else; disabling it
//! stops it. An engine's own token can't add one.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{TempDir, TestService, gathr, held_recording, sign_in_account, until};
use gathr::http;
use serde_json::{Value, json};

const KEY: &str = "made-up-notes-key-8812";

/// The made-up tool (`examples/pc_tool.rs`), which `cargo test` builds next to the replay engine.
fn pc_tool() -> PathBuf {
    let exe = format!("pc_tool{}", std::env::consts::EXE_SUFFIX);
    let path = common::replay_engine().with_file_name(exe);
    assert!(path.is_file(), "{} isn't built", path.display());
    path
}

/// An MCP request to Gathr's relay at `path`, as an engine sends it with its token.
fn relay(port: u16, path: &str, token: &str, method: &str, params: Value) -> String {
    let name = params["name"].as_str().unwrap_or("").to_string();
    let mut params = params;
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("MCP-Protocol-Version", "2026-07-28"),
        ("Mcp-Method", method),
        ("Mcp-Name", name.as_str()),
    ];
    http::post(port, path, &headers, &body.to_string())
        .unwrap()
        .body
}

fn first_line(file: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(text.lines().next()?).ok()
}

/// Whether the process `pid` is running.
fn running(pid: &str) -> bool {
    if cfg!(windows) {
        let filter = format!("PID eq {pid}");
        let listed = Command::new("tasklist")
            .args(["/FI", &filter, "/NH", "/FO", "CSV"])
            .output();
        listed.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains(&format!("\"{pid}\"")))
    } else {
        let checked = Command::new("kill").args(["-0", pid]).output();
        checked.is_ok_and(|o| o.status.success())
    }
}

#[test]
fn an_engine_calls_a_tool_on_this_pc_through_gathr_and_only_the_program_gets_its_variable() {
    let data = TempDir::new();
    let (args, env) = (data.0.join("args.jsonl"), data.0.join("env.jsonl"));
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let port = service.info.port;
    let program = pc_tool();
    let notes = json!({ "command": program, "args": [r"D:\Notes"], "env": { "NOTES_KEY": KEY } });
    let text = json!({ "mcpServers": { "notes": notes } }).to_string();

    let read = service.send("POST", "/api/tools/read", json!({ "text": text }));
    assert_eq!(read.status, 200, "{}", read.body);
    let read_back = read.json();
    assert_eq!(read_back["program"], json!(program), "{read_back}");
    assert_eq!(read_back["variables"], json!(["NOTES_KEY"]));
    assert_eq!(read_back["name"], "notes");

    let added = service.send("POST", "/api/tools", json!({ "text": text }));
    assert_eq!(added.status, 200, "{}", added.body);
    let tool = added.json()["tool"].clone();
    assert_eq!(
        (&tool["kind"], &tool["name"]),
        (&json!("pc"), &json!("notes"))
    );
    let id = tool["id"].as_str().unwrap().to_string();
    let shown = service.call("GET", &format!("/api/tools/{id}")).json();
    assert_eq!(shown["state"], "connected", "{shown}");
    let mut names: Vec<&str> = shown["actions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    names.sort();
    assert_eq!(names, ["read_key", "search_notes"], "{shown}");

    let account = service.add_account("Mira's");
    sign_in_account(&data.0, &account);
    let agent = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Mira", "account": account }),
    );
    let agent = agent.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let message = json!({ "text": "Read the key", "agent": agent });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), message);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let given: Vec<String> =
        serde_json::from_value(until(10, "the engine's start", || first_line(&args))).unwrap();
    let env = until(10, "the engine's environment", || first_line(&env));
    let token = env["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string();
    let at = given.iter().position(|a| a == "--mcp-config").unwrap();
    let servers: Value = serde_json::from_str(&given[at + 1]).unwrap();
    let url = servers["mcpServers"]["notes"]["url"].as_str().unwrap();
    assert_eq!(url, format!("http://127.0.0.1:{port}/mcp/tools/{id}"));
    let relayed = url.trim_start_matches(&format!("http://127.0.0.1:{port}"));

    let listed = relay(port, relayed, &token, "tools/list", json!({}));
    assert!(listed.contains("search_notes"), "{listed}");
    let call = json!({ "name": "read_key", "arguments": {} });
    let called = relay(port, relayed, &token, "tools/call", call.clone());
    assert!(called.contains(&format!("NOTES_KEY is {KEY}")), "{called}");
    let pid = called.split("in process ").nth(1).unwrap();
    let pid: String = pid.chars().take_while(char::is_ascii_digit).collect();
    assert!(running(&pid), "the tool isn't running: {called}");

    // An engine's token adds no program to run.
    for path in [
        "/api/tools",
        "/api/tools/read",
        &format!("/api/tools/{id}/start"),
    ] {
        let auth = format!("Bearer {token}");
        let headers = [
            ("Authorization", auth.as_str()),
            ("Content-Type", "application/json"),
        ];
        let refused = http::post(port, path, &headers, &json!({ "text": text }).to_string());
        assert_eq!(refused.unwrap().status, 401, "{path}");
    }

    let list = service.call("GET", "/api/tools").body;
    let off = service.send(
        "PATCH",
        &format!("/api/tools/{id}"),
        json!({ "enabled": false }),
    );
    assert_eq!(off.status, 200, "{}", off.body);
    until(10, "the tool to stop", || (!running(&pid)).then_some(()));
    let disabled = relay(port, relayed, &token, "tools/call", call);
    assert!(disabled.contains("disabled"), "{disabled}");

    let (ok, events) = gathr(&data.0, &["diag", "query", "SELECT * FROM event_log"]);
    assert!(ok, "{events}");
    for (what, text) in [
        ("the read back", read.body),
        ("the add's answer", added.body),
        ("the tool's page", shown.to_string()),
        ("the list", list),
        ("the engine's arguments", given.join(" ")),
        ("the engine's environment", env.to_string()),
        ("the events", events.to_string()),
    ] {
        assert!(!text.contains(KEY), "the variable's value is in {what}");
    }
}

/// `npx` on Windows is `npx.cmd`: a bare name is found on the PATH with its extension, and a batch
/// file starts the tool as an `.exe` would.
#[cfg(windows)]
#[test]
fn a_tool_named_without_its_cmd_extension_starts_on_windows() {
    let data = TempDir::new();
    let folder = data.0.join("bin");
    std::fs::create_dir_all(&folder).unwrap();
    let script = format!("@\"{}\" %*\r\n", pc_tool().display());
    std::fs::write(folder.join("notes-mcp.cmd"), script).unwrap();
    let path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths([folder.clone().into_os_string(), path]).unwrap();
    let service = TestService::start_with(&data.0, &[("PATH", Path::new(&path))]);

    let read = service.send(
        "POST",
        "/api/tools/read",
        json!({ "text": r"notes-mcp D:\Notes" }),
    );
    assert_eq!(
        read.json()["program"],
        json!(folder.join("notes-mcp.cmd")),
        "{}",
        read.body
    );
    let added = service.send(
        "POST",
        "/api/tools",
        json!({ "text": r"notes-mcp D:\Notes" }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    let id = added.json()["tool"]["id"].as_str().unwrap().to_string();
    let shown = service.call("GET", &format!("/api/tools/{id}")).json();
    assert_eq!(shown["state"], "connected", "{shown}");
    assert_eq!(
        shown["actions"].as_array().map(Vec::len),
        Some(2),
        "{shown}"
    );
}
