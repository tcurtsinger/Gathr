//! Agents message each other (plan agent-messages.md, owner's room task 36): `message` shows in the
//! team chat as the sender's, wakes the member it names at most once, and counts toward the
//! hand-off limit, past which the chat pauses.

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, gathr, held_recording, nth, team, text, until};
use serde_json::{Value, json};

struct Room {
    service: TestService,
    chat: String,
    lead: String,
    ann: String,
    data: std::path::PathBuf,
}

/// Lead (Supervisor) and Ann in a chat, each engine held once its message arrives.
fn room(data: &Path) -> Room {
    let (service, chat, lead) = team(data, &held_recording(data), &[]);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let ann = history["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Ann")
        .unwrap()["agent"]
        .as_str()
        .unwrap()
        .to_string();
    Room {
        service,
        chat,
        lead,
        ann,
        data: data.to_path_buf(),
    }
}

impl Drop for Room {
    fn drop(&mut self) {
        let _ = std::fs::write(self.data.join("go"), "");
    }
}

impl Room {
    /// Sends `agent` the person's message; the token of the `n`th engine started, once it runs.
    fn turn_of(&self, agent: &str, n: usize) -> String {
        let path = format!("/api/chats/{}/messages", self.chat);
        let body = json!({ "text": "Start", "agent": agent });
        assert_eq!(self.service.send("POST", &path, body).status, 200);
        let started = nth(&self.data.join("env.jsonl"), n);
        started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
    }

    fn message(&self, token: &str, to: &str, words: &str) -> Value {
        let ask = json!({ "to": to, "text": words });
        call_tool(self.service.info.port, token, "message", ask)
    }

    fn history(&self) -> Value {
        self.service
            .call("GET", &format!("/api/chats/{}", self.chat))
            .json()
    }

    /// The agent's turns in the chat.
    fn turns_of(&self, agent: &str) -> Vec<Value> {
        let history = self.history();
        let turns = history["turns"].as_array().unwrap().iter();
        turns.filter(|t| t["agent"] == agent).cloned().collect()
    }

    /// What `gathr.message.send` said each message's delivery was, in order.
    fn delivered(&self) -> Vec<String> {
        let query = "SELECT attrs FROM event_log WHERE name = 'gathr.message.send' ORDER BY time";
        let (ok, rows) = gathr(&self.data, &["diag", "query", query]);
        assert!(ok, "{rows}");
        let rows = rows["rows"].as_array().unwrap().iter();
        rows.map(|r| {
            r["attrs"]["delivered"]
                .as_str()
                .unwrap_or_default()
                .to_string()
        })
        .collect()
    }

    /// The bodies of the messages saved on a turn.
    fn said_in(&self, turn: &Value) -> Vec<String> {
        let history = self.history();
        let messages = history["messages"].as_array().unwrap().iter();
        let on = messages.filter(|m| m["turn"] == turn["id"]);
        on.map(|m| m["body"].as_str().unwrap_or_default().to_string())
            .collect()
    }
}

#[test]
fn a_message_to_an_idle_member_shows_and_wakes_it_once_answering_the_senders_turn() {
    let data = TempDir::new();
    let room = room(&data.0);
    let lead = room.turn_of(&room.lead, 0);
    let sent = room.message(&lead, "ann", "Please look at #12");
    assert_eq!(sent["isError"], false, "{sent}");

    let history = room.history();
    let shown = history["messages"].as_array().unwrap().iter();
    let shown: Vec<&Value> = shown.filter(|m| m["to"] == room.ann.as_str()).collect();
    assert_eq!(shown.len(), 1, "{history}");
    assert_eq!(shown[0]["body"], "Please look at #12");
    assert_eq!(
        shown[0]["agent"],
        room.lead.as_str(),
        "shown as the sender's: {history}"
    );
    let lead_turn = room.turns_of(&room.lead)[0]["id"].clone();
    assert_eq!(shown[0]["turn"], lead_turn, "on the sender's turn");

    let anns = room.turns_of(&room.ann);
    assert_eq!(anns.len(), 1, "{anns:?}");
    assert_eq!(anns[0]["answers"], lead_turn, "{anns:?}");
    assert_eq!(anns[0]["task"], Value::Null, "{anns:?}");
    let asked = room.said_in(&anns[0]).join("\n");
    assert!(
        asked.contains("Please look at #12") && asked.contains("Lead"),
        "{asked}"
    );
    assert_eq!(room.delivered(), ["queued"]);
}

