//! Agents message each other (plan agent-messages.md, owner's room task 36): `message` shows in the
//! team chat as the sender's, wakes the member it names at most once, and counts toward the
//! hand-off limit, past which the chat pauses.

mod common;

use std::path::Path;

use common::{TempDir, TestService, call_tool, gathr, held_recording, nth, team, text, until};
use serde_json::{Value, json};

struct Room {
    _release: Release,
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
        _release: Release(data.join("go")),
        service,
        chat,
        lead,
        ann,
        data: data.to_path_buf(),
    }
}

/// Lets the held engines go however a test ends.
struct Release(std::path::PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
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

    /// What Gathr told the member in its turn points at the sender's message, so the chat shows it once.
    fn told_once(&self, turn: &Value, sent: &Value) {
        let history = self.history();
        let messages = history["messages"].as_array().unwrap().iter();
        let told: Vec<&Value> = messages
            .filter(|m| m["turn"] == turn["id"] && m["role"] == "gathr")
            .collect();
        let copies: Vec<&&Value> = told.iter().filter(|m| m["copy_of"] == *sent).collect();
        assert_eq!(copies.len(), 1, "no copy marked: {told:?}");
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
    room.told_once(&anns[0], &shown[0]["id"]);
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
    let history = room.history();
    let mut shown = history["messages"].as_array().unwrap().iter();
    let sent = shown.rfind(|m| m["to"] == room.ann.as_str()).unwrap();
    room.told_once(&anns[0], &sent["id"]);
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

/// Vale, #118: a turn the person queued for a member that hasn't started (the member is busy in
/// another chat) takes the message too: one turn, never a second, and its ask outlasts a stop.
#[test]
fn a_message_joins_the_persons_queued_turn_and_both_outlast_a_stop() {
    let data = TempDir::new();
    let room = room(&data.0);
    let elsewhere = room.service.send(
        "POST",
        "/api/chats",
        json!({ "members": [{ "agent": room.ann, "role": "supervisor" }] }),
    );
    let elsewhere = elsewhere.json()["id"].as_str().unwrap().to_string();
    let body = json!({ "text": "Busy here", "agent": room.ann });
    let path = format!("/api/chats/{elsewhere}/messages");
    assert_eq!(room.service.send("POST", &path, body).status, 200);
    nth(&room.data.join("env.jsonl"), 0);
    let lead = room.turn_of(&room.lead, 1);
    let path = format!("/api/chats/{}/messages", room.chat);
    let queued = room.service.send(
        "POST",
        &path,
        json!({ "text": "Your turn next", "agent": room.ann }),
    );
    assert_eq!(queued.json()["status"], "queued", "{}", queued.body);

    room.message(&lead, "Ann", "And look at #12");
    let anns = room.turns_of(&room.ann);
    assert_eq!(anns.len(), 1, "the message made a second turn: {anns:?}");
    let asked = room.said_in(&anns[0]).join("\n");
    assert!(
        asked.contains("Your turn next") && asked.contains("And look at #12"),
        "{asked}"
    );
    assert_eq!(room.delivered(), ["joined"]);

    let Room { service, .. } = room;
    service.stop();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    let waiting = gathr::store::take_over(&conn).unwrap();
    let ann = waiting
        .iter()
        .find(|w| w.turn == anns[0]["id"].as_str().unwrap())
        .unwrap();
    assert!(
        ann.text.contains("Your turn next") && ann.text.contains("And look at #12"),
        "{}",
        ann.text
    );
}

/// Vale, #118: the line a chain past the limit adds reaches every open window, as history has it.
#[test]
fn the_pause_line_a_chain_adds_reaches_every_open_window() {
    let data = TempDir::new();
    let room = room(&data.0);
    room.service
        .send("PATCH", "/api/settings", json!({ "handoff_limit": 1 }));
    let lead = room.turn_of(&room.lead, 0);
    room.message(&lead, "Ann", "One");
    let ann = nth(&room.data.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let mut live = room.service.live();
    room.message(&ann, "Lead", "Two");
    let line = "Paused after 1 hand-offs between agents. Resume to carry on.";
    let heard = until(10, "the pause line, live", || {
        let event = common::next_live(&mut live);
        (event["type"] == "message" && event["message"]["body"] == line).then_some(event)
    });
    let history = room.history();
    let saved = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["body"] == line);
    assert_eq!(saved, Some(&heard["message"]), "live and history differ");
    // A window keeps only the live events that name its chat (G161).
    assert_eq!(heard["chat"], room.chat.as_str(), "{heard}");
}

/// G161: the owner's window showed an agent's message only after a reload. A window keeps only the
/// live events that name its chat, and the message went out without one.
#[test]
fn a_message_goes_out_live_naming_its_chat_and_whom_its_for() {
    let data = TempDir::new();
    let room = room(&data.0);
    let lead = room.turn_of(&room.lead, 0);
    let mut live = room.service.live();
    room.message(&lead, "Ann", "Heads up: main moved");
    let heard = until(10, "the message, live", || {
        let event = common::next_live(&mut live);
        (event["type"] == "message" && event["message"]["body"] == "Heads up: main moved")
            .then_some(event)
    });
    assert_eq!(heard["chat"], room.chat.as_str(), "{heard}");
    assert_eq!(heard["message"]["to"], room.ann.as_str(), "{heard}");
}

/// Scenario 6b through the service (0050): one answer says what the chat waits on, the same by the
/// route, the `waiting` tool and the command line. A turn queued behind its agent's running turn
/// says so, which only that agent's queue knows; when the turn ahead ends, a live message names the
/// chat and the line is gone. With the service stopped, the command line answers from the database.
#[test]
fn what_a_chat_waits_on_is_one_answer_by_the_route_the_tool_and_the_command_line() {
    let data = TempDir::new();
    let room = room(&data.0);
    let lead = room.turn_of(&room.lead, 0);
    room.turn_of(&room.ann, 1);
    // Ann's task from Lead waits behind the turn Ann is running.
    let task = json!({ "to": "Ann", "what": "Fix the build" });
    let given = call_tool(room.service.info.port, &lead, "give_task", task);
    assert_eq!(given["isError"], false, "{}", text(&given));
    let path = format!("/api/chats/{}/waiting", room.chat);
    let behind = "is queued: behind this agent's turn";
    let says = |answer: &Value| {
        let lines = answer["lines"].as_array().cloned().unwrap_or_default();
        let said = |l: &Value| l["what"].as_str().is_some_and(|w| w.contains(behind));
        lines.iter().any(said)
    };
    let asked = until(20, "the queue to say why Ann's task waits", || {
        let answer = room.service.call("GET", &path).json();
        says(&answer).then_some(answer)
    });
    let line = |kind: &str| {
        let lines = asked["lines"].as_array().unwrap();
        let of = lines.iter().filter(|l| l["kind"] == kind);
        of.cloned().collect::<Vec<_>>()
    };
    let queued = &line("queued")[0];
    assert_eq!(queued["moves"], "itself", "{queued}");
    assert_eq!(queued["from"], "service", "{queued}");
    assert_eq!(queued["task"], 1, "{queued}");
    assert_eq!(line("turn").len(), 2, "both running turns: {asked}");
    let states: Vec<&str> = asked["agents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["state"].as_str())
        .collect();
    assert_eq!(states, ["working", "working"], "{asked}");

    let told = text(&call_tool(
        room.service.info.port,
        &lead,
        "waiting",
        json!({}),
    ));
    assert!(
        told.contains(behind) && told.contains("Moving by itself:"),
        "{told}"
    );
    let words = asked["in_words"].as_str().unwrap_or_default();
    assert!(words.contains(behind), "{asked}");
    let (ok, cli) = gathr(&room.data, &["diag", "waiting", "--chat", &room.chat]);
    assert!(ok, "{cli}");
    assert_eq!(cli["from"], "service", "{cli}");
    assert!(says(&cli), "{cli}");

    // The turn ahead ends: the window is told to ask again, and the line is gone.
    let mut live = room.service.live();
    std::fs::write(room.data.join("go"), "").unwrap();
    loop {
        let update = common::next_live(&mut live);
        if update["type"] == "waiting" && update["chat"] == room.chat.as_str() {
            break;
        }
    }
    until(20, "the queued turn's line to go", || {
        let answer = room.service.call("GET", &path).json();
        (!says(&answer)).then_some(())
    });

    let (chat, folder) = (room.chat.clone(), room.data.clone());
    let Room { service, .. } = room;
    service.stop();
    let (ok, cli) = gathr(&folder, &["diag", "waiting", "--chat", &chat]);
    assert!(ok, "{cli}");
    assert_eq!(cli["from"], "database", "{cli}");
    let (ok, unknown) = gathr(&folder, &["diag", "waiting", "--chat", "nope"]);
    let why = unknown["why"].as_str().unwrap_or_default();
    assert!(!ok && why.contains("no chat"), "{unknown}");
}

/// G163: a message whose turn is held says what really holds it: the pause of the agent, of the
/// chat or of the whole app, never "the chat" for all three.
#[test]
fn a_message_to_a_held_member_names_the_pause_that_holds_it() {
    for (scope, says) in [
        ("agent", "when Ann is resumed"),
        ("chat", "when the chat is resumed"),
        ("app", "when Gathr is resumed"),
    ] {
        let data = TempDir::new();
        let room = room(&data.0);
        let lead = room.turn_of(&room.lead, 0);
        let target = match scope {
            "agent" => room.ann.as_str(),
            "chat" => room.chat.as_str(),
            _ => "",
        };
        let body = json!({ "scope": scope, "target": target });
        let paused = room.service.send("POST", "/api/pauses", body);
        assert_eq!(paused.status, 200, "{}", paused.body);
        let sent = text(&room.message(&lead, "Ann", "Held?"));
        assert!(sent.contains(says), "{scope}: {sent}");
    }
}
