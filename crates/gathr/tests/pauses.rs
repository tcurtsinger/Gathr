//! Pause, Stop now, usage holds and background commands through a real service (0010, F10): a
//! pause holds Gathr's turns but never the person's, and outlasts a restart; Stop now ends what
//! runs and leaves a carry-on for Restart; a usage limit holds the turn instead of failing it; and
//! the turn Claude Code starts itself when a background command ends is kept.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, first_token as token, gathr, next_live, recording,
    sign_in_account, team, text, until,
};
use serde_json::{Value, json};

/// Recordings played one after the other by the same engine, one turn each.
fn joined(dir: &Path, name: &str, parts: &[PathBuf]) -> PathBuf {
    let text: Vec<String> = parts
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap().trim_end().to_string())
        .collect();
    let file = dir.join(name);
    std::fs::write(&file, text.join("\n") + "\n").unwrap();
    file
}

fn held(dir: &Path) -> PathBuf {
    common::held_recording(dir)
}

fn send(service: &TestService, chat: &str, text: &str) -> Value {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": text }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

/// The first turn's live updates until `last` takes one, each checked against history: when the
/// turn was made, and its place once it started, so a window following live shows and orders it
/// as a reload would (0028).
fn live_matches(live: &mut std::net::TcpStream, history: &Value, last: impl Fn(&Value) -> bool) {
    let turn = &history["turns"][0];
    let mut started = false;
    loop {
        let update = next_live(live);
        if update["type"] != "turn" || update["turn"] != turn["id"] {
            continue;
        }
        started |= update["status"] != "queued";
        assert_eq!(update["created"], turn["created"], "{update} vs {turn}");
        if started {
            assert_eq!(update["position"], turn["position"], "{update} vs {turn}");
        }
        if last(&update) {
            return;
        }
    }
}

fn pause(service: &TestService, path: &str, chat: &str) -> Value {
    let paused = service.send("POST", path, json!({ "scope": "chat", "target": chat }));
    assert_eq!(paused.status, 200, "{}", paused.body);
    paused.json()
}

#[test]
fn a_pause_holds_gathrs_turns_not_the_persons_and_outlasts_a_restart() {
    let data = TempDir::new();
    let both = joined(
        &data.0,
        "both.jsonl",
        &[held(&data.0), recording("signed_in")],
    );
    let (service, chat, _) = team(&data.0, &both, &[]);
    let paused = pause(&service, "/api/pauses", &chat);
    assert_eq!(
        paused["pauses"][0]["state"], "paused",
        "nothing runs yet: {paused}"
    );
    assert_eq!(
        send(&service, &chat, "Plan it")["status"],
        "running",
        "the person's own message"
    );
    let lead = token(&data.0);
    let ask = json!({ "to": "Ann", "what": "Write the notes" });
    let given = text(&call_tool(service.info.port, &lead, "give_task", ask));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    let history = service.wait_for(&chat, 10, "Ann's turn held", |h| {
        h["turns"][1]["held"] == "pause"
    });
    assert_eq!(
        history["run"]["state"], "pausing",
        "the lead's turn still runs: {history}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "nothing running", |h| {
        h["turns"][0]["status"] == "done" && h["run"]["state"] == "paused"
    });
    assert_eq!(history["turns"][1]["status"], "queued", "{history}");
    service.stop();

    let again = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let history = again.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(
        history["turns"][1]["held"], "pause",
        "held after a restart: {history}"
    );
    assert_eq!(history["run"]["state"], "paused", "{history}");
    pause(&again, "/api/pauses/resume", &chat);
    again.wait_for(&chat, 20, "Ann's turn after Resume", |h| {
        h["turns"][1]["status"] == "done"
    });
}

#[test]
fn stop_now_ends_the_turn_and_its_carry_on_waits_for_restart() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let parts = [recording("stopped_before_reply"), recording("signed_in")];
    let replay = joined(&data.0, "stop.jsonl", &parts);
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    let mut live = service.live();
    send(&service, &chat, "Write 2,000 words on rivers");
    service.wait_for(&chat, 10, "the turn", |h| {
        h["turns"][0]["status"] == "running"
    });
    let stopped = pause(&service, "/api/pauses/stop", &chat);
    assert_eq!(stopped["pauses"][0]["state"], "stopped", "{stopped}");
    let history = service.wait_for(&chat, 10, "the carry-on", |h| {
        h["turns"][0]["status"] == "stopped" && h["turns"][1]["held"] == "pause"
    });
    assert_eq!(history["run"]["state"], "stopped", "{history}");
    live_matches(&mut live, &history, |u| u["status"] == "stopped");
    assert_eq!(history["turns"][1]["agent"], lead.as_str());
    assert_eq!(
        history["turns"][1]["answers"], history["turns"][0]["id"],
        "{history}"
    );
    let said = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "gathr");
    let said = said
        .map(|m| m["body"].as_str().unwrap())
        .unwrap_or_default();
    assert!(said.starts_with("Stop now ended your turn."), "{said}");
    pause(&service, "/api/pauses/resume", &chat);
    let history = service.wait_for(&chat, 20, "the carry-on after Restart", |h| {
        h["turns"][1]["status"] == "done"
    });
    assert_eq!(history["run"]["state"], "running", "{history}");
}

