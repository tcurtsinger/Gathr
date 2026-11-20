//! Which conversation a turn runs in (0055, scenarios 8h to 8k), through a real service. Every
//! engine is the replay stand-in, started on the session Gathr gives it and naming that session in
//! its frames (`GATHR_REPLAY_SESSION=echo`), so each task's conversation has an id of its own.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, rows, sign_in_account, until};
use serde_json::{Value, json};

const REFUSED: &str = "it couldn't resume its conversation";

/// `n` turns of `signed_in` for each engine to play from its start; `change` edits turn `i`.
fn played(dir: &Path, n: usize, change: impl Fn(usize, &mut Vec<String>)) -> PathBuf {
    let one = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let mut all = Vec::new();
    for i in 0..n {
        let mut turn: Vec<String> = one.lines().map(String::from).collect();
        change(i, &mut turn);
        all.extend(turn);
    }
    let path = dir.join("played.jsonl");
    std::fs::write(&path, all.join("\n") + "\n").unwrap();
    path
}

fn plain(dir: &Path) -> PathBuf {
    played(dir, 12, |_, _| {})
}

struct Room {
    service: TestService,
    data: PathBuf,
    chat: String,
    lead: String,
    ann: String,
}

impl Room {
    /// The token of the lead's engine now: it starts again when the chat's roles change.
    fn lead_token(&self) -> String {
        token_of(self, &self.lead)
    }
}

/// A signed-in team chat on `engine` (Lead, Supervisor; Ann, member) whose lead has run a turn.
fn room(data: &Path, recording: &Path, engine: &str, more: &[(&str, &Path)]) -> Room {
    let answers = data.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let replay = common::replay_engine();
    let mut env: Vec<(&str, &Path)> = vec![
        ("GATHR_CLAUDE_PATH", &replay),
        ("GATHR_CODEX_PATH", &replay),
        ("GATHR_REPLAY", recording),
        ("GATHR_REPLAY_SESSION", Path::new("echo")),
        ("GATHR_GH_PATH", Path::new("")),
        ("GATHR_FAKE_GH", &answers),
    ];
    let (args, starts, gh) = (
        data.join("args.jsonl"),
        data.join("env.jsonl"),
        common::fake_gh(),
    );
    env[4].1 = &gh;
    env.extend([
        ("GATHR_REPLAY_ARGS", args.as_path()),
        ("GATHR_REPLAY_ENV", &starts),
    ]);
    env.extend_from_slice(more);
    let service = TestService::start_with(data, &env);
    let account = signed_in(&service, data, engine);
    let agent = |name: &str| new_agent(&service, name, &account);
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    common::old_chat(data, &chat);
    let room = Room {
        service,
        data: data.to_path_buf(),
        chat,
        lead,
        ann,
    };
    say(&room, &room.lead, "Plan it");
    common::fix_it(room.service.info.port, &room.lead_token());
    room
}

/// A new account on `engine`, signed in with a made-up login: its id.
fn signed_in(service: &TestService, data: &Path, engine: &str) -> String {
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": engine, "name": engine }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    let account = added.json()["id"].as_str().unwrap().to_string();
    match engine {
        "codex" => {
            let folder = data.join("accounts").join(&account).join("codex");
            std::fs::create_dir_all(&folder).unwrap();
            let login = r#"{"tokens":{"refresh_token":"made-up"}}"#;
            std::fs::write(folder.join("auth.json"), login).unwrap();
        }
        _ => sign_in_account(data, &account),
    }
    account
}

