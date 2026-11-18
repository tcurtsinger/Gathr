//! The names the person picked from the window's `@` list are kept with their message (0028, 0072):
//! saved with a message that opens a turn and with one sent into a running turn, returned live and
//! in history, kept across a restart, and refused with a reason and a reference when they don't fit
//! the text. Spans count UTF-16 code units, as the window does.

mod common;

use common::{TempDir, TestService, gathr, held_recording, next_live, recording, team};
use serde_json::{Value, json};

// "Hi 😀 " is 6 UTF-16 units (the emoji is 2) but 9 bytes; "é" is 1 unit but 2 bytes.
const FIRST: &str = "Hi 😀 @Lead and é @Ann, read this";
const SECOND: &str = "Also ü @Ann please";

fn ann(service: &TestService, chat: &str) -> String {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history["members"].as_array().unwrap();
    let ann = members.iter().find(|m| m["name"] == "Ann").unwrap();
    ann["agent"].as_str().unwrap().to_string()
}

/// The person's messages in the chat's history, in order: their text and picks.
fn said(service: &TestService, chat: &str) -> Vec<(Value, Value)> {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().unwrap().iter();
    let mine = messages.filter(|m| m["role"] == "user");
    mine.map(|m| (m["body"].clone(), m["picks"].clone()))
        .collect()
}

/// The next live message the person sent.
fn live_said(socket: &mut std::net::TcpStream) -> Value {
    loop {
        let event = next_live(socket);
        if event["type"] == "message" && event["message"]["role"] == "user" {
            return event["message"].clone();
        }
    }
}

#[test]
fn picks_are_kept_with_a_new_turn_and_a_steered_message_live_in_history_and_after_a_restart() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &held_recording(&data.0, "signed_in"), &[]);
    let ann = ann(&service, &chat);
    let mut socket = service.live();
    let path = format!("/api/chats/{chat}/messages");

    let first_picks = json!([
        { "at": 6, "len": 5, "agent": lead },
        { "at": 18, "len": 4, "agent": ann },
    ]);
    let body = json!({ "text": FIRST, "agent": lead, "picks": first_picks });
    let sent = service.send("POST", &path, body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let live = live_said(&mut socket);
    assert_eq!(live["picks"], first_picks, "the live message: {live}");

    // Into the lead's running turn: its engine holds until `go`.
    let second_picks = json!([{ "at": 7, "len": 4, "agent": ann }]);
    let body = json!({ "text": SECOND, "agent": lead, "picks": second_picks });
    let steered = service.send("POST", &path, body);
    assert_eq!(steered.json()["steered"], true, "{}", steered.body);
    let live = live_said(&mut socket);
    assert_eq!(
        live["picks"], second_picks,
        "the live steered message: {live}"
    );

    // A message with no pick keeps none.
    let plain = json!({ "text": "And @Ann typed", "agent": lead });
    assert_eq!(service.send("POST", &path, plain).status, 200);

    let expected = vec![
        (json!(FIRST), first_picks),
        (json!(SECOND), second_picks),
        (json!("And @Ann typed"), Value::Null),
    ];
    assert_eq!(
        said(&service, &chat),
        expected,
        "history before the restart"
    );

    std::fs::write(data.0.join("go"), "").unwrap();
    service.stop();
    let again = TestService::start(&data.0);
    assert_eq!(said(&again, &chat), expected, "history after the restart");
    let replies = "SELECT count(*) AS n FROM messages WHERE role <> 'user' AND picks IS NOT NULL";
    let (_, rows) = gathr(&data.0, &["diag", "query", replies]);
    assert_eq!(
        rows["rows"][0]["n"], 0,
        "only the person's messages keep picks: {rows}"
    );
}

#[test]
fn picks_that_dont_fit_the_text_are_refused_with_a_reason_and_a_reference() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &recording("signed_in"), &[]);
    let path = format!("/api/chats/{chat}/messages");
    let pick = |at: u64, len: u64| json!({ "at": at, "len": len, "agent": lead });
    let cases = [
        // "Hi 😀 @Lead…": past the text's end.
        (vec![pick(30, 5)], "a picked name isn't inside the text"),
        (vec![pick(6, 99)], "a picked name isn't inside the text"),
        // Unit 4 is the emoji's second half.
        (vec![pick(4, 3)], "a picked name cuts a character in half"),
        (vec![pick(2, 2)], "a picked name cuts a character in half"),
        (vec![pick(0, 2)], "a picked name doesn't start with @"),
        (vec![pick(6, 1)], "a picked name has no name after @"),
        (vec![pick(6, 5), pick(9, 4)], "two picked names overlap"),
        (
            vec![json!({ "at": 6, "len": 5, "agent": "nobody" })],
            "a picked name's agent isn't known",
        ),
    ];
    for (picks, why) in cases {
        let body = json!({ "text": FIRST, "agent": lead, "picks": picks });
        let refused = service.send("POST", &path, body);
        let answer = refused.json();
        assert_eq!(refused.status, 400, "{picks:?}: {}", refused.body);
        assert_eq!(answer["why"], why, "{picks:?}: {answer}");
        let reference = answer["ref"].as_str().unwrap_or_default();
        let (found, trace) = gathr(&data.0, &["diag", "trace", reference]);
        assert!(
            found,
            "{picks:?}: the reference {reference} found nothing: {trace}"
        );
        assert!(trace.to_string().contains(why), "{picks:?}: {trace}");
    }
    assert!(
        said(&service, &chat).is_empty(),
        "a refused send saved a message"
    );
}
