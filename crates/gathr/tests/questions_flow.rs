//! Questions in the lead's words (0065) through a real service and Gathr's tools, run once, on
//! Claude Code's stand-in: no engine frame is in play, only who may call what, the replies and the
//! references. Scenarios 4a to 4c and 4e.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, nth, rows, text, until};
use serde_json::{Value, json};

const NEED: &str = "What should the reconnect notice say?";
const ASKED: &str = "Which notice text should ship?";

/// Claude Code's stand-in plays one recording per engine: its first turn runs straight through,
/// and each turn after it holds until the test lets one go (`go`, taken by the held turn that sees
/// it first).
fn turns(data: &Path) {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let mut all: Vec<&str> = lines.clone();
    for _ in 0..6 {
        all.push(lines[0]);
        all.push(r#"{"dir":"hold","line":"take"}"#);
        all.extend(&lines[1..]);
    }
    std::fs::write(data.join("held.jsonl"), all.join("\n") + "\n").unwrap();
}

struct Room {
    service: TestService,
    chat: String,
    data: PathBuf,
    port: u16,
    lead: String,
}

/// A failing test's service is killed, not shut down, and an engine still held would outlive it:
/// each held turn is let go first, one `go` each, until none takes one.
impl Drop for Room {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            return;
        }
        let go = self.data.join("go");
        for _ in 0..8 {
            let _ = std::fs::write(&go, "");
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
            while go.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            if go.exists() {
                return;
            }
        }
    }
}

impl Room {
    /// A team chat (Lead, Ann) on Claude Code's stand-in, after the person's message: the lead's
    /// turn has run, and its engine's token calls the tools.
    fn new(data: &Path) -> Room {
        let (service, chat) = common::team_on(data, "claude");
        turns(data);
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send("POST", &path, json!({ "text": "Word the notice" }));
        assert_eq!(sent.status, 200, "{}", sent.body);
        let lead = common::first_token(data);
        let port = service.info.port;
        Room {
            service,
            chat,
            data: data.to_path_buf(),
            port,
            lead,
        }
    }

    fn history(&self) -> Value {
        self.service
            .call("GET", &format!("/api/chats/{}", self.chat))
            .json()
    }

    /// The `n`th turn on `task` (from 1), once it runs: its id.
    fn running_on(&self, task: i64, n: usize) -> String {
        let what = format!("turn {n} on #{task} to run");
        let history = self.service.wait_for(&self.chat, 30, &what, |h| {
            let on = on_task(h, task);
            on.len() >= n && on[n - 1]["status"] == "running"
        });
        on_task(&history, task)[n - 1]["id"]
            .as_str()
            .unwrap()
            .to_string()
    }

    /// Lets each of `held` go on, one `go` at a time, until each has ended. The next `go` waits
    /// until the last one is taken and its turn has ended: a held turn takes its `go` before it
    /// ends, and one written in between was left for the next held turn, which then ran through.
    fn release(&self, held: &[&str]) {
        let go = self.data.join("go");
        let ended = || {
            let history = self.history();
            let turns = history["turns"].as_array().cloned().unwrap_or_default();
            let over = |id: &&&str| {
                let turn = turns.iter().find(|t| t["id"] == **id);
                turn.is_some_and(|t| {
                    ["done", "failed", "stopped"]
                        .iter()
                        .any(|s| t["status"] == *s)
                })
            };
            held.iter().filter(over).count()
        };
        loop {
            let before = ended();
            if before == held.len() {
                return;
            }
            std::fs::write(&go, "").unwrap();
            let what = format!("one of {held:?} to take its go and end");
            until(30, &what, || {
                (!go.exists() && ended() > before).then_some(())
            });
        }
    }

    fn answer(&self, answers: Value) -> Value {
        let body = json!({ "answers": answers });
        let sent = self.service.send("POST", "/api/person-tasks/answer", body);
        assert_eq!(sent.status, 200, "{}", sent.body);
        sent.json()
    }

    fn listed(&self) -> Value {
        self.service.call("GET", "/api/person-tasks").json()["tasks"].clone()
    }

    fn tool(&self, token: &str, tool: &str, args: Value) -> String {
        text(&call_tool(self.port, token, tool, args))
    }

    /// The history once a message holds `said`.
    fn said(&self, what: &str, said: impl Fn(&str) -> bool) -> Value {
        self.service.wait_for(&self.chat, 20, what, |h| {
            let messages = h["messages"].as_array().cloned().unwrap_or_default();
            messages
                .iter()
                .any(|m| m["body"].as_str().is_some_and(&said))
        })
    }
}

