//! 0059, scenario 3f through a real service, on each engine's replay stand-in: a work task is
//! refused without where its work goes, a brief edited while its turn waits is the one the engine
//! gets, and once that turn has started an edit is refused.

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, gathr, held_recording, rows, text, until};
use serde_json::json;

/// A signed-in team chat on `engine` (Lead, supervisor; Ann, member) whose engines hold their
/// first turn until `go` exists: the service and the chat.
fn team(data: &Path, engine: &str) -> (TestService, String) {
    let recorded = match engine {
        "codex" => "codex_signed_in",
        _ => "signed_in",
    };
    let (held, replay) = (held_recording(data, recorded), common::replay_engine());
    let (go, tokens) = (data.join("go"), data.join("env.jsonl"));
    let env = [
        ("GATHR_CLAUDE_PATH", replay.as_path()),
        ("GATHR_CODEX_PATH", &replay),
        ("GATHR_REPLAY", &held),
        ("GATHR_REPLAY_HOLD", &go),
        ("GATHR_REPLAY_ENV", &tokens),
    ];
    let service = TestService::start_with(data, &env);
    let body = json!({ "engine": engine, "name": engine });
    let added = service.send("POST", "/api/accounts", body);
    assert_eq!(added.status, 200, "{}", added.body);
    let account = added.json()["id"].as_str().unwrap().to_string();
    match engine {
        "codex" => {
            let folder = data.join("accounts").join(&account).join("codex");
            std::fs::create_dir_all(&folder).unwrap();
            let login = r#"{"tokens":{"refresh_token":"made-up"}}"#;
            std::fs::write(folder.join("auth.json"), login).unwrap();
        }
        _ => common::sign_in_account(data, &account),
    }
    let agent = |name: &str| {
        let body = json!({ "name": name, "account": account });
        let made = service.send("POST", "/api/agents", body);
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    (service, chat)
}

/// The id of the turn on `task` once its status is one of `statuses`.
fn turn_on(service: &TestService, chat: &str, task: i64, statuses: &[&str]) -> String {
    let what = format!("a turn on #{task} to be {statuses:?}");
    let history = service.wait_for(chat, 30, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["task"] == task && statuses.iter().any(|s| t["status"] == *s))
    });
    let turns = history["turns"].as_array().unwrap();
    let found = turns
        .iter()
        .find(|t| t["task"] == task && statuses.iter().any(|s| t["status"] == *s));
    let found = found.unwrap();
    found["id"].as_str().unwrap().to_string()
}

fn brief_edited_while_its_turn_waits_is_the_one_the_engine_gets(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = team(&data.0, engine);
    let port = service.info.port;
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Plan it" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead = common::first_token(&data.0);
    let first = common::briefed(json!({ "to": "Ann", "what": "Write the notes" }));
    let given = text(&call_tool(port, &lead, "give_task", first));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    turn_on(&service, &chat, 1, &["running"]);

    // A work task without where its work goes is refused, naming it, with a reference.
    let bare = json!({ "to": "Ann", "what": "Fix the export", "why": "It fails",
                       "done": "The export works", "proof": "the export test" });
    let refused = text(&call_tool(port, &lead, "give_task", bare));
    let missing =
        "give_task failed: A work task needs why, done, proof and where. Missing: `where`.";
    assert!(refused.starts_with(missing), "{refused}");
    let reference = refused
        .rsplit("(ref ")
        .next()
        .unwrap()
        .trim_end_matches(')');
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let call = events
        .iter()
        .filter(|e| e["name"] == "gathr.tool.call" && e["attrs"]["gen_ai.tool.name"] == "give_task")
        .find(|e| e["status"] == "error");
    let why = call.map(|e| e["attrs"]["why"].as_str().unwrap_or_default());
    assert!(
        ok && why.is_some_and(|w| w.contains("Missing: `where`")),
        "{trace}"
    );

    let full = common::briefed(json!({ "to": "Ann", "what": "Fix the export", "why": "It fails" }));
    let given = text(&call_tool(port, &lead, "give_task", full));
    assert!(given.starts_with("Task #2 for Ann: ready."), "{given}");
    let left = turn_on(&service, &chat, 2, &["queued"]);
    // Made to wait and freed again while its turn waits: the turn the first edit left is dropped,
    // and its task is never stopped for it.
    let wait = |after: serde_json::Value| {
        text(&call_tool(
            port,
            &lead,
            "edit_task",
            json!({ "task": 2, "after": after }),
        ))
    };
    assert_eq!(wait(json!([1])), "Task #2 edited (after): waiting on #1.");
    assert_eq!(wait(json!([])), "Task #2 edited (after): ready.");
    let edit =
        json!({ "task": 2, "what": "Fix the CSV export", "done": "A CSV opens in a spreadsheet" });
    let edited = text(&call_tool(port, &lead, "edit_task", edit));
    assert_eq!(edited, "Task #2 edited (what, done): ready.");

    std::fs::write(data.0.join("go"), "").unwrap();
    let turn = turn_on(&service, &chat, 2, &["running", "done", "failed"]);
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{turn}' ORDER BY id"
    );
    let sent = until(30, "the frames sent on #2's turn", || {
        let bodies: Vec<String> = rows(&data.0, &sql)
            .iter()
            .filter_map(|r| r["body"].as_str().map(String::from))
            .collect();
        let sent = bodies.join("\n");
        sent.contains("Task #2 from Lead").then_some(sent)
    });
    assert!(
        sent.contains("Task #2 from Lead: Fix the CSV export")
            && sent.contains("Done when: A CSV opens in a spreadsheet")
            && !sent.contains("Fix the export"),
        "{sent}"
    );
    assert_ne!(turn, left, "the turn the first edit left never ran");
    let stopped = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.task.state'
                   AND CAST(task_id AS INTEGER) = 2 AND attrs ->> '$.\"gathr.task.state\"' = 'stopped'";
    assert_eq!(rows(&data.0, stopped)[0]["n"], 0, "#2 was never stopped");

    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let tasks = listed["tasks"].as_array().cloned().unwrap_or_default();
    let task = tasks.iter().find(|t| t["number"] == 2).cloned();
    let task = task.unwrap_or_else(|| panic!("no #2: {listed}"));
    let shown = (&task["done"], &task["where"], &task["edited"]);
    let place = common::COMPLETE.place.unwrap();
    let want = (
        &json!("A CSV opens in a spreadsheet"),
        &json!(place),
        &json!(["what", "done"]),
    );
    assert_eq!(shown, want, "{task}");
    assert!(task["edited_us"].is_i64(), "{task}");

    let late = json!({ "task": 2, "what": "Fix it" });
    let refused = text(&call_tool(port, &lead, "edit_task", late));
    let started = "edit_task failed: Task #2 has started, so it can't be edited: message Ann with the \
                   change, or cancel it and give a new task. (ref ";
    assert!(refused.starts_with(started), "{refused}");
}

#[test]
fn a_brief_edited_while_its_turn_waits_is_the_one_claude_gets_and_an_edit_after_it_starts_is_refused()
 {
    brief_edited_while_its_turn_waits_is_the_one_the_engine_gets("claude");
}

#[test]
fn a_brief_edited_while_its_turn_waits_is_the_one_codex_gets_and_an_edit_after_it_starts_is_refused()
 {
    brief_edited_while_its_turn_waits_is_the_one_the_engine_gets("codex");
}
