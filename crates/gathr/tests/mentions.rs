//! `@Name` in an agent's words wakes that member when its turn ends (0081), through a real service
//! on Claude Code's replay stand-in whose words are made up for each test: once, with the sender's
//! words, in the thread of the message that named it; into the member's running turn, or joined to
//! its queued one; counted as a hand-off, with the pause at the limit; naming nobody, itself or a
//! stranger wakes nobody; and a wake that fails is found from its reference.

mod common;

use std::path::{Path, PathBuf};

use common::replay::LetGo;
use common::{TempDir, TestService, gathr, nth, rows, team, until};
use serde_json::{Value, json};

/// Claude Code's stand-in saying `words` instead of "Hi!", `n` turns for each engine; with `hold`,
/// each turn holds after its message arrives until the test lets that engine go (`go.<pid>`).
fn saying(dir: &Path, words: &str, n: usize, hold: bool) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let frames = frames.replace("Hi!", words);
    let (first, rest) = frames.split_once('\n').unwrap();
    let take = r#"{"dir":"hold","line":"take"}"#;
    let turn = match hold {
        true => format!("{first}\n{take}\n{}\n", rest.trim_end()),
        false => format!("{first}\n{}\n", rest.trim_end()),
    };
    let file = dir.join("saying.jsonl");
    std::fs::write(&file, turn.repeat(n)).unwrap();
    file
}

struct Room {
    service: TestService,
    data: PathBuf,
    chat: String,
    lead: String,
    ann: String,
}

fn room(data: &Path, words: &str, n: usize, hold: bool) -> Room {
    let (service, chat, lead) = team(data, &saying(data, words, n, hold), &[]);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history["members"].as_array().unwrap();
    let ann = members.iter().find(|m| m["name"] == "Ann").unwrap()["agent"]
        .as_str()
        .unwrap()
        .to_string();
    Room {
        service,
        data: data.to_path_buf(),
        chat,
        lead,
        ann,
    }
}

impl Room {
    fn history(&self) -> Value {
        self.service
            .call("GET", &format!("/api/chats/{}", self.chat))
            .json()
    }

    /// The person's message to `agent` in `chat`: the turn it opened.
    fn say_in(&self, chat: &str, agent: &str, words: &str) -> String {
        let path = format!("/api/chats/{chat}/messages");
        let body = json!({ "text": words, "agent": agent });
        let sent = self.service.send("POST", &path, body);
        assert_eq!(sent.status, 200, "{}", sent.body);
        sent.json()["turn"].as_str().unwrap().to_string()
    }

    fn turns_of(&self, agent: &str) -> Vec<Value> {
        let history = self.history();
        let turns = history["turns"].as_array().unwrap().iter();
        turns.filter(|t| t["agent"] == agent).cloned().collect()
    }

    /// Every turn of the chat has ended.
    fn quiet(&self) -> Value {
        self.service
            .wait_for(&self.chat, 30, "every turn to end", |h| {
                let turns = h["turns"].as_array().cloned().unwrap_or_default();
                !turns.is_empty()
                    && turns
                        .iter()
                        .all(|t| !matches!(t["status"].as_str(), Some("queued" | "running")))
            })
    }

    /// The attributes of each `gathr.message.mention`, in order.
    fn mentions(&self) -> Vec<Value> {
        let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.message.mention' ORDER BY time";
        let found = rows(&self.data, sql).into_iter();
        found.map(|r| r["attrs"].clone()).collect()
    }

    /// The `mentions` once there are `n`.
    fn mentions_once(&self, n: usize) -> Vec<Value> {
        until(30, &format!("{n} wakes by a mention"), || {
            let found = self.mentions();
            (found.len() >= n).then_some(found)
        })
    }

    /// Lets the `n`th engine started go on, and no other.
    fn go_to(&self, n: usize) {
        let started = nth(&self.data.join("env.jsonl"), n);
        let pid = started["pid"].as_u64().unwrap();
        let file = self.data.join("go").with_extension(pid.to_string());
        std::fs::write(file, "").unwrap();
    }

    /// The lead's text message in `turn`.
    fn words_in(&self, turn: &str) -> Value {
        let history = self.history();
        let mut messages = history["messages"].as_array().unwrap().iter();
        let found = messages
            .rfind(|m| m["turn"] == turn && m["role"] == "assistant" && m["kind"] == "text");
        found
            .unwrap_or_else(|| panic!("no words in {turn}: {history}"))
            .clone()
    }

    /// Gathr's messages in `turn`.
    fn told_in(&self, turn: &Value) -> Vec<Value> {
        let history = self.history();
        let messages = history["messages"].as_array().unwrap().iter();
        let told = messages.filter(|m| m["turn"] == *turn && m["role"] == "gathr");
        told.cloned().collect()
    }
}

