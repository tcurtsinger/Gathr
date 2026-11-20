//! What an agent hears of its team chat when its turn opens (0081), through a real service on
//! Claude Code's replay stand-in: a first turn or a new conversation hears the chat's last 30 lines,
//! a turn that goes on in its conversation hears what came since its last turn, without its own; a
//! member hears other members' lines cut to their first sentence, the lead hears every line whole,
//! a very long line is capped; the message that opened the turn is never a line; a chat of one hears
//! nothing; and what each turn was told is counted on its trace.

mod common;

use std::path::Path;

use common::replay::{escaped, read_in, turns as engine};
use common::{TempDir, TestService, rows, team, until};
use serde_json::{Value, json};

/// Made-up lines written straight into the chat, as turns that ran before would have left them.
struct Seed {
    conn: rusqlite::Connection,
    chat: String,
    n: i64,
}

impl Seed {
    fn new(data: &Path, chat: &str) -> Seed {
        let conn = rusqlite::Connection::open(data.join("gathr.db")).unwrap();
        conn.busy_timeout(std::time::Duration::from_secs(10))
            .unwrap();
        Seed {
            conn,
            chat: chat.to_string(),
            n: 0,
        }
    }

    fn next(&mut self) -> (String, i64) {
        self.n += 1;
        (format!("seed{:04}", self.n), gathr::ids::now_us() + self.n)
    }

    /// A turn of `agent` that ran and ended: its id.
    fn turn(&mut self, agent: &str) -> String {
        let (id, ts) = self.next();
        let turn = format!("turn-{id}");
        self.conn
            .execute(
                "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, ended_ts, agent_id, position)
                 VALUES (?1, ?2, 'seedtrace', 'done', ?3, ?3, ?4, ?5)",
                rusqlite::params![turn, self.chat, ts, agent, 10_000 + self.n],
            )
            .unwrap();
        turn
    }

    /// A message: in `turn`, or said in no turn; its id.
    fn say(&mut self, turn: Option<&str>, role: &str, kind: &str, body: &str) -> String {
        let (id, ts) = self.next();
        self.conn
            .execute(
                "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![id, self.chat, turn, ts, role, kind, body],
            )
            .unwrap();
        id
    }

    /// `agent`'s words in a turn of its own.
    fn agent_says(&mut self, agent: &str, body: &str) -> String {
        let turn = self.turn(agent);
        self.say(Some(&turn), "assistant", "text", body)
    }
}

/// Adds Bo to the chat as a member, on the lead's account: Bo's id.
fn with_bo(service: &TestService, chat: &str, lead: &str) -> String {
    let account = service.call("GET", &format!("/api/agents/{lead}")).json()["account"].clone();
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Bo", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let bo = made.json()["id"].as_str().unwrap().to_string();
    let path = format!("/api/chats/{chat}");
    let mut members: Vec<Value> = service.call("GET", &path).json()["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!({ "agent": m["agent"], "role": m["role"] }))
        .collect();
    members.push(json!({ "agent": bo, "role": "member" }));
    let changed = service.send("PATCH", &path, json!({ "members": members }));
    assert_eq!(changed.status, 200, "{}", changed.body);
    bo
}

fn member(service: &TestService, chat: &str, name: &str) -> String {
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history["members"].as_array().cloned().unwrap_or_default();
    let found = members.iter().find(|m| m["name"] == name);
    found.unwrap_or_else(|| panic!("no member {name}: {history}"))["agent"]
        .as_str()
        .unwrap()
        .to_string()
}

/// The person's message to `agent`: the turn it opened.
fn say_to(service: &TestService, chat: &str, agent: &str, words: &str) -> String {
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": words, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    sent.json()["turn"].as_str().unwrap().to_string()
}

fn ended(service: &TestService, chat: &str, turn: &str) {
    service.wait_for(chat, 30, &format!("turn {turn} to end"), |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && t["status"] == "done")
    });
}

/// What the engine read in `turn`, once it holds `words`.
fn read_once(data: &Path, turn: &str, words: &str) -> String {
    until(30, &format!("turn {turn} to read {words:?}"), || {
        let read = read_in(data, turn);
        read.contains(&escaped(words)).then_some(read)
    })
}

/// What `gathr.room.heard` recorded for `turn`.
fn heard(data: &Path, turn: &str) -> Value {
    let sql = format!(
        "SELECT attrs FROM event_log WHERE name = 'gathr.room.heard' AND json_extract(attrs, '$.\"gathr.turn.id\"') = '{turn}'"
    );
    let found = rows(data, &sql);
    assert_eq!(found.len(), 1, "one record for {turn}: {found:?}");
    found[0]["attrs"].clone()
}

const CUT: &str = "(Lines not for you are cut to their first sentence: read_chat has them whole.)";

