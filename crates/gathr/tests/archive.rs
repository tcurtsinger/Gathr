//! Archive, delete and the stale days through a real service (F11, 0010): archiving ends a chat's
//! work in one step and keeps it out of the list until it's brought back; deleting removes it, its
//! worktrees and its rows; the stale days have a read and a save.

mod common;

use std::path::Path;

use common::{TempDir, call_tool, first_token, gathr, git, team, text, until};
use serde_json::{Value, json};

fn listed(service: &common::TestService, query: &str) -> Vec<String> {
    let chats = service.call("GET", &format!("/api/chats{query}")).json();
    let ids = chats.as_array().unwrap().iter();
    ids.map(|c| c["id"].as_str().unwrap().to_string()).collect()
}

#[test]
fn archiving_ends_the_chats_work_in_one_step_and_unarchive_brings_it_back() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::held_recording(&data.0), &[]);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let (port, lead) = (service.info.port, first_token(&data.0));
    let first = text(&call_tool(
        port,
        &lead,
        "give_task",
        json!({ "to": "Ann", "what": "Notes" }),
    ));
    assert!(first.starts_with("Task #1 for Ann: ready."), "{first}");
    let after = json!({ "to": "Ann", "what": "Review notes", "after": [1] });
    text(&call_tool(port, &lead, "give_task", after));
    let check = json!({ "check": "exit 1", "then": "merge it" });
    let left = text(&call_tool(port, &lead, "wait", check));
    assert!(left.starts_with("Waiting ("), "{left}");

    let archived = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let ended = &archived.json()["ended"];
    assert_eq!(
        (ended["tasks"].as_i64(), ended["waits"].as_i64()),
        (Some(2), Some(1)),
        "{ended}"
    );
    // The held stand-in takes an interrupt only once it streams: released, each turn reads its stop.
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "every turn ended", |h| {
        let turns = h["turns"].as_array().unwrap();
        turns.iter().all(|t| t["status"] != "running")
    });
    assert_eq!(
        history["turns"].as_array().unwrap().len(),
        2,
        "nothing new: {history}"
    );
    let stops = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.turn.stop'";
    let (_, counted) = gathr(&data.0, &["diag", "query", stops]);
    assert_eq!(
        counted["rows"][0]["n"], 2,
        "both turns were stopped: {counted}"
    );
    assert!(history["archived"].is_string(), "{history}");
    let tasks = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    for task in tasks["tasks"].as_array().unwrap() {
        assert_eq!(task["state"], "cancelled", "{task}");
        assert_eq!(task["reason"], "its chat was archived", "{task}");
    }
    assert_eq!(tasks["waits"][0]["state"], "cancelled", "{tasks}");
    assert!(!listed(&service, "").contains(&chat));
    assert_eq!(
        listed(&service, "?archived=true"),
        std::slice::from_ref(&chat)
    );

    let refused = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "One more" }),
    );
    assert_eq!(refused.status, 409, "{}", refused.body);
    let refused = refused.json();
    assert_eq!(refused["why"], "the chat is archived", "{refused}");
    let reference = refused["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(
        ok && trace.to_string().contains("the chat is archived"),
        "{trace}"
    );
    let again = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(
        (again.status, again.json()["why"].clone()),
        (409, json!("it's already archived"))
    );
    let events = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.archive'";
    let (_, counted) = gathr(&data.0, &["diag", "query", events]);
    assert_eq!(counted["rows"][0]["n"], 1, "one event: {counted}");

    let back = service.call("POST", &format!("/api/chats/{chat}/unarchive"));
    assert_eq!(back.status, 200, "{}", back.body);
    assert!(listed(&service, "").contains(&chat));
    assert!(listed(&service, "?archived=true").is_empty());
}

#[test]
fn deleting_a_project_chat_removes_its_rows_and_worktrees_and_keeps_its_branches() {
    let data = TempDir::new();
    std::fs::write(data.0.join("go"), "").unwrap();
    let (service, _, lead) = team(&data.0, &common::recording("signed_in"), &[]);
    let (folder, _) = common::project(&data.0.join("repo"));
    let chat = common::project_chat(
        &service,
        &folder,
        json!([{ "agent": lead, "role": "supervisor" }]),
    );
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Hi" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(&chat, 20, "the turn", |h| h["turns"][0]["status"] == "done");
    let worktrees = data.0.join("worktrees").join(&chat);
    assert!(
        worktrees.join(&lead).is_dir(),
        "the lead's worktree was made"
    );

    let deleted = service.call("DELETE", &format!("/api/chats/{chat}"));
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(deleted.json()["left"], json!([]), "{}", deleted.body);
    assert_eq!(
        service.call("GET", &format!("/api/chats/{chat}")).status,
        404
    );
    assert!(!worktrees.exists(), "its worktrees are gone");
    let branches = git(&folder, &["branch", "--list", "gathr/*"]);
    assert!(!branches.trim().is_empty(), "the branch stays: {branches}");
    for table in ["messages", "turns", "chat_members", "engine_sessions"] {
        let sql = format!("SELECT count(*) AS n FROM {table} WHERE chat_id = '{chat}'");
        let (_, counted) = gathr(&data.0, &["diag", "query", &sql]);
        assert_eq!(counted["rows"][0]["n"], 0, "{table}: {counted}");
    }
    let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.delete'";
    let (_, counted) = gathr(&data.0, &["diag", "query", sql]);
    assert_eq!(counted["rows"][0]["n"], 1, "{counted}");
}

#[test]
fn the_stale_days_are_read_saved_and_refused_out_of_range_and_stale_tasks_reach_the_persons_list() {
    let data = TempDir::new();
    let fast = [
        ("GATHR_STALE_DAY_MS", Path::new("1")),
        ("GATHR_WAIT_EVERY_MS", Path::new("100")),
    ];
    let (service, chat, _) = team(&data.0, &common::held_recording(&data.0), &fast);
    let read = service.call("GET", "/api/settings").json();
    assert_eq!(read, json!({ "stale_days": 3, "cancel_days": 7 }));
    let saved = service.send("PATCH", "/api/settings", json!({ "cancel_days": 500 }));
    assert_eq!(saved.status, 409, "{}", saved.body);
    let saved = service.send("PATCH", "/api/settings", json!({ "cancel_days": 9 }));
    assert_eq!(saved.json(), json!({ "stale_days": 3, "cancel_days": 9 }));

    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead = first_token(&data.0);
    let asked = json!({ "what": "Pick a name", "form": "done" });
    text(&call_tool(service.info.port, &lead, "ask_person", asked));
    let listed: Value = until(10, "the person's task stale", || {
        let listed = service.call("GET", "/api/person-tasks").json();
        listed["tasks"][0]["stale_us"].is_i64().then_some(listed)
    });
    assert!(listed["stale"].as_array().is_some(), "{listed}");
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the Supervisor told", |h| {
        let messages = h["messages"].as_array().unwrap();
        messages.iter().any(|m| {
            m["role"] == "gathr"
                && m["body"]
                    .as_str()
                    .unwrap_or("")
                    .contains("haven't moved in 3 days")
        })
    });
    let told = history["messages"].as_array().unwrap().iter().rev();
    let told = told
        .filter_map(|m| m["body"].as_str())
        .find(|b| b.contains("haven't moved"));
    assert!(
        told.unwrap().contains("The person's questions stay open"),
        "the lead isn't told the person's question stays open: {told:?}"
    );
    let person = service.call("GET", "/api/person-tasks").json();
    assert_eq!(
        person["tasks"][0]["state"], "ready",
        "never cancelled: {person} {history}"
    );
}
