//! 0063 through a real service, on each engine's replay stand-in: a held ask refuses building gives
//! and a fix lets them go on (scenarios 2b, 2l), a queued task waits for its plan through the runner
//! (2j), and a held turn's worktree writes and pushes are refused while its own folder and Shared
//! stay writable (2g).

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, hook_at, nth, rows, team_on, text, turn_on};
use serde_json::{Value, json};

const TAGS: &str = "Add tags to notes";

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

fn send(service: &TestService, chat: &str, said: &str) -> Value {
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": said }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

fn listed(service: &TestService, chat: &str) -> Value {
    let listed = service.call("GET", &format!("/api/chats/{chat}/tasks"));
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

fn task(service: &TestService, chat: &str, n: i64) -> Value {
    let listed = listed(service, chat);
    let tasks = listed["tasks"].as_array().cloned().unwrap_or_default();
    let found = tasks.into_iter().find(|t| t["number"] == n);
    found.unwrap_or_else(|| panic!("no #{n}: {listed}"))
}

/// The person changes the size of the chat's first ask.
fn resize(service: &TestService, chat: &str, size: &str) {
    let ask = listed(service, chat)["asks"][0]["id"].clone();
    let path = format!("/api/asks/{}/size", ask.as_str().unwrap());
    let sized = service.send("POST", &path, json!({ "size": size }));
    assert_eq!(sized.status, 200, "{}", sized.body);
}

fn running(service: &TestService, chat: &str, turn: &Value) {
    service.wait_for(chat, 30, "the lead's turn to run", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == *turn && t["status"] == "running")
    });
}

/// The token of the `n`th engine the stand-in started.
fn token(data: &Path, n: usize) -> String {
    let started = nth(&data.join("env.jsonl"), n);
    started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
}

fn agent_of(data: &Path, turn: &Value) -> Value {
    let id = turn.as_str().unwrap();
    rows(
        data,
        &format!("SELECT agent_id FROM turns WHERE id = '{id}'"),
    )[0]["agent_id"]
        .clone()
}

/// Claude Code's stand-in plays one recording per engine: the first turn held until `go`, then a
/// whole turn for each Gathr queues after.
fn more_turns(data: &Path) {
    let held = std::fs::read_to_string(data.join("held.jsonl")).unwrap();
    let once = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let after = once.trim_end().to_string() + "\n";
    std::fs::write(data.join("held.jsonl"), held + &after.repeat(4)).unwrap();
}

fn a_held_ask_refuses_building_and_a_fix_lets_it_go_on(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, engine);
    let _go = LetGo(data.0.join("go"));
    if engine == "claude" {
        more_turns(&data.0);
    }
    let port = service.info.port;
    let lead_turn = send(&service, &chat, TAGS)["turn"].clone();
    let lead = common::first_token(&data.0);
    running(&service, &chat, &lead_turn);

    let work = common::briefed(json!({ "to": "Ann", "what": "Add the tags field" }));
    let refused = text(&call_tool(port, &lead, "give_task", work.clone()));
    let why = format!(
        "\"{TAGS}\" has no approved plan, so building waits. If it is a repair, size it repair with size; else \
         write its plan with plan, or give a look-into task (look_into: true) for what the plan needs first."
    );
    let failed = format!("give_task failed: {why} (ref ");
    assert!(refused.starts_with(&failed), "{refused}");
    let shown = listed(&service, &chat);
    let none = shown["tasks"].as_array().is_none_or(Vec::is_empty);
    assert!(shown["asks"][0]["held"] == true && none, "{shown}");

    let look = json!({ "to": "Ann", "what": "Survey tag libraries", "look_into": true });
    let given = text(&call_tool(port, &lead, "give_task", common::briefed(look)));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    let ann_turn = json!(turn_on(&service, &chat, 1, &["running"]));
    assert_eq!(common::opened(&data.0, &ann_turn).0, "brief");
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let brief = messages
        .iter()
        .find(|m| m["role"] == "gathr" && m["turn"] == ann_turn);
    let brief = brief.and_then(|m| m["body"].as_str()).unwrap_or_default();
    let line = "This is a look-into task: don't change the project's files or push.";
    assert!(brief.contains(line), "{history}");

    let ann = token(&data.0, 1);
    let building = common::briefed(json!({ "to": "Lead", "what": "Add the tags field" }));
    let refused_ann = text(&call_tool(port, &ann, "give_task", building));
    let look_why = "give_task failed: Task #1 is a look-into task, so it gives no building work: give a look-into \
                    task (look_into: true), or report what you found. (ref ";
    assert!(refused_ann.starts_with(look_why), "{refused_ann}");
    let report =
        json!({ "how": "done", "task": 1, "note": "Two tag libraries fit; notes in Shared." });
    let reported = text(&call_tool(port, &ann, "finish_task", report));
    assert!(reported.starts_with("Recorded: done."), "{reported}");
    // The lead hears back once Ann's turn ends.
    std::fs::write(data.0.join("go"), "").unwrap();
    let lead_id = agent_of(&data.0, &lead_turn);
    let back = common::until(30, "the lead's hear-back", || {
        let found = rows(
            &data.0,
            "SELECT agent_id FROM turns WHERE opened_by = 'hear_back'",
        );
        found.first().map(|r| r["agent_id"].clone())
    });
    assert_eq!(back, lead_id, "the hear-back is the lead's");

    resize(&service, &chat, "repair");
    let told = rows(
        &data.0,
        "SELECT agent_id, ask_id FROM turns WHERE opened_by = 'resized'",
    );
    let ask = listed(&service, &chat)["asks"][0]["id"].clone();
    assert_eq!(
        told,
        [json!({ "agent_id": lead_id, "ask_id": ask })],
        "the lead is told"
    );
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #2 for Ann: "), "{given}");

    // The forced failure: the first refusal's reference finds the call and the hold's record.
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
    let said = call.map(|e| e["attrs"]["why"].as_str().unwrap_or_default());
    let held = events.iter().any(|e| e["name"] == "gathr.hold.refused");
    assert!(ok && said == Some(why.as_str()) && held, "{trace}");
}

