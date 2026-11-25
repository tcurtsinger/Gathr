//! A tool that signs in through the browser, with a made-up sign-in service, both on 127.0.0.1: the
//! sign-in comes back to Gathr once, an engine's call is relayed with it, it renews by itself, a
//! refused renewal asks for a new sign-in, and no token is kept or said as text.

mod common;

use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use common::{TempDir, TestService, gathr, held_recording, sign_in_account, until};
use gathr::http;
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use rmcp::{ErrorData, ServerHandler, tool, tool_handler, tool_router};
use serde_json::{Value, json};

/// A made-up tool, "Service desk", with one action.
#[derive(Clone)]
struct Desk {
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl Desk {
    #[tool(description = "Find tickets by words.")]
    async fn find_tickets(&self) -> Result<CallToolResult, ErrorData> {
        Ok(CallToolResult::success(vec![ContentBlock::text(
            "Ticket 12: Bo's printer",
        )]))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Desk {
    fn get_info(&self) -> ServerConfig {
        let caps = ServerCapabilities::builder().enable_tools().build();
        ServerConfig::new(caps).with_server_info(Implementation::new("Service desk", "1.0.0"))
    }
}

/// The Client ID and secret an admin made for Gathr, where the service takes no app it doesn't know.
const CLIENT: &str = "made-up-client-7";
const SECRET: &str = "made-up-secret-9";

/// What the made-up service lets in, and what it was sent.
#[derive(Default)]
struct Desked {
    origin: String,
    /// Whether Gathr can register itself. When it can't, Gathr signs in as `CLIENT`, and each token
    /// asked without `SECRET` is refused; tokens then last 10 s, so each use renews them.
    registers: bool,
    /// Every body sent to the token address.
    exchanged: Vec<String>,
    /// The access tokens the tool takes now.
    valid: Vec<String>,
    /// Whether a renewal is taken.
    renews: bool,
    /// Each token handed out so far.
    handed: Vec<String>,
    /// Every Authorization the tool was sent.
    seen: Vec<String>,
}

type Shared = Arc<Mutex<Desked>>;

/// Hands out a new access token, and a refresh token with it.
fn token(desk: &mut Desked) -> Json<Value> {
    let access = format!("made-up-access-{}", desk.handed.len() + 1);
    desk.handed.push(access.clone());
    desk.valid = vec![access.clone()];
    Json(json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": if desk.registers { 3600 } else { 10 },
        "refresh_token": format!("made-up-refresh-{}", desk.handed.len()),
    }))
}

/// Starts Service desk and its sign-in service on one port: the tool's link, and what it shares.
fn desk(registers: bool) -> (String, Shared) {
    let shared: Shared = Arc::default();
    shared.lock().unwrap().registers = registers;
    let (tx, rx) = std::sync::mpsc::channel();
    let kept = shared.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
            kept.lock().unwrap().origin = origin.clone();
            let service: StreamableHttpService<Desk, LocalSessionManager> =
                StreamableHttpService::new(
                    || {
                        Ok(Desk {
                            tool_router: Desk::tool_router(),
                        })
                    },
                    Default::default(),
                    StreamableHttpServerConfig::default().with_json_response(true),
                );
            let guard = |State(desk): State<Shared>, req: Request, next: Next| async move {
                let auth = req.headers().get("authorization");
                let auth = auth.and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
                let (allowed, origin) = {
                    let mut desk = desk.lock().unwrap();
                    desk.seen.push(auth.clone());
                    let allowed = desk.valid.iter().any(|t| auth == format!("Bearer {t}"));
                    (allowed, desk.origin.clone())
                };
                if allowed {
                    return next.run(req).await;
                }
                let challenge = format!(
                    "Bearer resource_metadata=\"{origin}/.well-known/oauth-protected-resource/mcp\""
                );
                let mut refused = StatusCode::UNAUTHORIZED.into_response();
                refused
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, challenge.parse().unwrap());
                refused
            };
            let mcp = Router::new()
                .nest_service("/mcp", service)
                .layer(middleware::from_fn_with_state(kept.clone(), guard));
            let o = origin.clone();
            let resource = move || {
                let o = o.clone();
                async move {
                    Json(json!({ "resource": format!("{o}/mcp"), "authorization_servers": [o] }))
                }
            };
            let o = origin.clone();
            let server = move || {
                let o = o.clone();
                async move {
                    let mut server = json!({
                        "issuer": o,
                        "authorization_endpoint": format!("{o}/authorize"),
                        "token_endpoint": format!("{o}/token"),
                        "response_types_supported": ["code"],
                        "code_challenge_methods_supported": ["S256"],
                    });
                    match registers {
                        true => server["registration_endpoint"] = format!("{o}/register").into(),
                        false => {
                            server["token_endpoint_auth_methods_supported"] =
                                json!(["client_secret_post"])
                        }
                    }
                    Json(server)
                }
            };
            let register = |Json(asked): Json<Value>| async move {
                assert_eq!(asked["client_name"], "Gathr");
                Json(json!({ "client_id": "made-up-client", "redirect_uris": asked["redirect_uris"] }))
            };
            // The person signs in at once: the page sends the browser straight back with a code.
            let authorize = |req: Request| async move {
                let query = req.uri().query().unwrap_or("").to_string();
                let pick = |name: &str| {
                    query.split('&').find_map(|p| p.strip_prefix(&format!("{name}=")[..]))
                        .unwrap_or("").to_string()
                };
                let back = pick("redirect_uri").replace("%3A", ":").replace("%2F", "/");
                let to = format!("{back}?code=made-up-code&state={}", pick("state"));
                (StatusCode::FOUND, [(header::LOCATION, to)]).into_response()
            };
            let exchange = |State(desk): State<Shared>, body: String| async move {
                let mut desk = desk.lock().unwrap();
                desk.exchanged.push(body.clone());
                if !desk.registers && !body.contains(&format!("client_secret={SECRET}")) {
                    let refused = Json(json!({ "error": "invalid_client" }));
                    return (StatusCode::UNAUTHORIZED, refused).into_response();
                }
                let renewing = body.contains("grant_type=refresh_token");
                if renewing && !desk.renews {
                    let refused = Json(json!({ "error": "invalid_grant" }));
                    return (StatusCode::BAD_REQUEST, refused).into_response();
                }
                token(&mut desk).into_response()
            };
            let app = Router::new()
                .route("/.well-known/oauth-protected-resource/mcp", get(resource))
                .route("/.well-known/oauth-authorization-server", get(server))
                .route("/register", post(register))
                .route("/authorize", get(authorize))
                .route("/token", post(exchange))
                .with_state(kept.clone())
                .merge(mcp);
            tx.send(origin).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    let origin = rx.recv().unwrap();
    (format!("{origin}/mcp"), shared)
}

