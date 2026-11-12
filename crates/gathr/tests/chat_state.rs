//! A chat's state in the chat list (0040), through the real service with the engine played back:
//! `working` while a turn of it runs, `needs` while it waits on the person, and one live
//! `chat_state` to every window each time either changes.

mod common;

use std::net::TcpStream;
use std::path::Path;

use common::{
    TempDir, TestService, call_tool, first_token, gathr, held_recording, next_live, recording,
    team, text,
};
use serde_json::{Value, json};

/// The chat's `working` and `needs` in the list.
fn state(service: &TestService, chat: &str) -> (bool, bool) {
    let chats = service.call("GET", "/api/chats").json();
    let rows = chats.as_array().into_iter().flatten();
    let row = rows.clone().find(|c| c["id"] == chat);
    let row = row.unwrap_or_else(|| panic!("{chat} isn't in the list: {chats}"));
    match (row["working"].as_bool(), row["needs"].as_bool()) {
        (Some(working), Some(needs)) => (working, needs),
        _ => panic!("the chat's row carries no state: {row}"),
    }
}

fn until_state(service: &TestService, chat: &str, want: (bool, bool)) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let found = state(service, chat);
        if found == want {
            return;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "waited 10 s for the list to show {chat} as {want:?}; it shows {found:?}"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

/// Every `chat_state` since the socket opened or was last read, in order, as (chat, working,
/// needs). A chat made now marks the end: its `created` is sent after everything before it.
fn states(service: &TestService, live: &mut TcpStream) -> Vec<(String, bool, bool)> {
    let marker = service.new_chat();
    let mut seen = Vec::new();
    loop {
        let event = next_live(live);
        if event["type"] == "chat" && event["chat"] == marker.as_str() {
            return seen;
        }
        if event["type"] == "chat_state" {
            let chat = event["chat"].as_str().unwrap().to_string();
            let both = (event["working"].as_bool(), event["needs"].as_bool());
            seen.push((chat, both.0.unwrap(), both.1.unwrap()));
        }
    }
}

/// One chat's changes among `seen`.
fn of(seen: &[(String, bool, bool)], chat: &str) -> Vec<(bool, bool)> {
    let its = seen.iter().filter(|(c, _, _)| c == chat);
    its.map(|(_, working, needs)| (*working, *needs)).collect()
}

/// The chat's changes on this socket since it opened or was last read.
fn changes(service: &TestService, live: &mut TcpStream, chat: &str) -> Vec<(bool, bool)> {
    of(&states(service, live), chat)
}

fn say(service: &TestService, chat: &str, agent: Option<&str>) -> Value {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

fn ended(service: &TestService, chat: &str) -> Value {
    service.wait_for(chat, 10, "every turn to end", |h| {
        let turns = h["turns"].as_array().map_or(&[][..], Vec::as_slice);
        let over = |t: &Value| matches!(t["status"].as_str(), Some("done" | "failed" | "stopped"));
        !turns.is_empty() && turns.iter().all(over)
    })
}

/// A service whose engine holds each turn until `go` exists.
fn held(data: &Path) -> TestService {
    let (replay, go) = (held_recording(data), data.join("go"));
    TestService::with_engine(
        data,
        &[("GATHR_REPLAY", &replay), ("GATHR_REPLAY_HOLD", &go)],
    )
}

const WORKED: [(bool, bool); 2] = [(true, false), (false, false)];

#[test]
fn a_chat_works_from_its_turns_start_to_its_end_and_not_while_its_turn_is_queued() {
    let data = TempDir::new();
    let service = held(&data.0);
    let account = service.signed_in(&data.0);
    let agent = json!({ "name": "Orin", "account": account });
    let agent = service.send("POST", "/api/agents", agent).json();
    let agent = agent["id"].as_str().unwrap();
    let (a, b, idle) = (service.new_chat(), service.new_chat(), service.new_chat());
    let mut live = service.live();

    assert_eq!(say(&service, &a, Some(agent))["status"], "running");
    // One agent runs one turn at a time: its turn in the second chat waits.
    assert_eq!(say(&service, &b, Some(agent))["status"], "queued");
    assert_eq!(state(&service, &a), (true, false));
    assert_eq!(state(&service, &b), (false, false), "queued isn't working");
    assert_eq!(state(&service, &idle), (false, false));

    std::fs::write(data.0.join("go"), "").unwrap();
    ended(&service, &b);
    until_state(&service, &a, (false, false));
    until_state(&service, &b, (false, false));
    let seen = states(&service, &mut live);
    assert_eq!(of(&seen, &a), WORKED);
    assert_eq!(of(&seen, &b), WORKED, "its queued turn said something");
    assert_eq!(of(&seen, &idle), []);
}

#[test]
fn a_turn_that_fails_ends_the_chats_working() {
    let data = TempDir::new();
    let service =
        TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("engine_exits"))]);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    let history = ended(&service, &chat);
    assert_eq!(history["turns"][0]["status"], "failed", "{history}");
    until_state(&service, &chat, (false, false));
    assert_eq!(changes(&service, &mut live, &chat), WORKED);
}