#[test]
fn a_held_ask_refuses_building_and_a_fix_lets_it_go_on_on_claude() {
    a_held_ask_refuses_building_and_a_fix_lets_it_go_on("claude");
}

#[test]
fn a_held_ask_refuses_building_and_a_fix_lets_it_go_on_on_codex() {
    a_held_ask_refuses_building_and_a_fix_lets_it_go_on("codex");
}

fn a_queued_task_waits_for_its_plan_through_the_runner(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, engine);
    let _go = LetGo(data.0.join("go"));
    if engine == "claude" {
        more_turns(&data.0);
    }
    let port = service.info.port;
    let lead_turn = send(&service, &chat, "The export button does nothing")["turn"].clone();
    let lead = common::first_token(&data.0);
    running(&service, &chat, &lead_turn);
    common::fix_it(port, &lead);
    let give = |token: &str, what: &str| {
        let work = common::briefed(json!({ "to": "Ann", "what": what }));
        text(&call_tool(port, token, "give_task", work))
    };
    let first = give(&lead, "Fix the export button");
    assert!(first.starts_with("Task #1 for Ann: ready."), "{first}");
    turn_on(&service, &chat, 1, &["running"]);
    let second = give(&lead, "Add an export test");
    assert!(second.starts_with("Task #2 for Ann: ready."), "{second}");
    let queued = turn_on(&service, &chat, 2, &["queued"]);

    resize(&service, &chat, "new");
    let plan_wait = |n: i64| {
        let t = task(&service, &chat, n);
        assert_eq!(
            (&t["state"], &t["reason"]),
            (&json!("waiting"), &json!("waits for its plan")),
            "#{n}: {t}"
        );
    };
    plan_wait(2);
    // A started building task's give isn't refused, and what it gives waits for the plan.
    let ann = token(&data.0, 1);
    let third = give(&ann, "Check the CSV export");
    assert!(
        third.starts_with("Task #3 for Ann: waits for its plan."),
        "{third}"
    );
    plan_wait(3);
    let on_three = rows(&data.0, "SELECT count(*) AS n FROM turns WHERE task = 3");
    assert_eq!(on_three[0]["n"], 0, "a turn was queued for #3");

    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 30, "#2's queued turn to be dropped", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == queued.as_str() && t["status"] == "stopped")
    });
    let turns = history["turns"].as_array().unwrap();
    let dropped = turns.iter().find(|t| t["id"] == queued.as_str()).unwrap();
    assert_eq!(
        dropped["why"], "its task #2 waits for its plan",
        "{dropped}"
    );
    plan_wait(2);
    let sent = format!(
        "SELECT count(*) AS n FROM engine_raw WHERE direction = 'in' AND turn_id = '{queued}'"
    );
    assert_eq!(
        rows(&data.0, &sent)[0]["n"],
        0,
        "#2's turn reached the engine"
    );
    let said = common::waiting(&service, &chat);
    let lines = said["lines"].as_array().cloned().unwrap_or_default();
    let shown = lines
        .iter()
        .any(|l| l["kind"] == "plan_wait" && l["what"] == "#2 (Ann) waits for its plan");
    assert!(shown, "{said}");
    let refused = rows(
        &data.0,
        "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.hold.refused'",
    );
    assert_eq!(refused[0]["n"], 0, "something was refused");

    // Back to repair: what waited runs, and the lead, idle by now, is told and runs that turn.
    resize(&service, &chat, "repair");
    for n in [2, 3] {
        turn_on(&service, &chat, n, &["running", "done"]);
    }
    let told = "SELECT status FROM turns WHERE opened_by = 'resized' ORDER BY created_ts";
    common::until(30, "the lead's told turn to run", || {
        let found = rows(&data.0, told);
        let ran = found.len() == 2 && ["running", "done"].iter().any(|s| found[1]["status"] == *s);
        ran.then_some(())
    });
}

