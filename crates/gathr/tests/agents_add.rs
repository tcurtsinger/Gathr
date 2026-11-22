//! An agent adds a tool or writes a skill, through a real service and the replay engine: a tool on
//! this PC is saved and running, one on the web that wants a key is saved without one, a skill comes
//! only from a folder the agent may write, and the engine is never allowed these two without asking.

mod common;

use std::path::{Path, PathBuf};

use axum::response::IntoResponse;
use common::{TempDir, TestService, call_tool, held_recording, sign_in_account, text, until};
use serde_json::{Value, json};

/// The made-up tool (`examples/pc_tool.rs`), built next to the replay engine.
fn pc_tool() -> PathBuf {
    let exe = format!("pc_tool{}", std::env::consts::EXE_SUFFIX);
    common::replay_engine().with_file_name(exe)
}

fn first_line(file: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(text.lines().next()?).ok()
}

/// A made-up tool on the web that refuses every request without a key with a plain 401: its link.
fn wants_a_key() -> String {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async move {
            let refuse = || async { axum::http::StatusCode::UNAUTHORIZED.into_response() };
            let app = axum::Router::new().route("/mcp", axum::routing::any(refuse));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            tx.send(listener.local_addr().unwrap().port()).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    format!("http://127.0.0.1:{}/mcp", rx.recv().unwrap())
}

fn skill(folder: &Path, name: &str, description: &str) -> PathBuf {
    let path = folder.join(name);
    std::fs::create_dir_all(path.join("examples")).unwrap();
    let text = format!("---\nname: {name}\ndescription: {description}\n---\n\nUse it.\n");
    std::fs::write(path.join("SKILL.md"), text).unwrap();
    std::fs::write(path.join("examples").join("one.md"), "feat/123-short-name").unwrap();
    path
}

#[test]
fn an_agent_adds_tools_and_skills_and_its_engine_asks_first_as_its_approvals_say() {
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
    let account = service.add_account("Ann's");
    sign_in_account(&data.0, &account);
    let ann = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ann", "account": account }),
    );
    let ann = ann.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let message = json!({ "text": "Add the notes tool", "agent": ann });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), message);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let given: Vec<String> =
        serde_json::from_value(until(10, "the engine's start", || first_line(&args))).unwrap();
    let token = until(10, "the engine's token", || {
        first_line(&env)?["GATHR_AGENT_TOKEN"]
            .as_str()
            .map(String::from)
    });

    // Gathr's other tools are allowed without asking; these two are asked about as the mode says.
    let at = given.iter().position(|a| a == "--allowedTools").unwrap();
    let allowed: Vec<&str> = given[at + 1].split(',').collect();
    assert!(allowed.contains(&"mcp__gathr__note"), "{allowed:?}");
    for asked in ["mcp__gathr__add_tool", "mcp__gathr__add_skill"] {
        assert!(
            !allowed.contains(&asked),
            "{asked} is allowed without asking"
        );
    }

    // A tool on this PC, its secret left for the person: saved, and running.
    let notes = json!({ "command": pc_tool(), "args": [r"D:\Notes"], "env": { "NOTES_KEY": "" } });
    let command = json!({ "mcpServers": { "Notes": notes } }).to_string();
    let said = call_tool(port, &token, "add_tool", json!({ "command": command }));
    let said = text(&said);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines[..2], ["Added the tool:", "Notes"], "{said}");
    let notes_id = lines[2].strip_prefix("id ").unwrap().to_string();
    assert!(lines[3].contains("a value for NOTES_KEY"), "{said}");
    assert!(said.contains("It's running"), "{said}");
    let again = call_tool(port, &token, "add_tool", json!({ "command": command }));
    assert_eq!(again["isError"], true, "{again}");

    // A tool on the web that wants a key: saved without one, and it says so.
    let link = wants_a_key();
    let said = text(&call_tool(
        port,
        &token,
        "add_tool",
        json!({ "link": link }),
    ));
    assert!(said.contains("It needs a key"), "{said}");
    let again = call_tool(port, &token, "add_tool", json!({ "link": link }));
    assert_eq!(again["isError"], true, "{again}");
    let tools = service.call("GET", "/api/tools").json()["tools"].clone();
    let state = |id: &str| {
        let tools = tools.as_array().unwrap().iter();
        tools
            .clone()
            .find(|t| t["id"] == id || t["link"] == id)
            .unwrap()["state"]
            .clone()
    };
    assert_eq!(state(&notes_id), "connected", "{tools}");
    assert_eq!(state(&link), "needs_key", "{tools}");

    // A skill from a folder in Ann's own folder; one from anywhere else is refused.
    let own = until(10, "Ann's folder", || {
        let found = std::fs::read_dir(gathr::folders::real(&gathr::folders::beside(&data.0)))
            .ok()?
            .flatten()
            .map(|e| e.path().join("Ann"))
            .find(|p| p.is_dir());
        found
    });
    let written = skill(&own, "branch-names", "How we name branches");
    let folder = json!({ "folder": written });
    let said = text(&call_tool(port, &token, "add_skill", folder.clone()));
    assert!(
        said.starts_with("Wrote the skill:\nbranch-names\n"),
        "{said}"
    );
    let skills = service.call("GET", "/api/skills").json();
    assert_eq!(skills["skills"][0]["name"], "branch-names", "{skills}");
    assert_eq!(skills["skills"][0]["description"], "How we name branches");
    let taken = call_tool(port, &token, "add_skill", folder);
    assert_eq!(taken["isError"], true, "{taken}");
    let replaced = json!({ "folder": written, "replace": true });
    let said = text(&call_tool(port, &token, "add_skill", replaced));
    assert!(said.starts_with("Changed the skill:"), "{said}");
    let outside = skill(
        &data.0.join("elsewhere"),
        "outside-skill",
        "Not Ann's to give",
    );
    let refused = call_tool(port, &token, "add_skill", json!({ "folder": outside }));
    assert_eq!(refused["isError"], true, "{refused}");
    let skills = service.call("GET", "/api/skills").json();
    assert_eq!(
        skills["skills"].as_array().map(Vec::len),
        Some(1),
        "{skills}"
    );
}
