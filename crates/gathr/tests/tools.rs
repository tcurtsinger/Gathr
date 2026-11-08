//! Gathr's tools (0005) and each engine's briefing (0021, 0020): an engine gets a token of its own
//! for `/mcp`, the tools answer as its agent in its chat, the token ends with the engine, and every
//! turn records what Gathr gave it.

mod common;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use common::{TempDir, TestService, gathr, recording, sign_in_account};
use gathr::http::{self, Response};
use serde_json::{Value, json};

/// `signed_in` with the engine held after the message arrives, until `go` exists.
fn held_recording(dir: &Path) -> PathBuf {
    let held = dir.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    held
}

fn until<T>(secs: u64, what: &str, mut found: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(secs);
    loop {
        if let Some(found) = found() {
            return found;
        }
        assert!(Instant::now() < deadline, "waited {secs} s for {what}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn query(data: &Path, sql: &str) -> Vec<Value> {
    let (ok, rows) = gathr(data, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    rows["rows"].as_array().cloned().unwrap_or_default()
}

/// What the engine was started with: its arguments, and the token in its environment.
struct Started {
    args: Vec<String>,
    token: String,
}

impl Started {
    fn after(&self, switch: &str) -> &str {
        let at = self.args.iter().position(|a| a == switch);
        let at = at.unwrap_or_else(|| panic!("no {switch} in {:?}", self.args));
        &self.args[at + 1]
    }
}

fn started(args: &Path, env: &Path) -> Started {
    let args: Vec<String> = until(10, "the engine's start", || {
        let text = std::fs::read_to_string(args).ok()?;
        serde_json::from_str(text.lines().next()?).ok()
    });
    let env: Value = until(10, "the engine's environment", || {
        let text = std::fs::read_to_string(env).ok()?;
        serde_json::from_str(text.lines().next()?).ok()
    });
    let token = env["GATHR_AGENT_TOKEN"].as_str().unwrap_or_default();
    Started {
        args,
        token: token.to_string(),
    }
}

/// One JSON-RPC request to `/mcp` as an engine sends it under the 2026-07-28 revision.
fn mcp(port: u16, token: &str, method: &str, params: Value, name: Option<&str>) -> Response {
    let mut params = params;
    params["_meta"] = json!({
        "io.modelcontextprotocol/protocolVersion": "2026-07-28",
        "io.modelcontextprotocol/clientInfo": { "name": "test", "version": "1.0" },
        "io.modelcontextprotocol/clientCapabilities": {}
    });
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
    let auth = format!("Bearer {token}");
    let mut headers = vec![
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
        ("MCP-Protocol-Version", "2026-07-28"),
        ("Mcp-Method", method),
    ];
    if let Some(name) = name {
        headers.push(("Mcp-Name", name));
    }
    http::post(port, "/mcp", &headers, &body.to_string()).unwrap()
}

fn call_tool(port: u16, token: &str, tool: &str, arguments: Value) -> Value {
    let params = json!({ "name": tool, "arguments": arguments });
    let answer = mcp(port, token, "tools/call", params, Some(tool));
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["result"].clone()
}

fn text(result: &Value) -> String {
    result["content"][0]["text"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn an_engines_token_reaches_gathrs_tools_as_its_agent_until_the_engine_exits() {
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
            ("GATHR_IDLE_CLOSE_MS", Path::new("1000")),
        ],
    );
    let account = service.add_account("Orin's");
    sign_in_account(&data.0, &account);
    let body = json!({ "name": "Orin", "account": account });
    let orin = service.send("POST", "/api/agents", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": orin }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let trace_ref = sent.json()["ref"].as_str().unwrap().to_string();

    let engine = started(&args, &env);
    assert!(!engine.token.is_empty(), "the engine got no token");
    assert_ne!(
        engine.token, service.info.token,
        "the engine got the service's token"
    );
    let config: Value = serde_json::from_str(engine.after("--mcp-config")).unwrap();
    let server = &config["mcpServers"]["gathr"];
    let url = format!("http://127.0.0.1:{}/mcp", service.info.port);
    assert_eq!(server["url"], url.as_str(), "{config}");
    assert_eq!(
        server["headers"]["Authorization"], "Bearer ${GATHR_AGENT_TOKEN}",
        "the token went into the arguments, where any process can read it"
    );
    assert_eq!(engine.after("--allowedTools"), "mcp__gathr");

    let port = service.info.port;
    let listed = mcp(port, &engine.token, "tools/list", json!({}), None);
    assert_eq!(listed.status, 200, "{}", listed.body);
    let tools = listed.json()["result"]["tools"].clone();
    let names: Vec<&str> = tools
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    assert_eq!(names, ["read_chat"], "{tools}");

    // As Orin, in the chat the engine runs: the person's message, by its author.
    let read = call_tool(port, &engine.token, "read_chat", json!({}));
    assert_eq!(read["isError"], false, "{read}");
    assert!(text(&read).contains("person: Say hi"), "{read}");

    // A tool failure answers with a reference that traces to why.
    let failed = call_tool(
        port,
        &engine.token,
        "read_chat",
        json!({ "message": "nope" }),
    );
    assert_eq!(failed["isError"], true, "{failed}");
    let said = text(&failed);
    let reference = said.rsplit("ref ").next().unwrap().trim_end_matches(')');
    assert_eq!(
        reference, trace_ref,
        "a tool's failure isn't on its turn's trace: {said}"
    );
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let tool_events: Vec<&Value> = events
        .iter()
        .filter(|e| e["name"] == "gathr.tool.call")
        .collect();
    assert_eq!(tool_events.len(), 2, "{trace}");
    assert_eq!(
        tool_events[0]["attrs"]["gen_ai.agent.id"],
        orin.as_str(),
        "{trace}"
    );
    assert_eq!(tool_events[0]["attrs"]["gen_ai.tool.name"], "read_chat");
    assert_eq!(tool_events[1]["status"], "error", "{trace}");
    assert!(
        tool_events[1]["attrs"]["why"]
            .as_str()
            .unwrap()
            .contains("nope"),
        "{trace}"
    );

    // Only an engine's token opens /mcp: not the service's, not a made-up one.
    for (token, why) in [
        (service.info.token.as_str(), "the service's"),
        ("made-up", "a made-up"),
    ] {
        let refused = mcp(port, token, "tools/list", json!({}), None);
        assert_eq!(
            refused.status, 401,
            "{why} token opened /mcp: {}",
            refused.body
        );
    }

    // The token ends with its engine: after the turn, the idle close.
    std::fs::write(&go, "").unwrap();
    service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    until(10, "the token to end with its engine", || {
        let refused = mcp(port, &engine.token, "tools/list", json!({}), None);
        (refused.status == 401).then_some(())
    });
    let closed = query(
        &data.0,
        "SELECT 1 FROM event_log WHERE name = 'gathr.engine.close'",
    );
    assert_eq!(closed.len(), 1, "the token ended before its engine did");
}

#[test]
fn each_turn_records_the_briefing_its_engine_was_given_and_whats_off() {
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
            ("GATHR_IDLE_CLOSE_MS", Path::new("1000")),
        ],
    );
    let account = service.add_account("Vale's");
    sign_in_account(&data.0, &account);
    let body = json!({ "name": "Vale", "account": account });
    let vale = service.send("POST", "/api/agents", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": vale }));
    assert_eq!(sent.status, 200, "{}", sent.body);

    let engine = started(&args, &env);
    let file = PathBuf::from(engine.after("--append-system-prompt-file"));
    let briefing = std::fs::read_to_string(&file).unwrap();
    assert!(briefing.contains("You are Vale"), "{briefing}");
    assert!(
        briefing.contains("read_chat"),
        "the briefing doesn't name Gathr's tools: {briefing}"
    );
    // G21: what's off is said before the agent tries it.
    assert!(
        briefing.contains("ScheduleWakeup"),
        "the briefing doesn't say what's off: {briefing}"
    );
    assert!(
        !file.starts_with(data.0.join("chats")),
        "the briefing is in the agent's working folder, where it can edit it"
    );

    let rows = query(
        &data.0,
        "SELECT json_extract(attrs, '$.\"gathr.context.briefing.chars\"') AS briefing, \
         json_extract(attrs, '$.\"gathr.context.briefing.tokens_estimate\"') AS estimate, \
         json_extract(attrs, '$.\"gathr.context.ask.chars\"') AS ask, \
         json_extract(attrs, '$.\"gathr.tools.off\"') AS off \
         FROM event_log WHERE name = 'invoke_agent claude' AND phase = 'start'",
    );
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0]["briefing"], briefing.chars().count(), "{rows:?}");
    assert!(rows[0]["estimate"].as_u64().unwrap() > 0, "{rows:?}");
    assert_eq!(rows[0]["ask"], "Say hi".len(), "{rows:?}");
    assert!(
        rows[0]["off"].as_str().unwrap().contains("ScheduleWakeup"),
        "{rows:?}"
    );

    // Its file goes with its engine.
    std::fs::write(&go, "").unwrap();
    until(10, "the briefing to go with its engine", || {
        (!file.exists()).then_some(())
    });
}

/// A signed-in account and an agent on it: the agent's id.
fn agent(service: &TestService, data: &Path, name: &str) -> String {
    let account = service.add_account(&format!("{name}'s"));
    sign_in_account(data, &account);
    let body = json!({ "name": name, "account": account });
    let added = service.send("POST", "/api/agents", body);
    assert_eq!(added.status, 200, "{}", added.body);
    added.json()["id"].as_str().unwrap().to_string()
}

fn send(service: &TestService, chat: &str, agent: &str, text: &str) -> Value {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

#[test]
fn an_exited_engines_token_ends_before_the_next_engine_starts() {
    let data = TempDir::new();
    let (go, args, env, version_hold) = (
        data.0.join("go"),
        data.0.join("args.jsonl"),
        data.0.join("env.jsonl"),
        data.0.join("version-hold"),
    );
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0)),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
            ("GATHR_REPLAY_VERSION_HOLD", &version_hold),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let (a, b) = (service.new_chat(), service.new_chat());
    assert_eq!(send(&service, &a, &orin, "Say hi")["status"], "running");
    assert_eq!(send(&service, &b, &orin, "Say hi")["status"], "queued");
    let first = started(&args, &env);
    let briefing = PathBuf::from(first.after("--append-system-prompt-file"));

    // The switch to chat B closes A's engine; B's start then waits on its version check.
    std::fs::write(&version_hold, "").unwrap();
    std::fs::write(&go, "").unwrap();
    until(10, "A's engine to exit", || {
        let closed = query(
            &data.0,
            "SELECT 1 FROM event_log WHERE name = 'gathr.engine.close'",
        );
        (!closed.is_empty()).then_some(())
    });
    let port = service.info.port;
    let answer = mcp(port, &first.token, "tools/list", json!({}), None);
    assert_eq!(
        answer.status, 401,
        "an exited engine's token still works: {}",
        answer.body
    );
    assert!(
        !briefing.exists(),
        "an exited engine's briefing is still there"
    );

    std::fs::remove_file(&version_hold).unwrap();
    service.wait_for(&b, 10, "chat B's turn", |h| {
        h["turns"][0]["status"] == "done"
    });
}

