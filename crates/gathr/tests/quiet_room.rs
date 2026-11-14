//! 0052 through a real service, with the engine played back: a chat where work is owed and nothing
//! moves is noticed (scenario 6c), a restart wakes nobody twice and counts no time down (6h), and a
//! turn that goes silent shows, with its lead told once (6d). The rule itself is in `quiet.rs`.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, first_token, held_recording, recording, rows, text, until,
    waiting,
};
use rusqlite::params;
use serde_json::{Value, json};

/// The looks, close together. Stuck time is counted from one check to the next when they are at
/// most two looks apart, so on a slow machine the person is told later, never earlier.
const LOOKS: (&str, &str) = ("GATHR_WAIT_EVERY_MS", "200");
/// What Gathr says of the task nobody is on, after "Not moving" and how long: to the lead, and
/// to the person.
const WAKE: &str = ": #1 (Ann). Nothing waits on the person.\n\
                    - #1 (Ann) is ready, and no turn is on it, for ";
const STUCK: &str = ": #1 (Ann). Write here to move it on.\n\
                     - #1 (Ann) is ready, and no turn is on it, for ";

fn env<'a>(more: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a Path)> {
    let all = [LOOKS].into_iter().chain(more.iter().copied());
    all.map(|(name, value)| (name, Path::new(value))).collect()
}

/// The service again on the same data, as `common::team` started it.
fn again(data: &Path, replay: &Path, more: &[(&str, &Path)]) -> TestService {
    let (go, tokens) = (data.join("go"), data.join("env.jsonl"));
    let mut all = vec![
        ("GATHR_REPLAY", replay),
        ("GATHR_REPLAY_HOLD", go.as_path()),
        ("GATHR_REPLAY_ENV", tokens.as_path()),
    ];
    all.extend_from_slice(more);
    TestService::with_engine(data, &all)
}

/// Pauses the chat, or with `/api/pauses/resume` resumes it.
fn pause(service: &TestService, path: &str, chat: &str) {
    let scope = json!({ "scope": "chat", "target": chat });
    let done = service.send("POST", path, scope);
    assert_eq!(done.status, 200, "{}", done.body);
}

fn resume(service: &TestService, chat: &str) {
    pause(service, "/api/pauses/resume", chat);
}

/// A team whose chat owes work nobody is on: a ready task of Ann's with no turn, as a stop of
/// the service can leave one, written while the service is down. The chat is paused: it waits on
/// the person, so nothing is noticed until the test resumes it. The service, started again, the
/// chat and the lead.
fn owing(data: &Path, more: &[(&str, &Path)]) -> (TestService, String, String) {
    let replay = recording("signed_in");
    let (service, chat, lead) = common::team(data, &replay, more);
    pause(&service, "/api/pauses", &chat);
    service.stop();
    let conn = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
    let now = gathr::ids::now_us();
    conn.execute(
        "INSERT INTO tasks (chat_id, owner, kind, what, state, created_ts, updated_ts)
         SELECT ?1, id, 'work', 'Fix the build', 'ready', ?2, ?2 FROM agents WHERE name = 'Ann'",
        params![chat, now],
    )
    .unwrap();
    drop(conn);
    (again(data, &replay, more), chat, lead)
}

/// Gathr's messages in the chat that say the chat isn't moving, with `words`.
fn said(history: &Value, words: &str) -> Vec<Value> {
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let says = |m: &&Value| {
        let body = m["body"].as_str().unwrap_or_default();
        m["role"] == "gathr" && body.starts_with("Not moving") && body.contains(words)
    };
    messages.iter().filter(says).cloned().collect()
}

fn turns_of(history: &Value, agent: &str) -> Vec<Value> {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.into_iter().filter(|t| t["agent"] == agent).collect()
}

/// The chat's `quiet` row.
fn kept(data: &Path, chat: &str) -> Option<Value> {
    let sql = format!(
        "SELECT since_ts, stretch_us, counted_ts, total_us, wake_turn, told_ts FROM quiet
         WHERE chat_id = '{chat}'"
    );
    rows(data, &sql).into_iter().next()
}

/// The stuck stretches recorded so far, oldest first.
fn stretches(data: &Path) -> Vec<Value> {
    let sql =
        "SELECT duration_us, attrs FROM event_log WHERE name = 'gathr.quiet.stretch' ORDER BY ts";
    rows(data, sql)
}

/// Whether the chat needs the person, as the chat list says it.
fn needs(service: &TestService, chat: &str) -> Value {
    let chats = service.call("GET", "/api/chats").json();
    let row = chats.as_array().unwrap().iter().find(|c| c["id"] == chat);
    row.unwrap_or_else(|| panic!("the chat isn't listed: {chats}"))["needs"].clone()
}

/// A member of the chat, by name.
fn member(service: &TestService, chat: &str, name: &str) -> Value {
    let agents = waiting(service, chat)["agents"].clone();
    let found = agents
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["name"] == name);
    found.unwrap_or_else(|| panic!("{name} isn't in the chat: {agents}"))["agent"].clone()
}

fn write(service: &TestService, chat: &str, text: &str) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": text }));
    assert_eq!(sent.status, 200, "{}", sent.body);
}