/// The port and path-and-query of an http URL on 127.0.0.1.
fn at(url: &str) -> (u16, String) {
    let rest = url.strip_prefix("http://127.0.0.1:").unwrap();
    let (port, path) = rest.split_at(rest.find('/').unwrap());
    (port.parse().unwrap(), path.to_string())
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

/// The first line of `file` that holds `word`: the stand-in browser, the replay engine too, writes
/// its own lines first.
fn line_with(file: &Path, word: &str) -> Option<Value> {
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(text.lines().find(|l| l.contains(word))?).ok()
}

#[test]
fn a_tool_signs_in_through_the_browser_renews_and_asks_again_when_refused() {
    let (link, desk) = desk(true);
    let data = TempDir::new();
    let (args, env) = (data.0.join("args.jsonl"), data.0.join("env.jsonl"));
    let engine = common::replay_engine();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
            ("BROWSER", &engine),
        ],
    );
    let port = service.info.port;

    let asked = service.send("POST", "/api/tools", json!({ "link": link }));
    assert_eq!(
        asked.json(),
        json!({ "needs": "sign_in" }),
        "{}",
        asked.body
    );
    let started = service.send("POST", "/api/tools/sign-in", json!({ "link": link }));
    assert_eq!(started.status, 200, "{}", started.body);
    let (page, state) = (
        started.json()["url"].clone(),
        started.json()["sign_in"].clone(),
    );
    let (page, state) = (page.as_str().unwrap(), state.as_str().unwrap());

    // The browser, played here: the sign-in page sends it back to Gathr with a code.
    let wrong = service.get("/tools/signed-in?state=not-it&code=made-up-code", &[]);
    assert_eq!(wrong.status, 400, "{}", wrong.body);
    let (desk_port, path) = at(page);
    let signed = http::request(desk_port, "GET", &path, &[]).unwrap();
    let location = signed.headers.lines().find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.eq_ignore_ascii_case("location")
            .then(|| value.trim().to_string())
    });
    let (back_port, back) = at(&location.unwrap());
    assert_eq!(back_port, port);
    assert!(back.contains(&format!("state={state}")), "{back}");
    let came = service.get(&back, &[]);
    assert_eq!(came.status, 200, "{}", came.body);
    assert!(came.body.contains("Signed in"), "{}", came.body);
    let again = service.get(&back, &[]);
    assert_eq!(again.status, 400, "a state came back twice: {}", again.body);

    let tools = service.call("GET", "/api/tools").json();
    // Last: computer use (0085) is listed before it.
    let tool = tools["tools"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        (
            &tool["name"],
            &tool["signed_in"],
            &tool["has_key"],
            &tool["state"]
        ),
        (
            &json!("Service desk"),
            &json!(true),
            &json!(false),
            &json!("connected")
        ),
        "{tools}"
    );
    let id = tool["id"].as_str().unwrap().to_string();

    let account = service.add_account("Ann's");
    sign_in_account(&data.0, &account);
    let agent = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ann", "account": account }),
    );
    let agent = agent.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send(
        "POST",
        &path,
        json!({ "text": "Find tickets", "agent": agent }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let given: Vec<String> = serde_json::from_value(until(10, "the engine's start", || {
        line_with(&args, "--mcp-config")
    }))
    .unwrap();
    let env = until(10, "the engine's environment", || {
        line_with(&env, "\"GATHR_AGENT_TOKEN\":\"")
    });
    let token = env["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string();
    let relayed = format!("/mcp/tools/{id}");
    let call = json!({ "name": "find_tickets", "arguments": {} });

    let called = relay(port, &relayed, &token, "tools/call", call.clone());
    assert!(called.contains("Ticket 12"), "{called}");
    // The tool ends the sign-in's token early: the next call renews it by itself.
    desk.lock().unwrap().valid.clear();
    desk.lock().unwrap().renews = true;
    let renewed = relay(port, &relayed, &token, "tools/call", call.clone());
    assert!(renewed.contains("Ticket 12"), "{renewed}");
    assert!(
        desk.lock()
            .unwrap()
            .seen
            .contains(&"Bearer made-up-access-2".to_string())
    );

    // The service refuses the next renewal: the tool asks to be signed in again.
    desk.lock().unwrap().valid.clear();
    desk.lock().unwrap().renews = false;
    let refused = relay(port, &relayed, &token, "tools/call", call);
    assert!(refused.contains("signed in again"), "{refused}");
    let tools = service.call("GET", "/api/tools").json();
    assert_eq!(
        tools["tools"].as_array().unwrap().last().unwrap()["state"],
        "sign_in",
        "{tools}"
    );

    let handed = desk.lock().unwrap().handed.clone();
    let (ok, events) = gathr(&data.0, &["diag", "query", "SELECT * FROM event_log"]);
    assert!(ok, "{events}");
    let (ok, rows) = gathr(
        &data.0,
        &[
            "diag",
            "query",
            "SELECT hex(sign_in) AS s FROM tools WHERE id != 'computer'",
        ],
    );
    assert!(ok, "{rows}");
    let sealed = rows["rows"][0]["s"].as_str().unwrap().to_string();
    let folder = everything_in(&data.0);
    for secret in handed.iter().chain(&["made-up-refresh-1".to_string()]) {
        let hex: String = secret.bytes().map(|b| format!("{b:02X}")).collect();
        assert!(!sealed.contains(&hex), "the sign-in is stored as text");
        for (what, text) in [
            ("the data folder", &folder),
            ("the engine's arguments", &given.join(" ")),
            ("the engine's environment", &env.to_string()),
            ("the list", &tools.to_string()),
            (
                "the relayed answers",
                &format!("{called}{renewed}{refused}"),
            ),
            ("the events", &events.to_string()),
        ] {
            assert!(!text.contains(secret.as_str()), "a token is in {what}");
        }
    }
}

/// Every file in the data folder as text: the database and its journal, the logs, the engine's files.
fn everything_in(data: &Path) -> String {
    let mut folder = String::new();
    let mut dirs = vec![data.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            match entry.file_type().unwrap().is_dir() {
                true => dirs.push(entry.path()),
                false => folder.push_str(&String::from_utf8_lossy(
                    &std::fs::read(entry.path()).unwrap_or_default(),
                )),
            }
        }
    }
    folder
}

/// Starts a sign-in with `asked` and plays the browser through it: the sign-in page's address.
fn signs_in(service: &TestService, asked: Value) -> String {
    let started = service.send("POST", "/api/tools/sign-in", asked);
    assert_eq!(started.status, 200, "{}", started.body);
    let page = started.json()["url"].as_str().unwrap().to_string();
    let (desk_port, path) = at(&page);
    let signed = http::request(desk_port, "GET", &path, &[]).unwrap();
    let location = signed.headers.lines().find_map(|l| {
        let (name, value) = l.split_once(':')?;
        name.eq_ignore_ascii_case("location")
            .then(|| value.trim().to_string())
    });
    let came = service.get(&at(&location.unwrap()).1, &[]);
    assert!(came.body.contains("Signed in"), "{}", came.body);
    page
}

#[test]
fn a_tool_whose_service_takes_no_app_signs_in_with_a_client_id_and_its_secret() {
    let (link, desk) = desk(false);
    desk.lock().unwrap().renews = true;
    let data = TempDir::new();
    let engine = common::replay_engine();
    let service = TestService::start_with(&data.0, &[("BROWSER", &engine)]);
    let redirect = format!("http://127.0.0.1:{}/tools/signed-in", service.info.port);

    // Gathr knows before any sign-in that the tool needs a Client ID, and the address to give.
    let asked = service.send("POST", "/api/tools", json!({ "link": link }));
    let needs = json!({ "needs": "sign_in", "registers": false, "redirect": redirect });
    assert_eq!(asked.json(), needs, "{}", asked.body);
    let bare = service.send("POST", "/api/tools/sign-in", json!({ "link": link }));
    let needs = json!({ "registers": false, "redirect": redirect });
    assert_eq!(bare.json(), needs, "{}", bare.body);

    let client = json!({ "link": link, "client_id": CLIENT, "client_secret": SECRET });
    let page = signs_in(&service, client);
    assert!(page.contains(&format!("client_id={CLIENT}")), "{page}");
    // Its tokens run out at once: listing it renews them, with the sign-in read from the database.
    let tools = service.call("GET", "/api/tools").json();
    let tool = tools["tools"].as_array().unwrap().last().unwrap().clone();
    assert_eq!(
        (&tool["client_id"], &tool["state"]),
        (&json!(CLIENT), &json!("connected")),
        "{tools}"
    );
    let id = tool["id"].as_str().unwrap().to_string();
    let one = service.call("GET", &format!("/api/tools/{id}")).json();
    assert_eq!(one["redirect"], json!(redirect), "{one}");

    // Gathr restarts: the renewal still sends the secret.
    drop(service);
    let service = TestService::start_with(&data.0, &[("BROWSER", &engine)]);
    let renewals = |desk: &Shared| {
        let desk = desk.lock().unwrap();
        let renewals = desk
            .exchanged
            .iter()
            .filter(|b| b.contains("refresh_token"));
        renewals.count()
    };
    let before = renewals(&desk);
    let tools = service.call("GET", "/api/tools").json();
    assert_eq!(
        tools["tools"].as_array().unwrap().last().unwrap()["state"],
        "connected",
        "{tools}"
    );
    assert!(
        renewals(&desk) > before,
        "{:?}",
        desk.lock().unwrap().exchanged
    );

    // Sign in again signs in as the same client, with its secret, in one step.
    let page = signs_in(&service, json!({ "tool": id }));
    assert!(page.contains(&format!("client_id={CLIENT}")), "{page}");
    let exchanged = desk.lock().unwrap().exchanged.clone();
    let codes = exchanged
        .iter()
        .filter(|b| b.contains("authorization_code"));
    assert_eq!(codes.count(), 2, "{exchanged:?}");
    assert!(
        exchanged.iter().all(|b| b.contains(SECRET)),
        "a token was asked without the secret: {exchanged:?}"
    );

    let tools = service.call("GET", "/api/tools").json();
    let (ok, events) = gathr(&data.0, &["diag", "query", "SELECT * FROM event_log"]);
    assert!(ok, "{events}");
    let (ok, rows) = gathr(
        &data.0,
        &[
            "diag",
            "query",
            "SELECT hex(sign_in) AS s, client_id FROM tools WHERE id != 'computer'",
        ],
    );
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"][0]["client_id"], CLIENT, "{rows}");
    let sealed = rows["rows"][0]["s"].as_str().unwrap();
    let hex: String = SECRET.bytes().map(|b| format!("{b:02X}")).collect();
    assert!(!sealed.contains(&hex), "the secret is stored as text");
    let one = service.call("GET", &format!("/api/tools/{id}")).body;
    for (what, text) in [
        ("the data folder", &everything_in(&data.0)),
        ("the list", &tools.to_string()),
        ("the tool", &one),
        ("the events", &events.to_string()),
    ] {
        assert!(!text.contains(SECRET), "the secret is in {what}");
    }

    // Remove takes its Client ID and secret with it.
    let removed = service.call("DELETE", &format!("/api/tools/{id}"));
    assert_eq!(removed.status, 200, "{}", removed.body);
    let (ok, rows) = gathr(
        &data.0,
        &[
            "diag",
            "query",
            "SELECT count(*) AS n FROM tools WHERE id != 'computer'",
        ],
    );
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"][0]["n"], 0, "{rows}");
}
