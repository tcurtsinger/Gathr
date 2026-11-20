//! Archive, delete and the stale days through a real service (F11, 0010): archiving ends a chat's
//! work in one step and keeps it out of the list until it's brought back; deleting removes it, its
//! worktrees and its rows; the stale days have a read and a save.

mod common;

use std::path::PathBuf;

use common::{TempDir, call_tool, first_token, gathr, git, team, text, until};
use serde_json::json;

fn listed(service: &common::TestService, query: &str) -> Vec<String> {
    let chats = service.call("GET", &format!("/api/chats{query}")).json();
    let ids = chats.as_array().unwrap().iter();
    ids.map(|c| c["id"].as_str().unwrap().to_string()).collect()
}

#[test]
fn archiving_ends_the_chats_work_in_one_step_and_unarchive_brings_it_back() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::held_recording(&data.0, "signed_in"), &[]);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let (port, lead) = (service.info.port, first_token(&data.0));
    common::fix_it(port, &lead);
    let first = text(&call_tool(
        port,
        &lead,
        "give_task",
        common::briefed(json!({ "to": "Ann", "what": "Notes" })),
    ));
    assert!(first.starts_with("Task #1 for Ann: ready."), "{first}");
    let after = json!({ "to": "Ann", "what": "Review notes", "after": [1] });
    text(&call_tool(port, &lead, "give_task", common::briefed(after)));
    let check = json!({ "check": "exit 1", "then": "merge it" });
    let left = text(&call_tool(port, &lead, "wait", check.clone()));
    assert!(left.starts_with("Waiting ("), "{left}");

    let archived = service.call("POST", &format!("/api/chats/{chat}/archive"));
    assert_eq!(archived.status, 200, "{}", archived.body);
    let ended = &archived.json()["ended"];
    assert_eq!(
        (ended["tasks"].as_i64(), ended["waits"].as_i64()),
        (Some(2), Some(1)),
        "{ended}"
    );
    // Its engine still runs, so its token works: nothing new may start in the chat.
    let asked = json!({ "what": "Pick a name", "form": "done" });
    for (tool, arguments) in [("ask_person", asked), ("wait", check)] {
        let late = text(&call_tool(port, &lead, tool, arguments));
        assert!(
            late.starts_with(&format!("{tool} failed: the chat is archived")),
            "{late}"
        );
    }
    // The held stand-in takes an interrupt only once it streams: released, each turn reads its stop.
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "every turn ended", |h| {
        let turns = h["turns"].as_array().unwrap();
        turns.iter().all(|t| t["status"] != "running")
    });
    let turns = history["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 2, "nothing new: {history}");
    // Ann's turn is stopped running, or dropped before it's sent if the archive came first (0059).
    assert!(
        turns.iter().all(|t| t["status"] == "stopped"),
        "both turns were stopped: {history}"
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
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let worktree = PathBuf::from(history["members"][0]["folder"].as_str().unwrap());
    assert!(worktree.is_dir(), "the lead's worktree was made");

    let mut live = service.live();
    let deleted = service.call("DELETE", &format!("/api/chats/{chat}"));
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    assert_eq!(deleted.json()["left"], json!([]), "{}", deleted.body);
    // Windows hear only "deleted": an "archived" first moves the open chat to Archived, and the
    // window then picks its neighbour there.
    let mut changes = Vec::new();
    while changes.last() != Some(&json!("deleted")) {
        let update = common::next_live(&mut live);
        if update["type"] == "chat" {
            changes.push(update["change"].clone());
        }
    }
    assert_eq!(changes, [json!("deleted")]);
    assert_eq!(
        service.call("GET", &format!("/api/chats/{chat}")).status,
        404
    );
    assert!(!worktree.exists(), "its worktree is gone");
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

/// F1 (found in #148): a start under way when the chat is archived found the account signed out
/// and put its turn back to wait for a sign-in, though archiving had just stopped it: restored, the
/// chat sent it at the next sign-in. It stays stopped (answered, live and in its history, with no
/// carry-on), and a sign-in starts nothing there.
#[test]
fn a_start_under_way_when_its_chat_is_archived_never_waits_for_a_sign_in() {
    let data = TempDir::new();
    // The engine's version check holds while this file is there: the start waits inside it.
    let held = data.0.join("version-hold");
    std::fs::write(&held, "").unwrap();
    let service = common::TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &common::recording("signed_in")),
            ("GATHR_REPLAY_VERSION_HOLD", &held),
        ],
    );
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "claude", "name": "Signed out" }),
    );
    let account = added.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let mut live = service.live();
    let (port, token) = (service.info.port, service.info.token.clone());
    // The send answers once the start is through, which waits on the version check: under 5 s.
    let (path, key) = (format!("/api/chats/{chat}/messages"), token.clone());
    let sending = std::thread::spawn(move || {
        gathr::http::send(port, &key, "POST", &path, &json!({ "text": "Say hi" }))
    });
    // The chat's history asks the agent's queue, held in the start, so the database is read.
    let status = || {
        let (ok, rows) = gathr(&data.0, &["diag", "query", "SELECT status FROM turns"]);
        assert!(ok, "{rows}");
        rows["rows"][0]["status"].as_str().map(String::from)
    };
    let entered = data.0.join("version-hold.entered");
    until(5, "the start to wait in the version check", || {
        entered.exists().then_some(())
    });
    let archive = format!("/api/chats/{chat}/archive");
    let archiving = std::thread::spawn(move || gathr::http::call(port, &token, "POST", &archive));
    until(3, "archiving to stop the turn", || {
        (status().as_deref() == Some("stopped")).then_some(())
    });
    std::fs::remove_file(&held).unwrap();
    let sent = sending.join().unwrap().unwrap();
    assert_ne!(sent.json()["status"], "waiting_sign_in", "{}", sent.body);
    let archived = archiving.join().unwrap().unwrap();
    assert_eq!(archived.status, 200, "{}", archived.body);
    let back = service.call("POST", &format!("/api/chats/{chat}/unarchive"));
    assert_eq!(back.status, 200, "{}", back.body);
    common::sign_in_account(&data.0, &account);
    std::thread::sleep(std::time::Duration::from_millis(1500));

    let mut waited = Vec::new();
    live.set_read_timeout(Some(std::time::Duration::from_millis(300)))
        .unwrap();
    while let Ok(update) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        common::next_live(&mut live)
    })) {
        if update["type"] == "turn" && update["status"] != "stopped" {
            waited.push(update);
        }
    }
    assert!(waited.is_empty(), "the stopped turn came back: {waited:?}");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let turns = history["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 1, "{history}");
    assert_eq!(turns[0]["status"], "stopped", "{history}");
}

/// A thumbs-up points at its message and chat, so deleting the chat removes it first.
#[test]
fn deleting_an_archived_chat_removes_its_reactions() {
    let data = TempDir::new();
    let mut conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO messages (id, chat_id, ts, role, kind, body) VALUES ('m', 'c', 1, 'assistant', 'text', 'Done');
         INSERT INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES ('m', 'ann', 'c', 2);",
    )
    .unwrap();
    gathr::archive::archive(&mut conn, "c", "test-trace").unwrap();
    gathr::archive::delete(&mut conn, "c", "test-trace").unwrap();
    let left: i64 = conn
        .query_row("SELECT count(*) FROM reactions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0, "reactions left");
}
