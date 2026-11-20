//! A chat of one agent keeps its list too (0028): its agent leads it from its first message, and the
//! upgrade to 0058 gives each older chat the agent of its latest turn. Through a real service on
//! Claude Code's replay stand-in, what the list brings a chat of one: the person's like is told to
//! its agent, and a failed last turn marks the chat and says where the agent stands. On a real
//! database, what looks for a lead still finds none in a chat of one with no project, as before: a
//! stale question, the quiet room's wake, a task given, a turn's conversation, an agent's delete and
//! the briefing.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, gathr, recording, rows, sign_in_account};
use gathr::quiet::{self, Times};
use gathr::sessions::{self, Key};
use gathr::tasks::{self, Give, Kind};
use gathr::waiting::Live;
use rusqlite::Connection;
use serde_json::{Value, json};

const T: &str = "test-trace";
const S: i64 = 1_000_000;
const DAY: i64 = 86_400 * S;

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

/// 0078: a chat of one is drawn by the same rule as a team's. Each answer of its agent is a main
/// message of its own, and the person's reply to an older answer, and what its turn says, sit in
/// that answer's thread (0076).
#[test]
fn a_chat_of_one_has_each_answer_main_and_a_reply_in_the_thread_of_what_it_answers() {
    let data = TempDir::new();
    let replay = engine(&data.0, 4);
    let (service, ids) = service_with(&data.0, &[("GATHR_REPLAY", &replay)], &["Mira"]);
    let chat = service.new_chat();
    let asked = |body: Value| {
        let sent = post(&service, &chat, body);
        let turn = sent["turn"].as_str().unwrap().to_string();
        (turn.clone(), ended(&service, &chat, &turn, "done"))
    };
    let (first, shown) = asked(json!({ "text": "Plan the river walk", "agent": ids[0] }));
    let messages = shown["messages"].as_array().cloned().unwrap_or_default();
    let words = messages
        .iter()
        .find(|m| m["turn"] == first.as_str() && m["role"] == "assistant")
        .unwrap()["id"]
        .clone();
    let (second, _) = asked(json!({ "text": "And the map legend?", "agent": ids[0] }));
    let reply = json!({ "text": "Does it pass the old mill?", "agent": ids[0], "reply_to": words });
    let (third, shown) = asked(reply);
    let topic = |turn: &str| {
        let turns = shown["turns"].as_array().unwrap().iter();
        turns.clone().find(|t| t["id"] == turn).unwrap()["topic"].clone()
    };
    let found = [topic(&first), topic(&second), topic(&third)];
    let want = [
        json!(format!("b:{first}")),
        json!(format!("b:{second}")),
        json!(format!("b:{first}")),
    ];
    assert_eq!(found, want, "{shown}");
    common::record_ui(&service, &chat, "chat-of-one");
    common::topics_agree(&data.0, &chat);
}