/// Scenario 6c. Work is owed and nothing moves: the lead is woken once, on no task; once the chat
/// has been stuck for the telling time the person is told in the chat and the chat needs them; and
/// when they write, the mark goes and the stretch is recorded with its length and what wasn't
/// moving. Stuck again with the same task, nobody is woken or told a second time.
#[test]
fn a_stuck_chat_is_noticed_and_its_dead_time_recorded() {
    let data = TempDir::new();
    let (service, chat, lead) = owing(&data.0, &env(&[("GATHR_QUIET_TELL_MS", "300")]));
    let mut live = service.live();
    resume(&service, &chat);

    let history = service.wait_for(&chat, 30, "the person to be told", |h| {
        !said(h, STUCK).is_empty()
    });
    assert_eq!(said(&history, STUCK).len(), 1, "told once: {history}");
    assert_eq!(said(&history, STUCK)[0]["turn"], Value::Null, "{history}");
    let wakes = said(&history, WAKE);
    assert_eq!(wakes.len(), 1, "woken once: {history}");
    let woken = turns_of(&history, &lead);
    assert_eq!(woken.len(), 1, "{history}");
    assert_eq!(
        (&woken[0]["id"], &woken[0]["task"]),
        (&wakes[0]["turn"], &Value::Null),
        "the wake is the lead's turn, on no task: {history}"
    );
    assert_eq!(
        needs(&service, &chat),
        json!(true),
        "told, the chat needs the person"
    );
    // Every window hears it, so the chat's mark in the list needs no reload.
    let mut heard = std::iter::from_fn(|| Some(common::next_live(&mut live)));
    let marked =
        |e: &Value| e["type"] == "chat_state" && e["chat"] == chat.as_str() && e["needs"] == true;
    assert!(heard.any(|e| marked(&e)));

    // They write to Ann, whose turn ends at once and moves nothing. The lead's engine is still
    // open and has played its recording, which holds one turn: a second turn of the lead's would
    // never end, and the chat would never be stuck again.
    let to_ann = json!({ "text": "Go on", "agent": member(&service, &chat, "Ann") });
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), to_ann);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let stretch = until(20, "the stretch to be recorded", || {
        stretches(&data.0).into_iter().next()
    });
    let attrs = &stretch["attrs"];
    assert_eq!(
        (&attrs["ended_by"], &attrs["woke"], &attrs["told"]),
        (&json!("wrote"), &json!(true), &json!(true)),
        "{stretch}"
    );
    assert_eq!(
        attrs["what"],
        json!(["#1 (Ann) is ready, and no turn is on it"]),
        "{stretch}"
    );
    let length = stretch["duration_us"].as_i64().unwrap();
    assert!(length >= 300_000, "told at 300 ms of stuck time: {stretch}");
    assert_eq!(needs(&service, &chat), json!(false), "the mark goes");

    // Ann's turn over, the chat is stuck again, with the task they were told of. Twice the telling
    // time on, nothing more is said, to them or to the lead.
    let row = until(30, "the chat to be stuck again, for 600 ms", || {
        kept(&data.0, &chat).filter(|row| row["stretch_us"].as_i64() >= Some(600_000))
    });
    assert_eq!(row["told_ts"], Value::Null, "told of the same task: {row}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(said(&history, STUCK).len(), 1, "told again: {history}");
    assert_eq!(said(&history, WAKE).len(), 1, "woken again: {history}");
    assert_eq!(needs(&service, &chat), json!(false));
}

/// Scenario 6h. A restart wakes nobody twice, and the time the service was down isn't counted: an
/// hour down, with the person told after one minute, tells nobody.
#[test]
fn a_restart_wakes_nobody_twice_and_counts_no_time_down() {
    let data = TempDir::new();
    let more = env(&[("GATHR_QUIET_TELL_MS", "60000")]);
    let (service, chat, lead) = owing(&data.0, &more);
    resume(&service, &chat);
    service.wait_for(&chat, 30, "the lead's wake to end", |h| {
        turns_of(h, &lead).iter().any(|t| t["status"] == "done")
    });
    service.stop();

    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    let hour_back = gathr::ids::now_us() - 3_600_000_000;
    let stuck_since = "SELECT since_ts FROM quiet WHERE chat_id = ?1";
    let since: Option<i64> = conn.query_row(stuck_since, [&chat], |r| r.get(0)).unwrap();
    let changed = conn
        .execute(
            "UPDATE quiet SET counted_ts = ?1 WHERE chat_id = ?2 AND wake_turn IS NOT NULL",
            params![hour_back, chat],
        )
        .unwrap();
    assert_eq!(
        changed, 1,
        "the stretch and its wake are kept across the stop"
    );
    drop(conn);

    let service = again(&data.0, &recording("signed_in"), &more);
    let row = until(30, "the first check after the restart", || {
        kept(&data.0, &chat).filter(|row| row["counted_ts"].as_i64() > Some(hour_back))
    });
    assert_eq!(
        row["told_ts"],
        Value::Null,
        "the hour down was counted: {row}"
    );
    assert!(row["total_us"].as_i64() < Some(60_000_000), "{row}");
    assert_eq!(
        row["since_ts"],
        json!(since),
        "still the same stretch: {row}"
    );
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(turns_of(&history, &lead).len(), 1, "woken twice: {history}");
    assert_eq!(said(&history, WAKE).len(), 1, "{history}");
}