#[test]
fn messages_before_the_members_turn_starts_join_that_one_turn() {
    let data = TempDir::new();
    let room = room(&data.0);
    let lead = room.turn_of(&room.lead, 0);
    // A paused chat holds Ann's turn, so it hasn't started when the second message comes.
    let scope = json!({ "scope": "chat", "target": room.chat });
    assert_eq!(
        room.service
            .send("POST", "/api/pauses", scope.clone())
            .status,
        200
    );
    room.message(&lead, "Ann", "First, the summary");
    room.message(&lead, "Ann", "Then the tests");

    let anns = room.turns_of(&room.ann);
    assert_eq!(anns.len(), 1, "two messages made two turns: {anns:?}");
    let asked = room.said_in(&anns[0]).join("\n");
    assert!(
        asked.contains("First, the summary") && asked.contains("Then the tests"),
        "{asked}"
    );
    assert_eq!(room.delivered(), ["held", "joined"]);
    assert_eq!(
        room.service
            .send("POST", "/api/pauses/resume", scope)
            .status,
        200
    );
    room.service
        .wait_for(&room.chat, 20, "Ann's one turn to run", |h| {
            let turns = h["turns"].as_array().unwrap().iter();
            turns
                .filter(|t| t["agent"] == room.ann.as_str())
                .all(|t| t["status"] == "running")
        });
    assert_eq!(room.turns_of(&room.ann).len(), 1);
}

#[test]
fn a_member_mid_turn_reads_the_message_in_that_turn() {
    let data = TempDir::new();
    let room = room(&data.0);
    room.turn_of(&room.ann, 0);
    let lead = room.turn_of(&room.lead, 1);
    let sent = room.message(&lead, "Ann", "Heads up: main moved");
    assert_eq!(sent["isError"], false, "{sent}");

    let anns = room.turns_of(&room.ann);
    assert_eq!(
        anns.len(),
        1,
        "a running member got a second turn: {anns:?}"
    );
    let asked = room.said_in(&anns[0]).join("\n");
    assert!(asked.contains("Heads up: main moved"), "{asked}");
    assert_eq!(room.delivered(), ["in_turn"]);
}

