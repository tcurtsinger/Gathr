//! A room chat's turns (0065), through a real service on Claude Code's replay stand-in: who gets a
//! turn, and that Gathr says nothing itself; a turn with nothing to add posts nothing; a like starts
//! no turn; the cap on hand-offs pauses the room; Stop now holds the person's message queued there;
//! a REPLY sits under the message it answers. Each
//! engine plays its turns from the start of the recording, so an agent's nth turn says the nth words
//! given.

mod common;

use std::path::{Path, PathBuf};

use common::replay::escaped;
use common::{TempDir, TestService, rows, sign_in_account};
use serde_json::{Value, json};

/// Claude Code's stand-in saying each of `turns` in turn instead of "Hi!".
fn saying(dir: &Path, turns: &[&str]) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let frames = frames.trim_end();
    let said: Vec<String> = turns
        .iter()
        .map(|words| frames.replace("Hi!", &escaped(&escaped(words))))
        .collect();
    let file = dir.join("saying.jsonl");
    std::fs::write(&file, said.join("\n") + "\n").unwrap();
    file
}

struct Room {
    service: TestService,
    data: PathBuf,
    chat: String,
    lead: String,
    ann: String,
    bo: String,
}

/// A new chat, a room chat, with Lead (Supervisor), Ann and Bo, on `replay`.
fn room(data: &Path, replay: &Path) -> Room {
    room_with(data, &[("GATHR_REPLAY", replay)])
}