/// A chat is looked at as soon as a write may have moved it, not only at the service's next look.
/// With the looks two minutes apart, as they are, the first is made as the service starts, and by
/// the time the lead is woken it is over. The person's message then ends the stretch, well before
/// the second.
#[test]
fn a_chat_is_looked_at_when_a_write_may_have_moved_it() {
    let data = TempDir::new();
    let (service, chat, _) = owing(&data.0, &[]);
    resume(&service, &chat);
    service.wait_for(&chat, 20, "the lead's wake", |h| !said(h, WAKE).is_empty());
    write(&service, &chat, "Go on");
    let stretch = until(20, "the stretch to end before the next look", || {
        stretches(&data.0).into_iter().next()
    });
    assert_eq!(stretch["attrs"]["ended_by"], "wrote", "{stretch}");
}

/// Scenario 6d on Claude Code.
#[test]
fn a_silent_turn_shows_and_its_lead_is_told_once() {
    let data = TempDir::new();
    let stall = env(&[("GATHR_STALL_AFTER_MS", "200")]);
    let (service, chat, lead) = common::team(&data.0, &held_recording(&data.0), &stall);
    write(&service, &chat, "Plan it");
    let ask = json!({ "to": "Ann", "what": "Write the notes" });
    let token = first_token(&data.0);
    let given = text(&call_tool(service.info.port, &token, "give_task", ask));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");

    common::silent_turn_shows_and_its_lead_is_told_once(&service, &data.0, &chat, &lead);
    std::fs::write(data.0.join("go"), "").unwrap();
}

/// The recording that asks for an approval, held once the answer arrives until `go` exists: the
/// turn then runs on with nothing said.
fn approval_then_held(dir: &Path) -> PathBuf {
    let frames = std::fs::read_to_string(recording("approval")).unwrap();
    let rows: Vec<&str> = frames.lines().collect();
    let answer = rows
        .iter()
        .rposition(|row| row.starts_with(r#"{"dir":"in""#));
    let answer = answer.expect("the recording takes an answer");
    let mut held = rows[..=answer].to_vec();
    held.push(r#"{"dir":"hold"}"#);
    held.extend(&rows[answer + 1..]);
    let path = dir.join("approval-then-held.jsonl");
    std::fs::write(&path, held.join("\n")).unwrap();
    path
}

/// Scenario 6d: a turn that waits for the person's approval isn't silent, however long they take.
/// Answered late, its silence is counted from the answer, not from before it asked.
#[test]
fn an_approval_answered_late_doesnt_flag_its_turn() {
    let data = TempDir::new();
    let stall = env(&[("GATHR_STALL_AFTER_MS", "200")]);
    let (service, chat, _) = common::team(&data.0, &approval_then_held(&data.0), &stall);
    let ann = member(&service, &chat, "Ann");
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Write it", "agent": ann }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let history = service.wait_for(&chat, 20, "Ann's turn to ask", |h| {
        !h["approvals"].as_array().unwrap().is_empty()
    });

    // The limit is 400 ms while a tool is open: a second on, the wait is well past it.
    let seen = gathr::ids::now_us();
    let asked = until(20, "the approval to wait past the limit", || {
        let said = waiting(&service, &chat);
        (said["at_us"].as_i64()? >= seen + 1_000_000).then_some(said)
    });
    let lines = asked["lines"].as_array().unwrap();
    assert!(lines.iter().any(|l| l["kind"] == "approval"), "{asked}");
    let silent = |said: &Value| {
        let lines = said["lines"].as_array().cloned().unwrap_or_default();
        lines.into_iter().find(|l| l["silent_us"].is_i64())
    };
    assert_eq!(
        silent(&asked),
        None,
        "silent while it waits on the person: {asked}"
    );
    // From when it was seen to ask: an engine slow to start may have been flagged before it asked.
    let flags = format!(
        "SELECT name, attrs FROM event_log WHERE ts >= {seen}
         AND name IN ('gathr.turn.stalled', 'gathr.turn.silent_told')"
    );
    assert_eq!(
        rows(&data.0, &flags),
        Vec::<Value>::new(),
        "flagged while it waits"
    );

    let answered = gathr::ids::now_us();
    let approval = history["approvals"][0]["request_id"].as_str().unwrap();
    let to = format!("/api/chats/{chat}/approvals/{approval}");
    let allowed = service.send("POST", &to, json!({ "allow": true }));
    assert_eq!(allowed.status, 200, "{}", allowed.body);
    // The engine holds from here: the turn does go silent, and only from the answer on.
    let line = until(20, "the turn to go silent after the answer", || {
        silent(&waiting(&service, &chat))
    });
    assert!(
        line["silent_us"].as_i64() >= Some(answered),
        "its silence counts the time it waited on the person: {line}, answered at {answered}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
}
