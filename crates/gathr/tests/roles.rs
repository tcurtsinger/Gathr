//! Role text (0066): the few lines a team's briefing gives each role, recorded on each turn's start;
//! the switch that leaves them out for the trial's without arm; a code review's wrong note that
//! names its file; and the gate that holds each shipped line to the trial's kept result.

mod common;
#[path = "roles/results.rs"]
mod results;
#[path = "words/samples.rs"]
mod samples;

use std::path::Path;

use common::{
    TempDir, TestService, call_tool, held_recording, nth, recording, sign_in_account, text,
};
use gathr::roles;
use gathr::team::Role;
use serde_json::{Value, json};

/// The briefing the engine's last start was given, read while that engine still runs.
fn last_briefing(args: &Path) -> String {
    let starts = std::fs::read_to_string(args).unwrap();
    let last: Vec<String> = serde_json::from_str(starts.lines().last().unwrap()).unwrap();
    let at = last
        .iter()
        .position(|a| a == "--append-system-prompt-file")
        .unwrap();
    std::fs::read_to_string(&last[at + 1]).unwrap()
}

/// Each engine start's role and role version, in order, as the start events record them.
fn started_roles(data: &Path) -> Vec<(Value, Value)> {
    let sql = "SELECT json_extract(attrs, '$.\"gathr.role\"') AS role, \
               json_extract(attrs, '$.\"gathr.role.version\"') AS version \
               FROM event_log WHERE name LIKE 'invoke_agent %' AND phase = 'start' ORDER BY ts";
    let rows = common::rows(data, sql);
    rows.iter()
        .map(|r| (r["role"].clone(), r["version"].clone()))
        .collect()
}

fn ended(history: &Value, n: usize) -> bool {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.iter().filter(|t| t["ended"].is_string()).count() == n
}

fn send(service: &TestService, chat: &str, body: Value) {
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    assert_eq!(sent.status, 200, "{}", sent.body);
}

/// A service on the replay engine whose engines write their arguments to `args`, and an agent for
/// each name on one signed-in account: the service and the agents' ids.
fn agents(data: &Path, args: &Path, names: &[&str]) -> (TestService, Vec<String>) {
    let replay = recording("signed_in");
    let env = [
        ("GATHR_REPLAY", replay.as_path()),
        ("GATHR_REPLAY_ARGS", args),
    ];
    let service = TestService::with_engine(data, &env);
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let ids = names
        .iter()
        .map(|name| {
            let body = json!({ "name": name, "account": account });
            let made = service.send("POST", "/api/agents", body);
            assert_eq!(made.status, 200, "{}", made.body);
            made.json()["id"].as_str().unwrap().to_string()
        })
        .collect();
    (service, ids)
}

/// The kept sample's text: the lines the real code gives `role`.
fn role_sample(name: &str, role: Role) -> &'static str {
    samples::same(name, &roles::lines(role).join("\n"));
    samples::SAMPLES.iter().find(|s| s.1 == name).unwrap().2
}

