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
use gathr::approvals::GATHR_ASKS;
use gathr::notice::{self, Notice, Start};
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
    let (replay, go) = (held_recording(data, "signed_in"), data.join("go"));
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

/// A chat where the agent "Made-up Ann" asks for the recording's approval: the chat.
fn ann_asks(service: &TestService, data: &Path) -> String {
    let account = service.signed_in(data);
    let ann = json!({ "name": "Made-up Ann", "account": account });
    let ann = service.send("POST", "/api/agents", ann).json();
    let chat = service.new_chat();
    say(service, &chat, ann["id"].as_str());
    until_state(service, &chat, (true, true));
    chat
}

/// A route as the window reads it: its JSON when it answers 200.
fn route(service: &TestService, path: &str) -> Option<Value> {
    let answer = service.call("GET", path);
    (answer.status == 200).then(|| answer.json())
}

/// What the window reads every few seconds (0058).
fn window_reads(service: &TestService) -> notice::Read {
    let (chats, pauses) = (route(service, "/api/chats"), route(service, "/api/pauses"));
    notice::read(chats.as_ref(), pauses.as_ref()).expect("both routes answer")
}

/// What the window decides from the real routes, its window hidden.
fn window_decides(
    service: &TestService,
    last: Option<&notice::State>,
) -> (notice::State, Vec<Notice>) {
    let now = window_reads(service);
    let read = |path: String| route(service, &path).unwrap_or(Value::Null);
    let waiting = |chat: &str| read(format!("/api/chats/{chat}/waiting"));
    let approvals = |chat: &str| read(format!("/api/chats/{chat}"));
    notice::decide(last, Start::Other, &now, false, false, waiting, approvals)
}

fn title(service: &TestService, chat: &str) -> String {
    let chats = service.call("GET", "/api/chats").json();
    let row = chats.as_array().unwrap().iter().find(|c| c["id"] == chat);
    row.and_then(|c| c["title"].as_str())
        .unwrap_or_default()
        .to_string()
}

fn pause_all(service: &TestService) {
    let paused = service.send("POST", "/api/pauses", json!({ "scope": "app" }));
    assert_eq!(paused.status, 200, "{}", paused.body);
}

/// Scenario 6j: the routes the window reads, through the window's own decision. Whether a pause
/// lets an approval through rests on its reason starting with `GATHR_ASKS`, matched by agent.
#[test]
fn what_the_window_reads_names_the_chat_that_needs_you_and_whether_a_pause_holds_it() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("approval"))]);
    assert_eq!(window_reads(&service).needs.len(), 0);
    let (before, said) = window_decides(&service, None);
    assert_eq!(said, []);

    let chat = ann_asks(&service, &data.0);
    let asks = Notice::Show {
        chat: chat.clone(),
        title: title(&service, &chat),
        body: "Made-up Ann needs your approval".into(),
        why: None,
        rose: true,
    };
    assert_eq!(window_decides(&service, Some(&before)).1, [asks]);

    // The approval's line is as old as the approval, not its turn.
    let waiting = route(&service, &format!("/api/chats/{chat}/waiting")).unwrap();
    let line = waiting["lines"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["kind"] == "approval");
    let since = line.unwrap_or_else(|| panic!("no approval line: {waiting}"))["since_us"].as_i64();
    let (ok, turns) = gathr(&data.0, &["diag", "query", "SELECT created_ts FROM turns"]);
    assert!(ok, "{turns}");
    let created = turns["rows"][0]["created_ts"].as_i64();
    assert!(
        since > created,
        "the approval line's time {since:?}, its turn's {created:?}"
    );

    // Claude Code's own ask is held by an app pause.
    pause_all(&service);
    let held = Notice::Held { chat: chat.clone() };
    assert_eq!(window_decides(&service, None).1, [held]);

    // Gathr's own ask goes through it: the same recording, with Gathr's reason.
    let other = TempDir::new();
    let frames = std::fs::read_to_string(recording("approval")).unwrap();
    let reason = format!("{GATHR_ASKS} it writes outside the agent's folders");
    let frames = frames.lines().map(|row| {
        let mut row: Value = serde_json::from_str(row).unwrap();
        let Ok(mut frame) = serde_json::from_str::<Value>(row["line"].as_str().unwrap_or(""))
        else {
            return row.to_string();
        };
        if frame["request"]["subtype"] == "can_use_tool" {
            frame["request"]["decision_reason"] = reason.clone().into();
            row["line"] = frame.to_string().into();
        }
        row.to_string()
    });
    let written = other.0.join("gathr-asks.jsonl");
    std::fs::write(&written, frames.collect::<Vec<_>>().join("\n")).unwrap();
    let service = TestService::with_engine(&other.0, &[("GATHR_REPLAY", &written)]);
    let chat = ann_asks(&service, &other.0);
    pause_all(&service);
    let through = Notice::Show {
        chat: chat.clone(),
        title: title(&service, &chat),
        body: "Made-up Ann needs your approval".into(),
        why: Some("gathr_ask"),
        rose: true,
    };
    assert_eq!(window_decides(&service, None).1, [through]);
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
    // From the service, which alone knows of the approval; by id, as a title is conversation text.
    let shown = json!({ "from": "service", "why": null,
        "chats": [{ "chat": chat, "working": true, "needs": true }] });
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
    let (service, chat, _) = team(data, &held_recording(data, "signed_in"), &[]);
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
    // With no service to ask, health says the database answered, and why.
    let (ok, health) = gathr(&data.0, &["diag", "health"]);
    assert!(ok, "{health}");
    let shown = &health["chats"];
    assert_eq!(shown["from"], "database", "{health}");
    assert_eq!(shown["why"], "the service isn't running", "{health}");
    let asked = json!([{ "chat": chat, "working": false, "needs": true }]);
    assert_eq!(shown["chats"], asked, "{health}");

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
    // Not `tasks`: saving a turn reads it for the turn's ask (0061), and the send would fail first.
    db.execute("ALTER TABLE quiet RENAME TO quiet_gone", [])
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

