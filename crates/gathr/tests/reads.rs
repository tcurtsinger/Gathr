//! Where the person left off (0077) through a real service, on Claude Code's replay stand-in: the
//! marks of what they have seen, and their folds, kept per thread, read back with the chat's history
//! and after a restart; sent live to their other windows; never moved back; refused, with a reason and
//! a reference, for a thread that isn't the chat's; and gone with the chat. Scenario 6k.

mod common;

use common::{TempDir, TestService, gathr, next_live, rows, team};
use gathr::http::Response;
use serde_json::{Value, json};

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

/// The person's message to the lead, once its turn is done: the message's id and the turn's.
fn say(service: &TestService, chat: &str, text: &str) -> (String, String) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let shown = service.wait_for(chat, 30, "the turn to end", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && t["status"] == "done")
    });
    let messages = shown["messages"].as_array().unwrap();
    let mine = messages
        .iter()
        .find(|m| m["role"] == "user" && m["body"] == text);
    (mine.unwrap()["id"].as_str().unwrap().to_string(), turn)
}

fn mark(service: &TestService, chat: &str, reads: Value) -> Response {
    service.send(
        "PUT",
        &format!("/api/chats/{chat}/reads"),
        json!({ "reads": reads }),
    )
}

/// The chat's marks as its history reads them, by thread.
fn marks(service: &TestService, chat: &str) -> Vec<Value> {
    let shown = history(service, chat);
    let mut reads = shown["reads"]
        .as_array()
        .unwrap_or_else(|| panic!("no reads in the history: {shown}"))
        .clone();
    reads.sort_by_key(|r| r["topic"].as_str().unwrap_or_default().to_string());
    reads
}

fn live_reads(socket: &mut std::net::TcpStream) -> Value {
    loop {
        let event = next_live(socket);
        if event["type"] == "reads" {
            return event;
        }
    }
}

#[test]
fn marks_and_folds_are_kept_and_read_back_after_a_restart() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::recording("signed_in"), &[]);
    let (message, turn) = say(&service, &chat, "Plan the made-up shopping list");
    // A chat nobody has looked at since 0077 has no marks: the window takes it as seen up to then.
    assert_eq!(marks(&service, &chat), Vec::<Value>::new());

    let thread = format!("m:{message}");
    let lone = format!("b:{turn}");
    let put = mark(
        &service,
        &chat,
        json!([
            { "topic": "", "seen_us": 2_000 },
            { "topic": thread, "seen_us": 1_500, "folded": true },
            { "topic": lone, "folded": false },
        ]),
    );
    assert_eq!(put.status, 200, "{}", put.body);
    let expected = vec![
        json!({ "topic": "", "seen_us": 2_000, "folded": null }),
        json!({ "topic": lone, "seen_us": 0, "folded": false }),
        json!({ "topic": thread, "seen_us": 1_500, "folded": true }),
    ];
    assert_eq!(marks(&service, &chat), expected);

    service.stop();
    let again = TestService::start(&data.0);
    assert_eq!(marks(&again, &chat), expected, "after a restart");
}

#[test]
fn a_mark_goes_live_to_the_other_windows_and_never_moves_back() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::recording("signed_in"), &[]);
    let (message, _) = say(&service, &chat, "Plan the made-up shopping list");
    let thread = format!("m:{message}");
    let (mut one, mut other) = (service.live(), service.live());

    let put = mark(
        &service,
        &chat,
        json!([{ "topic": thread, "seen_us": 5_000, "folded": true }]),
    );
    assert_eq!(put.status, 200, "{}", put.body);
    let row = json!({ "topic": thread, "seen_us": 5_000, "folded": true });
    for socket in [&mut one, &mut other] {
        let event = live_reads(socket);
        assert_eq!(
            (&event["chat"], &event["reads"]),
            (&json!(chat), &json!([row])),
            "{event}"
        );
    }

    // An older mark leaves it where it was; the fold is the person's latest.
    let back = mark(
        &service,
        &chat,
        json!([{ "topic": thread, "seen_us": 3_000, "folded": false }]),
    );
    assert_eq!(back.status, 200, "{}", back.body);
    let kept = json!({ "topic": thread, "seen_us": 5_000, "folded": false });
    assert_eq!(live_reads(&mut other)["reads"], json!([kept]));
    assert_eq!(back.json()["reads"], json!([kept]), "{}", back.body);
    assert_eq!(marks(&service, &chat), vec![kept]);
}

#[test]
fn a_mark_for_another_chats_thread_is_refused_with_a_reason_and_a_reference() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::recording("signed_in"), &[]);
    let (message, turn) = say(&service, &chat, "Plan the made-up shopping list");
    let other = service.new_chat();

    for topic in [format!("m:{message}"), format!("b:{turn}"), "t:9".into()] {
        let put = mark(
            &service,
            &other,
            json!([{ "topic": "", "seen_us": 1 }, { "topic": topic, "seen_us": 1 }]),
        );
        let why = format!("No thread {topic} in this chat.");
        assert_eq!(put.status, 409, "{}", put.body);
        let said = put.json();
        assert_eq!(
            (&said["error"], &said["why"]),
            (&json!("Marks failed"), &json!(why)),
            "{said}"
        );
        let reference = said["ref"].as_str().expect("a reference");
        let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
        let events = trace["events"].as_array().cloned().unwrap_or_default();
        let failed = events.iter().find(|e| e["name"] == "gathr.api.request");
        let failed = failed.map(|e| e["attrs"]["reason"].clone());
        assert!(ok && failed == Some(json!(why)), "{reference}: {trace}");
    }
    // Nothing of a refused write is kept, not even its good part.
    assert_eq!(marks(&service, &other), Vec::<Value>::new());
}

#[test]
fn a_chat_deleted_takes_its_marks_with_it() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::recording("signed_in"), &[]);
    let (message, _) = say(&service, &chat, "Plan the made-up shopping list");
    let put = mark(
        &service,
        &chat,
        json!([{ "topic": format!("m:{message}"), "seen_us": 1, "folded": true }]),
    );
    assert_eq!(put.status, 200, "{}", put.body);
    let archived = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let deleted = service.call("DELETE", &format!("/api/chats/{chat}"));
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(
        rows(&data.0, "SELECT chat_id FROM reads"),
        Vec::<Value>::new()
    );
}
