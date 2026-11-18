//! A like that stands for a click (0057): on a bubble holding a waiting question with a pick, that
//! pick, through the path picking it takes; on a plan waiting for the person, Approve. Each is refused
//! as its click would be, with a reason and a reference.

use std::path::{Path, PathBuf};

use super::{engine, history, like, likes_shown, refused, said_in, say, turn_is};
use crate::common::{
    TempDir, call_tool, first_token, fix_it, rows, team, team_on_with, text, until, with_reviewer,
};
use serde_json::{Value, json};

/// Lets every held engine go when the test ends, passed or failed.
pub struct LetGo(pub PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

fn task(service: &crate::common::TestService, chat: &str, number: i64) -> Value {
    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let tasks = listed["tasks"].as_array().cloned().unwrap_or_default();
    let found = tasks.into_iter().find(|t| t["number"] == number);
    found.unwrap_or_else(|| panic!("no #{number}: {listed}"))
}

#[test]
fn a_like_answers_a_waiting_question_with_its_pick_as_picking_it_does() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &engine(&data.0, &[0], 4), &[]);
    let _go = LetGo(data.0.join("go"));
    let turn = say(&service, &chat, "Plan it");
    turn_is(&service, &chat, &turn, &["running"]);
    let (port, lead) = (service.info.port, first_token(&data.0));
    fix_it(port, &lead);
    let ask = |mut asked: Value, what: &str| {
        asked["what"] = what.into();
        let said = text(&call_tool(port, &lead, "ask_person", asked));
        assert!(said.contains("on the person's Tasks list"), "{said}");
    };
    ask(
        json!({ "choices": [{ "label": "Banner" }, { "label": "Toast" }] }),
        "Which notice?",
    );
    ask(json!({ "form": "yesno" }), "Ship it?");
    std::fs::write(data.0.join("go"), "").unwrap();
    turn_is(&service, &chat, &turn, &["done"]);
    let message = said_in(&service, &chat, &turn);

    let why = "Question #2 has no recommended choice: answer it in the question.";
    refused(
        &data.0,
        &like(&service, &chat, &message, json!({ "questions": [2] })),
        why,
    );
    let clicked = json!({ "answers": [{ "task": 2, "yes": true }] });
    let clicked = service.send("POST", "/api/person-tasks/answer", clicked);
    assert_eq!(clicked.status, 200, "{}", clicked.body);
    let why = "Question #2 is already done.";
    refused(
        &data.0,
        &like(&service, &chat, &message, json!({ "questions": [2] })),
        why,
    );
    let none = like(&service, &chat, &message, json!({ "questions": [9] }));
    refused(&data.0, &none, "There's no question #9 in this chat.");
    assert!(
        likes_shown(&service, &chat).is_empty(),
        "a refused like leaves no mark"
    );

    let given = like(&service, &chat, &message, json!({ "questions": [1] }));
    assert_eq!(given.status, 200, "{}", given.body);
    assert_eq!(given.json()["did"], "answered", "{}", given.body);
    let answered = task(&service, &chat, 1);
    assert_eq!(answered["state"], "done", "{answered}");
    let kept = &answered["answer"];
    assert_eq!(
        (&kept["pick"], &kept["by"], kept["at"].is_i64()),
        (&json!(["Banner"]), &Value::Null, true),
        "kept as a click keeps it: {answered}"
    );
    assert_eq!(likes_shown(&service, &chat), [json!(message)]);
    // The asker hears it as it hears a pick, in one turn; nothing else is delivered.
    let sql = "SELECT u.id, m.id AS line, m.body FROM turns u
               JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
               WHERE u.opened_by = 'answered' ORDER BY u.created_ts";
    let heard = until(30, "the lead to hear the answer", || {
        let found = rows(&data.0, sql);
        let has = |r: &&Value| {
            r["body"]
                .as_str()
                .is_some_and(|b| b.contains("#1 Which notice?"))
        };
        found.iter().find(has).cloned()
    });
    let said = heard["body"].as_str().unwrap();
    assert!(
        said.starts_with("The person answered:\n- #1 Which notice?: Picked: Banner\n"),
        "{said}"
    );
    let liked = rows(&data.0, "SELECT id FROM turns WHERE opened_by = 'liked'");
    assert!(liked.is_empty(), "{liked:?}");

    // Gathr's line takes no like.
    let line = heard["line"].as_str().unwrap();
    let why = format!("{line} is Gathr's line: a like goes on an agent's words.");
    refused(&data.0, &like(&service, &chat, line, json!({})), &why);
}