#[test]
fn stop_now_keeps_the_carry_on_in_a_chat_sent_to_no_agent() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let parts = [recording("stopped_before_reply"), recording("signed_in")];
    let replay = joined(&data.0, "stop.jsonl", &parts);
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &replay)]);
    let account = service.add_account("Main");
    sign_in_account(&data.0, &account);
    let chat = service.new_chat();
    send(&service, &chat, "Write 2,000 words on rivers");
    service.wait_for(&chat, 10, "the turn", |h| {
        h["turns"][0]["status"] == "running"
    });
    pause(&service, "/api/pauses/stop", &chat);
    let history = service.wait_for(&chat, 10, "the carry-on", |h| {
        h["turns"][0]["status"] == "stopped" && h["turns"][1]["held"] == "pause"
    });
    assert_eq!(history["turns"][1]["agent"], Value::Null, "{history}");
    pause(&service, "/api/pauses/resume", &chat);
    service.wait_for(&chat, 20, "the carry-on after Restart", |h| {
        h["turns"][1]["status"] == "done"
    });
}

/// How many times the check has run: one line in its log each.
fn runs(log: &Path) -> usize {
    std::fs::read_to_string(log).map_or(0, |t| t.lines().count())
}

#[test]
fn a_pause_holds_a_waits_check_and_stays_pausing_while_one_runs() {
    let data = TempDir::new();
    let every = [("GATHR_WAIT_EVERY_MS", Path::new("100"))];
    let (service, chat, _) = team(&data.0, &held(&data.0), &every);
    send(&service, &chat, "Plan it");
    let lead = token(&data.0);
    let log = data.0.join("check.log");
    let check = format!(
        "echo ran >> '{}'; sleep 3; exit 1",
        log.display().to_string().replace('\\', "/")
    );
    let ask = json!({ "check": check, "then": "merge it" });
    let left = text(&call_tool(service.info.port, &lead, "wait", ask));
    assert!(left.starts_with("Waiting ("), "{left}");
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 10, "the lead's turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    until(10, "the check running", || (runs(&log) > 0).then_some(()));
    let paused = pause(&service, "/api/pauses", &chat);
    assert_eq!(
        paused["pauses"][0]["state"], "pausing",
        "the check still runs: {paused}"
    );
    until(10, "the pause quiet", || {
        let listed = service.call("GET", "/api/pauses").json();
        (listed["pauses"][0]["state"] == "paused").then_some(())
    });
    let ran = runs(&log);
    std::thread::sleep(std::time::Duration::from_millis(1500));
    assert_eq!(runs(&log), ran, "a check ran while its chat was paused");
}

#[test]
fn an_engine_that_closed_leaves_no_background_command_behind() {
    let data = TempDir::new();
    let full = std::fs::read_to_string(recording("background")).unwrap();
    let first: Vec<&str> = full
        .lines()
        .take_while(|l| !l.contains(r#""dir":"hold""#))
        .collect();
    let replay = data.0.join("first.jsonl");
    std::fs::write(&replay, first.join("\n") + "\n").unwrap();
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Run the pings in the background");
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let members = json!([{ "agent": lead, "role": "supervisor" }]);
    let other = service.send("POST", "/api/chats", json!({ "members": members }));
    let other = other.json()["id"].as_str().unwrap().to_string();
    send(&service, &other, "Run the pings in the background");
    service.wait_for(&other, 10, "the turn in another chat", |h| {
        h["turns"][0]["status"] == "done"
    });
    let paused = pause(&service, "/api/pauses", &chat);
    assert_eq!(
        paused["pauses"][0]["state"], "paused",
        "the first chat's engine closed with its command: {paused}"
    );
}

#[test]
fn a_usage_limit_holds_the_turn_until_the_reset_instead_of_failing_it() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &recording("usage_limit"), &[]);
    let mut live = service.live();
    send(&service, &chat, "Say hi");
    let history = service.wait_for(&chat, 10, "the usage hold", |h| {
        h["turns"][0]["held"] == "usage"
    });
    live_matches(&mut live, &history, |u| u["held"] == "usage");
    let turn = &history["turns"][0];
    assert_eq!(
        turn["status"], "queued",
        "sent again at the reset, as no tool ran: {history}"
    );
    assert!(
        turn["why"]
            .as_str()
            .unwrap()
            .starts_with("out of usage until 2100-01-01"),
        "{turn}"
    );
    let lead_seat = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == lead.as_str());
    assert_eq!(
        lead_seat.unwrap()["usage_until_us"],
        4_102_444_800_000_000_i64,
        "{history}"
    );
    let listed = service.call("GET", "/api/pauses").json();
    assert_eq!(listed["usage"][0]["kind"], "five_hour", "{listed}");
    let (ok, events) = gathr(
        &data.0,
        &[
            "diag",
            "query",
            "SELECT name FROM event_log WHERE name = 'gathr.usage.hold'",
        ],
    );
    assert!(
        ok && events["rows"].as_array().is_some_and(|r| r.len() == 1),
        "{events}"
    );
}

