//! How full each agent's context is (compaction plan, K1): from the engine's own frames, on the
//! chat's members and sent live, for Claude Code and Codex alike.

mod common;

use common::{TempDir, TestService, next_live};
use serde_json::{Value, json};

/// An agent on `account` runs one turn in a team chat: the chat's id and the agent's.
fn one_turn(service: &TestService, account: &str) -> (String, String) {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Made-up", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let other = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Made-up other", "account": account }),
    );
    let other = other.json()["id"].as_str().unwrap().to_string();
    // A team chat, as Summary shows members; a plain chat lists none.
    let members =
        json!([{ "agent": agent, "role": "supervisor" }, { "agent": other, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    (chat, agent)
}

/// The live `context` update for the agent once it holds its window.
fn live_context(live: &mut std::net::TcpStream, agent: &str) -> Value {
    loop {
        let update = next_live(live);
        if update["type"] == "context"
            && update["agent"] == agent
            && !update["context"]["window"].is_null()
        {
            return update;
        }
    }
}

#[test]
fn a_claude_turn_shows_its_last_calls_fill_and_its_models_window() {
    let data = TempDir::new();
    let recording = common::recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording)]);
    let account = service.signed_in(&data.0);
    let mut live = service.live();
    let (chat, agent) = one_turn(&service, &account);

    let update = live_context(&mut live, &agent);
    assert_eq!(update["chat"], chat.as_str());
    // On its engine's default: the point shows once the engine has named its model and its window.
    let expected =
        json!({ "used": 2 + 23_627 + 1_711, "window": 1_000_000, "compact_at": 500_000 });
    assert_eq!(update["context"], expected, "{update}");
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let member = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == agent.as_str())
        .unwrap();
    assert_eq!(member["context"], expected, "{history}");
}

/// A service whose Codex engine replays `recording`, and a signed-in Codex account's id.
fn codex(data: &std::path::Path, recording: &std::path::Path) -> (TestService, String) {
    let replay = common::replay_engine();
    let service = TestService::start_with(
        data,
        &[("GATHR_CODEX_PATH", &replay), ("GATHR_REPLAY", recording)],
    );
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "codex", "name": "Codex test" }),
    );
    let account = added.json()["id"].as_str().unwrap().to_string();
    let folder = data.join("accounts").join(&account).join("codex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("auth.json"),
        r#"{"tokens":{"refresh_token":"made-up"}}"#,
    )
    .unwrap();
    (service, account)
}

#[test]
fn a_codex_turn_shows_its_last_calls_fill_and_its_window() {
    let data = TempDir::new();
    let (service, account) = codex(&data.0, &common::recording("codex_signed_in"));
    let mut live = service.live();
    let (chat, agent) = one_turn(&service, &account);

    let update = live_context(&mut live, &agent);
    assert_eq!(update["context"]["window"], 475_000, "{update}");
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    assert_eq!(
        history["members"][0]["context"],
        json!({ "used": 16_291, "window": 475_000, "compact_at": 380_000 }),
        "{history}"
    );
}

#[test]
fn a_compaction_says_so_in_the_chat_and_shows_the_fill_after_it() {
    let data = TempDir::new();
    let recording = common::recording("compact_claude");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording)]);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "/compact", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);

    let history = service.wait_for(&chat, 10, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    let lines: Vec<&str> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "gathr")
        .filter_map(|m| m["body"].as_str())
        .collect();
    assert_eq!(
        lines,
        ["Compacted Made-up's conversation: 23K to 3.6K"],
        "{history}"
    );
    let member = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["agent"] == agent.as_str())
        .unwrap();
    assert_eq!(member["context"]["used"], 3_611, "{history}");
}

