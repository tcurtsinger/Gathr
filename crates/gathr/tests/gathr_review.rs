//! 0060, scenario 3g through a real service, on each engine's replay stand-in: a builder's done
//! report that first names its PR gives the chat's Reviewer its review, which runs once the
//! builder's turn ends, and a review of that PR given by hand meanwhile is refused, naming it.

mod common;

use common::{TempDir, call_tool, gathr, nth, rows, team_on, text, turn_on, until};
use serde_json::{Value, json};

fn a_pr_a_builder_reports_gets_its_review_from_gathr(engine: &str) {
    let data = TempDir::new();
    let (service, chat) = team_on(&data.0, engine);
    let rev = common::with_reviewer(&service, &chat);
    let port = service.info.port;
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Plan it" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead = common::first_token(&data.0);
    let work = common::briefed(json!({ "to": "Ann", "what": "Write the notes" }));
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    let ann_turn = turn_on(&service, &chat, 1, &["running"]);
    let ann = nth(&data.0.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .expect("Ann's token")
        .to_string();

    let report = json!({ "how": "done", "pr": 7, "note": "see #7" });
    let answer = text(&call_tool(port, &ann, "finish_task", report));
    assert_eq!(
        answer,
        "Recorded: done. It applies when the turn running on task #1 ends.\nGathr gave #2 to Rev to review #7."
    );

    let by_hand = json!({ "to": "Rev", "what": "Review #7", "kind": "review", "pr": 7 });
    let refused = text(&call_tool(port, &ann, "give_task", by_hand));
    let why = "PR #7 already has an open review: #2, Rev's, given by Ann. Give one once #2 has ended or its giver cancels it.";
    assert!(
        refused.starts_with(&format!("give_task failed: {why} (ref ")),
        "{refused}"
    );
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
    assert!(ok && said.is_some_and(|w| w.contains(why)), "{trace}");

    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let tasks = listed["tasks"].as_array().cloned().unwrap_or_default();
    let review = tasks.iter().find(|t| t["number"] == 2).cloned();
    let review = review.unwrap_or_else(|| panic!("no #2: {listed}"));
    let ann_id = tasks.iter().find(|t| t["number"] == 1).unwrap()["owner"].clone();
    let shown = [
        &review["kind"],
        &review["owner"],
        &review["giver"],
        &review["given_in"],
        &review["pr"],
        &review["after"],
        &review["state"],
    ];
    let want = [
        &json!("review"),
        &json!(rev),
        &ann_id,
        &json!(ann_turn),
        &json!(7),
        &json!([1]),
        &json!("waiting"),
    ];
    assert_eq!(shown, want, "{review}");

    std::fs::write(data.0.join("go"), "").unwrap();
    let rev_turn = turn_on(&service, &chat, 2, &["running", "done", "failed"]);
    let opened = common::opened(&data.0, &json!(rev_turn));
    assert_eq!(opened.0, json!("review_brief"));
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{rev_turn}' ORDER BY id"
    );
    until(30, "Rev's brief sent to its engine", || {
        let bodies: Vec<String> = rows(&data.0, &sql)
            .iter()
            .filter_map(|r| r["body"].as_str().map(String::from))
            .collect();
        bodies
            .join("\n")
            .contains("Task #2 from Ann: Review #7")
            .then_some(())
    });

    let trace_of = format!("SELECT trace_id FROM turns WHERE id = '{ann_turn}'");
    let ann_trace = rows(&data.0, &trace_of)[0]["trace_id"].clone();
    let decided = rows(
        &data.0,
        "SELECT trace_id, attrs FROM event_log WHERE name = 'gathr.review.give'",
    );
    let found: Vec<(&Value, &Value, &Value, &Value)> = decided
        .iter()
        .map(|e| {
            let attrs = &e["attrs"];
            (
                &attrs["gathr.review.outcome"],
                &attrs["gathr.task.review"],
                &attrs["gathr.review.pr"],
                &e["trace_id"],
            )
        })
        .collect();
    assert_eq!(
        found,
        [(&json!("given"), &json!(2), &json!(7), &ann_trace)],
        "{decided:?}"
    );
}

#[test]
fn a_pr_a_builder_reports_gets_its_review_from_gathr_on_claude() {
    a_pr_a_builder_reports_gets_its_review_from_gathr("claude");
}

#[test]
fn a_pr_a_builder_reports_gets_its_review_from_gathr_on_codex() {
    a_pr_a_builder_reports_gets_its_review_from_gathr("codex");
}
