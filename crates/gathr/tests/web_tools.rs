//! A tool on the web, through a real service and the replay engine: Gathr relays an engine's list and
//! call to it with its key, and the key reaches nothing on the engine's side, no answer and no event.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use common::{TempDir, TestService, gathr, held_recording, sign_in_account, until};
use gathr::http;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde_json::{Value, json};

const KEY: &str = "made-up-key-for-Ledger-5521";

/// A made-up tool, "Ledger", with one action.
#[derive(Clone)]
struct Ledger {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Ledger {
    #[tool(description = "Find a record by its number.\nMore words that aren't shown.")]
    async fn find_record(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            "Record 7: Mira, paid",
        )]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Ledger {
    fn get_info(&self) -> ServerConfig {
        let caps = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(caps).with_server_info(Implementation::new("Ledger", "1.0.0"))
    }
}

/// Starts Ledger on 127.0.0.1, refusing a request without the key with a plain 401: its link, and
/// every Authorization it was sent.
fn ledger() -> (String, Arc<Mutex<Vec<String>>>) {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (tx, rx) = std::sync::mpsc::channel();
    let kept = seen.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let service: StreamableHttpService<Ledger, LocalSessionManager> =
                StreamableHttpService::new(
                    || {
                        Ok(Ledger {
                            tool_router: Ledger::tool_router(),
                        })
                    },
                    Default::default(),
                    StreamableHttpServerConfig::default().with_json_response(true),
                );
            let guard = move |req: Request, next: Next| {
                let kept = kept.clone();
                async move {
                    let auth = req.headers().get("authorization");
                    let auth = auth.and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                    kept.lock().unwrap().push(auth.clone());
                    match auth == format!("Bearer {KEY}") {
                        true => next.run(req).await,
                        false => axum::http::StatusCode::UNAUTHORIZED.into_response(),
                    }
                }
            };
            let app = axum::Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn(guard));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap().port()).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    let port = rx.recv().unwrap();
    (format!("http://127.0.0.1:{port}/mcp"), seen)
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

#[test]
fn an_engine_lists_and_calls_a_tool_through_gathr_and_never_holds_its_key() {
    let (link, seen) = ledger();
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

    let asked = service.send("POST", "/api/tools", json!({ "link": link }));
    assert_eq!(asked.json(), json!({ "needs": "key" }), "{}", asked.body);
    let wrong = json!({ "link": link, "key": "not-it" });
    let refused = service.send("POST", "/api/tools", wrong);
    assert_eq!(refused.status, 409, "{}", refused.body);
    let added = service.send("POST", "/api/tools", json!({ "link": link, "key": KEY }));
    assert_eq!(added.status, 200, "{}", added.body);
    let tool = added.json()["tool"].clone();
    assert_eq!(
        (&tool["name"], &tool["has_key"]),
        (&json!("Ledger"), &json!(true))
    );
    let id = tool["id"].as_str().unwrap().to_string();
    let shown = service.call("GET", &format!("/api/tools/{id}")).json();
    assert_eq!(shown["state"], "connected", "{shown}");
    assert_eq!(
        shown["actions"],
        json!([{ "name": "find_record", "description": "Find a record by its number." }])
    );

    let account = service.add_account("Mira's");
    sign_in_account(&data.0, &account);
    let agent = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Mira", "account": account }),
    );
    let agent = agent.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send(
        "POST",
        &path,
        json!({ "text": "Find record 7", "agent": agent }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let given: Vec<String> =
        serde_json::from_value(until(10, "the engine's start", || first_line(&args))).unwrap();
    let env = until(10, "the engine's environment", || first_line(&env));
    let token = env["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string();
    let at = given.iter().position(|a| a == "--mcp-config").unwrap();
    let servers: Value = serde_json::from_str(&given[at + 1]).unwrap();
    let entry = &servers["mcpServers"]["ledger"];
    let url = entry["url"].as_str().unwrap();
    assert_eq!(url, format!("http://127.0.0.1:{port}/mcp/tools/{id}"));
    assert!(entry.get("alwaysLoad").is_none(), "{entry}");
    let relayed = url.trim_start_matches(&format!("http://127.0.0.1:{port}"));

    let listed = relay(port, relayed, &token, "tools/list", json!({}));
    assert!(listed.contains("find_record"), "{}", listed);
    let called = relay(
        port,
        relayed,
        &token,
        "tools/call",
        json!({ "name": "find_record", "arguments": {} }),
    );
    assert!(called.contains("Record 7: Mira, paid"), "{}", called);
    let without = relay(port, relayed, "not-a-token", "tools/list", json!({}));
    assert!(!without.contains("find_record"), "{}", without);
    let bearer = format!("Bearer {KEY}");
    assert!(
        seen.lock().unwrap().contains(&bearer),
        "the tool never got its key"
    );

    let listed = service.call("GET", "/api/tools").body;
    let (ok, events) = gathr(&data.0, &["diag", "query", "SELECT * FROM event_log"]);
    assert!(ok, "{events}");
    for (what, text) in [
        ("the engine's arguments", given.join(" ")),
        ("the engine's environment", env.to_string()),
        ("the list", listed),
        ("the tool's page", shown.to_string()),
        ("the add's answer", added.body.clone()),
        ("the relayed answers", format!("{}{}", called, without)),
        ("the events", events.to_string()),
    ] {
        assert!(!text.contains(KEY), "the key is in {what}");
    }
}
