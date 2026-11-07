//! What the window reads and writes beyond chats: chat names, a chat's turn timelines for
//! Diagnostics, and its own errors and timings into the event record (0012, 0019).

mod common;

use std::time::Duration;

use common::{TempDir, TestService, gathr, recording, replay_engine, sign_in};
use serde_json::{Value, json};

/// The sidebar lists chats by name: a new chat takes its first message's first line.
#[test]
fn a_chat_is_named_after_its_first_message_and_keeps_that_name() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    // No engine at this path: the send fails, but the message is kept, which is all this needs.
    let missing = data.0.join("no-such-claude.exe");
    service.send("PUT", "/api/engines/claude", json!({ "path": missing }));
    let say = |chat: &str, text: &str| {
        service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({ "text": text }),
        );
    };
    let short = service.new_chat();
    say(
        &short,
        "
  Summarise the release notes for 2.4  
and list the fixes",
    );
    say(&short, "Now shorten it");
    let long = service.new_chat();
    say(&long, &"river ".repeat(30));

    let chats = service.call("GET", "/api/chats").json();
    let title = |id: &str| {
        let chat = chats
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap();
        chat["title"].as_str().unwrap().to_string()
    };
    assert_eq!(title(&short), "Summarise the release notes for 2.4");
    let cut = title(&long);
    assert!(cut.chars().count() <= 60 && cut.ends_with('…'), "{cut:?}");
}

/// A window that opens the chat after Claude Code asked (a switch back, a reload) still gets to answer.
#[test]
fn a_chats_history_holds_its_open_approvals_until_they_are_answered() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::start_with(&data.0, &[("GATHR_REPLAY", &recording("approval"))]);
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    let text = "Create a file named note.txt in the current folder containing the word hi. Use the Write tool.";
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": text }),
    );

    let asked = service.wait_for(&chat, 10, "the approval", |h| {
        h["approvals"][0]["tool"] == "Write"
    });
    let request = asked["approvals"][0]["request_id"].as_str().unwrap();
    let answer = service.send(
        "POST",
        &format!("/api/chats/{chat}/approvals/{request}"),
        json!({ "allow": true }),
    );
    assert_eq!(answer.status, 200, "{}", answer.body);
    let after = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(after["approvals"], json!([]), "{after}");
}

/// Recorded live: Claude Code asked to run Write, was allowed, and answered (153 output tokens).
#[test]
fn a_chats_turns_carry_their_ask_tools_and_token_totals() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::start_with(&data.0, &[("GATHR_REPLAY", &recording("approval"))]);
    let chat = service.new_chat();
    service.send(
        "PUT",
        "/api/engines/claude",
        json!({ "path": replay_engine() }),
    );
    let text = "Create a file named note.txt in the current folder containing the word hi. Use the Write tool.";
    service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": text }),
    );
    allow_the_first_approval(&service, &chat);
    service.wait_for(&chat, 10, "the turn to end", |h| {
        h["turns"][0]["status"] == "done"
    });

    let answer = service.call("GET", &format!("/api/chats/{chat}/turns"));
    assert_eq!(answer.status, 200, "{}", answer.body);
    let turns = answer.json();
    let turn = &turns[0];
    assert_eq!(turn["n"], 1, "{turn}");
    assert_eq!(turn["status"], "done", "{turn}");
    assert!(
        turn["asked"].as_str().unwrap().starts_with("Create a file"),
        "{turn}"
    );
    let events = turn["events"].as_array().unwrap();
    let tool = events.iter().find(|e| e["name"] == "execute_tool Write");
    assert!(
        tool.is_some_and(|t| t["duration_ms"].as_f64().unwrap() > 0.0),
        "{events:?}"
    );
    let ended = events
        .iter()
        .find(|e| e["attrs"]["gen_ai.usage.output_tokens"].is_number())
        .unwrap_or_else(|| panic!("no event with the turn's token totals: {events:?}"));
    assert_eq!(ended["attrs"]["gen_ai.usage.output_tokens"], 153);
}

#[test]
fn the_windows_own_events_are_recorded_and_other_names_refused() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let event =
        json!({ "name": "gathr.ui.chat_open", "duration_ms": 12.5, "attrs": { "messages": 500 } });
    let sent = service.send("POST", "/api/ui/events", event);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let refused = service.send(
        "POST",
        "/api/ui/events",
        json!({ "name": "gathr.service.start" }),
    );
    assert_eq!(refused.status, 400, "{}", refused.body);

    let sql = "SELECT duration_us, attrs FROM event_log WHERE name = 'gathr.ui.chat_open'";
    let (ok, found) = gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{found}");
    assert_eq!(found["rows"][0]["duration_us"], 12500, "{found}");
    assert_eq!(found["rows"][0]["attrs"]["messages"], 500, "{found}");
}

fn allow_the_first_approval(service: &TestService, chat: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let approve = format!("/api/chats/{chat}/approvals/{}", request_id());
        let answer = service.send("POST", &approve, json!({ "allow": true }));
        if answer.status == 200 {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "the approval was never open: {}",
            answer.body
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn request_id() -> String {
    std::fs::read_to_string(recording("approval"))
        .unwrap()
        .lines()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter_map(|row| serde_json::from_str::<Value>(row["line"].as_str().unwrap()).ok())
        .find(|f| f["request"]["subtype"] == "can_use_tool")
        .map(|f| f["request_id"].as_str().unwrap().to_string())
        .expect("the recording asks for an approval")
}