#[test]
fn the_turn_claude_code_starts_when_a_background_command_ends_is_kept() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &recording("background"), &[]);
    send(&service, &chat, "Run the pings in the background");
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let paused = pause(&service, "/api/pauses", &chat);
    let still = &paused["pauses"][0];
    assert_eq!(
        still["state"], "pausing",
        "a background command runs: {paused}"
    );
    assert_eq!(
        still["running"]["background"][0]["command"],
        "Run first ping in background"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the engine's own turn", |h| {
        h["turns"][1]["status"] == "done"
    });
    assert_eq!(
        history["turns"][1]["answers"], history["turns"][0]["id"],
        "{history}"
    );
    let last = history["messages"].as_array().unwrap().last().unwrap()["body"].clone();
    assert!(
        last.as_str().unwrap().starts_with("The first ping"),
        "{history}"
    );
    assert_eq!(history["run"]["state"], "paused", "{history}");
}

#[test]
fn a_turn_with_no_progress_is_flagged_never_stopped() {
    let data = TempDir::new();
    let stall = [("GATHR_STALL_AFTER_MS", Path::new("200"))];
    let (service, chat, _) = team(&data.0, &held(&data.0), &stall);
    send(&service, &chat, "Plan it");
    let sql = "SELECT name, error_type FROM event_log WHERE name = 'gathr.turn.stalled'";
    let found = until(10, "the stall flag", || {
        let (_, events) = gathr(&data.0, &["diag", "query", sql]);
        let rows = events["rows"].as_array().cloned().unwrap_or_default();
        (!rows.is_empty()).then_some(rows)
    });
    assert_eq!(found.len(), 1, "flagged once: {found:?}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(
        history["turns"][0]["status"], "running",
        "never stopped: {history}"
    );
}

/// G174, live on #139: a task's command ended while its owner's engine ran a turn on something
/// else, so Claude Code said so inside that turn and began none for the task. The task, held on the
/// command, settles there and then, as its owner had reported it done, not never.
#[test]
fn a_held_task_settles_when_its_command_ends_inside_another_turn() {
    let data = TempDir::new();
    let full = std::fs::read_to_string(recording("background")).unwrap();
    let rows: Vec<&str> = full.lines().collect();
    let held = rows
        .iter()
        .position(|l| l.contains(r#""dir":"hold""#))
        .unwrap();
    let init = r#"\"subtype\":\"init\""#;
    let woken = held + rows[held..].iter().position(|l| l.contains(init)).unwrap();
    // One engine, three turns: a first one; the task's, which starts the command; the person's,
    // which hears it end before it replies as the recording's last turn did.
    let first = std::fs::read_to_string(recording("signed_in")).unwrap();
    let mut played: Vec<&str> = first.lines().collect();
    played.extend(&rows[..held]);
    played.push(rows[0]);
    played.push(rows[woken]);
    played.extend(&rows[held + 1..woken]);
    played.extend(&rows[woken + 1..]);
    let replay = data.0.join("inside.jsonl");
    std::fs::write(
        &replay,
        played.join(
            "
",
        ) + "
",
    )
    .unwrap();
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Hi");
    service.wait_for(&chat, 10, "the lead's first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let given = call_tool(
        service.info.port,
        &token(&data.0),
        "give_task",
        json!({ "to": "Lead", "what": "Run the pings" }),
    );
    assert_ne!(given["isError"], true, "{given}");
    let tasks = format!("/api/chats/{chat}/tasks");
    let task = until(10, "the task held on its command", || {
        let listed = service.call("GET", &tasks).json();
        let task = listed["tasks"][0].clone();
        (task["state"] == "held").then_some(task)
    });
    assert!(
        task["reason"]
            .as_str()
            .unwrap()
            .contains("background command"),
        "{task}"
    );
    let report = json!({ "task": 1, "how": "done", "note": "the pings ran" });
    let said = text(&call_tool(
        service.info.port,
        &token(&data.0),
        "finish_task",
        report,
    ));
    assert!(
        said.starts_with("Recorded: done. Task #1 is held"),
        "{said}"
    );

    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Hi", "agent": lead }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let settled = until(10, "the task settled", || {
        let listed = service.call("GET", &tasks).json();
        let task = listed["tasks"][0].clone();
        (task["state"] != "held").then_some(task)
    });
    assert_eq!(settled["state"], "done", "{settled}");
}

