//! The endpoint for AI apps outside Gathr (`/api/mcp`, reached through `gathr connect`) is guarded as `/api` is: it
//! takes the service's own token, never a running engine's, and nothing without one.

mod common;

use common::{first_token, held_recording, team};
use gathr::http;
use serde_json::{Value, json};

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
        ["add_agent", "add_project", "add_skill", "add_tool", "look"]
    );
    std::fs::write(data.0.join("go"), "").unwrap();
}