fn new_agent(service: &TestService, name: &str, account: &str) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// The person's message to `agent`, once every turn has ended.
fn say(room: &Room, agent: &str, said: &str) {
    let path = format!("/api/chats/{}/messages", room.chat);
    let sent = room
        .service
        .send("POST", &path, json!({ "text": said, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    quiet(room);
}

/// Waits until no turn runs or waits in the queue.
fn quiet(room: &Room) -> Value {
    room.service
        .wait_for(&room.chat, 30, "every turn to end", |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns
                .iter()
                .all(|t| !matches!(t["status"].as_str(), Some("running" | "queued")))
        })
}

/// Each engine `agent` started, oldest first: `gathr.engine.start`'s attributes.
fn starts(room: &Room, agent: &str) -> Vec<Value> {
    let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.engine.start' ORDER BY id";
    let all = rows(&room.data, sql)
        .into_iter()
        .map(|r| r["attrs"].clone());
    all.filter(|a| a["gen_ai.agent.id"] == agent && a["process.pid"].is_number())
        .collect()
}

/// The token of `agent`'s newest engine: the last start in its working folder.
fn token_of(room: &Room, agent: &str) -> String {
    let cwd = until(20, "the agent's engine", || {
        starts(room, agent).last().cloned()
    })["cwd"]
        .clone();
    until(20, "the engine's token", || {
        let text = std::fs::read_to_string(room.data.join("env.jsonl")).ok()?;
        let mut lines = text
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok());
        let last = lines.rfind(|l| l["cwd"] == cwd)?;
        last["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

fn events(room: &Room, name: &str) -> Vec<Value> {
    let sql = format!("SELECT ref, status, attrs FROM event_log WHERE name = '{name}' ORDER BY id");
    rows(&room.data, &sql)
}

/// Waits until `agent`'s newest engine has closed idle.
fn idle_closed(room: &Room, agent: &str) {
    let last = starts(room, agent).last().unwrap()["process.pid"].clone();
    until(20, "the agent's engine to close idle", || {
        let closes = events(room, "gathr.engine.close");
        closes
            .iter()
            .any(|e| e["attrs"]["process.pid"] == last && e["attrs"]["why"] == "it was idle")
            .then_some(())
    });
}

/// The engine's close reasons, oldest first.
fn closes(room: &Room) -> Vec<Value> {
    let all = events(room, "gathr.engine.close");
    all.iter().map(|e| e["attrs"]["why"].clone()).collect()
}

/// The conversation `agent`'s turns on no task resume (`engine_sessions`).
fn latest_of(room: &Room, agent: &str) -> Option<String> {
    let sql = format!(
        "SELECT session_id FROM engine_sessions WHERE chat_id = '{}' AND agent_key = '{agent}'",
        room.chat
    );
    let found = rows(&room.data, &sql);
    found
        .first()
        .and_then(|r| r["session_id"].as_str().map(String::from))
}

/// N8 on the latest conversation: the lead's turn, and Ann's on no task, resume what the engine
/// refuses. The refused engine closes, and the turn runs once more in a new conversation.
#[test]
fn a_refused_resume_of_the_latest_conversation_starts_new() {
    let data = TempDir::new();
    let more = [
        ("GATHR_REPLAY_RESUME", Path::new("missing")),
        ("GATHR_IDLE_CLOSE_MS", Path::new("3000")),
    ];
    let room = room(&data.0, &plain(&data.0), "claude", &more);
    say(&room, &room.ann, "Hello");
    for agent in [room.lead.clone(), room.ann.clone()] {
        idle_closed(&room, &agent);
        let refused = latest_of(&room, &agent).expect("a latest conversation");
        let before = events(&room, "gathr.engine.resume_failed").len();
        say(&room, &agent, "Anything else?");

        let history = room
            .service
            .call("GET", &format!("/api/chats/{}", room.chat))
            .json();
        let turns = history["turns"].as_array().cloned().unwrap_or_default();
        let turn = turns
            .iter()
            .rfind(|t| t["agent"] == agent.as_str())
            .unwrap();
        assert_eq!(turn["status"], "done", "{turn}");
        let failed = events(&room, "gathr.engine.resume_failed");
        let failed = &failed[before..];
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert_eq!(
            failed[0]["ref"], turn["ref"],
            "on the turn's trace: {failed:?}"
        );
        assert_eq!(
            failed[0]["attrs"]["session"],
            refused.as_str(),
            "{failed:?}"
        );
        let started = starts(&room, &agent);
        let shown: Vec<&Value> = started
            .iter()
            .rev()
            .take(2)
            .map(|s| &s["resumed"])
            .collect();
        assert_eq!(shown, [&json!(false), &json!(true)], "{started:?}");
        let now = latest_of(&room, &agent);
        assert!(now.is_some_and(|s| s != refused), "{refused}");
    }
    let refusals = closes(&room).iter().filter(|w| *w == REFUSED).count();
    assert_eq!(refusals, 2, "{:?}", closes(&room));
}