#[test]
fn a_message_to_itself_a_stranger_or_in_a_chat_of_one_is_refused_with_a_reference() {
    let data = TempDir::new();
    let room = room(&data.0);
    let lead = room.turn_of(&room.lead, 0);
    for (to, why) in [("Lead", "itself"), ("Nobody", "isn't in this chat")] {
        let refused = room.message(&lead, to, "Hello");
        let said = text(&refused);
        assert_eq!(refused["isError"], true, "{to}: {said}");
        assert!(said.contains(why), "{to}: {said}");
        let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
        let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
        assert!(ok && trace.to_string().contains("message"), "{to}: {trace}");
    }

    let alone = TempDir::new();
    let (service, chat, solo) = team(&alone.0, &held_recording(&alone.0), &[]);
    let one = json!({ "members": [{ "agent": solo, "role": "supervisor" }] });
    assert_eq!(
        service
            .send("PATCH", &format!("/api/chats/{chat}"), one)
            .status,
        200
    );
    let body = json!({ "text": "Start", "agent": solo });
    service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    let token = nth(&alone.0.join("env.jsonl"), 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let ask = json!({ "to": "Ann", "text": "Hello" });
    let refused = call_tool(service.info.port, &token, "message", ask);
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(text(&refused).contains("team chat"), "{}", text(&refused));
    std::fs::write(alone.0.join("go"), "").unwrap();
}

#[test]
fn a_chain_past_the_limit_pauses_the_chat_wakes_nobody_and_the_persons_message_starts_it_over() {
    let data = TempDir::new();
    let room = room(&data.0);
    let set = room
        .service
        .send("PATCH", "/api/settings", json!({ "handoff_limit": 2 }));
    assert_eq!(set.status, 200, "{}", set.body);
    assert_eq!(set.json()["handoff_limit"], 2, "{}", set.body);
    let lead = room.turn_of(&room.lead, 0);
    room.message(&lead, "Ann", "One");
    let ann = nth(&room.data.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    room.message(&ann, "Lead", "Two");
    let third = room.message(&lead, "Ann", "Three");
    assert_eq!(third["isError"], false, "{third}");
    assert!(
        text(&third).contains("Paused after 2 hand-offs"),
        "{}",
        text(&third)
    );

    let pauses = room.service.call("GET", "/api/pauses").json();
    let paused = pauses["pauses"].as_array().unwrap().iter();
    assert!(
        paused
            .into_iter()
            .any(|p| p["scope"] == "chat" && p["target"] == room.chat.as_str()),
        "{pauses}"
    );
    assert_eq!(room.turns_of(&room.ann).len(), 1, "the third woke Ann");
    assert_eq!(room.delivered(), ["queued", "in_turn", "paused"]);
    let query = "SELECT ref FROM event_log WHERE name = 'gathr.handoff.limit'";
    let (_, rows) = gathr(&data.0, &["diag", "query", query]);
    let reference = rows["rows"][0]["ref"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    assert!(
        ok && trace.to_string().contains("gathr.handoff.limit"),
        "{trace}"
    );

    let scope = json!({ "scope": "chat", "target": room.chat });
    assert_eq!(
        room.service
            .send("POST", "/api/pauses/resume", scope)
            .status,
        200
    );
    let path = format!("/api/chats/{}/messages", room.chat);
    let body = json!({ "text": "Carry on", "agent": room.lead });
    assert_eq!(room.service.send("POST", &path, body).status, 200);
    until(5, "the person's message saved", || {
        let history = room.history();
        let all = history["messages"].as_array()?.clone();
        all.iter().any(|m| m["body"] == "Carry on").then_some(())
    });
    let again = room.message(&lead, "Ann", "Four");
    assert!(
        !text(&again).contains("Paused"),
        "the person's message didn't start it over"
    );
    assert_eq!(room.delivered().last().map(String::as_str), Some("in_turn"));
}

#[test]
fn the_hand_off_limit_is_a_setting_with_none_for_no_limit() {
    let data = TempDir::new();
    let service = TestService::start(&data.0);
    assert_eq!(
        service.call("GET", "/api/settings").json()["handoff_limit"],
        50
    );
    let none = service.send("PATCH", "/api/settings", json!({ "handoff_limit": null }));
    assert_eq!(none.json()["handoff_limit"], Value::Null, "{}", none.body);
    let zero = service.send("PATCH", "/api/settings", json!({ "handoff_limit": 0 }));
    assert_ne!(zero.status, 200, "{}", zero.body);
    assert!(zero.json()["ref"].is_string(), "{}", zero.body);
    assert_eq!(
        service.call("GET", "/api/settings").json()["handoff_limit"],
        Value::Null
    );
}

/// A message joined a queued turn before it started, so a stop before it starts still brings both
/// asks: the service takes queued turns over with their whole ask.
#[test]
fn a_queued_turn_a_message_joined_keeps_its_whole_ask_across_a_stop() {
    let data = TempDir::new();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'C', 1)",
        [],
    )
    .unwrap();
    gathr::store::add_turn(&conn, "c", "t", "trace", (Some("ann"), None, None)).unwrap();
    let say = |text: &str| gathr::store::add_message(&conn, "c", "t", "gathr", "text", text);
    say("Lead wrote to you: First").unwrap();
    say("Lead wrote to you: Then").unwrap();
    drop(conn);
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    let waiting = gathr::store::take_over(&conn).unwrap();
    assert_eq!(
        waiting[0].text,
        "Lead wrote to you: First\n\nLead wrote to you: Then"
    );
}