#[test]
fn the_persons_like_on_a_lone_agents_words_is_told_to_it() {
    let data = TempDir::new();
    let replay = engine(&data.0, 3);
    let (service, ids) = service_with(&data.0, &[("GATHR_REPLAY", &replay)], &["Mira"]);
    let chat = service.new_chat();
    let sent = post(
        &service,
        &chat,
        json!({ "text": "Plan it", "agent": ids[0] }),
    );
    let turn = sent["turn"].as_str().unwrap();
    let shown = ended(&service, &chat, turn, "done");
    let messages = shown["messages"].as_array().cloned().unwrap_or_default();
    let words = messages
        .iter()
        .find(|m| m["turn"] == turn && m["role"] == "assistant" && m["kind"] == "text")
        .unwrap_or_else(|| panic!("no words in {turn}: {shown}"));
    let path = format!(
        "/api/chats/{chat}/messages/{}/like",
        words["id"].as_str().unwrap()
    );
    let given = service.send("PUT", &path, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    assert_eq!(given.json()["did"], "delivered", "{}", given.body);
}

/// The forced failure for this change: `cargo nextest run -p gathr --test solo failed -- --nocapture`
/// prints the failed turn's trace, found from the reference the window shows.
#[test]
fn a_lone_agents_failed_turn_marks_the_chat_and_says_it_cant_run() {
    let data = TempDir::new();
    let env = [("GATHR_REPLAY", recording("engine_exits"))];
    let env: Vec<(&str, &Path)> = env.iter().map(|(k, v)| (*k, v.as_path())).collect();
    let (service, ids) = service_with(&data.0, &env, &["Mira"]);
    let chat = service.new_chat();
    let sent = post(
        &service,
        &chat,
        json!({ "text": "Say hi", "agent": ids[0] }),
    );
    let shown = ended(&service, &chat, sent["turn"].as_str().unwrap(), "failed");

    let listed = service.call("GET", "/api/chats").json();
    let row = listed.as_array().unwrap().iter().find(|c| c["id"] == chat);
    assert_eq!(
        row.map(|c| c["mark"].clone()),
        Some(json!("cant_run")),
        "{listed}"
    );
    let waiting = common::waiting(&service, &chat);
    let stand = &waiting["agents"][0];
    assert_eq!(
        (&stand["name"], &stand["state"], &stand["words"]),
        (
            &json!("Mira"),
            &json!("cant_run"),
            &json!("Its last turn failed")
        ),
        "{waiting}"
    );
    let reference = shown["turns"][0]["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events.iter().any(|e| e["error_type"].is_string());
    assert!(ok && failed, "{reference}: {trace}");
    println!("diag trace {reference}: {trace}");
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
fn a_stale_question_in_a_chat_of_one_tells_no_lead() {
    let dir = TempDir::new();
    let mut conn = solo(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts)
         VALUES ('solo', 'mira', 'person', 'Pick a colour', 'ready', 1, 1)",
        [],
    )
    .unwrap();
    let swept = tasks::stale::sweep(&mut conn, 30 * DAY, DAY, &[], T).unwrap();
    assert_eq!(swept.marked.len(), 1, "{swept:?}");
    assert!(swept.starts.is_empty(), "nobody is told: {swept:?}");
}

#[test]
fn a_quiet_chat_of_one_wakes_nobody() {
    let dir = TempDir::new();
    let mut conn = solo(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, owner, kind, what, state, created_ts, updated_ts)
         VALUES ('solo', 'mira', 'work', 'Fix the build', 'ready', 5, 5)",
        [],
    )
    .unwrap();
    let live = Live {
        running: true,
        ..Live::default()
    };
    let times = Times {
        tell_after_us: 300 * S,
        look_us: 120 * S,
    };
    let t0 = gathr::ids::now_us();
    for seconds in [0, 150, 300] {
        let woke = quiet::check(&mut conn, "solo", &live, t0 + seconds * S, times, T).unwrap();
        assert!(woke.is_empty(), "woke at {seconds} s: {woke:?}");
    }
}

#[test]
fn a_task_given_in_a_chat_of_one_is_refused() {
    let dir = TempDir::new();
    let mut conn = solo(&dir);
    let g = Give {
        chat: "solo",
        giver: Some("mira"),
        owner: "mira",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix the build",
        brief: common::COMPLETE,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: false,
    };
    let refused = tasks::give(&mut conn, &g, T).map(|(t, _)| t.number);
    assert_eq!(
        refused.map_err(|e| e.to_string()),
        Err("tasks are given in a chat with a team".to_string())
    );
}

#[test]
fn a_lone_agents_turn_keeps_the_conversation_key_it_had() {
    let dir = TempDir::new();
    let conn = solo(&dir);
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (1, 'solo', 'mira', 'mira', 'work', 'Fix the build', 'working', 1, 1)",
        [],
    )
    .unwrap();
    let add = |id: &str, task: Option<i64>| {
        let by = (Some("mira"), task, None);
        gathr::store::add_turn(&conn, "solo", id, T, by, gathr::words::Opened::Person).unwrap();
    };
    add("on-1", Some(1));
    add("free", None);
    let key = |turn: &str| sessions::want(&conn, turn).unwrap();
    assert_eq!((key("on-1"), key("free")), (Key::Task(1), Key::Latest));
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