#[test]
fn each_roles_briefing_carries_its_lines_and_its_start_records_their_version() {
    let data = TempDir::new();
    let args = data.0.join("args.jsonl");
    let (service, ids) = agents(&data.0, &args, &["Ana", "Bo", "Cy"]);
    let members = json!([
        { "agent": ids[0], "role": "supervisor" },
        { "agent": ids[1], "role": "member" },
        { "agent": ids[2], "role": "reviewer" },
    ]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let each = [
        ("Ana", "role: supervisor", Role::Supervisor),
        ("Bo", "role: member", Role::Member),
        ("Cy", "role: reviewer", Role::Reviewer),
    ];
    for (n, (name, sample, role)) in each.into_iter().enumerate() {
        send(&service, &chat, json!({ "text": format!("@{name} hi") }));
        service.wait_for(&chat, 20, &format!("{name}'s turn"), |h| ended(h, n + 1));
        let briefing = last_briefing(&args);
        let (first, rest) = briefing.split_once('\n').unwrap();
        assert!(
            first.starts_with(&format!("You are {name}, an agent")),
            "{briefing}"
        );
        let lines = role_sample(sample, role);
        assert!(
            rest.starts_with(&format!("{lines}\n")),
            "{name}'s briefing doesn't carry its lines after its first line:\n{briefing}"
        );
    }
    let version = json!(gathr::words::ROLE);
    let recorded = [
        (json!("supervisor"), version.clone()),
        (json!("member"), version.clone()),
        (json!("reviewer"), version),
    ];
    assert_eq!(started_roles(&data.0), recorded);
}

#[test]
fn a_one_agent_chats_briefing_has_no_role_lines() {
    let data = TempDir::new();
    let args = data.0.join("args.jsonl");
    let (service, ids) = agents(&data.0, &args, &["Dot"]);
    let chat = service.new_chat();
    send(
        &service,
        &chat,
        json!({ "text": "Say hi", "agent": ids[0] }),
    );
    service.wait_for(&chat, 20, "Dot's turn", |h| ended(h, 1));
    let briefing = last_briefing(&args);
    for (id, line) in roles::SHIPPED {
        assert!(
            !briefing.contains(line),
            "{id} is in a one-agent chat's briefing:\n{briefing}"
        );
    }
    assert_eq!(started_roles(&data.0), [(Value::Null, Value::Null)]);
}

/// A held team (Lead, Ann) with Rev as its Reviewer, on `more`: the service, the chat, and Rev's
/// review task's number and brief, given by the lead's tool.
fn review_given(data: &Path, more: &[(&str, &Path)]) -> (TestService, String, i64, String) {
    let held = held_recording(data, "signed_in");
    let (service, chat, _) = common::team(data, &held, more);
    common::with_reviewer(&service, &chat);
    send(&service, &chat, json!({ "text": "Plan it" }));
    let env = data.join("env.jsonl");
    let lead = nth(&env, 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let port = service.info.port;
    common::fix_it(port, &lead);
    let review = json!({ "to": "Rev", "what": "Review the notes", "kind": "review" });
    let given = text(&call_tool(port, &lead, "give_task", review));
    assert!(given.starts_with("Task #1 for Rev"), "{given}");
    let sql = "SELECT body FROM messages WHERE role = 'gathr' AND body LIKE 'Task #1 from Lead%'";
    let rows = common::rows(data, sql);
    let brief = rows[0]["body"].as_str().unwrap().to_string();
    (service, chat, 1, brief)
}

#[test]
fn with_the_switch_set_no_briefing_has_role_lines_and_the_review_brief_reads_as_version_1() {
    let data = TempDir::new();
    let args = data.0.join("args.jsonl");
    let more = [
        ("GATHR_TRIAL_WITHOUT", Path::new("1")),
        ("GATHR_REPLAY_ARGS", args.as_path()),
    ];
    let (_service, _, _, brief) = review_given(&data.0, &more);
    let version_1 = "Task #1 from Lead: Review the notes\n\
                     For a review, include the commit you reviewed when you finish the task.\n\
                     Say what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true.\n\
                     Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.";
    assert_eq!(brief, version_1);
    common::until(30, "Rev's engine to start", || {
        let starts = std::fs::read_to_string(&args).unwrap_or_default();
        (starts.lines().count() == 2).then_some(())
    });
    let briefing = last_briefing(&args);
    assert!(briefing.starts_with("You are Rev, an agent"), "{briefing}");
    for (id, line) in roles::SHIPPED {
        assert!(
            !briefing.contains(line),
            "{id} is in Rev's briefing with the switch set:\n{briefing}"
        );
    }
    let unversioned = [
        (json!("supervisor"), Value::Null),
        (json!("reviewer"), Value::Null),
    ];
    assert_eq!(started_roles(&data.0), unversioned);
}

/// The PR's forced failure (0019): a code review's wrong note with no file is refused, nothing is
/// written, and the refusal is found from its reference. The review's brief asks for wrong notes.
#[test]
fn a_code_reviews_wrong_note_with_no_file_is_refused_and_found_from_its_reference() {
    let data = TempDir::new();
    let (service, _, review, brief) = review_given(&data.0, &[]);
    let asks = "\nSay what is wrong or missing against the ask, as wrong notes. Say what in the change the \
                ask doesn't need, as unasked notes, or nothing_unasked: true.\n";
    assert!(brief.contains(asks), "{brief}");
    let rev = nth(&data.0.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let port = service.info.port;
    let left = |note: Value| {
        let said = json!({ "task": review, "how": "left", "note": "one gap", "commit": "abc1234",
                           "notes": [note] });
        text(&call_tool(port, &rev, "finish_task", said))
    };
    let refused = left(json!({ "kind": "wrong", "says": "Tags aren't saved" }));
    let why = "Name the file of each wrong note, with its line: path:line. Something missing is in the file \
               where it belongs.";
    assert!(
        refused.starts_with(&format!("finish_task failed: {why}")),
        "{refused}"
    );
    let notes = common::rows(&data.0, "SELECT count(*) AS n FROM review_notes");
    assert_eq!(notes[0]["n"], 0, "a refused note was written");
    let reference = refused
        .rsplit_once("(ref ")
        .map(|(_, r)| r.trim_end_matches(')').to_string())
        .unwrap_or_else(|| panic!("no reference: {refused}"));
    let (ok, trace) = common::gathr(&data.0, &["diag", "trace", &reference]);
    let found = trace.to_string();
    assert!(
        ok && found.contains("gathr.tool.call") && found.contains(why),
        "the refusal isn't found from its reference {reference}: {trace}"
    );

    let named =
        left(json!({ "kind": "wrong", "file": "src/tags.rs:12", "says": "Tags aren't saved" }));
    assert!(!named.starts_with("finish_task failed"), "{named}");
}

#[test]
fn the_shipped_lines_passed_the_trial() {
    let file = std::fs::read_to_string(results::path()).unwrap();
    let file: Value = serde_json::from_str(&file).unwrap();
    let unkept = results::unkept(&file, &roles::SHIPPED);
    assert!(
        unkept.is_empty(),
        "a shipped line has no kept result in crates/gathr/trial/results.json: run the trial (docs/testing.md), \
         or delete the line and its samples.\n{}",
        unkept.join("\n")
    );
}

#[test]
fn the_gate_names_each_line_with_no_kept_result_for_its_exact_text() {
    let shipped = [
        ("L1", "Ask less."),
        ("L2", "Build small."),
        ("L3", "React."),
    ];
    // The SHA-256 of "Ask less.", worked out apart from Gathr.
    let ask = "4573ab4887bee3c8d70b4b118654232da2a134628ae454863d433d969a09a44b";
    let build = results::line_sha256("Build small.");
    let file = json!({ "format": 1, "lines": [
        { "id": "L1", "sha256": ask, "case": "T5", "decision": "kept" },
        { "id": "L2", "sha256": build, "case": "T1", "decision": "dropped" },
        { "id": "L3", "sha256": results::line_sha256("React!"), "case": "T8", "decision": "kept" },
    ] });
    let said = [
        "L2 has no kept result for its text: \"Build small.\"",
        "L3 has no kept result for its text: \"React.\"",
    ];
    assert_eq!(results::unkept(&file, &shipped), said);
    assert_eq!(results::line_sha256("Ask less."), ask);
}

/// A full file as the trial writes it, made up.
fn full_file() -> Value {
    json!({
        "format": 1, "date": "2026-11-17", "gathr": "efaefbe",
        "copies": { "claude": "2.1.40", "codex": "0.80.0" },
        "words": { "brief": 3, "review_brief": 2, "role": 1 },
        "lines": [{ "id": "L2", "sha256": results::line_sha256("Build small."), "case": "T1", "decision": "kept" }],
        "runs": [{
            "case": "T1", "engine": "codex", "model": "gpt-6.1-sol", "effort": "medium", "arm": "with",
            "block": 3, "outcome": "passed", "src_lines": 31, "new_src_files": 0, "over_built": false,
            "hidden_test": true, "input_tokens": 1_900_000, "seconds": 640
        }, {
            "case": "T5", "engine": "claude", "model": "latest:sol", "effort": "high", "arm": "without",
            "block": 4, "outcome": "no_question", "named_answered": "key style", "questions": 0
        }],
        "spent": 2_400_000, "ended": "done"
    })
}

#[test]
fn the_trial_file_holds_only_its_fields() {
    let file = std::fs::read_to_string(results::path()).unwrap();
    let kept: Value = serde_json::from_str(&file).unwrap();
    assert_eq!(results::shape(&kept), Vec::<String>::new(), "{kept}");
    assert_eq!(results::shape(&full_file()), Vec::<String>::new());

    let mut path = full_file();
    path["runs"][0]["model"] = json!("C:\\Users\\ana\\model");
    let mut email = full_file();
    email["copies"]["claude"] = json!("2.1.40-bo@example.com");
    let mut unknown = full_file();
    unknown["runs"][1]["question_text"] = json!("camelCase or snake_case?");
    let mut top = full_file();
    top["machine"] = json!("fin-pc");
    let said = [
        (
            path,
            "runs/0/model isn't of its kind: \"C:\\\\Users\\\\ana\\\\model\"",
        ),
        (
            email,
            "copies isn't of its kind: {\"claude\":\"2.1.40-bo@example.com\",\"codex\":\"0.80.0\"}",
        ),
        (unknown, "runs/1 has a field not on the list: question_text"),
        (top, "the file has a field not on the list: machine"),
    ];
    for (file, why) in said {
        assert_eq!(results::shape(&file), [why]);
    }
}
