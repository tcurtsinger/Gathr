//! 0062, scenarios 2c, 2d and 3b through a real service, on each engine's replay stand-in: the lead's
//! plan is checked by the Reviewer's thumbs-up, waits on the person and is handed out by Approve; a
//! check that runs late reaches the person, and Send back wakes the lead, whose next version stops
//! the old check.

mod common;

use std::path::Path;

use common::{TempDir, call_tool, gathr, nth, rows, text, turn_on, until, waiting, with_reviewer};
use serde_json::{Value, json};

const ASK: &str = "Add tags to notes";

/// Claude Code's stand-in plays one recording per engine: the first turn held until `go`, then a
/// turn for each copy after it, for what Gathr queues later. Codex's replays and holds each turn.
fn claude_turns(data: &Path) {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let mut all = vec![lines[0].to_string(), r#"{"dir":"hold"}"#.to_string()];
    all.extend(lines[1..].iter().map(|l| l.to_string()));
    for _ in 0..4 {
        all.extend(lines.iter().map(|l| l.to_string()));
    }
    std::fs::write(data.join("held.jsonl"), all.join("\n") + "\n").unwrap();
}

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(std::path::PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

/// A team chat on `engine` (Lead, Ann, Rev as its Reviewer) whose engines hold until `go`.
fn room(data: &Path, engine: &str, more: &[(&str, &Path)]) -> (common::TestService, String) {
    let (service, chat) = common::team_on_with(data, engine, more);
    if engine == "claude" {
        claude_turns(data);
    }
    with_reviewer(&service, &chat);
    (service, chat)
}

/// The person's message: the turn it opened, once that turn runs, and the lead's token.
fn asked(service: &common::TestService, data: &Path, chat: &str) -> (String, String) {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": ASK }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let lead = common::first_token(data);
    turn_is(service, chat, &turn, &["running"]);
    (turn, lead)
}

fn turn_is(service: &common::TestService, chat: &str, turn: &str, statuses: &[&str]) -> Value {
    let what = format!("turn {turn} to be {statuses:?}");
    let history = service.wait_for(chat, 30, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && statuses.iter().any(|s| t["status"] == *s))
    });
    let turns = history["turns"].as_array().unwrap();
    turns.iter().find(|t| t["id"] == turn).unwrap().clone()
}

fn part(what: &str, after: Value) -> Value {
    json!({ "to": "Ann", "what": what, "why": "Notes need tags", "done": "A note carries its tags",
        "proof": "tests/tags.rs passes", "where": "no code", "size": "src/tags.rs", "lines": 60, "after": after })
}

fn plan_call(parts: Vec<Value>) -> Value {
    json!({ "title": ASK, "text": "Notes carry tags, shown in the list.", "parts": parts })
}

fn first_plan() -> Value {
    plan_call(vec![
        part("Add the tags table", json!([])),
        part("Show tags in the list", json!([1])),
    ])
}

fn plan(service: &common::TestService, id: i64) -> Value {
    let read = service.call("GET", &format!("/api/plans/{id}"));
    assert_eq!(read.status, 200, "{}", read.body);
    read.json()
}