/// `signed_in`, held until `go` exists, with a message sent into the turn read during it.
fn read_in_turn_recording(dir: &Path, follow_up: &str) -> PathBuf {
    let rows: Vec<Value> = std::fs::read_to_string(recording("signed_in"))
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    let echo = json!({ "type": "user", "isReplay": true,
        "message": { "role": "user", "content": follow_up } });
    let mut all = rows[..2].to_vec();
    all.extend([json!({ "dir": "hold" }), json!({ "dir": "in" })]);
    all.push(json!({ "dir": "out", "line": echo.to_string() }));
    all.extend(rows[2..].iter().cloned());
    let file = dir.join("read-in-turn.jsonl");
    let text: Vec<String> = all.iter().map(Value::to_string).collect();
    std::fs::write(&file, text.join("\n")).unwrap();
    file
}

#[test]
fn a_follow_up_read_during_a_turn_counts_in_what_the_turn_was_given() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let follow_up = "x".repeat(24_000);
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &read_in_turn_recording(&data.0, &follow_up)),
            ("GATHR_REPLAY_HOLD", &go),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let chat = service.new_chat();
    assert_eq!(send(&service, &chat, &orin, "Say hi")["status"], "running");
    assert_eq!(send(&service, &chat, &orin, &follow_up)["steered"], true);
    std::fs::write(&go, "").unwrap();
    service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });

    let rows = query(
        &data.0,
        "SELECT json_extract(attrs, '$.\"gathr.context.ask.chars\"') AS ask, \
         json_extract(attrs, '$.\"gathr.context.ask.tokens_estimate\"') AS estimate \
         FROM event_log WHERE name = 'invoke_agent claude' AND phase = 'end'",
    );
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        rows[0]["ask"],
        6 + 24_000,
        "the follow-up isn't counted: {rows:?}"
    );
    assert_eq!(
        rows[0]["estimate"],
        (6 + 24_000_u64).div_ceil(4),
        "{rows:?}"
    );
}