#[test]
fn a_members_first_turn_hears_the_last_30_lines_others_cut_and_a_long_line_capped() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[], 2), &[]);
    let bo = with_bo(&service, &chat, &lead);
    let ann = member(&service, &chat, "Ann");
    let mut seed = Seed::new(&data.0, &chat);
    let asked = seed.turn(&lead);
    for n in 1..=27 {
        seed.say(Some(&asked), "user", "text", &format!("Filler {n}."));
    }
    let long = format!("Plan: {}", "x".repeat(2000));
    let plan = seed.agent_says(&lead, &long);
    seed.agent_says(&bo, "The map is wrong. The river bends north there.");
    let bo_turn = seed.turn(&bo);
    seed.say(Some(&bo_turn), "assistant", "tool", "{}");
    seed.say(
        Some(&bo_turn),
        "assistant",
        "text",
        "@Ann can you check it? The scale is off too.",
    );
    seed.say(
        None,
        "gathr",
        "text",
        "A task is done:\n- #1 Bo, Draw the map: done",
    );
    let copy = seed.say(
        Some(&bo_turn),
        "gathr",
        "text",
        "Lead: a copy, never a line",
    );
    seed.conn
        .execute(
            "UPDATE messages SET copy_of = ?1 WHERE id = ?2",
            [&plan, &copy],
        )
        .unwrap();
    seed.say(Some(&asked), "user", "text", "Which way is north?");

    let turn = say_to(&service, &chat, &ann, "Start");
    let capped = format!(
        "Plan: {} [… 106 of 2006 characters cut: read message {plan} for all of it …] {}",
        "x".repeat(1494),
        "x".repeat(400)
    );
    let fillers: Vec<String> = (3..=27).map(|n| format!("Person: Filler {n}.")).collect();
    let block = [
        "Recent lines in this chat:".to_string(),
        fillers.join("\n"),
        format!("Lead: {capped}"),
        "Bo: The map is wrong. …".to_string(),
        "Bo: @Ann can you check it? The scale is off too.".to_string(),
        "Gathr: A task is done:\n- #1 Bo, Draw the map: done".to_string(),
        "Person: Which way is north?".to_string(),
        CUT.to_string(),
    ]
    .join("\n");
    let read = read_once(&data.0, &turn, &format!("{block}\n\nStart"));
    assert!(!read.contains("Filler 2."), "past the last 30: {read}");
    ended(&service, &chat, &turn);
    let counted = heard(&data.0, &turn);
    assert_eq!(
        (
            &counted["lines"],
            &counted["chars"],
            &counted["fresh"],
            &counted["cut"]
        ),
        (
            &json!(30),
            &json!(block.chars().count()),
            &json!(true),
            &json!(1)
        ),
        "{counted}"
    );
}

#[test]
fn the_lead_hears_what_came_since_its_last_turn_whole_without_its_own_or_the_opening() {
    let data = TempDir::new();
    let (service, chat, lead) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    let bo = with_bo(&service, &chat, &lead);
    let ann = member(&service, &chat, "Ann");
    let first = say_to(&service, &chat, &lead, "Plan the walk");
    ended(&service, &chat, &first);
    let mut seed = Seed::new(&data.0, &chat);
    seed.agent_says(&lead, "My own words, already in my conversation.");
    seed.agent_says(&bo, "The map is wrong. The river bends north there.");
    seed.agent_says(&ann, "@Lead done.");

    let next = say_to(&service, &chat, &lead, "Next");
    let block = "New in this chat since your last turn:\n\
                 Bo: The map is wrong. The river bends north there.\n\
                 Ann: @Lead done.";
    let read = read_once(&data.0, &next, &format!("{block}\n\nNext"));
    assert!(!read.contains("My own words"), "its own line: {read}");
    assert!(
        !read.contains("Person: Next"),
        "the opening as a line: {read}"
    );
    ended(&service, &chat, &next);
    let counted = heard(&data.0, &next);
    assert_eq!(
        (&counted["lines"], &counted["fresh"], &counted["cut"]),
        (&json!(2), &json!(false), &json!(0)),
        "{counted}"
    );
}

#[test]
fn a_chat_of_one_agent_hears_nothing_of_it() {
    let data = TempDir::new();
    let (service, _, lead) = team(&data.0, &engine(&data.0, &[], 2), &[]);
    let one = json!({ "members": [{ "agent": lead, "role": "supervisor" }] });
    let made = service.send("POST", "/api/chats", one);
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let first = say_to(&service, &chat, &lead, "Plan the walk");
    ended(&service, &chat, &first);
    let mut seed = Seed::new(&data.0, &chat);
    seed.say(None, "gathr", "text", "A line said in no turn.");

    let next = say_to(&service, &chat, &lead, "Next");
    let read = read_once(&data.0, &next, "Next");
    assert!(
        !read.contains("in this chat") && !read.contains("A line said"),
        "a chat of one heard the chat: {read}"
    );
    ended(&service, &chat, &next);
    let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.room.heard'";
    assert_eq!(rows(&data.0, sql)[0]["n"], 0);
}
