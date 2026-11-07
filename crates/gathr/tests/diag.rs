//! The `gathr diag` seam: the command against a data folder, as an agent runs it.

mod common;

use common::{TempDir, TestService, gathr, gathrd};

/// Hand check 10 of docs/first-slice.md.
#[test]
fn health_explains_a_database_that_wont_open() {
    let dir = TempDir::new();
    std::fs::write(dir.0.join("gathr.db"), "these are a few bytes of text").unwrap();
    let start = gathrd(&dir.0).status().unwrap();
    assert!(!start.success(), "gathrd started on a broken database");

    let (ok, health) = gathr(&dir.0, &["diag", "health"]);
    assert!(ok, "{health}");
    assert_eq!(health["database"]["opened"], false, "{health}");
    assert!(
        health["database"]["why"]
            .as_str()
            .unwrap()
            .contains("not a database"),
        "{health}"
    );
    let lines = health["crash_log"]["last_lines"].to_string();
    assert!(
        lines.contains("start failed: the database didn't open"),
        "{health}"
    );
}

#[test]
fn the_next_good_start_moves_the_crash_log_into_the_record() {
    let dir = TempDir::new();
    std::fs::write(dir.0.join("gathr.db"), "not a database").unwrap();
    assert!(!gathrd(&dir.0).status().unwrap().success());
    std::fs::remove_file(dir.0.join("gathr.db")).unwrap();

    TestService::start(&dir.0).stop();
    let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.service.early_log'";
    let (_, answer) = gathr(&dir.0, &["diag", "query", sql]);
    assert!(
        answer["rows"][0]["attrs"]["lines"]
            .to_string()
            .contains("start failed"),
        "{answer}"
    );
    let (_, health) = gathr(&dir.0, &["diag", "health"]);
    assert_eq!(
        health["crash_log"]["last_lines"],
        serde_json::json!([]),
        "{health}"
    );
}

#[test]
fn query_refuses_to_write() {
    let dir = TempDir::new();
    TestService::start(&dir.0).stop();
    let (ok, answer) = gathr(&dir.0, &["diag", "query", "DELETE FROM events"]);
    assert!(!ok, "{answer}");
    let (_, count) = gathr(
        &dir.0,
        &["diag", "query", "SELECT count(*) AS n FROM events"],
    );
    assert!(count["rows"][0]["n"].as_i64().unwrap() > 0, "{count}");
}

#[test]
fn failures_are_grouped_by_cause_with_references() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    let refs: Vec<String> = (0..3)
        .map(|_| {
            service.get("/health", &[]).json()["ref"]
                .as_str()
                .unwrap()
                .to_string()
        })
        .collect();
    let (ok, answer) = gathr(&dir.0, &["diag", "failures", "--since", "1h"]);
    assert!(ok, "{answer}");
    let group = &answer["groups"][0];
    assert_eq!(group["name"], "gathr.api.request", "{answer}");
    assert_eq!(group["error_type"], "401", "{answer}");
    assert_eq!(group["count"], 3, "{answer}");
    assert!(
        refs.iter()
            .all(|r| group["references"].to_string().contains(r)),
        "{answer}"
    );
}

#[test]
fn seed_makes_the_three_test_chats() {
    let dir = TempDir::new();
    let (ok, seeded) = gathr(&dir.0, &["seed"]);
    assert!(ok, "{seeded}");
    let sql = "SELECT c.title,
                 (SELECT count(*) FROM messages m WHERE m.chat_id = c.id) AS messages,
                 (SELECT max(n) FROM (SELECT count(*) AS n FROM messages m WHERE m.chat_id = c.id AND kind = 'tool' GROUP BY turn_id)) AS most_tools,
                 (SELECT max(length(body) - length(replace(body, ' ', '')) + 1) FROM messages m WHERE m.chat_id = c.id AND kind = 'text') AS longest
               FROM chats c ORDER BY c.created_ts";
    let (_, chats) = gathr(&dir.0, &["diag", "query", sql]);
    let rows = &chats["rows"];
    assert_eq!(rows[0]["messages"], 500, "{chats}");
    assert_eq!(rows[1]["most_tools"], 50, "{chats}");
    assert_eq!(rows[2]["longest"], 20_000, "{chats}");
}

#[test]
fn seed_waits_for_the_service_to_stop() {
    let dir = TempDir::new();
    let _service = TestService::start(&dir.0);
    let (ok, answer) = gathr(&dir.0, &["seed"]);
    assert!(!ok, "{answer}");
    assert!(
        answer["why"]
            .as_str()
            .unwrap()
            .contains("service is running"),
        "{answer}"
    );
}