/// Gathr's worktree hook asking about a push, as an engine's does: one that isn't to the worktree's
/// origin, so Gathr asks the person.
const PUSH: [(&str, &str); 2] = [
    ("gathr-remote", "elsewhere"),
    ("gathr-url", "https://example.invalid/made-up.git"),
];

#[test]
fn a_push_gathr_asks_about_needs_the_person_until_it_is_answered() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &held_recording(&data.0, "signed_in"), &[]);
    let mut live = service.live();
    say(&service, &chat, None);
    let (port, auth) = (
        service.info.port,
        format!("Bearer {}", first_token(&data.0)),
    );
    // A held turn's push is refused before anyone is asked (0063).
    common::fix_it(port, &first_token(&data.0));
    let asked = std::thread::spawn(move || {
        let headers = [("Authorization", auth.as_str()), PUSH[0], PUSH[1]];
        gathr::http::post(port, "/hook/push", &headers, "")
            .unwrap()
            .body
    });
    until_state(&service, &chat, (true, true));
    let open = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(open["approvals"][0]["tool"], "git push", "{open}");
    let request = open["approvals"][0]["request_id"].as_str().unwrap();
    let approve = format!("/api/chats/{chat}/approvals/{request}");
    let answered = service.send("POST", &approve, json!({ "allow": true }));
    assert_eq!(answered.status, 200, "{}", answered.body);
    assert_eq!(asked.join().unwrap(), "allow");
    until_state(&service, &chat, (true, false));
    let seen = changes(&service, &mut live, &chat);
    assert_eq!(seen, [(true, false), (true, true), (true, false)]);
}

#[test]
fn a_chat_deleted_while_its_turn_runs_stops_working_once() {
    let data = TempDir::new();
    let service = held(&data.0);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    assert_eq!(state(&service, &chat), (true, false));
    let (port, token) = (service.info.port, service.info.token.clone());
    let path = format!("/api/chats/{chat}");
    let deleting =
        std::thread::spawn(move || gathr::http::call(port, &token, "DELETE", &path).unwrap());
    let gone = || {
        let listed = service.call("GET", "/api/chats").json();
        let mut ids = listed.as_array().unwrap().iter().map(|c| &c["id"]);
        !ids.any(|id| *id == chat.as_str())
    };
    common::until(10, "the chat to leave the list", || gone().then_some(()));
    // The delete waits for the chat's engine, and the held stand-in ends only once it streams.
    std::fs::write(data.0.join("go"), "").unwrap();
    let deleted = deleting.join().unwrap();
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(changes(&service, &mut live, &chat), WORKED);
}

