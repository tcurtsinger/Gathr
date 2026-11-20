//! A chat of one agent keeps its list too (0028): its agent leads it from its first message, and the
//! upgrade to 0058 gives each older chat the agent of its latest turn. Through a real service on
//! Claude Code's replay stand-in, what the list brings a chat of one: the person's like is told to
//! its agent, and a failed last turn marks the chat and says where the agent stands. On a real
//! database, what looks for a lead still finds none in a chat of one with no project, as before: a
//! stale question, the quiet room's wake, a task given, a turn's conversation, an agent's delete and
//! the briefing.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, recording, rows, sign_in_account};
use rusqlite::Connection;
use serde_json::{Value, json};

/// Claude Code's stand-in for `n` turns of the signed-in recording.
fn engine(data: &Path, n: usize) -> PathBuf {
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let all: Vec<&str> = (0..n).flat_map(|_| frames.lines()).collect();
    let file = data.join("turns.jsonl");
    std::fs::write(&file, all.join("\n") + "\n").unwrap();
    file
}

/// A service on `replay`, a signed-in account and an agent of each name on it: the service and the
/// agents' ids.
fn service_with(data: &Path, env: &[(&str, &Path)], names: &[&str]) -> (TestService, Vec<String>) {
    let service = TestService::with_engine(data, env);
    let account = service.add_account("Main");
    sign_in_account(data, &account);
    let ids = names.iter().map(|name| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    });
    let ids = ids.collect();
    (service, ids)
}

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

fn post(service: &TestService, chat: &str, body: Value) -> Value {
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()
}

/// The chat's members as `[agent, role]`.
fn members(service: &TestService, chat: &str) -> Vec<Value> {
    let shown = history(service, chat);
    let all = shown["members"].as_array().cloned().unwrap_or_default();
    all.iter().map(|m| json!([m["agent"], m["role"]])).collect()
}

fn ended(service: &TestService, chat: &str, turn: &str, status: &str) -> Value {
    let what = format!("turn {turn} to be {status}");
    service.wait_for(chat, 30, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && t["status"] == status)
    })
}

fn changes(data: &Path, chat: &str) -> Vec<Value> {
    let sql = format!(
        "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.members' AND conversation_id = '{chat}'"
    );
    rows(data, &sql)
}

#[test]
fn a_chat_with_no_project_has_its_first_agent_as_its_lead_and_a_joiner_as_a_member() {
    let data = TempDir::new();
    let replay = engine(&data.0, 3);
    let (service, ids) = service_with(&data.0, &[("GATHR_REPLAY", &replay)], &["Mira", "Bo"]);
    let (mira, bo) = (&ids[0], &ids[1]);
    let chat = service.new_chat();
    assert_eq!(
        members(&service, &chat),
        Vec::<Value>::new(),
        "no agent yet"
    );

    let first = post(
        &service,
        &chat,
        json!({ "text": "Start here", "agent": mira }),
    );
    assert_eq!(members(&service, &chat), [json!([mira, "supervisor"])]);
    ended(&service, &chat, first["turn"].as_str().unwrap(), "done");

    let picks = json!([{ "at": 0, "len": 3, "agent": bo }]);
    post(
        &service,
        &chat,
        json!({ "text": "@Bo join us", "agent": bo, "picks": picks }),
    );
    assert_eq!(
        members(&service, &chat),
        [json!([mira, "supervisor"]), json!([bo, "member"])]
    );
    assert_eq!(
        changes(&data.0, &chat),
        [json!({ "n": 2 })],
        "Mira's seat, then Bo's join in one change"
    );
}

/// Mira, Ann and Zed (removed) on one account, as a database before 0058 might hold them.
fn agents_sql() -> &'static str {
    "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
     INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
         ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
     INSERT INTO agents (id, name, name_key, account_id, created_ts, removed_ts) VALUES
         ('zed', 'Zed', 'zed', NULL, 1, 2);"
}

/// A turn of `agent`'s in `chat` at `at`.
fn turn_sql(id: &str, chat: &str, agent: Option<&str>, at: i64) -> String {
    let agent = agent.map_or("NULL".into(), |a| format!("'{a}'"));
    format!(
        "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id) VALUES
             ('{id}', '{chat}', 'x', 'done', {at}, {agent});"
    )
}

/// Mira's chat with no project and her alone on its list, and Ann's team chat `c` in no project.
fn solo(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(agents_sql()).unwrap();
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('solo', 'Solo', 1);
         INSERT INTO chat_members VALUES ('solo', 'mira', 'supervisor', 1);",
    )
    .unwrap();
    conn
}

#[test]
fn deleting_an_agent_keeps_a_chat_of_one_where_others_spoke() {
    let dir = TempDir::new();
    let conn = solo(&dir);
    let mut sql = String::from(
        "INSERT INTO chats (id, title, created_ts) VALUES ('mixed', 'Mixed', 1);
         INSERT INTO chat_members VALUES ('mixed', 'mira', 'supervisor', 1);",
    );
    sql.push_str(&turn_sql("s1", "solo", Some("mira"), 10));
    sql.push_str(&turn_sql("m1", "mixed", Some("ann"), 10));
    sql.push_str(&turn_sql("m2", "mixed", Some("mira"), 20));
    conn.execute_batch(&sql).unwrap();
    let plan = gathr::deleting::agent_plan(&conn, "mira").unwrap();
    assert_eq!(plan.chats, ["solo"], "Ann's words stay: {plan:?}");
}
