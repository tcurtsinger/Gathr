//! A room chat's question to the person (0067), through a real service on Claude Code's replay stand-in:
//! `ask_person` stores it open, and the chat needs the person; the answer is stored, and
//! the asking agent's next turn opens with it.

mod common;

use common::{TempDir, TestService, call_tool, first_token, held_recording, sign_in_account};
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