fn on_task(history: &Value, task: i64) -> Vec<Value> {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.into_iter().filter(|t| t["task"] == task).collect()
}

/// Up to the lead's question: Lead gave Ann #1; Ann's request #2 reached the lead, who filed it
/// as #3 in its own words from its turn on #2. That turn and Ann's still hold: their ids.
fn filed(room: &Room) -> (String, String) {
    common::fix_it(room.port, &room.lead);
    let gave = json!({ "to": "Ann", "what": "Word the notice" });
    let given = room.tool(&room.lead, "give_task", common::briefed(gave));
    assert!(given.starts_with("Task #1 for Ann"), "{given}");
    // Ann's first turn runs through with no report; the one that asks for it holds.
    let ann_turn = room.running_on(1, 2);
    let ann = nth(&room.data.join("env.jsonl"), 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let need = json!({ "what": NEED, "why": "the notice ships today",
        "choices": [{ "label": "Reconnecting…", "detail": "it comes back by itself" }, { "label": "Connection lost" }] });
    let routed = room.tool(&ann, "ask_person", need);
    assert_eq!(
        routed,
        "Lead checks request #2 first. The person's answer comes back to you here."
    );
    let lead_turn = room.running_on(2, 1);
    let copied = room.tool(&room.lead, "ask_person", json!({ "for_task": 2 }));
    let refused = "ask_person failed: Write `what` in your own words: request #2 isn't sent to the person as it is \
                   (ref ";
    assert!(copied.starts_with(refused), "{copied}");
    let own = json!({ "for_task": 2, "what": ASKED,
        "choices": [{ "label": "Connection lost", "detail": "it says what happened" }, { "label": "Reconnecting…" }] });
    let asked = room.tool(&room.lead, "ask_person", own);
    assert_eq!(
        asked,
        "Task #3 is on the person's Tasks list. Their answer wakes you here, and reaches Ann."
    );
    (ann_turn, lead_turn)
}

#[test]
fn a_members_question_reaches_the_person_in_the_leads_words_and_its_answer_reaches_the_member() {
    let data = TempDir::new();
    let room = Room::new(&data.0);
    let (ann_turn, lead_turn) = filed(&room);
    let listed = room.listed();
    assert_eq!(
        (&listed[0]["what"], &listed[0]["choices"][0]["label"]),
        (&json!(ASKED), &json!("Connection lost")),
        "{listed}"
    );
    room.release(&[&ann_turn, &lead_turn]);
    let before = rows(&room.data, "SELECT max(rowid) AS n FROM turns")[0]["n"]
        .as_i64()
        .unwrap();

    let sent = room
        .answer(json!([{ "task": 3, "pick": ["Connection lost"], "text": "and say we retry" }]));
    assert_eq!(sent["refused"], json!([]), "{sent}");
    let line = format!(
        "- #2 Lead, {NEED}: done: the person answered #3 ({ASKED}): Picked: Connection lost. Their words: and say \
         we retry"
    );
    room.said("Ann's hear-back", |b| b.lines().any(|l| l == line));
    room.said("the lead's", |b| {
        b.starts_with("The person answered:")
            && b.contains("#3 was asked for Ann (#2): Ann has the answer.")
    });
    let sql = format!("SELECT opened_by FROM turns WHERE rowid > {before} ORDER BY rowid");
    let opened: Vec<Value> = rows(&room.data, &sql)
        .iter()
        .map(|r| r["opened_by"].clone())
        .collect();
    assert_eq!(opened, ["hear_back", "answered"], "Ann's turn comes first");

    // 0075: the lead's turn on Ann's request sits in her task's thread, and its turn that heard the
    // answer in the question's.
    let heard = rows(
        &room.data,
        "SELECT id FROM turns WHERE opened_by = 'answered'",
    );
    let history = room.history();
    let topic = |turn: &Value| {
        let turns = history["turns"].as_array().unwrap();
        turns.iter().find(|t| t["id"] == *turn).unwrap()["topic"].clone()
    };
    let found = [topic(&json!(lead_turn)), topic(&heard[0]["id"])];
    assert_eq!(found, [json!("t:1"), json!("t:3")], "{history}");
}

#[test]
fn discuss_reaches_the_lead_only_and_the_question_comes_back() {
    let data = TempDir::new();
    let room = Room::new(&data.0);
    let (ann_turn, lead_turn) = filed(&room);
    room.release(&[&ann_turn, &lead_turn]);
    let sent = room.answer(json!([{ "task": 3, "discuss": true }]));
    assert_eq!(
        sent["refused"][0]["why"], "Discuss #3 with a message",
        "{sent}"
    );
    let reference = sent["refused"][0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&room.data, &["diag", "trace", reference]);
    let found = trace.to_string();
    assert!(
        ok && found.contains("gathr.task.answer")
            && found.contains("refused")
            && found.contains("Discuss #3 with a message"),
        "{trace}"
    );
    // The PR's forced failure: `cargo nextest run -p gathr --test questions_flow discuss -- --nocapture`.
    println!("diag trace {reference}: {trace}");

    let ann_turns = |h: &Value| -> usize {
        let ann = &on_task(h, 1)[0]["agent"];
        h["turns"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|t| t["agent"] == *ann)
            .count()
    };
    let before = ann_turns(&room.history());
    let sent =
        room.answer(json!([{ "task": 3, "discuss": true, "text": "Does it retry by itself?" }]));
    assert_eq!(sent["refused"], json!([]), "{sent}");
    assert_eq!(room.listed(), json!([]), "off the person's list");
    let chats = room.service.call("GET", "/api/chats").json();
    let chat = chats
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == room.chat.as_str());
    assert_eq!(chat.unwrap()["needs"], false, "{chats}");
    let asks = |b: &str| {
        b.starts_with("The person wants to discuss:") && b.contains("Does it retry by itself?")
    };
    let history = room.said("the lead's turn", asks);
    assert_eq!(ann_turns(&history), before, "no message to Ann");
    let replying = room.running_on(2, 2);
    // 0075: the lead's turn that heard the Discuss sits in the question's thread.
    let history = room.history();
    let turns = history["turns"].as_array().unwrap();
    let found = turns.iter().find(|t| t["id"] == replying.as_str());
    assert_eq!(found.unwrap()["topic"], "t:3", "{history}");
    room.release(&[&replying]);
    until(20, "the question back on the list", || {
        (room.listed()[0]["number"] == 3).then_some(())
    });

    room.answer(json!([{ "task": 3, "discuss": true, "text": "And if it can't?" }]));
    // The lead's reply runs: its question is asked in that turn.
    room.running_on(2, 3);
    let again = json!({ "what": "Should the notice say we retry?", "form": "yesno" });
    let asked = room.tool(&room.lead, "ask_person", again);
    assert_eq!(
        asked,
        "Task #4 is on the person's Tasks list. Their answer wakes you here, and reaches Ann."
    );
    let tasks = room
        .service
        .call("GET", &format!("/api/chats/{}/tasks", room.chat))
        .json();
    let old = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["number"] == 3);
    assert_eq!(old.unwrap()["state"], "cancelled", "{tasks}");
    let numbers: Vec<Value> = room
        .listed()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["number"].clone())
        .collect();
    assert_eq!(numbers, [4]);
    room.answer(json!([{ "task": 4, "yes": true }]));
    let line = "- #2 Lead, What should the reconnect notice say?: done: the person answered #4 (Should the notice \
                say we retry?): Picked: Yes";
    room.said("Ann's hear-back", |b| b.lines().any(|l| l == line));
}

