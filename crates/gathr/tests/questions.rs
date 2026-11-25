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

/// Several questions in one `ask_person` call: one card with each part, refused an answer that skips one, and
/// answered whole it opens one turn of the asker's with every answer.
#[test]
fn several_questions_are_one_card_answered_in_one_turn() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let (service, chat, lead) = team(&data.0, &turns(&data.0, &[0], 2), &[]);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Set up computer use", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let token = first_token(&data.0);
    let one = json!({ "questions": [{ "topic": "Cursor", "what": "Draw it?" }] });
    let refused = call_tool(service.info.port, &token, "ask_person", one);
    assert_eq!(refused["isError"], true, "{refused}");
    let ask = json!({ "what": "Computer use", "questions": [
        { "topic": "Cursor", "what": "Draw each agent's cursor?", "why": "Your mouse never moves.",
          "choices": [{ "label": "Draw it" }, { "label": "Don't" }] },
        { "topic": "Clipboard", "what": "May agents use your clipboard?",
          "choices": [{ "label": "No" }, { "label": "Allow it" }] },
    ] });
    let asked = call_tool(service.info.port, &token, "ask_person", ask);
    assert_eq!(asked["isError"], false, "{asked}");

    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let question = &history["questions"][0];
    assert_eq!(
        question["question"],
        "Computer use\n\n- Cursor: Draw each agent's cursor?\n- Clipboard: May agents use your clipboard?",
        "{history}"
    );
    let parts = &question["parts"];
    assert_eq!(
        (&parts[0]["question"], &parts[1]["choices"][1]["label"]),
        (
            &json!("Draw each agent's cursor?\n\nYour mouse never moves."),
            &json!("Allow it")
        ),
        "{history}"
    );

    let path = format!("/api/questions/{}/answer", question["id"]);
    let alone = service.send("POST", &path, json!({ "pick": "Draw it" }));
    assert_eq!(alone.status, 409, "{}", alone.body);
    let skipped = json!({ "answers": [{ "pick": "Draw it" }, {}] });
    let skipped = service.send("POST", &path, skipped);
    assert_eq!(skipped.status, 409, "{}", skipped.body);

    std::fs::write(data.0.join("go"), "").unwrap();
    let answers =
        json!({ "answers": [{ "pick": "Draw it" }, { "pick": "No", "words": "Not yet." }] });
    let answered = service.send("POST", &path, answers);
    assert_eq!(answered.status, 200, "{}", answered.body);
    let turn = answered.json()["turn"].clone();
    let history = service.wait_for(&chat, 10, "the answer's turn", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns.iter().any(|t| t["id"] == turn)
    });
    let question = &history["questions"][0];
    assert_eq!(
        question["answers"],
        json!([{ "pick": "Draw it", "words": null }, { "pick": "No", "words": "Not yet." }]),
        "{history}"
    );
    let messages = history["messages"].as_array().unwrap().iter();
    let ask = messages
        .into_iter()
        .find(|m| m["turn"] == turn && m["role"] == "gathr");
    let said = "The person answered your questions \"Computer use\":\n\
        - Cursor, \"Draw each agent's cursor?\n\nYour mouse never moves.\": Picked: Draw it\n\
        - Clipboard, \"May agents use your clipboard?\": Picked: No. Their words: Not yet.";
    assert_eq!(ask.unwrap()["body"], said, "{history}");
}

/// Archiving closes an open question unanswered: it's refused an answer, and Restore brings the
/// chat back without needing the person.
#[test]
fn archiving_closes_an_open_question_and_restore_leaves_it_closed() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let (service, chat, lead) = team(&data.0, &turns(&data.0, &[0], 1), &[]);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Name the walk", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let ask = json!({ "what": "Which name?", "choices": [{ "label": "Ridge" }] });
    let asked = call_tool(service.info.port, &first_token(&data.0), "ask_person", ask);
    assert_eq!(asked["isError"], false, "{asked}");
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 30, "the turn to end", |h| {
        h["turns"][0]["status"] == "done"
    });

    let archived = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let question = &history["questions"][0];
    assert!(question["closed"].is_string(), "{history}");
    assert_eq!(question["answered"], Value::Null, "{history}");
    let path = format!("/api/questions/{}/answer", question["id"]);
    let refused = service.send("POST", &path, json!({ "pick": "Ridge" }));
    assert_eq!(refused.status, 409, "{}", refused.body);
    let turns = service.call("GET", &format!("/api/chats/{chat}")).json()["turns"].clone();
    assert_eq!(
        turns.as_array().unwrap().len(),
        1,
        "no turn for the refused answer: {turns}"
    );

    let restored = service.call("POST", &format!("/api/chats/{chat}/unarchive"));
    assert_eq!(restored.status, 200, "{}", restored.body);
    let listed = service.call("GET", "/api/chats").json();
    let row = listed
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == chat.as_str());
    let row = row.unwrap();
    assert_eq!(
        (&row["needs"], &row["mark"]),
        (&json!(false), &Value::Null),
        "{listed}"
    );
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert!(history["questions"][0]["closed"].is_string(), "{history}");
}

/// A question's files are stored as the lines a person's message names its files with; a file
/// outside the chat's folders is refused, and nothing is asked.
#[test]
fn a_question_carries_files_of_the_chat_only() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &turns(&data.0, &[0], 1), &[]);
    // Let go before the service stops, so it doesn't wait on the held turn.
    let _go = LetGo(data.0.join("go"));
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Pick a page", "agent": lead }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let auth = format!("Bearer {}", service.info.token);
    let path = format!("/api/chats/{chat}/files?name=page.html");
    let auth = [("Authorization", auth.as_str())];
    let saved = gathr::http::post(service.info.port, &path, &auth, "<p>A</p>").unwrap();
    assert_eq!(saved.status, 200, "{}", saved.body);
    let saved: Value = serde_json::from_str(&saved.body).unwrap();
    let elsewhere = data.0.join("elsewhere.md");
    std::fs::write(&elsewhere, "out").unwrap();

    let token = first_token(&data.0);
    let ask = |file: &str| json!({ "what": "Which page?", "files": [file] });
    let out = ask(&elsewhere.to_string_lossy());
    let refused = call_tool(service.info.port, &token, "ask_person", out);
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        refused
            .to_string()
            .contains("isn't a file in this chat's folders"),
        "{refused}"
    );
    let asked = call_tool(
        service.info.port,
        &token,
        "ask_person",
        ask(saved["path"].as_str().unwrap()),
    );
    assert_eq!(asked["isError"], false, "{asked}");

    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let questions = history["questions"].as_array().unwrap();
    assert_eq!(questions.len(), 1, "{history}");
    let stored = questions[0]["question"].as_str().unwrap();
    assert!(
        stored.starts_with("Which page?\n\nAttached: ") && stored.ends_with("page.html"),
        "{stored}"
    );
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
