//! The Changes tab's route through a real service: in a chat with no project, what an agent wrote
//! with its file tools in a git checkout elsewhere shows as that checkout's uncommitted change of
//! those files only; the person's own changes there don't, nor anything in the data folder. A file
//! in no checkout shows from the steps that changed it.

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
        json!({ "name": "Rook", "account": account }),
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
    let rook = &got["agents"][0];
    assert_eq!(got["agents"].as_array().unwrap().len(), 1, "{got}");
    assert_eq!(
        (&rook["id"], &rook["branch"]),
        (&json!(agent), &Value::Null)
    );
    let outside = rook["outside"].as_array().unwrap();
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

/// A turn `turn` of `agent` that ended with one step that changed `files`, as `changed` in a step's
/// body keeps them.
fn changed(data: &Path, chat: &str, agent: &str, turn: &str, files: Value) {
    let conn = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    let ts = gathr::ids::now_us();
    conn.execute(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, ended_ts, agent_id, position)
         VALUES (?1, ?2, 'seedtrace', 'done', ?3, ?3, ?4,
             (SELECT coalesce(max(position), 0) + 1 FROM turns WHERE chat_id = ?2))",
        rusqlite::params![turn, chat, ts, agent],
    )
    .unwrap();
    let body = json!({ "tool": "Edit", "status": "ok", "changed": files });
    conn.execute(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body)
         VALUES (?1, ?2, ?1, ?3, 'assistant', 'tool', ?4)",
        rusqlite::params![turn, chat, ts, body.to_string()],
    )
    .unwrap();
}

#[test]
fn a_file_in_no_checkout_shows_from_its_steps() {
    let (data, loose) = (TempDir::new(), TempDir::new());
    let service = TestService::start(&data.0);
    let account = service.add_account("Work");
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (rook, wren) = (agent("Rook"), agent("Wren"));
    let members =
        json!([{ "agent": rook, "role": "supervisor" }, { "agent": wren, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();

    // Rook's engine runs in `work`: its files there read from there; `other` is a folder elsewhere.
    let (work, other) = (loose.0.join("work"), loose.0.join("other"));
    std::fs::create_dir_all(&work).unwrap();
    std::fs::create_dir_all(&other).unwrap();
    let at = |dir: &Path, name: &str| dir.join(name).to_string_lossy().into_owned();
    let own = |name: &str, added: u64, removed: u64, new: bool, diff: &str| {
        json!({ "path": name, "full": at(&work, name), "added": added, "removed": removed,
            "new": new, "cut": false, "diff": diff })
    };
    changed(
        &data.0,
        &chat,
        &rook,
        "t1",
        json!([own("plan.md", 2, 0, true, "@@ -0,0 +1,2 @@\n+one\n+two\n")]),
    );
    changed(
        &data.0,
        &chat,
        &rook,
        "t2",
        json!([own("plan.md", 1, 1, false, "@@ -2,1 +2,1 @@\n-two\n+2\n")]),
    );
    changed(
        &data.0,
        &chat,
        &rook,
        "t3",
        json!([own("plan.md", 1, 1, false, "@@ -2,1 +2,1 @@\n-2\n+II\n")]),
    );
    let elsewhere = json!({ "path": at(&other, "list.md"), "added": 1, "removed": 0,
        "new": true, "cut": false, "diff": "@@ -0,0 +1,1 @@\n+item\n" });
    changed(&data.0, &chat, &rook, "t4", json!([elsewhere]));
    let theirs = json!({ "path": at(&other, "theirs.md"), "added": 1, "removed": 0,
        "new": true, "cut": false, "diff": "@@ -0,0 +1,1 @@\n+mine\n" });
    changed(&data.0, &chat, &wren, "t5", json!([theirs]));
    let stray = json!({ "path": at(&data.0, "stray.md"), "added": 1, "removed": 0,
        "new": true, "cut": false, "diff": "@@ -0,0 +1,1 @@\n+no\n" });
    changed(&data.0, &chat, &rook, "t6", json!([stray]));

    let got = service.call("GET", &format!("/api/chats/{chat}/changes"));
    assert_eq!(got.status, 200, "{}", got.body);
    let got = got.json();
    let first = &got["agents"][0];
    assert_eq!(first["id"], json!(rook), "{got}");
    let files = |list: &Value| -> Vec<(String, u64, u64, String)> {
        let each = list.as_array().unwrap().iter().map(|f| {
            let n = |k: &str| f[k].as_u64().unwrap();
            let s = |k: &str| f[k].as_str().unwrap().to_string();
            (s("path"), n("added"), n("removed"), s("kind"))
        });
        each.collect()
    };
    let plan = ("plan.md".to_string(), 4, 2, "new".to_string());
    assert_eq!(files(&first["files"]), [plan], "{got}");
    let outside = first["outside"].as_array().unwrap();
    assert_eq!(outside.len(), 1, "not the data folder: {got}");
    let folder = outside[0]["folder"].as_str().unwrap().to_string();
    assert_eq!(Path::new(&folder), real(&other));
    let list = ("list.md".to_string(), 1, 0, "new".to_string());
    assert_eq!(files(&outside[0]["files"]), [list]);

    let change = |agent: &str, path: &str, folder: Option<&str>| {
        let folder = folder.map_or(String::new(), |f| format!("&folder={}", encoded(f)));
        let q = format!("agent={agent}&path={path}{folder}");
        service.call("GET", &format!("/api/chats/{chat}/change?{q}"))
    };
    let lines = change(&rook, "plan.md", None);
    assert_eq!(lines.status, 200, "{}", lines.body);
    let hunks: Vec<Vec<(String, String)>> = lines.json()["hunks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| {
            let each = h["lines"].as_array().unwrap().iter();
            let line = |l: &Value| {
                (
                    l["kind"].as_str().unwrap().into(),
                    l["text"].as_str().unwrap().into(),
                )
            };
            each.map(line).collect()
        })
        .collect();
    let line = |kind: &str, text: &str| (kind.to_string(), text.to_string());
    assert_eq!(
        hunks,
        [
            vec![line("added", "one"), line("added", "two")],
            vec![line("removed", "two"), line("added", "2")],
            vec![line("removed", "2"), line("added", "II")],
        ],
        "each step's hunks, the first first"
    );
    assert_eq!(change(&rook, "list.md", Some(&folder)).status, 200);
    assert_eq!(
        change(&rook, "theirs.md", Some(&folder)).status,
        404,
        "Wren's file"
    );
    assert_eq!(change(&wren, "theirs.md", Some(&folder)).status, 200);
    let data_folder = real(&data.0).to_string_lossy().into_owned();
    assert_eq!(
        change(&rook, "stray.md", Some(&data_folder)).status,
        404,
        "the data folder"
    );
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
