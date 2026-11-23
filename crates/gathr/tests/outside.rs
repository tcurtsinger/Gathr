//! The endpoint for AI apps outside Gathr (`/api/mcp`, reached through `gathr connect`) is guarded as `/api` is: it
//! takes the service's own token, never a running engine's, and nothing without one. What an app sends costs the
//! person's tokens once: one send, one message and one turn; a read, none.

mod common;

use common::{first_token, held_recording, recording, rows, team};
use gathr::http;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

/// `tools/list` at `/api/mcp` under the 2026-07-28 revision, with `token` if one is given.
fn list(port: u16, token: Option<&str>) -> http::Response {
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": { "_meta": {
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "garden-planner", "version": "1.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    } } });
    let auth = token.map(|t| format!("Bearer {t}"));
    let mut headers = vec![
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("MCP-Protocol-Version", "2026-07-28"),
        ("Mcp-Method", "tools/list"),
    ];
    headers.extend(auth.as_deref().map(|a| ("Authorization", a)));
    http::post(port, "/api/mcp", &headers, &body.to_string()).unwrap()
}

#[test]
fn an_outside_app_needs_the_services_token_and_never_an_engines() {
    let data = common::TempDir::new();
    let (service, chat, lead) = team(&data.0, &held_recording(&data.0, "signed_in"), &[]);
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send(
        "POST",
        &path,
        json!({ "text": "Plan the beds", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let engine = first_token(&data.0);
    let port = service.info.port;

    let without = list(port, None);
    assert_eq!(without.status, 401, "with no token: {}", without.body);
    let as_engine = list(port, Some(&engine));
    assert_eq!(
        as_engine.status, 401,
        "with an engine's token: {}",
        as_engine.body
    );

    let listed = list(port, Some(&service.info.token));
    assert_eq!(listed.status, 200, "{}", listed.body);
    let tools = listed.json()["result"]["tools"].clone();
    let mut names: Vec<&str> = (tools.as_array().into_iter().flatten())
        .filter_map(|t: &Value| t["name"].as_str())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "add_agent",
            "add_project",
            "add_skill",
            "add_tool",
            "look",
            "read",
            "send"
        ]
    );
    std::fs::write(data.0.join("go"), "").unwrap();
}

/// A `tools/call` at `/api/mcp`, as `gathr connect` passes one on for the app it names: the tool's text, and whether
/// it failed.
fn call(port: u16, token: &str, tool: &str, arguments: Value) -> (String, bool) {
    let body = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
        "params": { "name": tool, "arguments": arguments } });
    let auth = format!("Bearer {token}");
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("x-gathr-app", "Garden Planner"),
    ];
    let answer = http::post(port, "/api/mcp", &headers, &body.to_string()).unwrap();
    assert_eq!(answer.status, 200, "{}", answer.body);
    let result = answer.json()["result"].clone();
    let text = result["content"][0]["text"].as_str().unwrap_or_default();
    (text.to_string(), result["isError"] == true)
}

/// `signed_in` played twice, each turn with a working note before its last words: "Hi!", then NO_REPLY.
fn noted(dir: &Path) -> PathBuf {
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let block = |t: &str| format!(r#"{{\"type\":\"text\",\"text\":\"{t}\"}}"#);
    let noted = |last: &str| format!("[{},{}]", block("Looking at the beds."), block(last));
    let one = format!("[{}]", block("Hi!"));
    let turns = [noted("Hi!"), noted("NO_REPLY")].map(|n| frames.trim_end().replace(&one, &n));
    let file = dir.join("noted.jsonl");
    std::fs::write(&file, turns.join("\n") + "\n").unwrap();
    file
}

#[test]
fn one_send_makes_one_message_and_one_turn_and_a_read_makes_none() {
    let data = common::TempDir::new();
    let (service, _, _) = team(&data.0, &noted(&data.0), &[]);
    let (port, token) = (service.info.port, service.info.token.clone());
    let made = || {
        let sql = "SELECT (SELECT count(*) FROM messages WHERE role = 'user') AS messages,
                          (SELECT count(*) FROM turns) AS turns, (SELECT group_concat(app) FROM from_app) AS apps";
        let row = rows(&data.0, sql)[0].clone();
        (
            row["messages"].clone(),
            row["turns"].clone(),
            row["apps"].clone(),
        )
    };

    let (sent, failed) = call(port, &token, "send", json!({ "words": "Plan the beds" }));
    assert!(!failed, "{sent}");
    assert!(sent.ends_with(": done.\n\nLead: Hi!"), "{sent}");
    assert!(
        !sent.contains("Looking at the beds."),
        "a working note: {sent}"
    );
    let once = (json!(1), json!(1), json!("Garden Planner"));
    assert_eq!(made(), once);

    let chat = sent.trim_start_matches("Chat ").split(':').next().unwrap();
    for asked in [json!({ "chat": chat }), json!({})] {
        let (read, failed) = call(port, &token, "read", asked);
        assert!(!failed, "{read}");
    }
    assert_eq!(made(), once, "a read starts nothing");

    // Neither a working note nor the empty last words NO_REPLY leaves is read back as said.
    let (again, failed) = call(
        port,
        &token,
        "send",
        json!({ "words": "And the paths?", "chat": chat }),
    );
    assert!(!failed, "{again}");
    assert_eq!(again, format!("Chat {chat}: done."));
}
