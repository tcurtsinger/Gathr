//! How full each agent's context is (compaction plan, K1): from the engine's own frames, on the
//! chat's members and sent live, for Claude Code and Codex alike.

mod common;

use common::{TempDir, TestService, next_live};
use serde_json::{Value, json};

/// An agent on `account` runs one turn in a team chat: the chat's id and the agent's.
fn one_turn(service: &TestService, account: &str) -> (String, String) {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Made-up", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let other = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Made-up other", "account": account }),
    );
    let other = other.json()["id"].as_str().unwrap().to_string();
    // A team chat, as Summary shows members; a plain chat lists none.
    let members =
        json!([{ "agent": agent, "role": "supervisor" }, { "agent": other, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    (chat, agent)
}

/// The live `context` update for the agent once it holds its window.
fn live_context(live: &mut std::net::TcpStream, agent: &str) -> Value {
    loop {
        let update = next_live(live);
        if update["type"] == "context"
            && update["agent"] == agent
            && !update["context"]["window"].is_null()
        {
            return update;
        }
    }
}

#[test]
fn a_claude_turn_shows_its_last_calls_fill_and_its_models_window() {
    let data = TempDir::new();
    let recording = common::recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording)]);
    let account = service.signed_in(&data.0);
    let mut live = service.live();
    let (chat, agent) = one_turn(&service, &account);

    let update = live_context(&mut live, &agent);
    assert_eq!(update["chat"], chat.as_str());
    let expected = json!({ "used": 2 + 23_627 + 1_711, "window": 1_000_000 });
    assert_eq!(update["context"], expected, "{update}");
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let member = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == agent.as_str())
        .unwrap();
    assert_eq!(member["context"], expected, "{history}");
}

/// A service whose Codex engine replays `recording`, and a signed-in Codex account's id.
fn codex(data: &std::path::Path, recording: &std::path::Path) -> (TestService, String) {
    let replay = common::replay_engine();
    let service = TestService::start_with(
        data,
        &[("GATHR_CODEX_PATH", &replay), ("GATHR_REPLAY", recording)],
    );
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "codex", "name": "Codex test" }),
    );
    let account = added.json()["id"].as_str().unwrap().to_string();
    let folder = data.join("accounts").join(&account).join("codex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("auth.json"),
        r#"{"tokens":{"refresh_token":"made-up"}}"#,
    )
    .unwrap();
    (service, account)
}

#[test]
fn a_codex_turn_shows_its_last_calls_fill_and_its_window() {
    let data = TempDir::new();
    let (service, account) = codex(&data.0, &common::recording("codex_signed_in"));
    let mut live = service.live();
    let (chat, agent) = one_turn(&service, &account);

    let update = live_context(&mut live, &agent);
    assert_eq!(update["context"]["window"], 475_000, "{update}");
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    assert_eq!(
        history["members"][0]["context"],
        json!({ "used": 16_291, "window": 475_000 }),
        "{history}"
    );
}

#[test]
fn a_compaction_says_so_in_the_chat_and_shows_the_fill_after_it() {
    let data = TempDir::new();
    let recording = common::recording("compact_claude");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording)]);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "/compact", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);

    let history = service.wait_for(&chat, 10, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    let lines: Vec<&str> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "gathr")
        .filter_map(|m| m["body"].as_str())
        .collect();
    assert_eq!(
        lines,
        ["Compacted Made-up's conversation: 23K to 3.6K"],
        "{history}"
    );
    let member = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == agent.as_str())
        .unwrap();
    assert_eq!(member["context"]["used"], 3_611, "{history}");
}

/// Codex reports a compaction's usage as a call with no input (#125's review): the saved fill stays
/// the last real call's, whether a call follows in the turn or the turn ends first.
#[test]
fn a_codex_compaction_keeps_the_last_real_fill() {
    let full = std::fs::read_to_string(common::recording("compact_codex")).unwrap();
    let rows: Vec<&str> = full.lines().collect();
    let compacted = rows
        .iter()
        .position(|r| r.contains("item/completed") && r.contains("contextCompaction"))
        .unwrap();
    let ended = rows
        .iter()
        .rposition(|r| r.contains("turn/completed"))
        .unwrap();
    // The same turn, ending as soon as its compaction does: no call after it.
    let cut = [&rows[..=compacted], &rows[ended..]].concat().join(
        "
",
    );
    for (recording, used) in [(full, 14_835), (cut, 14_577)] {
        let data = TempDir::new();
        let path = data.0.join("compact_codex.jsonl");
        std::fs::write(&path, recording).unwrap();
        let (service, account) = codex(&data.0, &path);
        let (chat, agent) = one_turn(&service, &account);
        service.wait_for(&chat, 10, "the first turn", |h| {
            h["turns"][0]["status"] == "done"
        });
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send(
            "POST",
            &path,
            json!({ "text": "Say hi again", "agent": agent }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        let history = service.wait_for(&chat, 10, "the compaction's turn", |h| {
            h["turns"][1]["status"] == "done"
        });
        assert_eq!(history["members"][0]["context"]["used"], used, "{history}");
    }
}
