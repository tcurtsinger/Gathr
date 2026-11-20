//! Review regressions through the real service and recorded Codex protocol seam.
mod common;
use common::{TempDir, TestService, first_token};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

fn recording(data: &Path, rows: Vec<(&str, Value)>) -> PathBuf {
    let file = data.join("codex.jsonl");
    let text = rows
        .into_iter()
        .map(|(dir, line)| json!({"dir":dir,"line":line.to_string()}).to_string())
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(&file, text).unwrap();
    file
}

/// A service whose Codex is the stand-in playing `file`, with `more` in its environment, and a
/// signed-in Codex account's id.
fn codex(data: &Path, file: &Path, more: &[(&str, &Path)]) -> (TestService, String) {
    let replay = common::replay_engine();
    let (tokens, go) = (data.join("env.jsonl"), data.join("go"));
    let mut env = vec![
        ("GATHR_CODEX_PATH", replay.as_path()),
        ("GATHR_REPLAY", file),
        ("GATHR_REPLAY_ENV", tokens.as_path()),
        ("GATHR_REPLAY_HOLD", go.as_path()),
        ("GATHR_WAIT_EVERY_MS", Path::new("100")),
    ];
    env.extend_from_slice(more);
    let service = TestService::with_engine(data, &env);
    let account = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex","name":"Test"}),
        )
        .json();
    let id = account["id"].as_str().unwrap();
    let folder = data.join("accounts").join(id).join("codex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("auth.json"),
        r#"{"tokens":{"refresh_token":"made-up"}}"#,
    )
    .unwrap();
    (service, id.to_string())
}

fn agent(service: &TestService, name: &str, account: &str, mode: &str) -> Value {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({"name":name,"account":account,"approvals":mode}),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].clone()
}

fn setup(data: &Path, file: &Path, mode: &str) -> (TestService, String) {
    let (service, account) = codex(data, file, &[]);
    let agent = json!({ "id": agent(&service, "Test", &account, mode) });
    let made = service.send(
        "POST",
        "/api/chats",
        json!({"members":[{"agent":agent["id"],"role":"supervisor"}]}),
    );
    (service, made.json()["id"].as_str().unwrap().to_string())
}

#[test]
fn a_rejected_start_recovers_held_messages_and_allows_a_later_send() {
    let data = TempDir::new();
    let file = recording(
        &data.0,
        vec![
            ("in", json!({"method":"turn/start"})),
            ("hold", json!("")),
            (
                "out",
                json!({"id":3,"error":{"code":-32000,"message":"Turn start refused by test engine"}}),
            ),
        ],
    );
    let (service, chat) = setup(&data.0, &file, "edits");
    let path = format!("/api/chats/{chat}/messages");
    assert_eq!(
        service.send("POST", &path, json!({"text":"First"})).status,
        200
    );
    first_token(&data.0);
    // The recording holds after turn/start was written, before its response.
    let sent = service.send("POST", &path, json!({"text":"While starting"}));
    assert_eq!(sent.json()["steered"], true, "{}", sent.body);
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 5, "both rejected starts to settle", |h| {
        h["turns"].as_array().unwrap().len() == 2
            && h["turns"]
                .as_array()
                .unwrap()
                .iter()
                .all(|t| t["status"] == "failed")
    });
    let text: Vec<_> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| m["body"].as_str().unwrap())
        .collect();
    assert_eq!(
        text,
        ["First", "While starting"],
        "message lost or duplicated: {history}"
    );
    assert_eq!(
        service.send("POST", &path, json!({"text":"Later"})).status,
        200
    );
    service.wait_for(&chat, 5, "a later send to settle after rejection", |h| {
        h["turns"].as_array().unwrap().len() == 3 && h["turns"][2]["status"] == "failed"
    });
}

#[test]
fn a_steer_rejected_after_completion_is_requeued_and_does_not_block_later_turns() {
    let data = TempDir::new();
    let file = recording(
        &data.0,
        vec![
            ("in", json!({"method":"turn/start"})),
            ("out", json!({"id":3,"result":{"turn":{"id":"u1"}}})),
            (
                "out",
                json!({"method":"turn/started","params":{"turn":{"id":"u1"}}}),
            ),
            (
                "out",
                json!({"method":"item/agentMessage/delta","params":{"delta":"Working"}}),
            ),
            ("hold", json!("")),
            (
                "out",
                json!({"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}),
            ),
            ("in", json!({"method":"turn/steer"})),
            (
                "out",
                json!({"id":4,"error":{"code":-32000,"message":"expectedTurnId no longer matches the active turn"}}),
            ),
        ],
    );
    let (service, chat) = setup(&data.0, &file, "edits");
    let path = format!("/api/chats/{chat}/messages");
    assert_eq!(
        service.send("POST", &path, json!({"text":"First"})).status,
        200
    );
    service.wait_for(
        &chat,
        5,
        "the first turn to be streaming before steering",
        |h| {
            h["streaming"]
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["text"].as_str().is_some_and(|s| s.contains("Working")))
        },
    );
    // The first engine already loaded its rows. A recovered session gets successful turns.
    recording(
        &data.0,
        vec![
            ("in", json!({"method":"turn/start"})),
            ("out", json!({"id":3,"result":{"turn":{"id":"u2"}}})),
            (
                "out",
                json!({"method":"turn/started","params":{"turn":{"id":"u2"}}}),
            ),
            (
                "out",
                json!({"method":"item/agentMessage/delta","params":{"delta":"Recovered"}}),
            ),
            (
                "out",
                json!({"method":"turn/completed","params":{"turn":{"id":"u2","status":"completed"}}}),
            ),
        ],
    );
    let sent = service.send("POST", &path, json!({"text":"Second"}));
    assert_eq!(sent.json()["steered"], true, "{}", sent.body);
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(
        &chat,
        5,
        "the rejected input to finish on its recovered turn",
        |h| h["turns"].as_array().unwrap().len() == 2 && h["turns"][1]["status"] == "done",
    );
    assert_eq!(
        service.send("POST", &path, json!({"text":"Third"})).status,
        200
    );
    let history = service.wait_for(
        &chat,
        5,
        "rejected steering and the later send to finish",
        |h| {
            h["turns"].as_array().unwrap().len() == 3
                && h["turns"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|t| t["status"] == "done")
        },
    );
    let users: Vec<_> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "user")
        .map(|m| m["body"].as_str().unwrap())
        .collect();
    assert_eq!(
        users,
        ["First", "Second", "Third"],
        "input was lost/duplicated: {history}"
    );
}
