//! The Changes tab's route through a real service: in a chat with no project, what an agent wrote
//! with its file tools in a git checkout elsewhere shows as that checkout's uncommitted change of
//! those files only; the person's own changes there don't, nor anything in the data folder.

mod common;

use std::path::Path;

use common::{TempDir, TestService, git};
use gathr::folders::real;
use serde_json::{Value, json};

/// A turn of `agent` that ended, with one step of `tool` naming each of `paths`, written straight
/// into the chat as an engine's turn would have left it.
fn wrote(data: &Path, chat: &str, agent: &str, steps: &[(&str, String)]) {
    let conn = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let ts = gathr::ids::now_us();
    conn.execute(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, ended_ts, agent_id, position)
         VALUES ('turn-wrote', ?1, 'seedtrace', 'done', ?2, ?2, ?3, 1)",
        rusqlite::params![chat, ts, agent],
    )
    .unwrap();
    for (n, (tool, text)) in steps.iter().enumerate() {
        let body = json!({ "tool": tool, "status": "ok", "input": { "text": text } });
        conn.execute(
            "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body)
             VALUES (?1, ?2, 'turn-wrote', ?3, 'assistant', 'tool', ?4)",
            rusqlite::params![format!("step{n}"), chat, ts + n as i64, body.to_string()],
        )
        .unwrap();
    }
}

/// A git checkout at `dir` with `files` committed.
fn checkout(dir: &Path, files: &[&str]) {
    std::fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    for name in files {
        std::fs::write(dir.join(name), "one\ntwo\n").unwrap();
    }
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "start"]);
}

#[test]
fn an_agents_writes_in_a_checkout_elsewhere_show_and_nothing_else_does() {
    let (data, elsewhere) = (TempDir::new(), TempDir::new());
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ziggy", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": agent, "role": "supervisor" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let changes = format!("/api/chats/{chat}/changes");
    let none = service.call("GET", &changes).json();
    assert_eq!(
        none,
        json!({ "agents": [], "more": false }),
        "nothing written yet"
    );

    let mcp = elsewhere.0.join("mcp");
    checkout(&mcp, &["a.json", "b.json", "c.json", "d.json"]);
    // The agent changes a.json, writes new.json, deletes c.json, and writes d.json as it was.
    std::fs::write(mcp.join("a.json"), "one\n2\n").unwrap();
    std::fs::write(mcp.join("new.json"), "x\n").unwrap();
    std::fs::remove_file(mcp.join("c.json")).unwrap();
    // The person's own: b.json changed and mine.json new, neither written by the agent.
    std::fs::write(mcp.join("b.json"), "mine\n").unwrap();
    std::fs::write(mcp.join("mine.json"), "mine\n").unwrap();
    // A checkout in the data folder, changed where the agent wrote.
    let inner = data.0.join("inner");
    checkout(&inner, &["x.json"]);
    std::fs::write(inner.join("x.json"), "changed\n").unwrap();
    let at = |dir: &Path, name: &str| dir.join(name).to_string_lossy().into_owned();
    wrote(
        &data.0,
        &chat,
        &agent,
        &[
            ("Edit", at(&mcp, "a.json")),
            // Codex's file change names its first file only.
            ("Edit", format!("{} +1 more", at(&mcp, "new.json"))),
            ("Edit", at(&mcp, "c.json")),
            ("Write", at(&mcp, "d.json")),
            ("Edit", at(&inner, "x.json")),
            ("Write", "relative.json".into()),
        ],
    );

    let got = service.call("GET", &changes);
    assert_eq!(got.status, 200, "{}", got.body);
    let got = got.json();
    let ziggy = &got["agents"][0];
    assert_eq!(got["agents"].as_array().unwrap().len(), 1, "{got}");
    assert_eq!(
        (&ziggy["id"], &ziggy["branch"]),
        (&json!(agent), &Value::Null)
    );
    let outside = ziggy["outside"].as_array().unwrap();
    assert_eq!(
        outside.len(),
        1,
        "only the checkout outside the data folder: {got}"
    );
    let folder = outside[0]["folder"].as_str().unwrap();
    assert_eq!(Path::new(folder), real(&mcp));
    let files: Vec<_> = outside[0]["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| (f["path"].as_str().unwrap(), f["kind"].as_str().unwrap()))
        .collect();
    assert_eq!(
        files,
        [
            ("a.json", "changed"),
            ("c.json", "deleted"),
            ("new.json", "new")
        ],
        "{got}"
    );

    let change = |path: &str, folder: &str| {
        let q = format!("agent={agent}&path={path}&folder={}", encoded(folder));
        service.call("GET", &format!("/api/chats/{chat}/change?{q}"))
    };
    let lines = change("a.json", folder);
    assert_eq!(lines.status, 200, "{}", lines.body);
    let kinds: Vec<_> = lines.json()["hunks"][0]["lines"]
        .as_array()
        .unwrap()
        .iter()
        .map(|l| (l["kind"].clone(), l["text"].clone()))
        .collect();
    assert_eq!(
        kinds,
        [
            (json!("context"), json!("one")),
            (json!("removed"), json!("two")),
            (json!("added"), json!("2")),
        ]
    );
    assert_eq!(
        change("b.json", folder).status,
        404,
        "the person's own file"
    );
    let inner = real(&inner).to_string_lossy().into_owned();
    assert_eq!(change("x.json", &inner).status, 404, "the data folder");
}

/// `text` as a URL's query value.
fn encoded(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (b as char).to_string()
            }
            b => format!("%{b:02X}"),
        })
        .collect()
}