#[test]
fn a_stopped_turn_ends_the_chats_working_and_a_paused_chat_works_only_while_a_turn_runs() {
    let data = TempDir::new();
    let service = held(&data.0);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    let scope = json!({ "scope": "chat", "target": chat });
    let paused = service.send("POST", "/api/pauses", scope);
    assert_eq!(paused.status, 200, "{}", paused.body);
    assert_eq!(state(&service, &chat), (true, false), "its turn still runs");

    let stopped = service.send("POST", &format!("/api/chats/{chat}/stop"), json!({}));
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    // The held stand-in reads its stop only once it streams.
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = ended(&service, &chat);
    assert_eq!(history["turns"][0]["status"], "stopped", "{history}");
    until_state(&service, &chat, (false, false));
    let pauses = service.call("GET", "/api/pauses").json();
    assert_eq!(pauses["pauses"][0]["target"], chat.as_str(), "{pauses}");
    assert_eq!(changes(&service, &mut live, &chat), WORKED);
}

/// The approval the recording asks for, and a service playing it.
fn asking(data: &Path) -> (TestService, String) {
    let fixture = recording("approval");
    let request = std::fs::read_to_string(&fixture)
        .unwrap()
        .lines()
        .map(|row| serde_json::from_str::<Value>(row).unwrap())
        .filter_map(|row| serde_json::from_str::<Value>(row["line"].as_str().unwrap()).ok())
        .find(|f| f["request"]["subtype"] == "can_use_tool")
        .map(|f| f["request_id"].as_str().unwrap().to_string())
        .expect("the recording asks for an approval");
    let service = TestService::with_engine(data, &[("GATHR_REPLAY", &fixture)]);
    service.signed_in(data);
    (service, request)
}

#[test]
fn a_waiting_approval_needs_the_person_until_it_is_answered_and_every_window_hears() {
    let data = TempDir::new();
    let (service, request) = asking(&data.0);
    let chat = service.new_chat();
    let (mut one, mut two) = (service.live(), service.live());
    say(&service, &chat, None);
    until_state(&service, &chat, (true, true));

    let (ok, health) = gathr(&data.0, &["diag", "health"]);
    assert!(ok, "{health}");
    let shown = json!([{ "chat": chat, "title": "Say hi", "working": true, "needs": true }]);
    assert_eq!(health["chats"], shown, "{health}");

    let approve = format!("/api/chats/{chat}/approvals/{request}");
    let answered = service.send("POST", &approve, json!({ "allow": true }));
    assert_eq!(answered.status, 200, "{}", answered.body);
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    let asked = [(true, false), (true, true), (true, false), (false, false)];
    for live in [&mut one, &mut two] {
        assert_eq!(changes(&service, live, &chat), asked);
    }
}

