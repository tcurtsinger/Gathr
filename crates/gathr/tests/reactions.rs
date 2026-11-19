//! An agent's thumbs-up on the person's message (0057, the threads build's PR 7) through a real service
//! and Gathr's MCP tools, on Claude Code's replay stand-in: by the message's id, or `person` for the
//! person's message its turn answers; one row, on the person's own message even through Gathr's copy of
//! it, and it wakes nobody. Gathr's line, the agent's own message, and `person` with no message of the
//! person's to it are refused with a reference.

mod common;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, gathr, held_recording, nth, rows, team, text, until,
};
use serde_json::{Value, json};

struct Room {
    _go: common::replay::LetGo,
    service: TestService,
    chat: String,
    lead: String,
    ann: String,
    data: PathBuf,
}

/// Lead (Supervisor) and Ann in a chat, each engine held once its message arrives.
fn room(data: &Path) -> Room {
    let (service, chat, lead) = team(data, &held_recording(data, "signed_in"), &[]);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history["members"].as_array().unwrap();
    let ann = members.iter().find(|m| m["name"] == "Ann").unwrap()["agent"]
        .as_str()
        .unwrap()
        .to_string();
    Room {
        _go: common::replay::LetGo(data.join("go")),
        service,
        chat,
        lead,
        ann,
        data: data.to_path_buf(),
    }
}

impl Room {
    fn history(&self) -> Value {
        let path = format!("/api/chats/{}", self.chat);
        self.service.call("GET", &path).json()
    }

    fn post(&self, body: Value) -> Value {
        let path = format!("/api/chats/{}/messages", self.chat);
        let sent = self.service.send("POST", &path, body);
        assert_eq!(sent.status, 200, "{}", sent.body);
        sent.json()
    }

    /// The token of the `n`th engine started, once it runs.
    fn token(&self, n: usize) -> String {
        let started = nth(&self.data.join("env.jsonl"), n);
        started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
    }

    fn react(&self, token: &str, message: &str) -> Value {
        let ask = json!({ "message": message });
        call_tool(self.service.info.port, token, "react", ask)
    }

    /// The id of the person's message with these words.
    fn persons(&self, words: &str) -> String {
        let history = self.history();
        let messages = history["messages"].as_array().unwrap();
        let found = messages
            .iter()
            .find(|m| m["role"] == "user" && m["body"] == words);
        found.unwrap()["id"].as_str().unwrap().to_string()
    }

    /// What a thumbs-up must leave alone: the chat's messages and turns, its row in the list, the
    /// messages sent and the hand-offs counted.
    fn untouched(&self) -> Value {
        let history = self.history();
        let listed = self.service.call("GET", "/api/chats").json();
        let listed = listed.as_array().cloned().unwrap_or_default();
        let row = listed.into_iter().find(|c| c["id"] == self.chat.as_str());
        let count = |sql: &str| rows(&self.data, sql);
        json!({
            "messages": history["messages"],
            "turns": history["turns"],
            "row": row,
            "sends": count("SELECT count(*) AS n FROM event_log WHERE name = 'gathr.message.send'"),
            "handoffs": count("SELECT count(*) AS n FROM handoffs"),
        })
    }
}

/// The reactions in history, as `(message, agent)`.
fn reactions(history: &Value) -> Vec<(String, String)> {
    let all = history["reactions"].as_array().cloned().unwrap_or_default();
    let pair = |r: &Value| {
        let s = |k: &str| r[k].as_str().unwrap_or_default().to_string();
        (s("message"), s("agent"))
    };
    all.iter().map(pair).collect()
}

#[test]
fn a_thumbs_up_on_the_persons_message_by_person_or_by_id_is_one_row_each_and_wakes_nobody() {
    let data = TempDir::new();
    let room = room(&data.0);
    room.post(json!({ "text": "Made-up plan for the tags", "agent": room.lead }));
    let lead = room.token(0);
    room.post(json!({ "text": "Made-up note for Ann", "agent": room.ann }));
    let ann = room.token(1);
    let (plan, note) = (
        room.persons("Made-up plan for the tags"),
        room.persons("Made-up note for Ann"),
    );
    let before = room.untouched();
    let mut live = room.service.live();

    let by_word = room.react(&lead, "person");
    assert_eq!(by_word["isError"], false, "{by_word}");
    assert_eq!(text(&by_word), format!("Gave {plan} a thumbs-up."));
    let by_id = room.react(&ann, &plan);
    assert_eq!(by_id["isError"], false, "{by_id}");
    let again = room.react(&lead, &plan);
    assert_eq!(text(&again), format!("{plan} already has your thumbs-up."));
    let own_turn = room.react(&ann, "person");
    assert_eq!(text(&own_turn), format!("Gave {note} a thumbs-up."));

    let history = room.history();
    let want = [
        (plan.clone(), room.lead.clone()),
        (plan.clone(), room.ann.clone()),
        (note.clone(), room.ann.clone()),
    ];
    assert_eq!(reactions(&history), want, "{history}");
    assert_eq!(room.untouched(), before, "a thumbs-up changed the chat");
    let events = rows(
        &data.0,
        "SELECT attrs FROM event_log WHERE name = 'gathr.message.reaction'",
    );
    assert_eq!(events.len(), 3, "{events:?}");
    // Live as in history, one event for each new row: the repeat sent nothing.
    let saved = history["reactions"].as_array().unwrap();
    for (n, kept) in saved.iter().enumerate() {
        let heard = until(10, &format!("thumbs-up {n}, live"), || {
            let event = common::next_live(&mut live);
            (event["type"] == "reaction").then_some(event)
        });
        assert_eq!(heard["chat"], room.chat.as_str(), "{heard}");
        assert_eq!(&heard["reaction"], kept, "live and history differ");
    }
}