/// Codex reports a compaction's usage as a call with no input (#125's review): the saved fill stays
/// the last real call's, whether a call follows in the turn or the turn ends first.
#[test]
fn a_codex_compaction_keeps_the_last_real_fill() {
    let full = std::fs::read_to_string(common::recording("compact_codex")).unwrap();
    let rows: Vec<&str> = full.lines().collect();
    let compacted = rows
        .iter()
        .position(|r| r.contains("item/completed") && r.contains("contextCompaction"))
        .unwrap();
    let ended = rows
        .iter()
        .rposition(|r| r.contains("turn/completed"))
        .unwrap();
    // The same turn, ending as soon as its compaction does: no call after it.
    let cut = [&rows[..=compacted], &rows[ended..]].concat().join(
        "
",
    );
    for (recording, used) in [(full, 14_835), (cut, 14_577)] {
        let data = TempDir::new();
        let path = data.0.join("compact_codex.jsonl");
        std::fs::write(&path, recording).unwrap();
        let (service, account) = codex(&data.0, &path);
        let (chat, agent) = one_turn(&service, &account);
        service.wait_for(&chat, 10, "the first turn", |h| {
            h["turns"][0]["status"] == "done"
        });
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send(
            "POST",
            &path,
            json!({ "text": "Say hi again", "agent": agent }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        let history = service.wait_for(&chat, 10, "the compaction's turn", |h| {
            h["turns"][1]["status"] == "done"
        });
        assert_eq!(history["members"][0]["context"]["used"], used, "{history}");
    }
}

/// Settings > General's Auto-compact, saved.
fn auto_compact(service: &TestService, at: Value) {
    let saved = service.send("PATCH", "/api/settings", json!({ "auto_compact": at }));
    assert_eq!(saved.status, 200, "{}", saved.body);
    assert_eq!(saved.json()["auto_compact"], at, "{}", saved.body);
}

/// A window the agent showed in another chat while set to `set` (`context::model_key`), as a turn
/// there would have saved it: on the model `ran`, or none named, as a window from before 0031 has.
fn seen_window(
    data: &std::path::Path,
    service: &TestService,
    agent: &str,
    window: i64,
    (set, ran): (&str, Option<&str>),
) {
    let other = service.new_chat();
    let db = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
    db.execute(
        "INSERT INTO agent_context (chat_id, agent_id, window, window_of, ran, updated_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, 1)",
        rusqlite::params![other, agent, window, set, ran],
    )
    .unwrap();
}

/// An agent on `account`, on `model` (`None`: the engine's default).
fn agent(service: &TestService, account: &str, model: Option<&str>) -> String {
    let body = json!({ "name": "Made-up", "account": account, "model": model });
    let made = service.send("POST", "/api/agents", body);
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// The agent's turn in a chat of its own, once it's done: the chat.
fn turn_in_a_new_chat(service: &TestService, agent: &str) -> String {
    let chat = service.new_chat();
    turn(service, &chat, agent);
    chat
}

fn turn(service: &TestService, chat: &str, agent: &str) {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Hi", "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(chat, 10, "the turn's end", |h| {
        let turns = h["turns"].as_array().unwrap();
        turns.iter().all(|t| t["status"] == "done")
    });
}

const OPUS: &str = "claude-opus-5";
/// The model `signed_in`'s turn runs on, whatever the agent is set to.
const RUNS: &str = "claude-opus-5-5";

/// A service whose Claude Code plays `recording`, each start's environment in `env.jsonl`, and a
/// signed-in account.
fn claude_env(data: &std::path::Path, recording: &std::path::Path) -> (TestService, String) {
    let env = [
        ("GATHR_REPLAY", recording),
        ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
    ];
    let service = TestService::with_engine(data, &env);
    let account = service.signed_in(data);
    (service, account)
}

/// The Auto-compact point the `n`th engine started with.
fn started_with(data: &std::path::Path, n: usize) -> Value {
    common::nth(&data.join("env.jsonl"), n)["CLAUDE_CODE_AUTO_COMPACT_WINDOW"].clone()
}

/// K3: Claude Code is started with Auto-compact, never past 80% of the window the agent's model
/// showed while set as it is now, on the same account: pinned, a Latest or the engine's default
/// alike. With the engine's own, another account's window, one seen while set to another model, one
/// with no model named or none seen yet, with none (#168's review and second look).
#[test]
fn claude_code_starts_with_auto_compact_capped_at_80_percent_of_its_window() {
    // (the setting, the agent's model, the window seen: its size, what the agent was set to then
    // (`same` for as now), whether its model was named, on the same account; the point).
    type Seen = (i64, &'static str, bool, bool);
    let seen = |window, set, named, account| Some((window, set, named, account));
    let cases: [(Value, Option<&str>, Option<Seen>, Value); 10] = [
        (
            json!(500_000),
            Some(OPUS),
            seen(200_000, "same", true, true),
            json!("160000"),
        ),
        (
            json!(300_000),
            Some(OPUS),
            seen(1_000_000, "same", true, true),
            json!("300000"),
        ),
        (
            json!(300_000),
            None,
            seen(1_000_000, "same", true, true),
            json!("300000"),
        ),
        (
            json!(500_000),
            Some("opus"),
            seen(200_000, "same", true, true),
            json!("160000"),
        ),
        (json!(300_000), Some(OPUS), None, Value::Null),
        (json!(300_000), Some("opus"), None, Value::Null),
        (
            json!(300_000),
            Some(OPUS),
            seen(1_000_000, "same", true, false),
            Value::Null,
        ),
        (
            json!(300_000),
            Some("opus"),
            seen(1_000_000, OPUS, true, true),
            Value::Null,
        ),
        (
            json!(300_000),
            Some(OPUS),
            seen(1_000_000, "same", false, true),
            Value::Null,
        ),
        (
            Value::Null,
            Some("opus"),
            seen(200_000, "same", true, true),
            Value::Null,
        ),
    ];
    for (at, model, window, expected) in cases {
        let data = TempDir::new();
        let (service, account) = claude_env(&data.0, &common::recording("signed_in"));
        auto_compact(&service, at.clone());
        let ann = agent(&service, &account, model);
        if let Some((window, set, named, same_account)) = window {
            let set = match set {
                "same" => model.unwrap_or("default"),
                other => other,
            };
            let account = if same_account { &account } else { "another" };
            let set = format!("{account}:claude:{set}");
            let on = (set.as_str(), named.then_some(RUNS));
            seen_window(&data.0, &service, &ann, window, on);
        }
        let chat = service.new_chat();
        let sent = service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({ "text": "Hi", "agent": ann }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        // An agent given a model has the account's models listed first, by an engine of its own.
        let started = started_with(&data.0, usize::from(model.is_some()));
        assert_eq!(started, expected, "{at} on {model:?} with {window:?}");
    }
}

/// Auto-compact reaches every agent: one on a Latest or its engine's default starts with the point
/// from its second start on, once the engine has named its model and shown its window.
#[test]
fn an_agent_on_a_latest_or_the_engines_default_starts_with_its_point_once_its_window_is_seen() {
    for model in [Some("opus"), None] {
        let data = TempDir::new();
        let (service, account) = claude_env(&data.0, &common::recording("signed_in"));
        auto_compact(&service, json!(300_000));
        let ann = agent(&service, &account, model);
        turn_in_a_new_chat(&service, &ann);
        turn_in_a_new_chat(&service, &ann);

        let listed = usize::from(model.is_some());
        let points = [0, 1].map(|n| started_with(&data.0, listed + n));
        assert_eq!(points, [Value::Null, json!("300000")], "{model:?}");
    }
}

/// A Latest that moved to a model with a smaller window never keeps the old model's point: the
/// engine names its model in its first call, its window from the old one is dropped, and it starts
/// again before its next turn, with the point its own window allows.
#[test]
fn a_latest_that_moved_to_a_smaller_model_starts_again_with_that_models_point() {
    let data = TempDir::new();
    let small = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    assert!(small.contains(r#"\"contextWindow\":1000000"#), "{small}");
    let small = small.replace(
        r#"\"contextWindow\":1000000"#,
        r#"\"contextWindow\":200000"#,
    );
    let recording = data.0.join("small.jsonl");
    std::fs::write(&recording, small).unwrap();
    let (service, account) = claude_env(&data.0, &recording);
    let ann = agent(&service, &account, Some("opus"));
    let set = format!("{account}:claude:opus");
    seen_window(
        &data.0,
        &service,
        &ann,
        1_000_000,
        (&set, Some("claude-opus-4")),
    );

    let chat = turn_in_a_new_chat(&service, &ann);
    // The same chat, agent and model: nothing else would start the engine again.
    turn(&service, &chat, &ann);

    // The models' list, the start on the old model's point, then the start on its own.
    let points = [1, 2].map(|n| started_with(&data.0, n));
    assert_eq!(points, [json!("500000"), json!("160000")]);
    let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.context.moved'";
    let (ok, moved) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{moved}");
    let rows = moved["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{moved}");
    let attrs = rows[0]["attrs"].to_string();
    assert!(
        attrs.contains("claude-opus-4") && attrs.contains(RUNS),
        "{moved}"
    );
}

/// K3: Codex's thread opens with Auto-compact as `model_auto_compact_token_limit`, once its
/// model's window is known, pinned or its engine's default alike: capped at 80% of it. Before, the
/// engine's own.
#[test]
fn codex_opens_its_thread_with_auto_compact() {
    for model in [Some("gpt-6.1-sol"), None] {
        let data = TempDir::new();
        let (service, account) = codex(&data.0, &common::recording("codex_signed_in"));
        auto_compact(&service, json!(400_000));
        let ann = agent(&service, &account, model);
        // Each in a chat of its own: the engine starts again, the second time knowing the window.
        turn_in_a_new_chat(&service, &ann);
        turn_in_a_new_chat(&service, &ann);
        let sql = "SELECT body FROM engine_raw WHERE direction = 'in' AND body LIKE '%\"thread/start\"%' ORDER BY id";
        let (ok, rows) = common::gathr(&data.0, &["diag", "query", sql]);
        assert!(ok, "{rows}");
        let opened = |n: usize| -> Value {
            serde_json::from_str(rows["rows"][n]["body"].as_str().unwrap()).unwrap()
        };
        let limit =
            |n: usize| opened(n)["params"]["config"]["model_auto_compact_token_limit"].clone();
        assert_eq!(
            [limit(0), limit(1)],
            [Value::Null, json!(380_000)],
            "{model:?}: {rows}"
        );
    }
}

/// #169's review: a result that lists only another model's window than the one the agent's calls
/// named saves none, so that window never gives the calls' model a point.
#[test]
fn a_window_named_for_another_model_than_the_calls_isnt_the_agents() {
    let data = TempDir::new();
    let other = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let listed = r#"\"modelUsage\":{\"claude-opus-5-5\""#;
    assert!(other.contains(listed), "{other}");
    let other = other.replace(listed, r#"\"modelUsage\":{\"claude-haiku-5-5\""#);
    let recording = data.0.join("other.jsonl");
    std::fs::write(&recording, other).unwrap();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording)]);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    let history = service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let members = history["members"].as_array().unwrap();
    let member = members.iter().find(|m| m["agent"] == agent.as_str());
    let expected = json!({ "used": 2 + 23_627 + 1_711, "window": null, "compact_at": null });
    assert_eq!(member.unwrap()["context"], expected, "{history}");
}

/// #169's review: the restart for a moved model waits for the engine's background commands, whose
/// end and the turn it starts would go with the engine: the next turn goes to the same engine. (With
/// none running, `a_latest_that_moved_to_a_smaller_model_starts_again_with_that_models_point`.)
#[test]
fn a_moved_model_never_closes_the_engine_under_its_background_commands() {
    let data = TempDir::new();
    let (recording, started, go) = (
        common::recording("background"),
        data.0.join("env.jsonl"),
        data.0.join("go"),
    );
    let env = [
        ("GATHR_REPLAY", recording.as_path()),
        ("GATHR_REPLAY_ENV", &started),
        ("GATHR_REPLAY_HOLD", &go),
    ];
    let service = TestService::with_engine(&data.0, &env);
    let account = service.signed_in(&data.0);
    let ann = agent(&service, &account, None);
    let set = format!("{account}:claude:default");
    seen_window(
        &data.0,
        &service,
        &ann,
        1_000_000,
        (&set, Some("claude-opus-4")),
    );
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let send = || service.send("POST", &path, json!({ "text": "Hi", "agent": ann }));

    assert_eq!(send().status, 200);
    service.wait_for(&chat, 10, "the turn that starts them", |h| {
        h["turns"][0]["status"] == "done"
    });
    let sql = "SELECT name FROM event_log WHERE name = 'gathr.context.moved'";
    let (_, moved) = common::gathr(&data.0, &["diag", "query", sql]);
    assert_eq!(moved["rows"].as_array().unwrap().len(), 1, "{moved}");
    // Its command still runs: a closed engine would leave the turn queued behind its exit.
    assert_eq!(send().json()["status"], "running");
    let starts = std::fs::read_to_string(&started).unwrap();
    assert_eq!(starts.lines().count(), 1, "{starts}");
    // Lets the held engine end, so the service stops without waiting for it.
    std::fs::write(&go, "").unwrap();
}

/// A window from an engine that named no model never sets a point, even after one that did: the
/// model an earlier engine named isn't carried over to it.
#[test]
fn a_window_seen_with_no_model_named_never_sets_the_point() {
    let data = TempDir::new();
    let named = std::fs::read_to_string(common::recording("codex_signed_in")).unwrap();
    let recording = data.0.join("codex.jsonl");
    std::fs::write(&recording, &named).unwrap();
    let (service, account) = codex(&data.0, &recording);
    auto_compact(&service, json!(400_000));
    let ann = agent(&service, &account, None);
    turn_in_a_new_chat(&service, &ann);

    let (model, window) = (
        r#"\"model\":\"gpt-6.1-sol\","#,
        r#"\"modelContextWindow\":475000"#,
    );
    assert!(named.contains(model) && named.contains(window), "{named}");
    let unnamed = named.replace(model, "");
    let unnamed = unnamed.replace(window, r#"\"modelContextWindow\":1000000"#);
    std::fs::write(&recording, unnamed).unwrap();
    turn_in_a_new_chat(&service, &ann);
    turn_in_a_new_chat(&service, &ann);

    let sql = "SELECT body FROM engine_raw WHERE direction = 'in' AND body LIKE '%\"thread/start\"%' ORDER BY id";
    let (ok, rows) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    let limits: Vec<Value> = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| serde_json::from_str::<Value>(row["body"].as_str().unwrap()).unwrap())
        .map(|opened| opened["params"]["config"]["model_auto_compact_token_limit"].clone())
        .collect();
    // 80% of the named model's 475,000 each time, never the unnamed 1,000,000's 400,000.
    assert_eq!(
        limits,
        [Value::Null, json!(380_000), json!(380_000)],
        "{rows}"
    );
}

/// K3: a turn whose agent's last call filled 75% of where it compacts is told to save its notes.
#[test]
fn a_turn_near_the_compaction_point_is_told_to_save_its_notes() {
    let data = TempDir::new();
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_REPLAY", &common::recording("signed_in"))],
    );
    let account = service.signed_in(&data.0);
    let ann = agent(&service, &account, Some(OPUS));
    let chat = service.new_chat();
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute(
        "INSERT INTO agent_context (chat_id, agent_id, used, window, window_of, ran, updated_ts)
         VALUES (?1, ?2, 380000, 1000000, ?3, ?4, 1)",
        rusqlite::params![chat, ann, format!("{account}:claude:{OPUS}"), RUNS],
    )
    .unwrap();
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Hi", "agent": ann }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let sql = "SELECT body FROM engine_raw WHERE direction = 'in' AND frame_type = 'user'";
    let (ok, rows) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    let first = rows["rows"][0]["body"].as_str().unwrap();
    assert!(
        first.contains("Your conversation is close to its compaction point"),
        "{first}"
    );
    let sql = "SELECT name FROM event_log WHERE name = 'gathr.context.soon'";
    let (_, told) = common::gathr(&data.0, &["diag", "query", sql]);
    assert_eq!(told["rows"].as_array().unwrap().len(), 1, "{told}");
}

/// Compact (K4) for the agent's conversation in the chat.
fn compact(service: &TestService, chat: &str, agent: &str) -> gathr::http::Response {
    let path = format!("/api/chats/{chat}/agents/{agent}/compact");
    service.send("POST", &path, json!({}))
}

/// The chat's quiet lines: what Gathr itself wrote in it.
fn gathr_lines(history: &Value) -> Vec<String> {
    let messages = history["messages"].as_array().unwrap();
    let gathr = messages.iter().filter(|m| m["role"] == "gathr");
    gathr
        .filter_map(|m| m["body"].as_str().map(String::from))
        .collect()
}

/// `compact_claude` cut at its `/compact`: the turn before it, and the compaction.
fn claude_parts() -> (Vec<String>, Vec<String>) {
    let text = std::fs::read_to_string(common::recording("compact_claude")).unwrap();
    let rows: Vec<String> = text.lines().map(String::from).collect();
    let at = rows
        .iter()
        .position(|r| r.contains(r#"\"content\":\"/compact\""#));
    let at = at.unwrap();
    (rows[..at].to_vec(), rows[at..].to_vec())
}

/// A service whose Claude Code plays `turn`, or `compaction` when started with its slash commands;
/// each start's arguments go in `args.jsonl`, and a `hold` row waits for `go`.
fn claude(data: &std::path::Path, turn: &[String], compaction: &[String]) -> TestService {
    let (played, compacting) = (data.join("turn.jsonl"), data.join("compaction.jsonl"));
    std::fs::write(&played, turn.join("\n")).unwrap();
    std::fs::write(&compacting, compaction.join("\n")).unwrap();
    let env = [
        ("GATHR_REPLAY", played.as_path()),
        ("GATHR_REPLAY_SLASH", &compacting),
        ("GATHR_REPLAY_ARGS", &data.join("args.jsonl")),
        ("GATHR_REPLAY_HOLD", &data.join("go")),
    ];
    TestService::with_engine(data, &env)
}

/// Whether the engine's `n`th start had its slash commands on.
fn slash(data: &std::path::Path, n: usize) -> bool {
    let args = common::nth(&data.join("args.jsonl"), n);
    !args
        .as_array()
        .unwrap()
        .iter()
        .any(|a| a == "--disable-slash-commands")
}

#[test]
fn compact_runs_claude_codes_compact_with_its_slash_commands_and_says_so() {
    let data = TempDir::new();
    let (turn, compaction) = claude_parts();
    let service = claude(&data.0, &turn, &compaction);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });

    let asked = compact(&service, &chat, &agent);
    assert_eq!(asked.status, 200, "{}", asked.body);
    let history = service.wait_for(&chat, 10, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    let line = "Compacted Made-up's conversation: 23K to 3.6K";
    assert_eq!(gathr_lines(&history), [line], "{history}");
    let asks = history["messages"].as_array().unwrap();
    assert_eq!(
        asks.iter().filter(|m| m["role"] == "user").count(),
        1,
        "{history}"
    );
    let reference = asked.json()["ref"].as_str().unwrap().to_string();
    let trace = service.call("GET", &format!("/api/trace/{reference}")).body;
    assert!(
        trace.contains("gathr.context.compact") && trace.contains("manual"),
        "{trace}"
    );

    // The agent's next turn runs without them again.
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send(
        "POST",
        &path,
        json!({ "text": "Say hi again", "agent": agent }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    service.wait_for(&chat, 10, "the turn after", |h| {
        h["turns"][2]["status"] == "done"
    });
    assert_eq!([0, 1, 2].map(|n| slash(&data.0, n)), [false, true, false]);
}

#[test]
fn compact_waits_for_the_turn_under_way_and_a_turn_queued_meanwhile_waits_for_it() {
    let data = TempDir::new();
    let (mut turn, compaction) = claude_parts();
    turn.insert(2, r#"{"dir":"hold"}"#.into());
    let service = claude(&data.0, &turn, &compaction);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "running"
    });

    let asked = compact(&service, &chat, &agent);
    assert_eq!(asked.json()["status"], "queued", "{}", asked.body);
    // In another chat: a message in this one would go into the turn under way.
    let other = service.new_chat();
    let path = format!("/api/chats/{other}/messages");
    let text = json!({ "text": "Say hi again", "agent": agent });
    let sent = service.send("POST", &path, text);
    assert_eq!(sent.json()["status"], "queued", "{}", sent.body);
    std::fs::write(data.0.join("go"), "").unwrap();

    service.wait_for(&other, 20, "the other chat's turn", |h| {
        h["turns"][0]["status"] == "done"
    });
    let history = service.wait_for(&chat, 5, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    assert_eq!(gathr_lines(&history).len(), 1, "{history}");
    // Started in order: the turn, the compaction, the other chat's turn.
    assert_eq!([0, 1, 2].map(|n| slash(&data.0, n)), [false, true, false]);
}

#[test]
fn compact_is_refused_with_a_reference_until_the_agent_has_a_conversation() {
    let data = TempDir::new();
    let replay = common::recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &replay)]);
    let account = service.signed_in(&data.0);
    let made = json!({ "name": "Made-up", "account": account });
    let made = service.send("POST", "/api/agents", made);
    let agent = made.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();

    let refused = compact(&service, &chat, &agent);
    assert_eq!(refused.status, 409, "{}", refused.body);
    let why = "there's no conversation to compact yet";
    assert_eq!(refused.json()["why"], why, "{}", refused.body);
    let reference = refused.json()["ref"].as_str().unwrap().to_string();
    let trace = service.call("GET", &format!("/api/trace/{reference}")).body;
    assert!(trace.contains("gathr.context.compact"), "{trace}");
}

#[test]
fn a_compaction_the_engine_doesnt_make_fails_with_why() {
    let data = TempDir::new();
    let (turn, compaction) = claude_parts();
    let unmade: Vec<String> = compaction
        .into_iter()
        .filter(|r| !r.contains("compact_boundary") && !r.contains("compact_result"))
        .collect();
    let service = claude(&data.0, &turn, &unmade);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });

    assert_eq!(compact(&service, &chat, &agent).status, 200);
    let history = service.wait_for(&chat, 10, "the compaction", |h| {
        h["turns"][1]["status"] == "failed"
    });
    let why = history["turns"][1]["why"].as_str().unwrap_or_default();
    let said = "Couldn't compact Made-up's conversation: ";
    assert!(why.starts_with(said), "{history}");
    assert!(gathr_lines(&history).is_empty(), "{history}");
}

#[test]
fn compact_asks_codex_to_compact_its_thread_and_says_so() {
    let data = TempDir::new();
    // A real compaction asked for: `thread/compact/start`, which Codex runs as a turn of its own.
    let (service, account) = codex(&data.0, &common::recording("compact_codex_asked"));
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });

    assert_eq!(compact(&service, &chat, &agent).status, 200);
    let history = service.wait_for(&chat, 10, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    let line = "Compacted Made-up's conversation";
    assert_eq!(gathr_lines(&history), [line], "{history}");
    let sql = "SELECT count(*) AS n FROM engine_raw \
               WHERE direction = 'in' AND body LIKE '%thread/compact/start%'";
    let (ok, asked) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{asked}");
    assert!(asked.to_string().contains(r#""n":1"#), "{asked}");
}

#[test]
fn compact_waits_for_the_engines_background_commands_and_the_turn_they_end_in() {
    let data = TempDir::new();
    let background = std::fs::read_to_string(common::recording("background")).unwrap();
    let turn: Vec<String> = background.lines().map(String::from).collect();
    let service = claude(&data.0, &turn, &claude_parts().1);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the turn that starts them", |h| {
        h["turns"][0]["status"] == "done"
    });

    let asked = compact(&service, &chat, &agent);
    assert_eq!(asked.json()["status"], "queued", "{}", asked.body);
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 20, "every turn", |h| {
        let turns = h["turns"].as_array().unwrap();
        turns.len() == 3 && turns.iter().all(|t| t["status"] == "done")
    });
    assert_eq!(gathr_lines(&history).len(), 1, "{history}");
    assert_eq!([0, 1].map(|n| slash(&data.0, n)), [false, true]);
}

