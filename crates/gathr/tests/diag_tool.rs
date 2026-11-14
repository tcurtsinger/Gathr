//! The `diag` tool (0019, scenario 6i): an agent reads Gathr's own record of what failed, for its
//! own chat and events of no chat, with secrets hidden and a long answer cut at the cap.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, held_recording, nth, recording, sign_in_account, team, text,
    until,
};
use serde_json::{Value, json};

const CAP: usize = 16_000;

/// The token of the `n`th engine the replay engine started.
fn token(data: &Path, n: usize) -> String {
    let started = nth(&data.join("env.jsonl"), n);
    started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
}

/// Sends the chat's lead a message; its engine's token once the turn holds.
fn lead_turn(service: &TestService, data: &Path, chat: &str, n: usize) -> String {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Plan it" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    token(data, n)
}

/// `held_recording`, then a second turn on the same engine: the giver's hear-back.
fn twice(dir: &Path) -> PathBuf {
    let first = std::fs::read_to_string(held_recording(dir)).unwrap();
    let second = std::fs::read_to_string(recording("signed_in")).unwrap();
    let both = dir.join("twice.jsonl");
    std::fs::write(&both, format!("{}\n{second}", first.trim_end())).unwrap();
    both
}

/// A `read_chat` that fails: its reference, from "(ref …)".
fn forced_failure(port: u16, token: &str) -> String {
    let failed = call_tool(port, token, "read_chat", json!({ "message": "nope" }));
    assert_eq!(failed["isError"], true, "{failed}");
    let said = text(&failed);
    let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
    reference.to_string()
}

/// The `diag` tool's answer as JSON, failing the test when it refused.
fn diag(port: u16, token: &str, arguments: Value) -> Value {
    let answered = call_tool(port, token, "diag", arguments.clone());
    assert_eq!(answered["isError"], false, "diag {arguments}: {answered}");
    let said = text(&answered);
    serde_json::from_str(&said).unwrap_or_else(|e| panic!("diag {arguments} ({e}): {said}"))
}

fn give(port: u16, lead: &str, to: &str, what: &str) {
    let given = text(&call_tool(
        port,
        lead,
        "give_task",
        json!({ "to": to, "what": what }),
    ));
    assert!(given.starts_with("Task #"), "{given}");
}

#[test]
fn a_forced_failure_is_found_from_its_reference() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &held_recording(&data.0), &[]);
    let lead = lead_turn(&service, &data.0, &chat, 0);
    let port = service.info.port;
    let reference = forced_failure(port, &lead);

    let trace = diag(
        port,
        &lead,
        json!({ "command": "trace", "reference": reference }),
    );
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events.iter().find(|e| {
        e["name"] == "gathr.tool.call"
            && e["status"] == "error"
            && e["attrs"]["gen_ai.tool.name"] == "read_chat"
    });
    let failed = failed.unwrap_or_else(|| panic!("no failed read_chat in the trace: {trace}"));
    let why = failed["attrs"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("nope"), "the why isn't the failure's: {trace}");
}

#[test]
fn a_secret_in_a_tasks_reason_comes_back_hidden() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &twice(&data.0), &[]);
    let lead = lead_turn(&service, &data.0, &chat, 0);
    let port = service.info.port;
    give(port, &lead, "ann", "Write the notes");
    let ann = token(&data.0, 1);
    let blocked = json!({ "how": "blocked", "note": "on ghp_madeup0123456789abcdef0123456789" });
    let said = text(&call_tool(port, &ann, "finish_task", blocked));
    assert!(said.starts_with("Recorded: blocked"), "{said}");
    // The reason is written when Ann's turn ends.
    std::fs::write(data.0.join("go"), "").unwrap();
    until(20, "the task to show as blocked", || {
        let listed = service
            .call("GET", &format!("/api/chats/{chat}/tasks"))
            .json();
        (listed["tasks"][0]["state"] == "blocked").then_some(())
    });

    let health = diag(port, &lead, json!({ "command": "health" }));
    let shown = health.to_string();
    assert!(
        !shown.contains("ghp_madeup"),
        "the secret came back: {health}"
    );
    let tasks = health["coordination"]["tasks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let reason = tasks
        .first()
        .map(|t| t["reason"].to_string())
        .unwrap_or_default();
    assert!(reason.contains("[redacted:"), "{health}");
}

