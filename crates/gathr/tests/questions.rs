//! A room chat's question to the person (0067), through a real service on Claude Code's replay stand-in:
//! `ask_person` stores it open, and the chat needs the person; the answer is stored, and
//! the asking agent's next turn opens with it.

mod common;

use common::replay::{LetGo, read_in, turns};
use common::{
    TempDir, TestService, call_tool, first_token, held_recording, sign_in_account, team, until,
};
use serde_json::{Value, json};

#[test]
fn a_question_is_stored_open_and_its_answer_starts_the_askers_turn() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let env = [
        ("GATHR_REPLAY", held_recording(&data.0, "signed_in")),
        ("GATHR_REPLAY_HOLD", go.clone()),
        ("GATHR_REPLAY_ENV", data.0.join("env.jsonl")),
    ];
    let env: Vec<(&str, &std::path::Path)> = env.iter().map(|(k, v)| (*k, v.as_path())).collect();
    let service = TestService::with_engine(&data.0, &env);
    let account = service.add_account("Room");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Mira", "account": account }),
    );
    let mira = made.json()["id"].as_str().unwrap().to_string();
    let members = json!([{ "agent": mira, "role": "supervisor" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Name the walk", "agent": mira }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);

    let token = first_token(&data.0);
    let ask = json!({ "what": "Which name?", "why": "Both fit.",
        "choices": [{ "label": "Ridge" }, { "label": "Brook", "detail": "shorter" }] });
    let asked = call_tool(service.info.port, &token, "ask_person", ask);
    assert_eq!(asked["isError"], false, "{asked}");

    // Stored open, on history; the chat needs the person.
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let question = &history["questions"][0];
    assert_eq!(
        question["question"], "Which name?\n\nBoth fit.",
        "{history}"
    );
    assert_eq!(question["choices"][1]["label"], "Brook", "{history}");
    assert_eq!(question["answered"], Value::Null, "{history}");
    let listed = service.call("GET", "/api/chats").json();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == chat.as_str());
    let row = row.unwrap();
    assert_eq!(
        (&row["needs"], &row["mark"]),
        (&json!(true), &json!("needs_you"))
    );

    // Answered: stored, the chat no longer needs the person, and Mira's next turn opens with it.
    std::fs::write(&go, "").unwrap();
    let path = format!("/api/questions/{}/answer", question["id"]);
    let answered = service.send("POST", &path, json!({ "pick": "Brook", "words": "Go." }));
    assert_eq!(answered.status, 200, "{}", answered.body);
    let turn = answered.json()["turn"].clone();
    let history = service.wait_for(&chat, 10, "the answer's turn", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns.iter().any(|t| t["id"] == turn)
    });
    let question = &history["questions"][0];
    assert_eq!(
        (&question["pick"], &question["words"]),
        (&json!("Brook"), &json!("Go."))
    );
    assert!(question["answered"].is_string(), "{history}");
    let opened = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == turn);
    let opened = opened.unwrap();
    assert_eq!(
        (&opened["agent"], &opened["opened"]),
        (&json!(mira), &json!("picked"))
    );
    let messages = history["messages"].as_array().unwrap().iter();
    let ask = messages
        .into_iter()
        .find(|m| m["turn"] == turn && m["role"] == "gathr");
    let said = "The person answered your question \"Which name?\n\nBoth fit.\": Picked: Brook. Their words: Go.";
    assert_eq!(ask.unwrap()["body"], said, "{history}");
    let listed = service.call("GET", "/api/chats").json();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == chat.as_str());
    assert_eq!(row.unwrap()["needs"], false, "{listed}");
}

/// The lead's question and the person's answer reach a member whose turn opens after.
#[test]
fn a_member_hears_the_leads_question_and_the_answer() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let (service, chat, lead) = team(&data.0, &turns(&data.0, &[0], 3), &[]);
    let members = service.call("GET", &format!("/api/chats/{chat}")).json()["members"].clone();
    let members = members.as_array().unwrap().iter();
    let ann = members.into_iter().find(|m| m["name"] == "Ann").unwrap()["agent"].clone();
    let say = |agent: &str, text: &str| {
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send("POST", &path, json!({ "text": text, "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
        sent.json()["turn"].clone()
    };
    let ended = |turn: &Value| {
        service.wait_for(&chat, 30, "the turn to end", |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns
                .iter()
                .any(|t| t["id"] == *turn && t["status"] == "done")
        });
    };
    let first = say(&lead, "Name the walk");
    let ask =
        json!({ "what": "Which name?", "choices": [{ "label": "Ridge" }, { "label": "Brook" }] });
    let asked = call_tool(service.info.port, &first_token(&data.0), "ask_person", ask);
    assert_eq!(asked["isError"], false, "{asked}");
    std::fs::write(data.0.join("go"), "").unwrap();
    ended(&first);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let path = format!("/api/questions/{}/answer", history["questions"][0]["id"]);
    let answered = service.send("POST", &path, json!({ "pick": "Brook", "words": "Go." }));
    assert_eq!(answered.status, 200, "{}", answered.body);
    let answer = answered.json()["turn"].clone();
    ended(&answer);
    let lead_read = read_in(&data.0, answer.as_str().unwrap());
    assert!(
        !lead_read.contains("asked the person"),
        "the asker heard it twice: {lead_read}"
    );

    let turn = say(ann.as_str().unwrap(), "Your view?");
    let turn = turn.as_str().unwrap();
    let read = until(30, "Ann's turn to read", || {
        let read = read_in(&data.0, turn);
        read.contains("Your view?").then_some(read)
    });
    let line = "Gathr: Lead asked the person \\\"Which name?\\\". They answered: Picked: Brook. Their words: Go.";
    assert_eq!(read.matches(line).count(), 1, "{read}");
}