#[test]
fn a_queued_task_waits_for_its_plan_through_the_runner_on_claude() {
    a_queued_task_waits_for_its_plan_through_the_runner("claude");
}

#[test]
fn a_queued_task_waits_for_its_plan_through_the_runner_on_codex() {
    a_queued_task_waits_for_its_plan_through_the_runner("codex");
}

/// `team_on`'s Lead and Ann in a chat of a new project.
fn project_team(data: &Path, engine: &str) -> (TestService, String) {
    let (service, chat) = team_on(data, engine);
    let (project, _) = common::project(data);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history["members"].as_array().cloned().unwrap_or_default();
    let members: Vec<Value> = members
        .iter()
        .map(|m| json!({ "agent": m["agent"], "role": m["role"] }))
        .collect();
    let chat = common::project_chat(&service, &project, json!(members));
    (service, chat)
}

fn a_held_write_is_denied_and_a_push_refused(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = project_team(&data.0, engine);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let lead_turn = send(&service, &chat, TAGS)["turn"].clone();
    running(&service, &chat, &lead_turn);
    let started = nth(&data.0.join("env.jsonl"), 0);
    let token = started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string();
    let worktree = PathBuf::from(started["cwd"].as_str().unwrap());
    // Its own folder holds its worktree; Shared is beside it (0034).
    let own = worktree.parent().unwrap().to_path_buf();
    let shared = own.parent().unwrap().join("Shared");
    let tags = worktree.join("src").join("tags.rs");
    let outside = data.0.join("elsewhere.txt");
    let write = |at: &str, path: &Path| {
        let input = json!({ "file_path": path, "content": "made up" });
        hook_at(port, at, &token, "Write", input).json()
    };
    let decided = |answer: &Value| {
        let out = &answer["hookSpecificOutput"];
        (
            out["permissionDecision"].clone(),
            out["permissionDecisionReason"].clone(),
        )
    };
    if engine == "claude" {
        let deny = format!(
            "Held until the plan for \"{TAGS}\" is approved: don't change files in {}. Write notes, experiments and \
             mock-ups in {} or {} instead. If it is a repair, size it repair with size.",
            worktree.display(),
            own.display(),
            shared.display()
        );
        let denied = (json!("deny"), json!(deny));
        assert_eq!(decided(&write("/hook", &tags)), denied);
        for path in [own.join("notes.md"), shared.join("plan-notes.md")] {
            assert_eq!(write("/hook", &path), json!({}), "{}", path.display());
        }
        assert_eq!(decided(&write("/hook", &outside)).0, "ask");
        // Full never asks: its hook answers only the hold.
        assert_eq!(decided(&write("/hook?full", &tags)), denied, "Full");
        assert_eq!(write("/hook?full", &outside), json!({}), "Full asked");
        let full =
            |tool: &str, input: Value| hook_at(port, "/hook?full", &token, tool, input).json();
        let push = full(
            "Bash",
            json!({ "command": "git push --force origin room/x" }),
        );
        let check = json!({ "check": "printf x > ../out", "then": "go on" });
        let wait = full("mcp__gathr__wait", check);
        assert_eq!((push, wait), (json!({}), json!({})), "Full asked");
    }
    common::git(&worktree, &["commit", "-q", "--allow-empty", "-m", "tags"]);
    let to = ["push", "origin", "HEAD:refs/heads/room/tags"];
    let (pushed, said) = common::push_as_engine(&started, &to).join().unwrap();
    let refused = format!(
        "Gathr: the push wasn't allowed (held: \"{TAGS}\" has no approved plan, so nothing is pushed for it)"
    );
    assert!(!pushed && said.contains(&refused), "{said}");

    // "Just fix it."
    resize(&service, &chat, "repair");
    if engine == "claude" {
        assert_eq!(write("/hook", &tags), json!({}), "a repair's write");
    }
}

#[test]
fn a_held_write_is_denied_and_a_push_refused_on_claude() {
    a_held_write_is_denied_and_a_push_refused("claude");
}

#[test]
fn a_held_write_is_denied_and_a_push_refused_on_codex() {
    a_held_write_is_denied_and_a_push_refused("codex");
}