/// The person's message, a running lead turn and its token; the lead's turn then ends.
fn asked_and_ended(service: &crate::common::TestService, data: &Path, chat: &str) -> String {
    let go = data.join("go");
    let turn = say(service, chat, "Add tags to notes");
    turn_is(service, chat, &turn, &["running"]);
    let lead = first_token(data);
    std::fs::write(&go, "").unwrap();
    turn_is(service, chat, &turn, &["done"]);
    std::fs::remove_file(&go).unwrap();
    lead
}

fn plan_call(parts: usize) -> Value {
    let part = |what: &str| {
        json!({ "to": "Ann", "what": what, "why": "Notes need tags", "done": "A note carries its tags",
            "proof": "tests/tags.rs passes", "where": "no code", "size": "src/tags.rs", "lines": 60, "after": [] })
    };
    let all: Vec<Value> = [
        "Add the tags table",
        "Show tags in the list",
        "Filter by tag",
    ]
    .iter()
    .take(parts)
    .map(|w| part(w))
    .collect();
    json!({ "title": "Add tags to notes", "text": "Notes carry tags.", "parts": all })
}

/// The plan once version `n` waits on the person: that version's message.
fn waiting_version(service: &crate::common::TestService, n: usize) -> String {
    let shown = until(30, &format!("version {n} to wait on the person"), || {
        let shown = service.call("GET", "/api/plans/1").json();
        (shown["state"] == "waiting" && shown["versions"].as_array()?.len() == n).then_some(shown)
    });
    shown["versions"][n - 1]["message"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn a_like_on_a_plan_waiting_for_the_person_approves_it_as_approve_does() {
    let data = TempDir::new();
    let more = [
        ("GATHR_PLAN_CHECK_MS", Path::new("500")),
        ("GATHR_WAIT_EVERY_MS", Path::new("200")),
    ];
    let (service, chat) = team_on_with(&data.0, "claude", &more);
    std::fs::copy(engine(&data.0, &[0], 5), data.0.join("held.jsonl")).unwrap();
    with_reviewer(&service, &chat);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let lead = asked_and_ended(&service, &data.0, &chat);

    let wrote = text(&call_tool(port, &lead, "plan", plan_call(2)));
    assert!(wrote.starts_with("Wrote version 1 of plan #1"), "{wrote}");
    let first = waiting_version(&service, 1);
    let words = json!({ "version": 1, "words": "Add a filter" });
    let back = service.send("POST", "/api/plans/1/send-back", words);
    assert_eq!(back.status, 200, "{}", back.body);
    refused(
        &data.0,
        &like(&service, &chat, &first, json!({})),
        "Version 1 is already sent back.",
    );

    let wrote = text(&call_tool(port, &lead, "plan", plan_call(3)));
    assert!(wrote.starts_with("Wrote version 2 of plan #1"), "{wrote}");
    let why = "Version 1 isn't the newest: version 2 is.";
    refused(&data.0, &like(&service, &chat, &first, json!({})), why);
    let second = waiting_version(&service, 2);

    let given = like(&service, &chat, &second, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    assert_eq!(given.json()["did"], "approved", "{}", given.body);
    let plan = service.call("GET", "/api/plans/1").json();
    assert_eq!(plan["state"], "approved", "{plan}");
    assert_eq!(plan["versions"][1]["decided"], "approved", "{plan}");
    let listed = service
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    let made = listed["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["plan"] == 1)
        .count();
    assert_eq!(made, 3, "each part a task, as Approve makes them: {listed}");
    let said = history(&service, &chat)["messages"].clone();
    let approved = "Approved: version 2 of the plan \"Add tags to notes\", 3 tasks.";
    let lines = said
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["body"] == approved)
        .count();
    assert_eq!(lines, 1, "Gathr's line, as for Approve: {said}");
    assert_eq!(likes_shown(&service, &chat), [json!(second)]);
}
