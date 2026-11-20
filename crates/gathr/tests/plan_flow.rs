//! 0062 and 0080 through a real service, on each engine's replay stand-in: the lead's plan is
//! checked in the open by the members who took no part, one with a thumbs-up and one in words; the
//! lead's last word writes the next version, which reaches the person and is handed out by Approve.
//! A round that runs late still reaches the person, and Send back wakes the lead.

mod common;

use std::path::Path;

use common::{TempDir, call_tool, gathr, nth, rows, text, until, waiting, with_reviewer};
use serde_json::{Value, json};

const ASK: &str = "Add tags to notes";

/// Every turn of every engine holds until `go`, and each `go` lets one held turn on. Claude Code's
/// stand-in plays one recording per engine, a copy for each turn Gathr queues; Codex's replays it
/// for each turn.
fn every_turn_holds(data: &Path, engine: &str) {
    let take = r#"{"dir":"hold","line":"take"}"#;
    let held = match engine {
        "codex" => {
            let held = common::held_recording(data, "codex_signed_in");
            let text = std::fs::read_to_string(&held).unwrap();
            text.replace(r#"{"dir":"hold"}"#, take)
        }
        _ => {
            let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
            let lines: Vec<&str> = frames.lines().collect();
            let mut all = Vec::new();
            for _ in 0..6 {
                all.push(lines[0].to_string());
                all.push(take.to_string());
                all.extend(lines[1..].iter().map(|l| l.to_string()));
            }
            all.join("\n") + "\n"
        }
    };
    std::fs::write(data.join("held.jsonl"), held).unwrap();
}

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(std::path::PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        for _ in 0..20 {
            let _ = std::fs::write(&self.0, "");
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
}

/// Lets the held turn of the engine started `n`th go on, once it has taken its own file.
fn go_on(data: &Path, n: usize) {
    let pid = nth(&data.join("env.jsonl"), n)["pid"].to_string();
    let go = data.join(format!("go.{pid}"));
    std::fs::write(&go, "").unwrap();
    until(30, &format!("engine {n}'s held turn to go on"), || {
        (!go.exists()).then_some(())
    });
}

/// A team chat on `engine` (Lead, Ann, Rev as its Reviewer) whose every turn holds.
fn room(data: &Path, engine: &str, more: &[(&str, &Path)]) -> (common::TestService, String) {
    let mut more = more.to_vec();
    more.push(("GATHR_OPEN_CHECK", Path::new("on")));
    let (service, chat) = common::team_on_with(data, engine, &more);
    every_turn_holds(data, engine);
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
    turns_are(service, chat, |t| {
        t["id"] == turn.as_str() && t["status"] == "running"
    });
    (turn, lead)
}

/// The chat's history once `holds` holds for some turn of it.
fn turns_are(service: &common::TestService, chat: &str, holds: impl Fn(&Value) -> bool) -> Value {
    service.wait_for(chat, 30, "a turn", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns.iter().any(&holds)
    })
}

/// The turns the words of `kind` opened, oldest first, with their status.
fn opened(data: &Path, kind: &str) -> Vec<(String, String)> {
    let sql = format!("SELECT id, status FROM turns WHERE opened_by = '{kind}' ORDER BY rowid");
    let found = rows(data, &sql);
    let row = |r: &Value| {
        (
            r["id"].as_str().unwrap().into(),
            r["status"].as_str().unwrap().into(),
        )
    };
    found.iter().map(row).collect()
}

fn part(what: &str) -> Value {
    json!({ "to": "Ann", "what": what, "why": "Notes need tags", "done": "A note carries its tags",
        "proof": "tests/tags.rs passes", "where": "no code", "size": "src/tags.rs", "lines": 60, "after": [] })
}

fn plan_call(parts: Vec<Value>) -> Value {
    json!({ "title": ASK, "text": "Notes carry tags, shown in the list.", "parts": parts })
}

fn plan(service: &common::TestService, id: i64) -> Value {
    let read = service.call("GET", &format!("/api/plans/{id}"));
    assert_eq!(read.status, 200, "{}", read.body);
    read.json()
}

/// A turn's topic as history gives it.
fn topic_of(history: &Value, turn: &str) -> Value {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["id"] == turn);
    found.unwrap_or_else(|| panic!("no turn {turn}: {history}"))["topic"].clone()
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

fn a_plan_is_checked_in_the_open_and_offered_after_the_last_word(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = room(&data.0, engine, &[]);
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    let port = service.info.port;
    let (lead_turn, lead) = asked(&service, &data.0, &chat);

    let wrote = text(&call_tool(
        port,
        &lead,
        "plan",
        plan_call(vec![part("Add the tags table")]),
    ));
    assert_eq!(
        wrote,
        format!(
            "Wrote version 1 of plan #1, \"{ASK}\": 1 part. The members who took no part check it once this turn \
             ends; the person sees it after."
        )
    );
    // The forced failure: Approve before the round, refused with a reference that finds it.
    let early = service.send("POST", "/api/plans/1/approve", json!({ "version": 1 }));
    assert_eq!(early.status, 409, "{}", early.body);
    let refused = early.json();
    let why = "Version 1 is still being checked.";
    assert_eq!(refused["why"], why, "{refused}");
    let (ok, trace) = gathr(
        &data.0,
        &["diag", "trace", refused["ref"].as_str().unwrap()],
    );
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events
        .iter()
        .find(|e| e["name"] == "gathr.api.request" && e["status"] == "error");
    assert!(
        ok && failed.is_some_and(|e| e["attrs"]["reason"] == why),
        "{trace}"
    );

    // A check names what it agrees with as `answer` (0082); a turn that checks nothing has none.
    let answer = json!({ "message": "answer" });
    let refused = call_tool(port, &lead, "react", answer.clone());
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        text(&refused).starts_with(
            "react failed: only a check has an answer to agree with: give the message's id. (ref "
        ),
        "{refused}"
    );

    // The lead's turn ends: Ann and Rev, who took no part, each get a check turn.
    go_on(&data.0, 0);
    turns_are(&service, &chat, |t| {
        t["id"] == lead_turn.as_str() && t["status"] == "done"
    });
    let checks = until(30, "both check turns to run", || {
        let checks = opened(&data.0, "check");
        (checks.len() == 2 && checks.iter().all(|c| c.1 == "running")).then_some(checks)
    });
    let version = plan(&service, 1)["versions"][0]["message"]
        .as_str()
        .unwrap()
        .to_string();
    let (first, second) = (
        nth(&data.0.join("env.jsonl"), 1),
        nth(&data.0.join("env.jsonl"), 2),
    );
    let agreer = first["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string();
    let reacted = text(&call_tool(port, &agreer, "react", answer));
    assert_eq!(
        reacted,
        format!(
            "Gave {version} a thumbs-up. That is your whole check: end your turn now and write nothing more."
        )
    );
    assert!(second["GATHR_AGENT_TOKEN"].is_string());
    go_on(&data.0, 1);
    go_on(&data.0, 2);
    // Both said the recording's words; the one with the thumbs-up agreed, the other questioned it.
    let last = until(30, "the lead's last word to run", || {
        let last = opened(&data.0, "last_word");
        last.first()
            .filter(|l| l.1 == "running")
            .map(|l| l.0.clone())
    });
    assert_eq!(plan(&service, 1)["state"], "checking");
    let next = plan_call(vec![part("Add the tags table, indexed")]);
    let wrote = text(&call_tool(port, &lead, "plan", next));
    assert_eq!(
        wrote,
        format!(
            "Wrote version 2 of plan #1, \"{ASK}\": 1 part. The person sees it when this turn ends."
        )
    );
    go_on(&data.0, 0);
    let shown = until(30, "version 2 to wait on the person", || {
        let shown = plan(&service, 1);
        (shown["state"] == "waiting").then_some(shown)
    });
    let sent: Vec<bool> = [0, 1]
        .map(|i| shown["versions"][i]["sent"].is_string())
        .into();
    assert_eq!(sent, [false, true], "{shown}");
    assert_eq!(opened(&data.0, "check").len(), 2, "no second round");
    let round = rows(
        &data.0,
        "SELECT attrs FROM event_log WHERE name = 'gathr.check.round'",
    );
    let attrs = &round[0]["attrs"];
    let counted = ["asked", "agreed", "objected", "silent", "last_word"]
        .map(|k| attrs[format!("gathr.check.{k}")].clone());
    assert_eq!(
        counted,
        [json!(2), json!(1), json!(1), json!(0), json!(true)],
        "{round:?}"
    );
    let lines = waiting(&service, &chat)["lines"].clone();
    let lines = lines.as_array().cloned().unwrap_or_default();
    assert!(lines.iter().any(|l| l["kind"] == "plan"), "{lines:?}");

    // Each check sits under the bubble that holds version 1; the last word, which wrote version 2,
    // is a main message of its own, and Approve's line sits under it.
    let approved = service.send("POST", "/api/plans/1/approve", json!({ "version": 2 }));
    assert_eq!(approved.status, 200, "{}", approved.body);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let topics: Vec<Value> = [&checks[0].0, &checks[1].0, &last]
        .iter()
        .map(|t| topic_of(&history, t))
        .collect();
    let (v1, v2) = (json!(format!("b:{lead_turn}")), json!(format!("b:{last}")));
    assert_eq!(topics, [v1.clone(), v1, v2], "{history}");
    let turns = history["turns"].as_array().unwrap();
    let kinds: Vec<&Value> = [&checks[0].0, &last]
        .iter()
        .map(|id| &turns.iter().find(|t| t["id"] == id.as_str()).unwrap()["opened"])
        .collect();
    assert_eq!(kinds, [&json!("check"), &json!("last_word")]);
    assert!(
        said(&service, &chat).iter().all(|l| !l.contains("agrees")),
        "no check line"
    );
    common::topics_agree(&data.0, &chat);
}

fn a_late_round_reaches_the_person_and_send_back_wakes_the_lead(engine: &str) {
    let data = TempDir::new();
    let more = [
        ("GATHR_PLAN_CHECK_MS", Path::new("500")),
        ("GATHR_WAIT_EVERY_MS", Path::new("200")),
    ];
    let (service, chat) = room(&data.0, engine, &more);
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    let port = service.info.port;
    let (_, lead) = asked(&service, &data.0, &chat);
    let wrote = text(&call_tool(
        port,
        &lead,
        "plan",
        plan_call(vec![part("Add the tags table")]),
    ));
    assert!(wrote.starts_with("Wrote version 1 of plan #1"), "{wrote}");
    go_on(&data.0, 0);
    // The check turns hold: the version reaches the person anyway.
    let shown = until(30, "the late version to reach the person", || {
        let shown = plan(&service, 1);
        (shown["state"] == "waiting").then_some(shown)
    });
    assert_eq!(shown["versions"][0]["check"], Value::Null, "{shown}");
    let late = format!("Ann and Rev didn't finish checking version 1 of the plan \"{ASK}\".");
    assert_eq!(said(&service, &chat), [late]);

    let words = json!({ "version": 1, "words": "Split part 1" });
    let back = service.send("POST", "/api/plans/1/send-back", words);
    assert_eq!(back.status, 200, "{}", back.body);
    assert_eq!(back.json()["state"], "revising", "{}", back.body);
    let sql = "SELECT u.id, u.status, u.ask_id = (SELECT ask_id FROM turns WHERE id = u.answers) AS same_ask
               FROM turns u WHERE u.opened_by = 'sent_back'";
    let woken = until(30, "the lead's sent-back turn to run", || {
        let found = rows(&data.0, sql);
        found.first().filter(|r| r["status"] == "running").cloned()
    });
    assert_eq!(woken["same_ask"], 1, "{woken}");
    let next = plan_call(vec![
        part("Add the tags table"),
        part("Show tags on each note"),
    ]);
    let wrote = text(&call_tool(port, &lead, "plan", next));
    assert!(
        wrote
            .starts_with("Wrote version 2 of plan #1, \"Add tags to notes\": 2 parts. The members"),
        "{wrote}"
    );
}

#[test]
fn a_plan_is_checked_in_the_open_and_offered_after_the_last_word_on_claude() {
    a_plan_is_checked_in_the_open_and_offered_after_the_last_word("claude");
}

#[test]
fn a_plan_is_checked_in_the_open_and_offered_after_the_last_word_on_codex() {
    a_plan_is_checked_in_the_open_and_offered_after_the_last_word("codex");
}

#[test]
fn a_late_round_reaches_the_person_and_send_back_wakes_the_lead_on_claude() {
    a_late_round_reaches_the_person_and_send_back_wakes_the_lead("claude");
}

#[test]
fn a_late_round_reaches_the_person_and_send_back_wakes_the_lead_on_codex() {
    a_late_round_reaches_the_person_and_send_back_wakes_the_lead("codex");
}
