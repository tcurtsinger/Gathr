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

/// A window the agent's model showed in another chat, as a turn there would have saved it.
fn seen_window(data: &std::path::Path, service: &TestService, agent: &str, window: i64) {
    let other = service.send("POST", "/api/chats", json!({})).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let db = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
    db.execute(
        "INSERT INTO agent_context (chat_id, agent_id, window, window_of, updated_ts) VALUES (?1, ?2, ?3, 'claude:', 1)",
        rusqlite::params![other, agent, window],
    )
    .unwrap();
}

fn agent(service: &TestService, account: &str) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Made-up", "account": account }),
    );
    made.json()["id"].as_str().unwrap().to_string()
}

/// K3: Claude Code is started with Auto-compact, never past 80% of the window its model showed; with
/// the engine's own, or no window seen on its model yet, with none (#168's review).
#[test]
fn claude_code_starts_with_auto_compact_capped_at_80_percent_of_its_window() {
    for (at, window, expected) in [
        (json!(300_000), None, Value::Null),
        (json!(300_000), Some(1_000_000), json!("300000")),
        (json!(500_000), Some(200_000), json!("160000")),
        (Value::Null, Some(200_000), Value::Null),
    ] {
        let data = TempDir::new();
        let env = data.0.join("env.jsonl");
        let service = TestService::with_engine(
            &data.0,
            &[
                ("GATHR_REPLAY", &common::recording("signed_in")),
                ("GATHR_REPLAY_ENV", &env),
            ],
        );
        let account = service.signed_in(&data.0);
        auto_compact(&service, at.clone());
        let ann = agent(&service, &account);
        if let Some(window) = window {
            seen_window(&data.0, &service, &ann, window);
        }
        let chat = service.new_chat();
        let sent = service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({ "text": "Hi", "agent": ann }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        let started = common::nth(&env, 0);
        assert_eq!(
            started["CLAUDE_CODE_AUTO_COMPACT_WINDOW"], expected,
            "{at} with {window:?}"
        );
    }
}

/// K3: Codex's thread opens with Auto-compact as `model_auto_compact_token_limit`, once its model's
/// window is known: capped at 80% of it. Before, the engine's own.
#[test]
fn codex_opens_its_thread_with_auto_compact() {
    let data = TempDir::new();
    let (service, account) = codex(&data.0, &common::recording("codex_signed_in"));
    auto_compact(&service, json!(400_000));
    let (chat, agent) = one_turn(&service, &account);
    service.wait_for(&chat, 10, "the turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    // Another chat: the engine starts again, now knowing the window (475K).
    let other = service.new_chat();
    let path = format!("/api/chats/{other}/messages");
    service.send("POST", &path, json!({ "text": "Hi", "agent": agent }));
    service.wait_for(&other, 10, "the second turn's end", |h| {
        h["turns"][0]["status"] == "done"
    });
    let sql = "SELECT body FROM engine_raw WHERE direction = 'in' AND body LIKE '%\"thread/start\"%' ORDER BY id";
    let (ok, rows) = common::gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    let opened = |n: usize| -> Value {
        serde_json::from_str(rows["rows"][n]["body"].as_str().unwrap()).unwrap()
    };
    let limit = |n: usize| opened(n)["params"]["config"]["model_auto_compact_token_limit"].clone();
    assert_eq!(
        [limit(0), limit(1)],
        [Value::Null, json!(380_000)],
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
    let ann = agent(&service, &account);
    let chat = service.new_chat();
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute(
        "INSERT INTO agent_context (chat_id, agent_id, used, window, window_of, updated_ts) VALUES (?1, ?2, 380000, 1000000, 'claude:', 1)",
        rusqlite::params![chat, ann],
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