/// A usage limit that cuts a turn after a tool ran ends the turn with a carry-on waiting for the
/// account. The turn's task stays as it was and says why: nobody is told it stopped (0047).
#[test]
fn a_usage_limit_after_a_tool_leaves_the_task_as_it_was() {
    let data = TempDir::new();
    // Made by hand from two hand-made recordings: the steps recording up to its first tool's
    // result, then the usage limit's event and the turn's end, in that session.
    let steps = std::fs::read_to_string(recording("steps_claude")).unwrap();
    let limit = std::fs::read_to_string(recording("usage_limit")).unwrap();
    let ran = steps.lines().position(|l| l.contains(r#"\"tool_result\""#));
    let mut limited: Vec<String> = steps
        .lines()
        .take(ran.unwrap() + 1)
        .map(String::from)
        .collect();
    let cut = limit
        .lines()
        .position(|l| l.contains(r#"\"type\":\"rate_limit_event\""#));
    let session = "a404d9af-ce93-4b65-bb15-1b300040a9f1";
    limited.extend(
        limit
            .lines()
            .skip(cut.unwrap())
            .map(|l| l.replace(session, "default")),
    );
    let first = std::fs::read_to_string(recording("signed_in")).unwrap();
    let replay = data.0.join("limited.jsonl");
    std::fs::write(
        &replay,
        format!("{}\n{}\n", first.trim_end(), limited.join("\n")),
    )
    .unwrap();
    let (service, chat, _) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Hi");
    service.wait_for(&chat, 10, "the lead's first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let ask = json!({ "to": "Lead", "what": "Run the steps" });
    let given = call_tool(service.info.port, &token(&data.0), "give_task", ask);
    assert_ne!(given["isError"], true, "{given}");
    let history = service.wait_for(&chat, 10, "the carry-on held for the account", |h| {
        h["turns"][1]["status"] == "stopped" && h["turns"][2]["held"] == "usage"
    });
    assert_eq!(history["turns"][2]["task"], 1, "{history}");
    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let task = &listed["tasks"][0];
    assert_eq!(
        (&task["state"], &task["reason"]),
        (
            &json!("working"),
            &json!("The account is out of usage. Carries on when it is back")
        ),
        "{listed}"
    );
}

/// The engine exits instead of answering Stop now's interrupt. The turn is over all the same: with
/// its carry-on saved, its task stays as it was, and nobody is told it failed (0047).
#[test]
fn an_engine_that_exits_after_stop_now_leaves_the_task_to_its_carry_on() {
    let data = TempDir::new();
    // The lead's first turn; then a turn that holds and exits with no word, as a crash would.
    let first = std::fs::read_to_string(recording("signed_in")).unwrap();
    let asked = first.lines().next().unwrap();
    let rows = [
        first.trim_end(),
        asked,
        r#"{"dir":"hold"}"#,
        r#"{"dir":"exit","line":"1"}"#,
    ];
    let replay = data.0.join("exits.jsonl");
    std::fs::write(&replay, rows.join("\n") + "\n").unwrap();
    let (service, chat, _) = team(&data.0, &replay, &[]);
    send(&service, &chat, "Hi");
    service.wait_for(&chat, 10, "the lead's first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let ask = json!({ "to": "Lead", "what": "Run the steps" });
    let given = call_tool(service.info.port, &token(&data.0), "give_task", ask);
    assert_ne!(given["isError"], true, "{given}");
    service.wait_for(&chat, 10, "the task's turn", |h| {
        h["turns"][1]["status"] == "running"
    });
    pause(&service, "/api/pauses/stop", &chat);
    service.wait_for(&chat, 10, "Stop now's carry-on", |h| {
        h["turns"][2]["held"] == "pause"
    });
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the turn to end", |h| {
        h["turns"][1]["status"] != "running"
    });
    assert_eq!(history["turns"][1]["status"], "stopped", "{history}");
    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let task = &listed["tasks"][0];
    assert_eq!(
        (&task["state"], &task["reason"]),
        (
            &json!("working"),
            &json!("Stopped by you. Carries on at Restart")
        ),
        "{listed}"
    );
}