#[test]
fn another_chats_events_dont_come_back() {
    let data = TempDir::new();
    let (service, chat_a, _) = team(&data.0, &held_recording(&data.0), &[]);
    let port = service.info.port;
    // B has agents of its own: one agent's turns run one at a time, so a turn of A's would hold
    // B's, and one left queued when the service stops outlives the test.
    let account = service.add_account("Other");
    sign_in_account(&data.0, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].clone()
    };
    let members = json!([{ "agent": agent("Bea"), "role": "supervisor" }, { "agent": agent("Cy"), "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat_b = made.json()["id"].as_str().unwrap().to_string();

    let lead_a = lead_turn(&service, &data.0, &chat_a, 0);
    let lead_b = lead_turn(&service, &data.0, &chat_b, 1);
    give(port, &lead_b, "cy", "B's own work");
    give(port, &lead_a, "ann", "A's own work");
    let ref_b = forced_failure(port, &lead_b);
    let ref_a = forced_failure(port, &lead_a);
    let missing = service.call("GET", "/api/nope");
    assert!(missing.status >= 400, "{}", missing.body);
    let chatless = missing.headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.eq_ignore_ascii_case("x-gathr-ref")
            .then(|| value.trim().to_string())
    });
    let chatless = chatless.unwrap_or_else(|| panic!("no reference: {}", missing.headers));

    let refused = call_tool(
        port,
        &lead_a,
        "diag",
        json!({ "command": "trace", "reference": ref_b }),
    );
    assert_eq!(
        refused["isError"], true,
        "B's trace came back to A: {refused}"
    );
    assert!(
        text(&refused).contains("no events for reference"),
        "{refused}"
    );

    let failures = diag(
        port,
        &lead_a,
        json!({ "command": "failures", "since": "1h" }),
    );
    let shown = failures.to_string();
    assert!(
        !shown.contains(&ref_b),
        "B's failure came back to A: {failures}"
    );
    assert!(
        shown.contains(&chatless),
        "a failure of no chat is left out: {failures}"
    );
    let groups = failures["groups"].as_array().cloned().unwrap_or_default();
    let read_chat = groups
        .iter()
        .find(|g| g["name"] == "gathr.tool.call" && g["error_type"] == "read_chat");
    let read_chat = read_chat.unwrap_or_else(|| panic!("no read_chat group: {failures}"));
    assert_eq!(
        read_chat["count"], 1,
        "B's failure is counted in A: {failures}"
    );
    assert_eq!(read_chat["references"], json!([ref_a]), "{failures}");

    let health = diag(port, &lead_a, json!({ "command": "health" }));
    assert!(
        !health.to_string().contains(&chat_b),
        "B came back to A: {health}"
    );
    let tasks = health["coordination"]["tasks"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let whats: Vec<&Value> = tasks.iter().map(|t| &t["chat"]).collect();
    assert_eq!(whats, [&json!(chat_a)], "A's own task is shown: {health}");
}

#[test]
fn a_long_answer_is_cut_at_the_cap_and_says_so() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &held_recording(&data.0), &[]);
    let lead = lead_turn(&service, &data.0, &chat, 0);
    let port = service.info.port;
    for _ in 0..80 {
        let read = call_tool(port, &lead, "read_chat", json!({}));
        assert_eq!(read["isError"], false, "{read}");
    }
    let reference = forced_failure(port, &lead);

    let answered = call_tool(
        port,
        &lead,
        "diag",
        json!({ "command": "trace", "reference": reference }),
    );
    assert_eq!(answered["isError"], false, "{answered}");
    let said = text(&answered);
    let length = said.chars().count();
    assert!(length <= CAP + 200, "{length} characters: past the cap");
    let (kept, line) = said
        .rsplit_once("\n[… ")
        .unwrap_or_else(|| panic!("no line says the answer was cut, in {length} characters"));
    let numbers: Vec<usize> = line
        .split(' ')
        .filter_map(|w| w.parse().ok())
        .take(2)
        .collect();
    assert!(line.contains("characters cut"), "{line}");
    assert_eq!(
        numbers[0] + kept.chars().count(),
        numbers[1],
        "the cut and the kept don't add up: {line}"
    );
    // The failure comes first, so the cut never loses it.
    let at = kept.find("\"events\":[").expect("no events") + "\"events\":[".len();
    let mut events = serde_json::Deserializer::from_str(&kept[at..]).into_iter::<Value>();
    let first = events.next().expect("no first event").unwrap();
    assert_eq!(
        (&first["status"], &first["attrs"]["gen_ai.tool.name"]),
        (&json!("error"), &json!("read_chat")),
        "the failure isn't first: {first}"
    );
}