fn room_with(data: &Path, env: &[(&str, &Path)]) -> Room {
    let service = TestService::with_engine(data, env);
    let account = service.add_account("Room");
    sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann, bo) = (agent("Lead"), agent("Ann"), agent("Bo"));
    let members = json!([{ "agent": lead, "role": "supervisor" },
        { "agent": ann, "role": "member" }, { "agent": bo, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    Room {
        service,
        data: data.to_path_buf(),
        chat,
        lead,
        ann,
        bo,
    }
}

impl Room {
    /// The person's words, to `agent` or with none to whoever the room sends them to.
    fn say(&self, words: &str, agent: Option<&str>) {
        let path = format!("/api/chats/{}/messages", self.chat);
        let sent = self
            .service
            .send("POST", &path, json!({ "text": words, "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
    }

    /// The chat once `n` turns have been made and every one has ended.
    fn quiet(&self, n: usize) -> Value {
        let what = format!("{n} turns, all ended");
        self.service.wait_for(&self.chat, 30, &what, |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns.len() == n
                && turns.iter().all(|t| {
                    !matches!(
                        t["status"].as_str(),
                        Some("queued" | "running" | "waiting_sign_in")
                    )
                })
        })
    }

    /// The opening of each of `agent`'s turns, in order, as `opened` records it.
    fn turns_of(&self, history: &Value, agent: &str) -> Vec<Value> {
        let turns = history["turns"].as_array().unwrap().iter();
        let theirs = turns.filter(|t| t["agent"] == agent);
        theirs
            .map(|t| common::opened(&self.data, &t["id"]).0)
            .collect()
    }

    fn count(&self, table: &str) -> i64 {
        let found = rows(&self.data, &format!("SELECT count(*) AS n FROM {table}"));
        found[0]["n"].as_i64().unwrap()
    }
}

/// An agent's words, as the room shows them: each with what it answers.
fn agents_words(history: &Value) -> Vec<(Value, Value, Value)> {
    let messages = history["messages"].as_array().unwrap().iter();
    let words = messages.filter(|m| m["role"] == "assistant" && m["kind"] == "text");
    words
        .map(|m| (m["agent"].clone(), m["body"].clone(), m["reply_to"].clone()))
        .collect()
}

#[test]
fn the_persons_words_go_to_the_lead_or_whom_they_name_and_an_agents_wake_only_whom_they_name() {
    let data = TempDir::new();
    let words = "@Bo the map is ready.";
    let room = room(&data.0, &saying(&data.0, &[words, words]));
    room.say("Plan the walk", None);
    room.quiet(2);
    room.say("@Ann look at it too", Some(&room.ann));
    let history = room.quiet(4);

    // The lead got the person's words; its words named Bo, whose own name only himself.
    assert_eq!(room.turns_of(&history, &room.lead), [json!("person")]);
    assert_eq!(room.turns_of(&history, &room.ann), [json!("person")]);
    let teammate = json!("teammate");
    assert_eq!(
        room.turns_of(&history, &room.bo),
        [teammate.clone(), teammate]
    );
    let gathr = history["messages"].as_array().unwrap().iter();
    let said = gathr.filter(|m| m["role"] == "gathr" && m["copy_of"].is_null());
    assert_eq!(said.count(), 0, "Gathr said something itself: {history}");
}

#[test]
fn a_turn_with_nothing_to_add_ends_done_and_posts_nothing() {
    let data = TempDir::new();
    let room = room(&data.0, &common::recording("thinking_only"));
    room.say("Anything to add?", None);
    let history = room.quiet(1);

    assert_eq!(history["turns"][0]["status"], "done", "{history}");
    assert!(agents_words(&history).is_empty(), "{history}");
    let messages = history["messages"].as_array().unwrap().iter();
    assert!(
        !messages.into_iter().any(|m| m["role"] == "gathr"),
        "{history}"
    );
}

#[test]
fn a_like_is_kept_and_starts_no_turn() {
    let data = TempDir::new();
    let room = room(&data.0, &saying(&data.0, &["The map is ready."]));
    room.say("Draw the map", Some(&room.ann));
    let history = room.quiet(1);
    let mut messages = history["messages"].as_array().unwrap().iter();
    let message = messages.find(|m| m["role"] == "assistant" && m["kind"] == "text");
    let message = message.unwrap()["id"].as_str().unwrap().to_string();

    let path = format!("/api/chats/{}/messages/{message}/like", room.chat);
    let given = room.service.send("PUT", &path, json!({}));
    assert_eq!(given.status, 200, "{}", given.body);
    assert_eq!(given.json()["did"], "marked", "{}", given.body);
    assert_eq!(room.count("reactions"), 1);
    let history = room.quiet(1);
    let mut messages = history["messages"].as_array().unwrap().iter();
    assert!(!messages.any(|m| m["role"] == "gathr"), "{history}");
}

#[test]
fn past_the_cap_on_hand_offs_the_room_pauses_and_wakes_nobody() {
    let data = TempDir::new();
    let words = "@Ann and @Lead, over to you.";
    let room = room(&data.0, &saying(&data.0, &[words, words]));
    let set = room
        .service
        .send("PATCH", "/api/settings", json!({ "handoff_limit": 1 }));
    assert_eq!(set.status, 200, "{}", set.body);
    room.say("Plan the walk", None);
    let history = room.quiet(2);

    // The lead's words woke Ann; Ann's, a second hand-off, went past the cap and woke nobody.
    for agent in [&room.lead, &room.ann] {
        assert_eq!(room.turns_of(&history, agent).len(), 1, "{history}");
    }
    let pauses = room.service.call("GET", "/api/pauses").json();
    let paused = pauses["pauses"].as_array().unwrap().iter();
    let mut paused = paused.into_iter();
    assert!(
        paused.any(|p| p["scope"] == "chat" && p["target"] == room.chat.as_str()),
        "{pauses}"
    );
}

#[test]
fn stop_now_holds_the_persons_message_queued_there_until_restart() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let held = std::fs::read_to_string(common::held_recording(&data.0, "signed_in")).unwrap();
    let replay = data.0.join("twice.jsonl");
    std::fs::write(&replay, held.repeat(2)).unwrap();
    let room = room_with(
        &data.0,
        &[("GATHR_REPLAY", &replay), ("GATHR_REPLAY_HOLD", &go)],
    );
    // Ann is busy in a chat of her own, so the person's message to her in the room queues.
    let members = json!([{ "agent": room.ann, "role": "supervisor" }]);
    let made = room
        .service
        .send("POST", "/api/chats", json!({ "members": members }));
    let other = made.json()["id"].as_str().unwrap().to_string();
    let to_ann = |chat: &str| {
        let path = format!("/api/chats/{chat}/messages");
        let sent = room
            .service
            .send("POST", &path, json!({ "text": "Pack", "agent": room.ann }));
        assert_eq!(sent.status, 200, "{}", sent.body);
    };
    let status = |h: &Value, at: usize| h["turns"][at]["status"].as_str().map(String::from);
    to_ann(&other);
    let busy = |h: &Value| status(h, 0).as_deref() == Some("running");
    room.service
        .wait_for(&other, 10, "Ann's turn running", busy);
    to_ann(&room.chat);
    let queued = |h: &Value| status(h, 0).as_deref() == Some("queued");
    room.service
        .wait_for(&room.chat, 10, "Ann's turn queued", queued);
    let scope = json!({ "scope": "chat", "target": room.chat });
    let stopped = room.service.send("POST", "/api/pauses/stop", scope.clone());
    assert_eq!(stopped.status, 200, "{}", stopped.body);

    // Ann's turns after the stop run in her own chat; the queued one in the room waits.
    std::fs::write(&go, "").unwrap();
    let first = |h: &Value| status(h, 0).as_deref() == Some("done");
    room.service.wait_for(&other, 10, "Ann's turn done", first);
    to_ann(&other);
    let both = |h: &Value| (0..2).all(|at| status(h, at).as_deref() == Some("done"));
    room.service.wait_for(&other, 10, "Ann's turns done", both);
    let history = room
        .service
        .call("GET", &format!("/api/chats/{}", room.chat));
    assert_eq!(status(&history.json(), 0).as_deref(), Some("queued"));

    let resumed = room.service.send("POST", "/api/pauses/resume", scope);
    assert_eq!(resumed.status, 200, "{}", resumed.body);
    room.quiet(1);
}

#[test]
fn a_reply_sits_under_the_message_that_woke_it_and_one_level_deep() {
    let data = TempDir::new();
    let first = "REPLY\n@Ann @Lead check the map.";
    let replay = saying(&data.0, &[first, "REPLY\nThe map holds."]);
    let room = room(&data.0, &replay);
    let mut live = room.service.live();
    room.say("Plan the walk", None);
    let mut deltas = Vec::new();
    while deltas.len() < 3 {
        let event = common::next_live(&mut live);
        if event["type"] == "delta" {
            deltas.push(event);
        }
    }
    let history = room.quiet(3);

    let said = agents_words(&history);
    let main = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "assistant" && m["kind"] == "text")
        .unwrap()["id"]
        .clone();
    // The person woke the lead: its REPLY is a main message. Ann's answers the lead's; the lead's
    // answer to Ann's reply sits under the same main message.
    let words = "@Ann @Lead check the map.";
    assert_eq!(
        said,
        [
            (json!(room.lead), json!(words), Value::Null),
            (json!(room.ann), json!(words), main.clone()),
            (json!(room.lead), json!("The map holds."), main.clone()),
        ],
        "{history}"
    );
    // Live, the words stream without REPLY and with what they answer from the first piece on.
    assert!(
        deltas
            .iter()
            .all(|d| !d["text"].as_str().unwrap().contains("REPLY")),
        "{deltas:?}"
    );
    let answered: Vec<&Value> = deltas.iter().map(|d| &d["reply_to"]).collect();
    assert_eq!(answered, [&Value::Null, &main, &main], "{deltas:?}");
}