#[test]
fn a_refused_approval_needs_the_person_no_more() {
    let data = TempDir::new();
    let (service, request) = asking(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    until_state(&service, &chat, (true, true));
    let approve = format!("/api/chats/{chat}/approvals/{request}");
    let answered = service.send("POST", &approve, json!({ "allow": false }));
    assert_eq!(answered.status, 200, "{}", answered.body);
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    let asked = [(true, false), (true, true), (true, false), (false, false)];
    assert_eq!(changes(&service, &mut live, &chat), asked);
}

#[test]
fn an_approval_nobody_answered_goes_with_its_turn() {
    let data = TempDir::new();
    let (service, _) = asking(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    until_state(&service, &chat, (true, true));
    let stopped = service.send("POST", &format!("/api/chats/{chat}/stop"), json!({}));
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    let asked = [(true, false), (true, true), (false, false)];
    assert_eq!(changes(&service, &mut live, &chat), asked);
}

/// A team chat whose Supervisor, mid-turn, waits on a check and asks the person a question.
fn asked_the_person(data: &Path) -> (TestService, String) {
    let (service, chat, _) = team(data, &held_recording(data), &[]);
    say(&service, &chat, None);
    let (port, lead) = (service.info.port, first_token(data));
    let check = json!({ "check": "exit 1", "then": "merge it" });
    let left = text(&call_tool(port, &lead, "wait", check));
    assert!(left.starts_with("Waiting ("), "{left}");
    let question = json!({ "what": "Pick a name", "form": "done" });
    let asked = text(&call_tool(port, &lead, "ask_person", question));
    assert!(asked.starts_with("Task #1 is on the person's"), "{asked}");
    (service, chat)
}

fn answer(service: &TestService) {
    let done = json!({ "answers": [{ "task": 1, "done": true }] });
    let sent = service.send("POST", "/api/person-tasks/answer", done);
    assert_eq!(sent.status, 200, "{}", sent.body);
    assert_eq!(sent.json()["refused"], json!([]), "{}", sent.body);
}

#[test]
fn a_question_to_the_person_needs_them_until_answered_and_an_agent_that_only_waits_isnt_working() {
    let data = TempDir::new();
    let (service, chat) = asked_the_person(&data.0);
    let mut live = service.live();
    until_state(&service, &chat, (true, true));
    std::fs::write(data.0.join("go"), "").unwrap();
    ended(&service, &chat);
    // Its Supervisor still waits on the check: that isn't working.
    until_state(&service, &chat, (false, true));
    let waits = service.call("GET", &format!("/api/chats/{chat}/tasks"));
    assert_eq!(waits.json()["waits"][0]["state"], "open", "{}", waits.body);

    answer(&service);
    // The answer wakes the Supervisor: its next turn runs (this stand-in has no more to say).
    until_state(&service, &chat, (true, false));
    let seen = changes(&service, &mut live, &chat);
    let after = [(false, true), (false, false), (true, false)];
    assert_eq!(
        seen, after,
        "needs ends with the answer, before the turn it starts"
    );
}

#[test]
fn a_message_waiting_for_a_sign_in_needs_the_person() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let chat = service.new_chat();
    let mut live = service.live();
    assert_eq!(say(&service, &chat, None)["status"], "waiting_sign_in");
    assert_eq!(state(&service, &chat), (false, true));
    service.signed_in(&data.0);
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    let signed_in = [(false, true), (true, false), (false, false)];
    assert_eq!(changes(&service, &mut live, &chat), signed_in);
}

#[test]
fn an_archived_chat_carries_no_state() {
    let data = TempDir::new();
    let (service, _) = asking(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    until_state(&service, &chat, (true, true));

    let archived = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let rows = service.call("GET", "/api/chats?archived=true").json();
    assert_eq!(rows[0]["id"], chat.as_str(), "{rows}");
    for field in ["working", "needs"] {
        assert!(
            rows[0].get(field).is_none(),
            "{field} on an archived chat: {rows}"
        );
    }
    // Archiving stopped its turn, which ends once the engine has read the stop.
    ended(&service, &chat);
    let back = service.call("POST", &format!("/api/chats/{chat}/unarchive"));
    assert_eq!(back.status, 200, "{}", back.body);
    assert_eq!(state(&service, &chat), (false, false));
    let seen = changes(&service, &mut live, &chat);
    assert_eq!(seen, [(true, false), (true, true), (false, false)]);
}

#[test]
fn a_restart_keeps_what_needs_the_person_and_drops_what_was_running() {
    let data = TempDir::new();
    let (service, chat) = asked_the_person(&data.0);
    until_state(&service, &chat, (true, true));
    service.stop();

    let (replay, go, tokens) = (
        data.0.join("held.jsonl"),
        data.0.join("go"),
        data.0.join("env.jsonl"),
    );
    std::fs::write(&go, "").unwrap();
    let env = [
        ("GATHR_REPLAY", replay.as_path()),
        ("GATHR_REPLAY_HOLD", go.as_path()),
        ("GATHR_REPLAY_ENV", tokens.as_path()),
    ];
    let service = TestService::with_engine(&data.0, &env);
    assert_eq!(state(&service, &chat), (false, true));
    let mut live = service.live();
    answer(&service);
    service.wait_for(&chat, 10, "the hear-back turn", |h| {
        h["turns"].as_array().is_some_and(|t| t.len() == 2)
    });
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    let seen = changes(&service, &mut live, &chat);
    assert_eq!(seen, [(false, false), (true, false), (false, false)]);
}

/// 0019: the list's failure carries a reference that finds why, and so does a state that couldn't
/// be worked out for a turn.
#[test]
fn a_state_that_cant_be_read_fails_with_a_reference_that_finds_why() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    let chat = service.new_chat();
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute("ALTER TABLE tasks RENAME TO tasks_gone", [])
        .unwrap();

    let listed = service.call("GET", "/api/chats");
    assert_eq!(listed.status, 500, "{}", listed.body);
    let answer = listed.json();
    assert_eq!(answer["error"], "Chats failed", "{answer}");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
    assert!(
        ok && trace.to_string().contains("no such table"),
        "the reference doesn't find why the list failed: {trace}"
    );

    // No engine copy: the turn is queued, and its state can't be worked out either.
    let sent = say(&service, &chat, None);
    let (ok, trace) = gathr(&data.0, &["diag", "trace", sent["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let events = trace["events"].as_array().unwrap();
    let failed = events.iter().find(|e| e["name"] == "gathr.chat.state");
    let failed = failed.unwrap_or_else(|| panic!("no gathr.chat.state in the trace: {trace}"));
    assert_eq!(failed["error_type"], "sqlite", "{failed}");
    let why = failed["attrs"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("no such table"), "{failed}");
}