/// #168's review: a compaction whose engine refuses the login waits for a sign-in like any turn,
/// and stays a compaction: after it, slash commands on again, one compaction and one quiet line.
#[test]
fn a_compaction_refused_a_login_compacts_once_after_a_sign_in() {
    let data = TempDir::new();
    let (turn, compaction) = claude_parts();
    let refused = std::fs::read_to_string(common::recording("signed_out")).unwrap();
    let refused: Vec<String> = refused.lines().map(String::from).collect();
    let service = claude(&data.0, &turn, &refused);
    let account = service.signed_in(&data.0);
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the first turn", |h| {
        h["turns"][0]["status"] == "done"
    });

    assert_eq!(compact(&service, &chat, &agent).status, 200);
    service.wait_for(&chat, 10, "the compaction to wait for a sign-in", |h| {
        h["turns"][1]["status"] == "waiting_sign_in"
    });
    std::fs::write(data.0.join("compaction.jsonl"), compaction.join("\n")).unwrap();
    common::sign_in_account(&data.0, &account);

    let history = service.wait_for(&chat, 20, "the compaction", |h| {
        h["turns"][1]["status"] == "done"
    });
    let line = "Compacted Made-up's conversation: 23K to 3.6K";
    assert_eq!(gathr_lines(&history), [line], "{history}");
    assert_eq!([0, 1, 2].map(|n| slash(&data.0, n)), [false, true, true]);
}
