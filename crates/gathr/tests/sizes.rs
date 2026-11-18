//! 0061, scenarios 2a, 2f, 2j and 2k through a real service, on each engine's replay stand-in: the
//! person's follow-ups join their ask, the lead sizes it and splits a message off, a member can't
//! size, and a repair whose work is in is said "Fixed" once the lead has heard back; the person's
//! change from repair to new is counted.

mod common;

use common::{TempDir, call_tool, gathr, nth, rows, team_on, text, turn_on};
use serde_json::{Value, json};

const EXPORT: &str = "The export button does nothing";
const UPDATE: &str = "It started after the update";
const TAGS: &str = "Add tags to notes";

/// Claude Code's stand-in plays one recording for the whole session: the first turn held until
/// `go`, then a turn for each message sent into it and read after it ended (as Claude Code
/// 2.1.318 does, `chats.rs`), then turns for what Gathr queues after. Codex's replays each turn.
fn claude_turns(data: &std::path::Path) {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let read = |said: &str| {
        let frame = json!({ "type": "user", "message": { "role": "user", "content": said },
            "session_id": "a404d9af-ce93-4b65-bb15-1b300040a9f1", "parent_tool_use_id": null,
            "isReplay": true });
        json!({ "dir": "out", "line": frame.to_string() }).to_string()
    };
    let mut all = vec![lines[0].to_string(), r#"{"dir":"hold"}"#.to_string()];
    all.extend(lines[1..].iter().map(|l| l.to_string()));
    for said in [UPDATE, TAGS] {
        all.extend([lines[0], lines[1], lines[2]].map(String::from));
        all.push(read(said));
        all.extend(lines[4..].iter().map(|l| l.to_string()));
    }
    for _ in 0..3 {
        all.extend(lines.iter().map(|l| l.to_string()));
    }
    std::fs::write(data.join("held.jsonl"), all.join("\n") + "\n").unwrap();
}

fn send(service: &common::TestService, chat: &str, said: &str) -> Value {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": said }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

fn listed(service: &common::TestService, chat: &str) -> Value {
    let listed = service.call("GET", &format!("/api/chats/{chat}/tasks"));
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

fn asks(service: &common::TestService, chat: &str) -> Vec<Value> {
    let listed = listed(service, chat);
    listed["asks"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| panic!("no asks: {listed}"))
}

/// The id of the person's message that says `said`.
fn message_id(service: &common::TestService, chat: &str, said: &str) -> String {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let found = messages
        .iter()
        .find(|m| m["role"] == "user" && m["body"] == said);
    let found = found.unwrap_or_else(|| panic!("no message {said:?}: {history}"));
    found["id"].as_str().unwrap().to_string()
}

fn ask_of(data: &std::path::Path, turn: &str) -> Value {
    let found = rows(
        data,
        &format!("SELECT ask_id FROM turns WHERE id = '{turn}'"),
    );
    found[0]["ask_id"].clone()
}

/// Lets every held engine go when the test ends, passed or failed: one held on a file in a folder
/// that is gone waits for ever.
struct LetGo(std::path::PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

fn asks_follow_ups_and_fixed_through_the_runner(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, engine);
    let _go = LetGo(data.0.join("go"));
    if engine == "claude" {
        claude_turns(&data.0);
    }
    let port = service.info.port;
    let first = send(&service, &chat, EXPORT);
    let lead_turn = first["turn"].as_str().unwrap().to_string();
    let lead = common::first_token(&data.0);
    turn_on_lead(&service, &chat, &lead_turn);
    send(&service, &chat, UPDATE);
    assert_eq!(asks(&service, &chat).len(), 1, "a follow-up joins the ask");
    let m1 = message_id(&service, &chat, EXPORT);

    let sized = text(&call_tool(port, &lead, "size", json!({ "size": "repair" })));
    assert_eq!(sized, format!("Sized \"{EXPORT}\" as repair."));
    let work = common::briefed(
        json!({ "to": "Ann", "what": "Fix the export button", "where": "no code" }),
    );
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    let ann_turn = turn_on(&service, &chat, 1, &["running"]);
    let ann = nth(&data.0.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .expect("Ann's token")
        .to_string();

    // The forced failure: a member's size is refused, with a reference that finds it.
    let refused = text(&call_tool(port, &ann, "size", json!({ "size": "new" })));
    let why = "Only Lead, the chat's Supervisor, sizes asks.";
    assert!(
        refused.starts_with(&format!("size failed: {why} (ref ")),
        "{refused}"
    );
    let reference = refused
        .rsplit("(ref ")
        .next()
        .unwrap()
        .trim_end_matches(')');
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let call = events
        .iter()
        .filter(|e| e["name"] == "gathr.tool.call" && e["attrs"]["gen_ai.tool.name"] == "size")
        .find(|e| e["status"] == "error");
    let said = call.map(|e| e["attrs"]["why"].as_str().unwrap_or_default());
    assert!(ok && said == Some(why), "{trace}");

    // To Ann as the window sends it: her id, picked from its list (a name in the text is only text).
    let agents = service.call("GET", "/api/agents").json();
    let to_ann = agents["agents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == "Ann");
    let body = json!({ "text": "@Ann also check the import", "agent": to_ann.unwrap()["id"] });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    assert_eq!(
        asks(&service, &chat).len(),
        1,
        "a message to a member joins too"
    );
    let report = json!({ "how": "done", "note": "The handler was missing; it's back." });
    let reported = text(&call_tool(port, &ann, "finish_task", report));
    assert!(reported.starts_with("Recorded: done."), "{reported}");

    // A new ask sent while the lead's turn runs joins it; the lead splits it off.
    let tags = send(&service, &chat, TAGS);
    let tags_id = message_id(&service, &chat, TAGS);
    assert_eq!(asks(&service, &chat).len(), 1, "it joined the open ask");
    let split = json!({ "size": "new", "message": tags_id });
    let made = text(&call_tool(port, &lead, "size", split));
    assert_eq!(
        made,
        format!("Made message {tags_id} an ask of its own and sized \"{TAGS}\" as new.")
    );
    assert_eq!(
        asks(&service, &chat).len(),
        2,
        "{}",
        listed(&service, &chat)
    );
    match tags["steered"] == true {
        true => assert_eq!(
            ask_of(&data.0, &lead_turn),
            json!(m1),
            "the turn it was sent into stays"
        ),
        false => {
            let own = tags["turn"].as_str().unwrap();
            assert_eq!(
                ask_of(&data.0, own),
                json!(tags_id),
                "the turn it opened moves"
            );
        }
    }

    std::fs::write(data.0.join("go"), "").unwrap();
    let fixed_line = format!("Fixed: {EXPORT}\n");
    let history = service.wait_for(&chat, 30, "the Fixed line", |h| {
        let messages = h["messages"].as_array().cloned().unwrap_or_default();
        messages.iter().any(|m| {
            m["role"] == "gathr"
                && m["turn"].is_null()
                && m["body"]
                    .as_str()
                    .is_some_and(|b| b.starts_with(&fixed_line))
        })
    });
    let messages = history["messages"].as_array().unwrap();
    let said_by_gathr: Vec<&Value> = messages
        .iter()
        .filter(|m| m["role"] == "gathr" && m["turn"].is_null())
        .collect();
    let at = |prefix: &str| {
        said_by_gathr
            .iter()
            .position(|m| m["body"].as_str().unwrap_or_default().starts_with(prefix))
    };
    let (done_at, fixed_at) = (at("A task is done:"), at("Fixed: "));
    assert!(done_at.is_some() && done_at < fixed_at, "{said_by_gathr:?}");
    let fixed = said_by_gathr[fixed_at.unwrap()];
    assert_eq!(
        said_by_gathr
            .iter()
            .filter(|m| m["body"]
                .as_str()
                .unwrap_or_default()
                .starts_with("Fixed: "))
            .count(),
        1
    );
    let backs = rows(
        &data.0,
        "SELECT id, ended_ts, ask_id FROM turns WHERE opened_by = 'hear_back'",
    );
    assert_eq!(backs.len(), 1, "{backs:?}");
    let ended = backs[0]["ended_ts"]
        .as_i64()
        .unwrap_or_else(|| panic!("{backs:?}"));
    let id = fixed["id"].as_str().unwrap();
    let said_at = rows(
        &data.0,
        &format!("SELECT ts FROM messages WHERE id = '{id}'"),
    );
    let said_at = said_at[0]["ts"]
        .as_i64()
        .unwrap_or_else(|| panic!("{said_at:?}"));
    assert!(
        said_at >= ended,
        "said after the hear-back ended: {fixed} {backs:?}"
    );

    let m1 = json!(m1);
    let carried = [
        ("the lead's first turn", ask_of(&data.0, &lead_turn)),
        ("Ann's task turn", ask_of(&data.0, &ann_turn)),
        ("the lead's hear-back", backs[0]["ask_id"].clone()),
    ];
    for (what, ask) in carried {
        assert_eq!(ask, m1, "{what}");
    }
    let task = rows(&data.0, "SELECT ask_id FROM tasks WHERE number = 1");
    assert_eq!(task[0]["ask_id"], m1);
    let listed = listed(&service, &chat);
    let one = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["number"] == 1)
        .cloned();
    assert_eq!(one.unwrap_or_default()["ask"], m1, "{listed}");
    let shown = &listed["asks"][0];
    let want = json!({ "id": m1, "title": EXPORT, "size": "repair", "by": shown["by"], "sized_in": lead_turn,
        "held": false });
    assert_eq!(shown, &want, "{listed}");
    assert!(
        shown["by"].is_string() && shown["by"] != "person",
        "{listed}"
    );

    let path = format!("/api/asks/{}/size", m1.as_str().unwrap());
    let resized = service.send("POST", &path, json!({ "size": "new" }));
    assert_eq!(resized.status, 200, "{}", resized.body);
    let resized = resized.json();
    assert_eq!(
        (&resized["size"], &resized["by"]),
        (&json!("new"), &json!("person")),
        "{resized}"
    );
    let corrections = rows(
        &data.0,
        "SELECT count(*) AS corrections FROM event_log WHERE name = 'gathr.ask.resized' \
         AND attrs ->> '$.\"gathr.ask.correction\"' = 1",
    );
    assert_eq!(corrections[0]["corrections"], 1);
    let nope = service.send("POST", "/api/asks/nope/size", json!({ "size": "new" }));
    assert_eq!(nope.status, 409, "{}", nope.body);
    assert_eq!(
        (&nope.json()["error"], &nope.json()["why"]),
        (&json!("Size failed"), &json!("there's no ask nope"))
    );
}

/// Waits until the lead's first turn runs, so what follows is sent into it.
fn turn_on_lead(service: &common::TestService, chat: &str, turn: &str) {
    service.wait_for(chat, 30, "the lead's turn to run", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && t["status"] == "running")
    });
}

#[test]
fn asks_follow_ups_and_fixed_through_the_runner_on_claude() {
    asks_follow_ups_and_fixed_through_the_runner("claude");
}

#[test]
fn asks_follow_ups_and_fixed_through_the_runner_on_codex() {
    asks_follow_ups_and_fixed_through_the_runner("codex");
}
