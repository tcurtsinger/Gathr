//! Scheduled work through a real service and the replay engine: a schedule that came due while the service was
//! stopped queues exactly one turn in its chat for its agent as the service starts, and a start after that queues no
//! second one. Each run would spend the person's tokens.

mod common;

use common::{TempDir, TestService, recording, sign_in_account};
use gathr::schedules::{self, When, when};
use serde_json::{Value, json};

/// The chat's turns that scheduled words opened, with their agents and states.
fn scheduled(history: &Value) -> Vec<Value> {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns
        .into_iter()
        .filter(|t| t["opened"] == "scheduled")
        .collect()
}

/// A schedule of `agent`'s in `chat` that is due now: made earlier, with its time the minute this PC's clock shows.
fn due_now(data: &std::path::Path, chat: &str, agent: &str, words: &str, once: bool) -> String {
    let conn = gathr::db::open(&data.join("gathr.db")).unwrap();
    let now = gathr::ids::now_us();
    let clock = when::local(&conn, now).unwrap();
    let (at, _) = when::at(&conn, &clock.date, 0, clock.minute).unwrap();
    let every = When::Days {
        minute: clock.minute,
        days: "0123456".into(),
    };
    let made = match once {
        true => schedules::add(
            &conn,
            (chat, agent),
            words,
            &When::Once(now + 60_000_000),
            now,
        ),
        false => schedules::add(&conn, (chat, agent), words, &every, now),
    };
    let id = made.unwrap().id;
    conn.execute(
        "UPDATE schedules SET next_ts = ?2, once_ts = CASE WHEN once_ts IS NOT NULL THEN ?2 END WHERE id = ?1",
        rusqlite::params![id, at],
    )
    .unwrap();
    id
}

#[test]
fn a_due_schedule_queues_one_turn_and_a_restart_queues_no_second() {
    let data = TempDir::new();
    let replay = recording("signed_in");
    let start = || TestService::with_engine(&data.0, &[("GATHR_REPLAY", &replay)]);
    let service = start();
    let account = service.add_account("Garden");
    sign_in_account(&data.0, &account);
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ann", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let ann = made.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    assert!(service.stop().success());

    // Due while the service was stopped: it runs as the service starts, once, for Ann.
    let water = due_now(
        &data.0,
        &chat,
        &ann,
        "Remind me to water the seed trays",
        false,
    );
    let service = start();
    let history = service.wait_for(&chat, 30, "the scheduled turn to end", |h| {
        scheduled(h).iter().any(|t| t["status"] == "done")
    });
    let turns = scheduled(&history);
    assert_eq!(turns.len(), 1, "{history}");
    assert_eq!(turns[0]["agent"], json!(ann));
    let messages = history["messages"].as_array().unwrap();
    let words = messages.iter().find(|m| m["role"] == "user").unwrap();
    assert_eq!(words["body"], "Remind me to water the seed trays");
    assert_eq!(words["turn"], turns[0]["id"]);
    assert!(words["scheduled"].is_string(), "{words}");
    let shown = service
        .call("GET", &format!("/api/schedules/{water}"))
        .json();
    assert_eq!(shown["runs"].as_array().unwrap().len(), 1, "{shown}");
    assert!(service.stop().success());

    // Started again: the first schedule's next time is tomorrow, so only the one due now runs.
    let report = due_now(&data.0, &chat, &ann, "Send the planting report", true);
    let service = start();
    let history = service.wait_for(&chat, 30, "the second scheduled turn to end", |h| {
        scheduled(h)
            .iter()
            .filter(|t| t["status"] == "done")
            .count()
            == 2
    });
    assert_eq!(scheduled(&history).len(), 2, "{history}");
    let shown = service
        .call("GET", &format!("/api/schedules/{water}"))
        .json();
    assert_eq!(shown["runs"].as_array().unwrap().len(), 1, "{shown}");
    // A once that ran leaves the list.
    let listed = service.call("GET", "/api/schedules").json();
    let ids: Vec<&Value> = listed["schedules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| &s["id"])
        .collect();
    assert_eq!(ids, [&json!(water)], "{listed} {report}");
}