fn listed(service: &common::TestService, chat: &str) -> Value {
    let listed = service.call("GET", &format!("/api/chats/{chat}/tasks"));
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

fn needs(service: &common::TestService, chat: &str) -> Value {
    let chats = service.call("GET", "/api/chats").json();
    let found = chats.as_array().unwrap().iter().find(|c| c["id"] == chat);
    found.unwrap()["needs"].clone()
}

/// What Gathr said in no turn.
fn said(service: &common::TestService, chat: &str) -> Vec<String> {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let lines = messages
        .iter()
        .filter(|m| m["role"] == "gathr" && m["turn"].is_null());
    lines
        .filter_map(|m| m["body"].as_str().map(String::from))
        .collect()
}

fn keys(value: &Value) -> Vec<&str> {
    let mut keys: Vec<&str> = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    keys
}

fn a_plan_is_checked_approved_and_handed_out_through_the_runner(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = room(&data.0, engine, &[]);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let (lead_turn, lead) = asked(&service, &data.0, &chat);

    let wrote = text(&call_tool(port, &lead, "plan", first_plan()));
    assert_eq!(
        wrote,
        format!(
            "Wrote version 1 of plan #1, \"{ASK}\": 2 parts. Rev checks it in #1; the person sees it after."
        )
    );
    let rev_turn = turn_on(&service, &chat, 1, &["running"]);
    assert_eq!(
        common::opened(&data.0, &json!(rev_turn)).0,
        json!("plan_check")
    );
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let brief = messages
        .iter()
        .find(|m| m["turn"] == rev_turn.as_str() && m["role"] == "gathr");
    let brief = brief.and_then(|m| m["body"].as_str()).unwrap_or_default();
    assert!(
        brief.starts_with("Task #1 from Lead: Check plan v1"),
        "{history}"
    );
    let rev = nth(&data.0.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .expect("Rev's token")
        .to_string();

    let message = plan(&service, 1)["versions"][0]["message"]
        .as_str()
        .unwrap()
        .to_string();
    let reacted = text(&call_tool(
        port,
        &rev,
        "react",
        json!({ "message": message }),
    ));
    assert_eq!(
        reacted,
        format!(
            "Gave {message} a thumbs-up.\nRecorded: your check #1 of version 1 is done when the turn running on it ends. \
             End it now. Write only what is still open, not that you agree."
        )
    );
    let member = text(&call_tool(port, &rev, "plan", first_plan()));
    let why = "Only Lead, the chat's Supervisor, writes plans.";
    assert!(
        member.starts_with(&format!("plan failed: {why} (ref ")),
        "{member}"
    );
    let cancel = text(&call_tool(port, &lead, "cancel_task", json!({ "task": 1 })));
    let why = "Task #1 is plan #1's check: Gathr ends it when a newer version is written or the plan is decided.";
    assert!(
        cancel.starts_with(&format!("cancel_task failed: {why} (ref ")),
        "{cancel}"
    );

    std::fs::write(data.0.join("go"), "").unwrap();
    let shown = until(30, "the plan to wait on the person", || {
        let shown = plan(&service, 1);
        (shown["state"] == "waiting").then_some(shown)
    });
    assert_eq!(shown["asked"], ASK, "{shown}");
    assert_eq!(
        shown["versions"][0]["check"]["outcome"], "agrees",
        "{shown}"
    );
    assert_eq!(
        shown["versions"][0]["parts"].as_array().map(Vec::len),
        Some(2)
    );
    until(30, "the chat to need the person", || {
        (needs(&service, &chat) == true).then_some(())
    });
    let lines = waiting(&service, &chat)["lines"].clone();
    let lines = lines.as_array().cloned().unwrap_or_default();
    let line = lines.iter().find(|l| l["kind"] == "plan");
    let line = line.unwrap_or_else(|| panic!("no plan line: {lines:?}"));
    assert_eq!(line["what"], format!("A plan for you: {ASK}"));
    let agrees = format!("Rev agrees with version 1 of the plan \"{ASK}\".");
    assert!(
        said(&service, &chat).contains(&agrees),
        "{:?}",
        said(&service, &chat)
    );
    turn_is(&service, &chat, &lead_turn, &["done"]);
    let backs = rows(
        &data.0,
        "SELECT id FROM turns WHERE opened_by = 'hear_back'",
    );
    assert_eq!(backs, Vec::<Value>::new(), "the lead isn't woken");

    let tasks = listed(&service, &chat);
    let summary = &tasks["plans"][0];
    assert_eq!(
        keys(summary),
        ["ask", "id", "lead", "state", "title", "versions"],
        "{tasks}"
    );
    let version = &summary["versions"][0];
    let want = [
        "check_line",
        "decided",
        "decided_line",
        "id",
        "message",
        "n",
        "parts",
        "sent",
    ];
    assert_eq!(keys(version), want, "{tasks}");
    assert_eq!(tasks["tasks"][0]["checks"], version["id"], "{tasks}");

    let approved = service.send("POST", "/api/plans/1/approve", json!({ "version": 1 }));
    assert_eq!(approved.status, 200, "{}", approved.body);
    assert_eq!(approved.json()["state"], "approved", "{}", approved.body);
    let tasks = listed(&service, &chat);
    let mut made: Vec<(i64, &str)> = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["plan"] == 1)
        .map(|t| (t["number"].as_i64().unwrap(), t["what"].as_str().unwrap()))
        .collect();
    made.sort();
    assert_eq!(
        made,
        [(2, "Add the tags table"), (3, "Show tags in the list")]
    );
    turn_on(&service, &chat, 2, &["queued", "running", "done"]);
    until(30, "the chat to stop needing the person", || {
        (needs(&service, &chat) == false).then_some(())
    });

    // The forced failure: the same Approve again, refused with a reference that finds it.
    let again = service.send("POST", "/api/plans/1/approve", json!({ "version": 1 }));
    assert_eq!(again.status, 409, "{}", again.body);
    let refused = again.json();
    let why = "Version 1 is already approved.";
    assert_eq!(
        (&refused["error"], &refused["why"]),
        (&json!("Approve failed"), &json!(why))
    );
    let reference = refused["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events
        .iter()
        .find(|e| e["name"] == "gathr.api.request" && e["status"] == "error");
    let failed = failed.map(|e| (&e["attrs"]["error"], &e["attrs"]["reason"]));
    assert!(
        ok && failed == Some((&json!("Approve failed"), &json!(why))),
        "{trace}"
    );
}

fn a_late_check_reaches_the_person_and_send_back_wakes_the_lead(engine: &str) {
    let data = TempDir::new();
    let more = [
        ("GATHR_PLAN_CHECK_MS", Path::new("500")),
        ("GATHR_WAIT_EVERY_MS", Path::new("200")),
    ];
    let (service, chat) = room(&data.0, engine, &more);
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    let port = service.info.port;
    let (lead_turn, lead) = asked(&service, &data.0, &chat);
    // The lead's first turn ends before the check starts, so only the check's turn is held.
    std::fs::write(&go, "").unwrap();
    turn_is(&service, &chat, &lead_turn, &["done"]);
    std::fs::remove_file(&go).unwrap();

    let wrote = text(&call_tool(port, &lead, "plan", first_plan()));
    assert!(wrote.starts_with("Wrote version 1 of plan #1"), "{wrote}");
    let rev_turn = turn_on(&service, &chat, 1, &["running"]);
    let shown = until(30, "the late version to reach the person", || {
        let shown = plan(&service, 1);
        (shown["state"] == "waiting").then_some(shown)
    });
    assert_eq!(shown["versions"][0]["check"]["outcome"], "late", "{shown}");
    let late = format!("Rev didn't finish checking version 1 of the plan \"{ASK}\".");
    assert_eq!(said(&service, &chat), [late]);

    let words = json!({ "version": 1, "words": "Split part 2" });
    let back = service.send("POST", "/api/plans/1/send-back", words);
    assert_eq!(back.status, 200, "{}", back.body);
    assert_eq!(back.json()["state"], "revising", "{}", back.body);
    let sql = "SELECT u.id, u.status, u.ask_id = (SELECT ask_id FROM turns WHERE id = u.answers) AS same_ask
               FROM turns u WHERE u.opened_by = 'sent_back'";
    let woken = until(30, "the lead's sent-back turn to run", || {
        let found = rows(&data.0, sql);
        let ran = |r: &Value| r["status"] == "running" || r["status"] == "done";
        found.first().filter(|r| ran(r)).cloned()
    });
    assert_eq!(woken["same_ask"], 1, "{woken}");

    let next = plan_call(vec![
        part("Add the tags table", json!([])),
        part("Show tags on each note", json!([1])),
        part("Show tags in the sidebar", json!([1])),
    ]);
    let wrote = text(&call_tool(port, &lead, "plan", next));
    assert_eq!(
        wrote,
        format!(
            "Wrote version 2 of plan #1, \"{ASK}\": 3 parts. Rev checks it in #2; the person sees it after."
        )
    );
    // Send back cancelled the old check (H6a.1); its held turn stops on the runner's stall look.
    let stopped = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.turn.stop'";
    until(40, "the old check's turn to be stopped", || {
        (rows(&data.0, stopped)[0]["n"] == 1).then_some(())
    });
    std::fs::write(&go, "").unwrap();
    let ended = turn_is(&service, &chat, &rev_turn, &["stopped"]);
    assert_eq!(
        ended["why"], "its task #1 was cancelled: version 1 was sent back",
        "{ended}"
    );
}

#[test]
fn a_plan_is_checked_approved_and_handed_out_through_the_runner_on_claude() {
    a_plan_is_checked_approved_and_handed_out_through_the_runner("claude");
}

#[test]
fn a_plan_is_checked_approved_and_handed_out_through_the_runner_on_codex() {
    a_plan_is_checked_approved_and_handed_out_through_the_runner("codex");
}

#[test]
fn a_late_check_reaches_the_person_and_send_back_wakes_the_lead_on_claude() {
    a_late_check_reaches_the_person_and_send_back_wakes_the_lead("claude");
}

#[test]
fn a_late_check_reaches_the_person_and_send_back_wakes_the_lead_on_codex() {
    a_late_check_reaches_the_person_and_send_back_wakes_the_lead("codex");
}
