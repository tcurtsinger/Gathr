//! `show`: an agent puts a table or a chart in the chat, through a real service and the replay
//! engine. Its engine may call it without asking, a good block is kept in the chat's history, and a
//! bad one is refused with nothing kept.

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, held_recording, sign_in_account, text, until};
use serde_json::{Value, json};

fn first_line(file: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(file).ok()?;
    serde_json::from_str(text.lines().next()?).ok()
}

fn blocks(service: &TestService, chat: &str) -> Vec<Value> {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    messages
        .into_iter()
        .filter(|m| m["kind"] == "block")
        .collect()
}

#[test]
fn an_agent_shows_a_table_and_a_chart_and_a_bad_one_keeps_nothing() {
    let data = TempDir::new();
    let (args, env, go) = (
        data.0.join("args.jsonl"),
        data.0.join("env.jsonl"),
        data.0.join("go"),
    );
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let port = service.info.port;
    let account = service.add_account("Ann's");
    sign_in_account(&data.0, &account);
    let ann = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ann", "account": account }),
    );
    let ann = ann.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let message = json!({ "text": "How did the seed orders go?", "agent": ann });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), message);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let given: Vec<String> =
        serde_json::from_value(until(10, "the engine's start", || first_line(&args))).unwrap();
    let token = until(10, "the engine's token", || {
        first_line(&env)?["GATHR_AGENT_TOKEN"]
            .as_str()
            .map(String::from)
    });

    // It changes nothing outside the chat, so its engine never asks the person first.
    let at = given.iter().position(|a| a == "--allowedTools").unwrap();
    assert!(
        given[at + 1].split(',').any(|t| t == "mcp__gathr__show"),
        "{}",
        given[at + 1]
    );

    let columns = json!([
        { "key": "bed", "label": "Bed" },
        { "key": "cost", "label": "Cost", "as": "money", "currency": "USD" },
        { "key": "change", "label": "Change", "as": "change", "unit": "%" },
    ]);
    let rows = json!([
        { "bed": "Tomatoes", "cost": 12.5, "change": 3.2 },
        { "bed": "Beans", "cost": null, "change": -1 },
    ]);
    let table = json!({ "kind": "table", "title": "Seed orders", "table": { "columns": columns, "rows": rows } });
    assert_eq!(text(&call_tool(port, &token, "show", table)), "Shown.");
    let chart = json!({ "kind": "chart",
        "chart": { "label": "Rain", "value": "16.8 mm", "points": [1, 4, 2.5], "style": "bars" } });
    assert_eq!(text(&call_tool(port, &token, "show", chart)), "Shown.");

    let kept = blocks(&service, &chat);
    assert_eq!(kept.len(), 2, "{kept:?}");
    assert_eq!(kept[0]["role"], "assistant");
    assert_eq!(kept[0]["body"]["title"], "Seed orders");
    assert_eq!(kept[0]["body"]["table"]["rows"], rows);
    assert_eq!(kept[0]["body"]["table"]["columns"][1]["as"], "money");
    assert_eq!(kept[1]["body"]["chart"]["points"], json!([1.0, 4.0, 2.5]));

    // Refused, each with what is wrong and the shape, and nothing kept.
    let no_table = call_tool(port, &token, "show", json!({ "kind": "table" }));
    assert_eq!(no_table["isError"], true, "{no_table}");
    assert!(text(&no_table).contains("needs table"), "{no_table}");
    let many = vec![json!({ "bed": "Peas" }); 201];
    let too_many = json!({ "kind": "table", "table": { "columns": [{ "key": "bed", "label": "Bed" }], "rows": many } });
    let refused = call_tool(port, &token, "show", too_many);
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(text(&refused).contains("201 rows"), "{refused}");
    assert_eq!(blocks(&service, &chat).len(), 2);

    // An agent reading the chat sees a block as one line.
    let read = text(&call_tool(port, &token, "read_chat", json!({})));
    assert!(read.contains("[table: Seed orders, 2 rows]"), "{read}");
    assert!(read.contains("[chart: Rain, 3 points]"), "{read}");
    std::fs::write(&go, "").unwrap();
}
