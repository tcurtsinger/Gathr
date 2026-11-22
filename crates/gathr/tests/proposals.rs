//! A project the person's assistant proposes (`propose_project`) is made from its card once: a second Make is refused,
//! and no second project, chat or turn is made, so the person's tokens are never spent twice.

mod common;

use common::{
    TempDir, TestService, call_tool, first_token, held_recording, project, rows, sign_in_account,
    text,
};
use serde_json::json;

#[test]
fn a_proposal_is_made_once() {
    let data = TempDir::new();
    let repos = TempDir::new();
    let (folder, _) = project(&repos.0);
    let go = data.0.join("go");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ENV", &data.0.join("env.jsonl")),
        ],
    );
    let account = service.add_account("Team");
    sign_in_account(&data.0, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (mira, ann, _ben) = (agent("Mira"), agent("Ann"), agent("Ben"));
    let chat = service.send("POST", "/api/chats", json!({ "assistant": true }));
    let chat = chat.json()["id"].as_str().unwrap().to_string();
    let message = json!({ "text": "Start a project for the garden app" });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), message);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let token = first_token(&data.0);

    let ask = json!({ "name": "Garden app", "folder": folder, "team": ["Ann", "Ben"], "lead": "Ann",
        "chat": "Planting calendar", "brief": "Build a planting calendar for the raised beds." });
    let said = call_tool(service.info.port, &token, "propose_project", ask);
    assert!(text(&said).contains("card is shown"), "{said}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().unwrap();
    let block = messages
        .iter()
        .find(|m| m["kind"] == "block")
        .expect("the proposal");
    let make = format!("/api/proposals/{}/make", block["id"].as_str().unwrap());

    let made = service.send("POST", &make, json!({}));
    assert_eq!(made.status, 200, "{}", made.body);
    let made = made.json();
    assert!(
        made["message"]["body"]["project"]["made"].is_object(),
        "{made}"
    );
    let again = service.send("POST", &make, json!({}));
    assert_eq!(again.status, 409, "{}", again.body);

    let count = |sql: &str| rows(&data.0, sql)[0]["n"].as_i64().unwrap();
    assert_eq!(count("SELECT count(*) AS n FROM projects"), 1);
    assert_eq!(
        count("SELECT count(*) AS n FROM chats WHERE project_id IS NOT NULL"),
        1
    );
    assert_eq!(
        count("SELECT count(*) AS n FROM turns WHERE opened_by = 'brief'"),
        1
    );
    // The brief is the person's message in the project's chat, to its lead, from the assistant that wrote it.
    let project_chat = made["chat"].as_str().unwrap();
    let history = service
        .call("GET", &format!("/api/chats/{project_chat}"))
        .json();
    let brief = &history["messages"][0];
    assert_eq!(brief["role"], "user", "{history}");
    assert_eq!(brief["from"], mira.as_str(), "{history}");
    assert_eq!(history["turns"][0]["agent"], ann.as_str(), "{history}");
    std::fs::write(&go, "").unwrap();
}