const WORDS: &str = "@Ann please check the map.";

#[test]
fn a_mention_wakes_the_named_member_once_with_the_senders_words_in_its_thread() {
    let data = TempDir::new();
    let room = room(&data.0, WORDS, 2, false);
    let asked = room.say_in(&room.chat.clone(), &room.lead, "Plan the walk");
    room.mentions_once(1);
    let history = room.quiet();

    let anns = room.turns_of(&room.ann);
    assert_eq!(anns.len(), 1, "{history}");
    assert_eq!(anns[0]["answers"], asked.as_str(), "{history}");
    assert_eq!(
        common::opened(&data.0, &anns[0]["id"]),
        (json!("teammate"), Value::Null)
    );
    let named = room.words_in(&asked);
    let told = room.told_in(&anns[0]["id"]);
    assert_eq!(told.len(), 1, "{told:?}");
    assert_eq!(
        (&told[0]["body"], &told[0]["copy_of"]),
        (&json!(format!("Lead: {WORDS}")), &named["id"]),
        "{told:?}"
    );
    // Ann's own words name only Ann: nobody is woken by them, the lead least of all.
    assert_eq!(room.turns_of(&room.lead).len(), 1, "{history}");
    let woken = room.mentions();
    assert_eq!(woken.len(), 1, "{woken:?}");
    assert_eq!(
        (
            &woken[0]["from"],
            &woken[0]["to"],
            &woken[0]["delivered"],
            &woken[0]["message"]
        ),
        (
            &json!(room.lead),
            &json!(room.ann),
            &json!("queued"),
            &named["id"]
        ),
        "{woken:?}"
    );
    assert_eq!(woken[0]["depth"], 1, "{woken:?}");
    // Ann reads the words once: as what opened her turn, never again as a line of the chat.
    let read = common::replay::read_in(&data.0, anns[0]["id"].as_str().unwrap());
    assert_eq!(read.matches("please check the map").count(), 1, "{read}");
    // In the thread of the message that named her: where the lead's turn sits.
    let lead_turn = &room.turns_of(&room.lead)[0];
    assert_eq!(anns[0]["topic"], lead_turn["topic"], "{history}");
    assert_eq!(anns[0]["topic"], json!(format!("b:{asked}")));
    common::topics_agree(&data.0, &room.chat);
}

#[test]
fn a_member_mid_turn_takes_the_mention_in_that_turn() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let room = room(&data.0, WORDS, 2, true);
    let chat = room.chat.clone();
    let running = room.say_in(&chat, &room.ann, "Start");
    nth(&data.0.join("env.jsonl"), 0);
    room.say_in(&chat, &room.lead, "Plan the walk");
    room.go_to(1);

    let woken = room.mentions_once(1);
    assert_eq!(woken[0]["delivered"], "in_turn", "{woken:?}");
    let anns = room.turns_of(&room.ann);
    assert_eq!(
        anns.len(),
        1,
        "a running member got a second turn: {anns:?}"
    );
    let told = room.told_in(&json!(running));
    let carried: Vec<&Value> = told.iter().filter(|m| m["copy_of"].is_string()).collect();
    assert_eq!(carried.len(), 1, "{told:?}");
    assert_eq!(carried[0]["body"], format!("Lead: {WORDS}"));
}

/// Never two turns for one thing: a member already queued in the chat takes the mention in that turn.
#[test]
fn a_mention_joins_the_members_queued_turn() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let room = room(&data.0, WORDS, 2, true);
    let one = json!({ "members": [{ "agent": room.ann, "role": "supervisor" }] });
    let elsewhere = room.service.send("POST", "/api/chats", one);
    let elsewhere = elsewhere.json()["id"].as_str().unwrap().to_string();
    room.say_in(&elsewhere, &room.ann, "Busy here");
    nth(&data.0.join("env.jsonl"), 0);
    let chat = room.chat.clone();
    let queued = room.say_in(&chat, &room.ann, "Your turn next");
    room.say_in(&chat, &room.lead, "Plan the walk");
    room.go_to(1);

    let woken = room.mentions_once(1);
    assert_eq!(woken[0]["delivered"], "joined", "{woken:?}");
    let anns = room.turns_of(&room.ann);
    assert_eq!(anns.len(), 1, "the mention made a second turn: {anns:?}");
    assert_eq!(anns[0]["id"], queued.as_str());
    let told = room.told_in(&json!(queued));
    assert!(
        told.iter().any(|m| m["body"] == format!("Lead: {WORDS}")),
        "{told:?}"
    );
}

