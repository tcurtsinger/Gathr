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
fn health_distinguishes_a_newer_database_from_this_builds_schema() {
    let dir = TempDir::new();
    rusqlite::Connection::open(dir.0.join("gathr.db"))
        .unwrap()
        .pragma_update(None, "user_version", 99)
        .unwrap();
    let (ok, health) = gathr(&dir.0, &["diag", "health"]);
    assert!(ok, "{health}");
    assert_eq!(health["database"]["opened"], false, "{health}");
    assert_eq!(
        health["database"]["why"],
        format!(
            "This database uses schema 99; this build supports schema {}. Update Gathr.",
            gathr::db::SCHEMA_VERSION
        ),
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

/// F8b's proof data: a five-agent chat whose rounds link up the way the inline thread reads them.
#[test]
fn seed_makes_a_five_agent_team_chat_whose_rounds_link_up() {
    let dir = TempDir::new();
    for _ in 0..2 {
        let (ok, seeded) = gathr(&dir.0, &["seed"]);
        assert!(ok, "each run adds another set: {seeded}");
    }
    let query = |sql: &str| gathr(&dir.0, &["diag", "query", sql]).1["rows"].clone();
    let team = query(
        "SELECT chat_id, group_concat(role) AS roles, count(*) AS n FROM chat_members
         GROUP BY chat_id HAVING n = 5 ORDER BY chat_id",
    );
    assert_eq!(team.as_array().unwrap().len(), 2, "{team}");
    for chat in team.as_array().unwrap() {
        let mut roles: Vec<&str> = chat["roles"].as_str().unwrap().split(',').collect();
        roles.sort();
        assert_eq!(
            roles,
            ["member", "member", "member", "reviewer", "supervisor"]
        );
    }
    let chat = team[1]["chat_id"].as_str().unwrap();
    let one = |sql: &str| query(&sql.replace("?chat", &format!("'{chat}'")))[0]["n"].clone();
    assert!(
        one("SELECT count(*) AS n FROM messages WHERE chat_id = ?chat").as_i64() > Some(300),
        "a real-size chat"
    );
    let unlinked = "SELECT count(*) AS n FROM tasks k JOIN turns u ON u.task = k.number
                    WHERE k.chat_id = ?chat AND u.answers IS NOT k.given_in";
    assert_eq!(
        one(unlinked),
        0,
        "a task's turns answer the turn that gave it"
    );
    let given_by_lead = "SELECT count(*) AS n FROM tasks k JOIN turns u ON u.id = k.given_in
                         JOIN chat_members m ON m.chat_id = k.chat_id AND m.agent_id = u.agent_id
                         WHERE k.chat_id = ?chat AND m.role = 'supervisor' AND k.kind IN ('work', 'review')";
    let tasks =
        one("SELECT count(*) AS n FROM tasks WHERE chat_id = ?chat AND kind IN ('work', 'review')");
    assert!(tasks.as_i64() > Some(20), "{tasks}");
    assert_eq!(one(given_by_lead), tasks, "every task is the lead's");
    let rounds = one(
        "SELECT count(DISTINCT given_in) AS n FROM tasks WHERE chat_id = ?chat AND kind IN ('work', 'review')",
    );
    let hear_backs = "SELECT count(DISTINCT u.answers) AS n FROM turns u
                      JOIN messages g ON g.turn_id = u.id AND g.role = 'gathr'
                      WHERE u.chat_id = ?chat AND u.task IS NULL AND u.answers IS NOT NULL";
    assert_eq!(one(hear_backs), rounds, "each round ends with a hear-back");
    let review = "SELECT count(*) AS n FROM tasks WHERE chat_id = ?chat AND kind = 'review'
                  AND state = 'done' AND reviewed IS NOT NULL";
    assert!(one(review).as_i64() > Some(0), "a done review");
    let late = "SELECT count(*) AS n FROM turns w JOIN tasks k ON k.number = w.task
                JOIN turns h ON h.answers = k.given_in AND h.task IS NULL
                WHERE w.chat_id = ?chat AND w.position > h.position";
    assert!(
        one(late).as_i64() > Some(0),
        "a reply after the lead's last word"
    );
}

/// F9's proof data: the person's open questions across two chats, one of each form, with files.
#[test]
fn seed_asks_the_person_one_of_each_form_across_two_chats() {
    let dir = TempDir::new();
    let (ok, seeded) = gathr(&dir.0, &["seed"]);
    assert!(ok, "{seeded}");
    let query = |sql: &str| gathr(&dir.0, &["diag", "query", sql]).1["rows"].clone();
    let open = query(
        "SELECT count(*) AS n, count(DISTINCT chat_id) AS chats, count(DISTINCT form) AS forms,
                count(files) AS with_files, count(request) AS for_members
         FROM tasks WHERE kind = 'person' AND state = 'ready'",
    );
    let open = &open[0];
    let counts =
        ["n", "chats", "forms", "with_files", "for_members"].map(|k| open[k].as_i64().unwrap());
    assert_eq!(counts, [6, 2, 5, 3, 1], "{open}");
    let files = query("SELECT files FROM tasks WHERE files IS NOT NULL");
    for row in files.as_array().unwrap() {
        let list: serde_json::Value = serde_json::from_str(row["files"].as_str().unwrap()).unwrap();
        let path = list[0]["path"].as_str().unwrap();
        assert!(std::path::Path::new(path).starts_with(&dir.0), "{path}");
        assert!(std::path::Path::new(path).is_file(), "{path}");
    }
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

/// Vale's review of #22: the count stopped at 1,000 and a rarer cause behind it vanished.
#[test]
fn failures_count_every_event_and_keep_rare_causes() {
    let dir = TempDir::new();
    let service = TestService::start(&dir.0);
    assert_eq!(service.call("GET", "/api/no-such-route").status, 404);
    for _ in 0..1001 {
        service.get("/health", &[]);
    }
    let (ok, answer) = gathr(&dir.0, &["diag", "failures", "--since", "1h"]);
    assert!(ok, "{answer}");
    let groups = answer["groups"].as_array().unwrap();
    let count = |error_type: &str| {
        groups
            .iter()
            .find(|g| g["error_type"] == error_type)
            .map(|g| g["count"].clone())
    };
    assert_eq!(count("401"), Some(1001.into()), "{answer}");
    assert_eq!(count("404"), Some(1.into()), "{answer}");
    assert_eq!(
        groups[0]["error_type"], "401",
        "newest cause first: {answer}"
    );
}

#[test]
fn a_bad_since_is_an_error_not_a_crash() {
    let dir = TempDir::new();
    assert!(gathr(&dir.0, &["seed"]).0);
    // A value of zero or less would reach past now, and a negative one overflowed it.
    for since in ["日", "1日", "m", "99999999999999999d", "-1m", "0h"] {
        let flag = format!("--since={since}");
        let (ok, answer) = gathr(&dir.0, &["diag", "failures", &flag]);
        assert!(!ok, "{flag}: {answer}");
        assert!(
            answer.to_string().contains("since \\\""),
            "{flag}: {answer}"
        );
    }
}