/// Since PR 5 a message to several picked agents is saved once, and each other agent's turn holds
/// Gathr's copy of it: a thumbs-up on the copy, by its id or by `person`, is on the person's own.
#[test]
fn a_thumbs_up_on_gathrs_copy_of_the_persons_message_lands_on_the_message() {
    let data = TempDir::new();
    let room = room(&data.0);
    let words = "@Lead @Ann made-up question for you both";
    let picks = json!([
        { "at": 0, "len": 5, "agent": room.lead },
        { "at": 6, "len": 4, "agent": room.ann },
    ]);
    room.post(json!({ "text": words, "agent": room.lead, "picks": picks }));
    let (first, second) = (room.token(0), room.token(1));
    let message = room.persons(words);
    let history = room.history();
    let all = history["messages"].as_array().unwrap();
    let copy = all.iter().find(|m| m["copy_of"] == message.as_str());
    let copy = copy.unwrap_or_else(|| panic!("no copy: {history}"))["id"]
        .as_str()
        .unwrap()
        .to_string();

    // One agent reads the message, the other the copy: by `person`, both land on the message.
    for token in [&first, &second] {
        let given = room.react(token, "person");
        assert_eq!(text(&given), format!("Gave {message} a thumbs-up."));
    }
    let by_copy = room.react(&second, &copy);
    assert_eq!(
        text(&by_copy),
        format!("{message} already has your thumbs-up.")
    );
    let on: Vec<String> = reactions(&room.history())
        .into_iter()
        .map(|(m, _)| m)
        .collect();
    assert_eq!(on, [message.clone(), message], "never on the copy {copy}");
}

#[test]
fn a_thumbs_up_on_gathrs_line_its_own_message_or_person_with_none_is_refused_with_a_reference() {
    let data = TempDir::new();
    let room = room(&data.0);
    room.post(json!({ "text": "Start", "agent": room.lead }));
    let lead = room.token(0);
    common::fix_it(room.service.info.port, &lead);
    let task = json!({ "to": "Ann", "what": "Fix the made-up build" });
    let port = room.service.info.port;
    let given = call_tool(port, &lead, "give_task", common::briefed(task));
    assert_eq!(given["isError"], false, "{}", text(&given));
    let ann = room.token(1);
    let sent = call_tool(
        port,
        &lead,
        "message",
        json!({ "to": "Ann", "text": "Made-up heads-up" }),
    );
    assert_eq!(sent["isError"], false, "{}", text(&sent));
    let history = room.history();
    let messages = history["messages"].as_array().unwrap();
    let id_of = |found: Option<&Value>| found.unwrap()["id"].as_str().unwrap().to_string();
    let line = id_of(
        messages
            .iter()
            .find(|m| m["role"] == "gathr" && m["copy_of"].is_null()),
    );
    let leads = id_of(messages.iter().find(|m| m["body"] == "Made-up heads-up"));

    let goes = "a thumbs-up goes on the person's message or another agent's.";
    let none = "your turn answers no message of the person's: give the message's id.";
    for (token, id, why) in [
        (
            &ann,
            line.as_str(),
            format!("{line} is Gathr's line: {goes}"),
        ),
        (
            &lead,
            leads.as_str(),
            format!("{leads} is your own message: {goes}"),
        ),
        (&ann, "person", none.to_string()),
    ] {
        let refused = room.react(token, id);
        let said = text(&refused);
        assert_eq!(refused["isError"], true, "{id}: {said}");
        let starts = format!("react failed: {why} (ref ");
        assert!(said.starts_with(&starts), "{id}: {said}");
        let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
        let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
        let events = trace["events"].as_array().cloned().unwrap_or_default();
        let found = events.iter().any(|e| {
            e["name"] == "gathr.tool.call"
                && e["attrs"]["gen_ai.tool.name"] == "react"
                && e["attrs"]["why"] == why.as_str()
        });
        assert!(ok && found, "{id}: {trace}");
        // The forced failure: `cargo nextest run -p gathr --test reactions refused -- --nocapture`.
        println!("diag trace {reference}: {trace:#}");
    }
    assert!(reactions(&room.history()).is_empty());
}