#[test]
fn a_mention_counts_as_a_hand_off_and_past_the_limit_pauses_the_chat_and_wakes_nobody() {
    let data = TempDir::new();
    let room = room(&data.0, "@Ann and @Lead, over to you.", 3, false);
    let set = room
        .service
        .send("PATCH", "/api/settings", json!({ "handoff_limit": 1 }));
    assert_eq!(set.status, 200, "{}", set.body);
    let chat = room.chat.clone();
    room.say_in(&chat, &room.lead, "Plan the walk");
    let woken = room.mentions_once(2);
    let history = room.quiet();

    let delivered: Vec<&Value> = woken.iter().map(|w| &w["delivered"]).collect();
    assert_eq!(delivered, [&json!("queued"), &json!("paused")], "{woken:?}");
    assert_eq!(
        (&woken[1]["from"], &woken[1]["to"], &woken[1]["depth"]),
        (&json!(room.ann), &json!(room.lead), &json!(2)),
        "{woken:?}"
    );
    assert_eq!(
        room.turns_of(&room.lead).len(),
        1,
        "the lead was woken: {history}"
    );
    let pauses = room.service.call("GET", "/api/pauses").json();
    let paused = pauses["pauses"].as_array().unwrap().iter();
    assert!(
        paused
            .into_iter()
            .any(|p| p["scope"] == "chat" && p["target"] == room.chat.as_str()),
        "{pauses}"
    );
    let line = "Paused after 1 hand-offs between agents. Resume to carry on.";
    let messages = history["messages"].as_array().unwrap();
    assert!(messages.iter().any(|m| m["body"] == line), "{history}");
    let limit = rows(
        &data.0,
        "SELECT ref FROM event_log WHERE name = 'gathr.handoff.limit'",
    );
    let reference = limit[0]["ref"].as_str().unwrap_or_default();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    let found = trace.to_string();
    assert!(
        ok && found.contains("gathr.handoff.limit") && found.contains("gathr.message.mention"),
        "{trace}"
    );
}

/// A forced failure: the hand-off can't be counted, so nobody is woken, and the reference the
/// failure carries finds it.
#[test]
fn a_wake_that_fails_wakes_nobody_and_is_found_from_its_reference() {
    let data = TempDir::new();
    let _go = LetGo(data.0.join("go"));
    let room = room(&data.0, WORDS, 2, true);
    let chat = room.chat.clone();
    room.say_in(&chat, &room.lead, "Plan the walk");
    nth(&data.0.join("env.jsonl"), 0);
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    conn.execute("ALTER TABLE handoffs RENAME TO handoffs_gone", [])
        .unwrap();
    room.go_to(0);

    let failed = until(30, "the failed wake", || {
        let sql =
            "SELECT ref, error_type, attrs FROM event_log WHERE name = 'gathr.message.mention'";
        rows(&data.0, sql).into_iter().next()
    });
    assert_eq!(failed["error_type"], "sqlite", "{failed}");
    let why = failed["attrs"]["why"].as_str().unwrap_or_default();
    assert!(why.contains("handoffs"), "{failed}");
    let (ok, trace) = gathr(&data.0, &["diag", "trace", failed["ref"].as_str().unwrap()]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let names: Vec<&str> = events.iter().filter_map(|e| e["name"].as_str()).collect();
    assert!(
        ok && names.contains(&"gathr.message.mention") && names.contains(&"invoke_agent claude"),
        "the failure and the turn that named Ann: {trace}"
    );
    assert!(room.turns_of(&room.ann).is_empty());
}

/// Naming nobody, itself or a stranger wakes nobody; a name counts only as a whole word.
#[test]
fn naming_nobody_itself_or_a_stranger_names_nobody() {
    let data = TempDir::new();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1);
         INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('acc', 'claude', 'A', 'a', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts)
             VALUES ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1);
         INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES
             ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 2);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, position)
             VALUES ('t', 'c', 'trace', 'done', 1, 'lead', 1);
         INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
             ('m1', 'c', 't', 2, 'assistant', 'text', '@Lead, @Nobody and @Annette: mail bob@ann.com'),
             ('m2', 'c', 't', 3, 'assistant', 'tool', '{\"said\": \"@Ann\"}');",
    )
    .unwrap();
    let mentions = gathr::handoffs::mentions(&conn, "t", "trace").unwrap();
    assert!(mentions.named.is_empty(), "{:?}", mentions.named);
    let handed: i64 = conn
        .query_row("SELECT count(*) FROM handoffs", [], |r| r.get(0))
        .unwrap();
    assert_eq!(handed, 0);
}