#[test]
fn stop_now_for_everything_ends_a_chats_working_and_its_held_carry_on_isnt_working() {
    let data = TempDir::new();
    let service = held(&data.0);
    service.signed_in(&data.0);
    let chat = service.new_chat();
    let mut live = service.live();
    say(&service, &chat, None);
    let stopped = service.send("POST", "/api/pauses/stop", json!({ "scope": "app" }));
    assert_eq!(stopped.status, 200, "{}", stopped.body);
    // The held stand-in reads its stop only once it streams.
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the turn to stop", |h| {
        h["turns"][0]["status"] == "stopped"
    });
    until_state(&service, &chat, (false, false));
    let last = history["turns"].as_array().unwrap().last().unwrap();
    assert_eq!(last["held"], "pause", "the carry-on waits: {history}");
    assert_eq!(changes(&service, &mut live, &chat), WORKED);
}

#[test]
fn two_agents_turns_in_one_chat_ending_together_end_its_working_once() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &held_recording(&data.0, "signed_in"), &[]);
    let mut live = service.live();
    let members = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = members["members"].as_array().unwrap().clone();
    let ann = members
        .iter()
        .find(|m| m["agent"] != lead.as_str())
        .unwrap();
    say(&service, &chat, Some(&lead));
    say(&service, &chat, ann["agent"].as_str());
    service.wait_for(&chat, 10, "both turns running", |h| {
        let turns = h["turns"].as_array().map_or(&[][..], Vec::as_slice);
        turns.len() == 2 && turns.iter().all(|t| t["status"] == "running")
    });
    std::fs::write(data.0.join("go"), "").unwrap();
    ended(&service, &chat);
    until_state(&service, &chat, (false, false));
    assert_eq!(changes(&service, &mut live, &chat), WORKED);
}

/// 0071: a chat's row in the list carries its most pressing mark of three, as the sidebar draws it:
/// `needs_you`, `cant_run` or `working`. A signed-out agent still makes the chat need the person for
/// a notice, but its row says it can't run.
#[test]
fn a_chats_row_carries_its_most_pressing_mark() {
    let data = TempDir::new();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('ann', 'Ann', 'ann', 'acc', 1), ('bo', 'Bo', 'bo', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO chat_members VALUES ('c', 'ann', 'supervisor', 1), ('c', 'bo', 'member', 1);",
    )
    .unwrap();
    let row = |conn: &rusqlite::Connection| {
        let chats = gathr::store::chats(conn, false).unwrap();
        let state = gathr::store::chat_state(conn, "c").unwrap().unwrap();
        (chats[0]["mark"].clone(), json!(state.mark), state.needs)
    };
    let turn = |id: &str, agent: &str, status: &str, at: i64| {
        let add = "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id)
                   VALUES (?1, 'c', 'x', ?2, ?3, ?4)";
        conn.execute(add, rusqlite::params![id, status, at, agent])
            .unwrap();
    };
    assert_eq!(row(&conn), (Value::Null, Value::Null, false));
    turn("a1", "ann", "running", 2);
    assert_eq!(row(&conn), (json!("working"), json!("working"), false));
    turn("b1", "bo", "failed", 3);
    assert_eq!(row(&conn), (json!("cant_run"), json!("cant_run"), false));
    conn.execute_batch(
        "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts)
         VALUES ('c', 'ann', 'person', 'Pick a colour', 'ready', 5, 5)",
    )
    .unwrap();
    assert_eq!(row(&conn), (json!("needs_you"), json!("needs_you"), true));
    conn.execute_batch("DELETE FROM tasks; UPDATE turns SET status = 'done' WHERE id = 'b1';")
        .unwrap();
    turn("b2", "bo", "waiting_sign_in", 4);
    assert_eq!(row(&conn), (json!("cant_run"), json!("cant_run"), true));
}