#[test]
fn ask_person_takes_files_from_shared() {
    let data = TempDir::new();
    let room = Room::new(&data.0);
    let started = nth(&data.0.join("env.jsonl"), 0);
    let cwd = PathBuf::from(started["cwd"].as_str().unwrap());
    let own = cwd.parent().unwrap().to_path_buf();
    let shared = own.parent().unwrap().join("Shared");
    std::fs::create_dir_all(&shared).unwrap();
    std::fs::write(own.join("plan.md"), "# Plan\n").unwrap();
    std::fs::write(shared.join("design.md"), "# Design\n").unwrap();
    let files = [own.join("plan.md"), shared.join("design.md")].map(|p| p.display().to_string());
    let asked = room.tool(
        &room.lead,
        "ask_person",
        json!({ "what": "Which design?", "files": files }),
    );
    assert!(
        asked.starts_with("Task #1 is on the person's Tasks list."),
        "{asked}"
    );
    for (n, body) in [(0, "# Plan\n"), (1, "# Design\n")] {
        let file = room.service.call("GET", &format!("/api/tasks/1/files/{n}"));
        assert_eq!((file.status, file.body.as_str()), (200, body));
    }
    let outside = data.0.join("env.jsonl").display().to_string();
    let refused = room.tool(
        &room.lead,
        "ask_person",
        json!({ "what": "Read this", "files": [outside] }),
    );
    assert!(
        refused.contains("isn't a file in your folders"),
        "{refused}"
    );
    let six =
        json!({ "what": "Read these", "files": ["a.md", "a.md", "a.md", "a.md", "a.md", "a.md"] });
    let refused = room.tool(&room.lead, "ask_person", six);
    assert!(refused.contains("at most 5 files"), "{refused}");
}
