//! Gathr's tools (0005) and each engine's briefing (0021, 0020): an engine gets a token of its own
//! for `/mcp`, the tools answer as its agent in its chat, the token ends with the engine, and every
//! turn records what Gathr gave it.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, gathr, held_recording, mcp, recording, sign_in_account, text,
    until,
};
use serde_json::{Value, json};

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
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
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

/// `signed_in`, held until `go` exists, with a message sent into the turn read during it; with
/// `exits`, the engine then exits with no result, as one that crashes mid-turn.
fn read_in_turn_recording(dir: &Path, follow_up: &str, exits: bool) -> PathBuf {
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
    match exits {
        true => all.push(json!({ "dir": "exit", "line": "1" })),
        false => all.extend(rows[2..].iter().cloned()),
    }
    let file = dir.join("read-in-turn.jsonl");
    let text: Vec<String> = all.iter().map(Value::to_string).collect();
    std::fs::write(&file, text.join("\n")).unwrap();
    file
}

#[test]
fn a_follow_up_read_during_a_turn_counts_in_what_the_turn_was_given_however_it_ends() {
    for (exits, ended) in [(false, "done"), (true, "failed")] {
        let data = TempDir::new();
        let go = data.0.join("go");
        let follow_up = "x".repeat(24_000);
        let service = TestService::with_engine(
            &data.0,
            &[
                (
                    "GATHR_REPLAY",
                    &read_in_turn_recording(&data.0, &follow_up, exits),
                ),
                ("GATHR_REPLAY_HOLD", &go),
            ],
        );
        let orin = agent(&service, &data.0, "Orin");
        let chat = service.new_chat();
        let sent = send(&service, &chat, &orin, "Say hi");
        assert_eq!(send(&service, &chat, &orin, &follow_up)["steered"], true);
        std::fs::write(&go, "").unwrap();
        service.wait_for(&chat, 10, "the turn's end", |h| {
            h["turns"][0]["status"] == ended
        });

        let sql = format!(
            "SELECT sum(json_extract(attrs, '$.\"gathr.context.ask.chars\"')) AS ask \
             FROM event_log WHERE ref = '{}'",
            sent["ref"].as_str().unwrap()
        );
        let rows = query(&data.0, &sql);
        assert_eq!(
            rows[0]["ask"],
            6 + 24_000,
            "exits {exits}: the follow-up isn't counted"
        );
    }
}

/// G162: Codex keeps an older revision's session open on a `GET /mcp` stream, which rmcp ends only
/// after 300 idle seconds. A stop waited for it, the listener already gone, for up to 5 minutes.
#[test]
fn a_stop_ends_an_engines_open_tool_stream_at_once() {
    use std::io::{Read, Write};
    use std::time::{Duration, Instant};

    let data = TempDir::new();
    let (args, env) = (data.0.join("args.jsonl"), data.0.join("env.jsonl"));
    let mut service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let chat = service.new_chat();
    send(&service, &chat, &orin, "Say hi");
    let token = started(&args, &env).token;
    let port = service.info.port;

    let auth = format!("Bearer {token}");
    let initialize = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {
        "protocolVersion": "2025-06-18", "capabilities": {},
        "clientInfo": { "name": "test", "version": "1.0" } } });
    let headers = [
        ("Authorization", auth.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json, text/event-stream"),
    ];
    let opened = gathr::http::post(port, "/mcp", &headers, &initialize.to_string()).unwrap();
    assert_eq!(opened.status, 200, "{}", opened.body);
    let session = opened
        .headers
        .lines()
        .find_map(|l| {
            let (name, value) = l.split_once(':')?;
            name.eq_ignore_ascii_case("mcp-session-id")
                .then(|| value.trim().to_string())
        })
        .unwrap_or_else(|| panic!("no session: {}", opened.headers));
    let mut with_session = headers.to_vec();
    with_session.push(("Mcp-Session-Id", session.as_str()));
    let initialized = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let noted = gathr::http::post(port, "/mcp", &with_session, &initialized.to_string()).unwrap();
    assert_eq!(noted.status, 202, "{}", noted.body);

    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    let get = format!(
        "GET /mcp HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: {auth}\r\n\
         Accept: text/event-stream\r\nMcp-Session-Id: {session}\r\n\r\n"
    );
    stream.write_all(get.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut head = [0u8; 12];
    stream.read_exact(&mut head).unwrap();
    assert_eq!(&head, b"HTTP/1.1 200", "the stream didn't open");

    let asked = Instant::now();
    let _ = service.call("POST", "/api/shutdown");
    while asked.elapsed() < Duration::from_secs(30) {
        if service.child.try_wait().unwrap().is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let took = asked.elapsed();
    assert!(
        took < Duration::from_secs(15),
        "the service took {took:?} to stop with a tool stream open"
    );
    drop(stream);
}

/// G176: agents end a message with "Friction: …", and `read_chat` cut every message at its first
/// 2,000 characters, so the lead's scans missed it. A long message keeps its start and its end, with
/// where it was cut and how to read the whole, which `message` still gives.
#[test]
fn a_long_message_keeps_its_end_when_read_chat_cuts_it() {
    let data = TempDir::new();
    let env = data.0.join("env.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let orin = agent(&service, &data.0, "Orin");
    let chat = service.new_chat();
    let long = format!("{}\nFriction: x", "word ".repeat(500));
    assert!(long.chars().count() > 2_500);
    send(&service, &chat, &orin, &long);
    let token = until(10, "the engine's token", || {
        let line = std::fs::read_to_string(&env).ok()?;
        let line: Value = serde_json::from_str(line.lines().next()?).ok()?;
        line["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    });
    let port = service.info.port;

    let read = text(&call_tool(port, &token, "read_chat", json!({})));
    let line = read
        .lines()
        .find(|l| l.contains("person: word"))
        .unwrap_or_default();
    assert!(read.contains("Friction: x"), "{read}");
    assert!(line.contains("characters cut"), "{line}");
    assert!(
        line.chars().count() < 2_200,
        "{} characters",
        line.chars().count()
    );
    let id = line.split(' ').next().unwrap();
    let whole = text(&call_tool(
        port,
        &token,
        "read_chat",
        json!({ "message": id }),
    ));
    assert!(whole.contains(&long), "{whole}");
}
