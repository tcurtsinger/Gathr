//! Review regressions through the real service and recorded Codex protocol seam.
mod common;
use common::{TempDir, TestService, call_tool, first_token, text, until};
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

fn held(data: &Path) -> PathBuf {
    recording(
        data,
        vec![
            ("in", json!({"method":"turn/start"})),
            ("out", json!({"id":3,"result":{"turn":{"id":"u1"}}})),
            (
                "out",
                json!({"method":"turn/started","params":{"turn":{"id":"u1"}}}),
            ),
            ("hold", json!("")),
            (
                "out",
                json!({"method":"turn/completed","params":{"turn":{"id":"u1","status":"completed"}}}),
            ),
        ],
    )
}

fn setup(data: &Path, file: &Path, mode: &str) -> (TestService, String) {
    let replay = common::replay_engine();
    let service = TestService::with_engine(
        data,
        &[
            ("GATHR_CODEX_PATH", &replay),
            ("GATHR_REPLAY", file),
            ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
            ("GATHR_REPLAY_HOLD", &data.join("go")),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
        ],
    );
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
    let agent = service
        .send(
            "POST",
            "/api/agents",
            json!({"name":"Test","account":id,"approvals":mode}),
        )
        .json();
    let made = service.send(
        "POST",
        "/api/chats",
        json!({"members":[{"agent":agent["id"],"role":"supervisor"}]}),
    );
    (service, made.json()["id"].as_str().unwrap().to_string())
}

#[test]
fn codex_wait_checks_require_one_approval_before_the_service_saves_or_runs_them() {
    for mode in ["ask", "edits", "auto", "full"] {
        let data = TempDir::new();
        let (service, chat) = setup(&data.0, &held(&data.0), mode);
        let send = service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({"text":"Wait for the check"}),
        );
        assert_eq!(send.status, 200, "{}", send.body);
        let token = first_token(&data.0);
        let marker = data.0.join("outside-engine-folder.txt");
        let check = format!(
            "printf allowed > '{}'",
            marker.display().to_string().replace('\\', "/")
        );
        let port = service.info.port;
        let ask = move || {
            call_tool(
                port,
                &token,
                "wait",
                json!({"check":check,"then":"Continue"}),
            )
        };
        if mode == "full" {
            let answer = ask();
            assert!(text(&answer).starts_with("Waiting"), "Full asked: {answer}");
        } else {
            let request = std::thread::spawn(ask.clone());
            let history = service.wait_for(&chat, 5, "wait check to ask before it runs", |h| {
                assert!(
                    !request.is_finished(),
                    "{mode} saved a wait without asking: {h}"
                );
                !h["approvals"].as_array().unwrap().is_empty()
            });
            assert!(
                !marker.exists(),
                "{mode} ran outside its sandbox before approval"
            );
            let waits = service
                .call("GET", &format!("/api/chats/{chat}/tasks"))
                .json();
            assert!(
                waits["waits"].as_array().unwrap().is_empty(),
                "unapproved check saved: {waits}"
            );
            let approval = history["approvals"][0]["request_id"].as_str().unwrap();
            let answer = service.send(
                "POST",
                &format!("/api/chats/{chat}/approvals/{approval}"),
                json!({"allow":false}),
            );
            assert_eq!(answer.status, 200, "{}", answer.body);
            let refused = request.join().unwrap();
            assert_eq!(refused["isError"], true, "denied check accepted: {refused}");
            assert!(!marker.exists());
            let request = std::thread::spawn(ask);
            let history = service.wait_for(&chat, 5, "a second check to ask once", |h| {
                !h["approvals"].as_array().unwrap().is_empty()
            });
            let approval = history["approvals"][0]["request_id"].as_str().unwrap();
            let answer = service.send(
                "POST",
                &format!("/api/chats/{chat}/approvals/{approval}"),
                json!({"allow":true}),
            );
            assert_eq!(answer.status, 200, "{}", answer.body);
            assert!(text(&request.join().unwrap()).starts_with("Waiting"));
            until(5, "the allowed check to execute", || {
                marker.exists().then_some(())
            });
            let history = service.call("GET", &format!("/api/chats/{chat}")).json();
            assert!(
                history["approvals"].as_array().unwrap().is_empty(),
                "check asked twice: {history}"
            );
        }
        std::fs::write(data.0.join("go"), "").unwrap();
        if mode == "full" {
            until(5, "Full check to execute", || marker.exists().then_some(()));
        }
    }
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

#[test]
fn shutdown_refuses_a_wait_check_whose_approval_is_still_open() {
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
            ("in", json!({"waiting":"until stdin closes"})),
        ],
    );
    let (service, chat) = setup(&data.0, &file, "ask");
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({"text":"Wait"}),
    );
    let token = first_token(&data.0);
    let port = service.info.port;
    let request = std::thread::spawn(move || {
        call_tool(
            port,
            &token,
            "wait",
            json!({"check":"exit 0","then":"Continue"}),
        )
    });
    service.wait_for(
        &chat,
        5,
        "an open wait-check approval before shutdown",
        |h| !h["approvals"].as_array().unwrap().is_empty(),
    );
    drop(service);
    assert_eq!(
        request.join().unwrap()["isError"],
        true,
        "shutdown admitted the check"
    );
}

/// 0036 C11 (owner, task 1450): a Codex wait whose check is one known read-only command saves
/// without asking, and says which rule let it through; the same command redirected still asks.
#[test]
fn a_known_read_only_wait_check_saves_without_asking() {
    let data = TempDir::new();
    let (service, chat) = setup(&data.0, &held(&data.0), "auto");
    let path = format!("/api/chats/{chat}/messages");
    let send = service.send("POST", &path, json!({"text":"Wait for the check"}));
    assert_eq!(send.status, 200, "{}", send.body);
    let (port, token) = (service.info.port, first_token(&data.0));

    let wait = |check: &str| json!({"check": check, "then": "Continue"});
    let answer = call_tool(port, &token, "wait", wait("git log -1 --oneline"));
    assert!(text(&answer).starts_with("Waiting"), "{answer}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(history["approvals"], json!([]), "a read-only check asked");
    let query = "SELECT attrs FROM event_log WHERE name = 'gathr.approvals.check'";
    let (ok, rows) = common::gathr(&data.0, &["diag", "query", query]);
    let allowed = rows["rows"][0]["attrs"]["allowed_by"].clone();
    assert!(ok && allowed == "read-only: git log", "{rows}");

    let redirected = wait("git log -1 --oneline > out.txt");
    let asking = std::thread::spawn(move || call_tool(port, &token, "wait", redirected));
    let history = service.wait_for(&chat, 5, "the redirected check to ask", |h| {
        !h["approvals"].as_array().unwrap().is_empty()
    });
    let approval = history["approvals"][0]["request_id"].as_str().unwrap();
    let to = format!("/api/chats/{chat}/approvals/{approval}");
    assert_eq!(
        service.send("POST", &to, json!({"allow":false})).status,
        200
    );
    assert_eq!(asking.join().unwrap()["isError"], true);
    std::fs::write(data.0.join("go"), "").unwrap();
}
