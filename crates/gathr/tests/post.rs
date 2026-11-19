//! 0079 through a real service and Gathr's MCP tools, on Claude Code's replay stand-in: an agent puts
//! its turn's bubble in the room, under a message or in a task's thread with `post`. History, the live
//! socket, `gathr diag threads` and a restart place it the same; what needs the person stays in the
//! room, and each refusal says why with a reference `gathr diag trace` finds.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, nth, rows, team, text};
use serde_json::{Value, json};

/// Lets every held engine go when the test ends, passed or failed.
struct LetGo(PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

/// Claude Code's recording held after its words are saved, so a turn has said something when it posts.
fn said_then_held(data: &Path) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let mut lines: Vec<&str> = frames.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains(r#"\"type\":\"assistant\""#));
    lines.insert(
        at.expect("the recording saves its words") + 1,
        r#"{"dir":"hold"}"#,
    );
    let held = data.join("said.jsonl");
    std::fs::write(&held, lines.join("\n") + "\n").unwrap();
    held
}

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

fn token(data: &Path, n: usize) -> String {
    nth(&data.join("env.jsonl"), n)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap_or_else(|| panic!("no token for engine {n}"))
        .to_string()
}

/// The person's message to the chat: the turn it opened and the message's id.
fn send(service: &TestService, chat: &str, body: Value) -> (String, String) {
    let said = body["text"].as_str().unwrap().to_string();
    let sent = service.send("POST", &format!("/api/chats/{chat}/messages"), body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent.json()["turn"].as_str().unwrap().to_string();
    let found = service.wait_for(chat, 30, "the person's message", |h| {
        message_of(h, |m| m["role"] == "user" && m["body"] == said.as_str()).is_some()
    });
    let id = message_of(&found, |m| {
        m["role"] == "user" && m["body"] == said.as_str()
    });
    (turn, id.unwrap())
}

fn message_of(history: &Value, found: impl Fn(&Value) -> bool) -> Option<String> {
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let message = messages.into_iter().find(|m| found(m))?;
    Some(message["id"].as_str()?.to_string())
}

/// The id of the turn's own words, once they are saved.
fn words_of(service: &TestService, chat: &str, turn: &str) -> String {
    let words = |m: &Value| m["turn"] == turn && m["role"] == "assistant" && m["kind"] == "text";
    let found = service.wait_for(chat, 30, "the turn's words", |h| {
        message_of(h, words).is_some()
    });
    message_of(&found, words).unwrap()
}

/// A new agent on the team's account, free to start while the others' turns hold: its id.
fn another(service: &TestService, data: &Path, name: &str) -> String {
    let lead = rows(data, "SELECT account_id FROM agents WHERE name = 'Lead'");
    let account = lead[0]["account_id"].clone();
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// Waits for `turn` to have `status`.
fn turn_is(service: &TestService, chat: &str, turn: &str, status: &str) {
    service.wait_for(chat, 30, &format!("turn {turn} to be {status}"), |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && t["status"] == status)
    });
}

fn topic_of(history: &Value, turn: &str) -> Value {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["id"] == turn);
    found.unwrap_or_else(|| panic!("no turn {turn}: {history}"))["topic"].clone()
}

fn post(service: &TestService, token: &str, to: &str) -> String {
    let posted = call_tool(service.info.port, token, "post", json!({ "to": to }));
    let said = text(&posted);
    assert_eq!(posted["isError"], false, "post {to}: {said}");
    said
}

/// The next live `turn` event for `turn` that carries `topic`.
fn moved_live(live: &mut std::net::TcpStream, turn: &str, topic: &str) -> Value {
    let mut sent = std::iter::from_fn(|| Some(common::next_live(live)));
    sent.find(|u| u["type"] == "turn" && u["turn"] == turn && u["topic"] == topic)
        .unwrap()
}

/// Each `gathr.turn.placed` event, oldest first, as (turn, to, words before, overridden, by).
fn placed_events(data: &Path) -> Vec<Value> {
    let sql = "SELECT json_extract(attrs, '$.turn') AS turn, json_extract(attrs, '$.to') AS t,
               json_extract(attrs, '$.words_before') AS words, json_extract(attrs, '$.overridden') AS over,
               json_extract(attrs, '$.by') AS by FROM event_log WHERE name = 'gathr.turn.placed' ORDER BY id";
    let found = rows(data, sql);
    // SQLite's JSON reads a true as 1.
    let row = |r: &Value| json!([r["turn"], r["t"], r["words"], r["over"] == 1, r["by"]]);
    found.iter().map(row).collect()
}

#[test]
fn a_turn_posted_to_the_room_a_message_a_task_or_the_persons_thread_moves_there_and_stays_after_a_restart()
 {
    let data = TempDir::new();
    let replay = said_then_held(&data.0);
    let (service, chat, _) = team(&data.0, &replay, &[]);
    let go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let mut live = service.live();
    let (lead_turn, ask) = send(
        &service,
        &chat,
        json!({ "text": "Made-up ask: tag the notes" }),
    );
    let lead = token(&data.0, 0);
    let leads_words = words_of(&service, &chat, &lead_turn);
    common::fix_it(port, &lead);
    let work = common::briefed(json!({ "to": "Ann", "what": "Tag the notes" }));
    let given = text(&call_tool(port, &lead, "give_task", work));
    assert!(given.starts_with("Task #1 for Ann"), "{given}");
    let ann_turn = common::turn_on(&service, &chat, 1, &["running"]);
    let ann = token(&data.0, 1);
    words_of(&service, &chat, &ann_turn);
    assert_eq!(topic_of(&history(&service, &chat), &ann_turn), "t:1");

    // A member's task turn put in the room is its own main message, moved live as it is chosen.
    let said = post(&service, &ann, "room");
    assert_eq!(said, "Your words this turn show in the room.");
    let own = format!("b:{ann_turn}");
    assert_eq!(topic_of(&history(&service, &chat), &ann_turn), own.as_str());
    let moved = moved_live(&mut live, &ann_turn, &own);
    assert_eq!(moved["chat"], chat.as_str(), "{moved}");

    // Each choice moves the whole bubble again.
    let at_lead = post(&service, &ann, &leads_words);
    assert_eq!(
        at_lead,
        "Your words this turn show in the thread of Lead's message."
    );
    let leads_own = format!("b:{lead_turn}");
    assert_eq!(
        topic_of(&history(&service, &chat), &ann_turn),
        leads_own.as_str()
    );
    moved_live(&mut live, &ann_turn, &leads_own);
    let in_task = post(&service, &ann, "#1");
    assert_eq!(in_task, "Your words this turn show in task #1's thread.");
    assert_eq!(topic_of(&history(&service, &chat), &ann_turn), "t:1");
    let at_ask = post(&service, &ann, &ask);
    assert_eq!(
        at_ask,
        "Your words this turn show in the thread of the person's message."
    );
    let under_ask = format!("m:{ask}");
    assert_eq!(
        topic_of(&history(&service, &chat), &ann_turn),
        under_ask.as_str()
    );

    // `person` names the person's message the lead's turn answers.
    let by_word = post(&service, &lead, "person");
    assert_eq!(
        by_word,
        "Your words this turn show in the thread of the person's message."
    );
    let before = history(&service, &chat);
    assert_eq!(topic_of(&before, &lead_turn), under_ask.as_str());
    moved_live(&mut live, &lead_turn, &under_ask);

    let (a, l) = (json!(ann_turn), json!(lead_turn));
    let want = [
        json!([a, "room", 1, false, null]),
        json!([a, "message", 1, false, null]),
        json!([a, "task", 1, false, null]),
        json!([a, "message", 1, false, null]),
        json!([l, "person", 1, false, null]),
    ];
    assert_eq!(placed_events(&data.0), want);
    let (ok, threads) = gathr(&data.0, &["diag", "threads", "--chat", &chat]);
    assert!(ok, "{threads}");
    let words = threads["in_words"].as_str().unwrap_or_default();
    assert_eq!(threads["counts"]["turns"]["placed"], 2, "{words}");
    let line =
        format!("turn {ann_turn} (brief) -> m:{ask}: the agent chose to reply to message {ask}");
    assert!(words.contains(&line), "{words}");
    common::topics_agree(&data.0, &chat);

    // History after a restart places them the same.
    drop(live);
    drop(go);
    service.stop();
    let env = [
        ("GATHR_REPLAY", replay.as_path()),
        ("GATHR_REPLAY_HOLD", &data.0.join("go")),
        ("GATHR_REPLAY_ENV", &data.0.join("env.jsonl")),
    ];
    let service = TestService::with_engine(&data.0, &env);
    let after = history(&service, &chat);
    for turn in [&ann_turn, &lead_turn] {
        assert_eq!(topic_of(&after, turn), topic_of(&before, turn), "{after}");
    }
}

/// Each refusal says why, with a reference `gathr diag trace` finds on the turn's trace.
fn refused(data: &Path, service: &TestService, token: &str, to: &str, why: &str) {
    let answer = call_tool(service.info.port, token, "post", json!({ "to": to }));
    let said = text(&answer);
    assert_eq!(answer["isError"], true, "{to}: {said}");
    let starts = format!("post failed: {why} (ref ");
    assert!(said.starts_with(&starts), "{to}: {said}");
    let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let found = events.iter().any(|e| {
        e["name"] == "gathr.tool.call"
            && e["attrs"]["gen_ai.tool.name"] == "post"
            && e["attrs"]["why"] == why
    });
    assert!(ok && found, "{to}: {trace}");
    // The forced failure: `cargo nextest run -p gathr --test post refused -- --nocapture`.
    println!("diag trace {reference}: {trace:#}");
}

#[test]
fn post_is_refused_in_a_chat_of_one_with_no_turn_running_and_for_what_isnt_this_chats_message_or_task()
 {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::held_recording(&data.0, "signed_in"), &[]);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let (lead_turn, _) = send(
        &service,
        &chat,
        json!({ "text": "Made-up ask: sort the bird list" }),
    );
    let lead = token(&data.0, 0);
    turn_is(&service, &chat, &lead_turn, "running");
    common::fix_it(port, &lead);
    let work = common::briefed(json!({ "to": "Ann", "what": "Sort the bird list" }));
    assert!(text(&call_tool(port, &lead, "give_task", work)).starts_with("Task #1 for Ann"));
    common::turn_on(&service, &chat, 1, &["running"]);
    let ann = token(&data.0, 1);
    let sent = call_tool(
        port,
        &lead,
        "message",
        json!({ "to": "Ann", "text": "Made-up heads-up" }),
    );
    assert_eq!(sent["isError"], false, "{}", text(&sent));
    let heads_up = message_of(&history(&service, &chat), |m| {
        m["body"] == "Made-up heads-up"
    })
    .unwrap();
    let (solo, on_own) = (service.new_chat(), another(&service, &data.0, "Solo"));
    let (solo_turn, _) = send(
        &service,
        &solo,
        json!({ "text": "Made-up ask on my own", "agent": on_own }),
    );
    let alone = token(&data.0, 2);
    turn_is(&service, &solo, &solo_turn, "running");
    let elsewhere = message_of(&history(&service, &solo), |m| m["role"] == "user").unwrap();
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    conn.execute(
        "INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body)
         VALUES ('step1', ?1, ?2, 1, 'assistant', 'tool', '{}')",
        [&chat, &lead_turn],
    )
    .unwrap();

    refused(
        &data.0,
        &service,
        &alone,
        "room",
        "post is for a team chat: this one has one agent.",
    );
    refused(
        &data.0,
        &service,
        &ann,
        "nope",
        "No message nope in this chat.",
    );
    let away = format!("No message {elsewhere} in this chat.");
    refused(&data.0, &service, &ann, &elsewhere, &away);
    let step = "step1 is a step, not a message: a reply answers a message's words.";
    refused(&data.0, &service, &ann, "step1", step);
    refused(&data.0, &service, &ann, "#99", "No task #99 in this chat.");
    let none = "your turn answers no message of the person's: give the message's id.";
    refused(&data.0, &service, &ann, "person", none);
    let own = format!("{heads_up} is this turn's own message: choose another, or the room.");
    refused(&data.0, &service, &lead, &heads_up, &own);
    // The result of an ask is recorded as its hear-back is queued (0051); set here on a running turn.
    conn.execute(
        "UPDATE turns SET result_of = 'made-up ask' WHERE id = ?1",
        [&lead_turn],
    )
    .unwrap();
    let result = "This turn is the result of the ask: it stays in the room.";
    refused(&data.0, &service, &lead, "room", result);
    conn.execute(
        "UPDATE turns SET result_of = NULL WHERE id = ?1",
        [&lead_turn],
    )
    .unwrap();
    drop(conn);

    // A turn that has ended, its engine still open: nothing to place.
    std::fs::write(data.0.join("go"), "").unwrap();
    turn_is(&service, &solo, &solo_turn, "done");
    refused(
        &data.0,
        &service,
        &alone,
        "room",
        "you have no turn running in this chat",
    );
    let placed = rows(
        &data.0,
        "SELECT count(*) AS n FROM turns WHERE placed IS NOT NULL",
    );
    assert_eq!(placed[0]["n"], 0, "a refused post placed a turn");
}

#[test]
fn a_plan_or_a_question_keeps_the_turn_in_the_room_whatever_it_chose() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &common::held_recording(&data.0, "signed_in"), &[]);
    let _go = LetGo(data.0.join("go"));
    let port = service.info.port;
    let mut live = service.live();

    // A question asked after choosing a thread: Gathr records it overrode the choice.
    let (asking, ask) = send(
        &service,
        &chat,
        json!({ "text": "Made-up ask: name the app" }),
    );
    let lead = token(&data.0, 0);
    turn_is(&service, &chat, &asking, "running");
    post(&service, &lead, &ask);
    let question = json!({ "what": "Which name do you like?", "form": "text" });
    let asked = call_tool(port, &lead, "ask_person", question);
    assert_eq!(asked["isError"], false, "{}", text(&asked));
    let questions = "This turn asks the person a question: it stays in the room.";
    refused(&data.0, &service, &lead, &ask, questions);

    // A plan written after choosing a thread puts the bubble back in the room, live too.
    let members = history(&service, &chat)["members"].clone();
    let ann = members
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Ann")
        .unwrap()["agent"]
        .clone();
    let members = json!([{ "agent": another(&service, &data.0, "Bo"), "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let other = made.json()["id"].as_str().unwrap().to_string();
    let (writing, other_ask) = send(&service, &other, json!({ "text": "Made-up ask: add tags" }));
    let planner = token(&data.0, 1);
    turn_is(&service, &other, &writing, "running");
    post(&service, &planner, &other_ask);
    let under = format!("m:{other_ask}");
    moved_live(&mut live, &writing, &under);
    let part = json!({ "to": "Ann", "what": "Add the tags table", "why": "Notes need tags",
        "done": "A note carries its tags", "proof": "tests/tags.rs passes", "where": "no code",
        "size": "src/tags.rs", "lines": 60 });
    let plan = json!({ "title": "Add tags", "text": "Notes carry tags.", "parts": [part] });
    let wrote = text(&call_tool(port, &planner, "plan", plan));
    assert!(wrote.starts_with("Wrote version 1 of plan #1"), "{wrote}");
    let own = format!("b:{writing}");
    assert_eq!(topic_of(&history(&service, &other), &writing), own.as_str());
    moved_live(&mut live, &writing, &own);
    let plans = "This turn holds a plan for the person: it stays in the room.";
    refused(&data.0, &service, &planner, &other_ask, plans);
    let room = post(&service, &planner, "room");
    assert_eq!(room, "Your words this turn show in the room.");

    let (q, w) = (json!(asking), json!(writing));
    let want = [
        json!([q, "message", 0, false, null]),
        json!([q, "message", 0, true, "question"]),
        json!([w, "message", 0, false, null]),
        // The version is the lead's own message in its turn: it counts among the words before.
        json!([w, "message", 1, true, "plan"]),
        json!([w, "room", 1, false, null]),
    ];
    assert_eq!(placed_events(&data.0), want);
    common::topics_agree(&data.0, &chat);
    common::topics_agree(&data.0, &other);
}

/// A database at 0058 upgrades by adding `turns.placed`: every rowid and the search index stay, and
/// its turns are placed by the default, as `gathr diag threads` read them before the upgrade.
#[test]
fn the_upgrade_from_0058_keeps_every_rowid_the_search_index_and_each_turns_place() {
    let data = TempDir::new();
    let db = gathr::data::DataDir::new(&data.0).db();
    std::fs::create_dir_all(&data.0).unwrap();
    let conn = gathr::db::open(&db).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('acc', 'claude', 'A', 'a', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('mira', 'Mira', 'mira', 'acc', 1), ('rowan', 'Rowan', 'rowan', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Made-up team', 1);
         INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES
             ('c', 'mira', 'supervisor', 1), ('c', 'rowan', 'member', 1);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, task, answers, opened_by) VALUES
             ('asked', 'c', 'a', 'done', 10, 'mira', NULL, NULL, 'person'),
             ('work', 'c', 'b', 'done', 20, 'rowan', 1, 'asked', 'brief');
         INSERT INTO tasks (number, chat_id, giver, owner, given_in, kind, what, state, created_ts, updated_ts)
             VALUES (1, 'c', 'mira', 'rowan', 'asked', 'work', 'Count the herons', 'done', 11, 11);
         INSERT INTO messages (id, chat_id, turn_id, ts, role, kind, body) VALUES
             ('m1', 'c', 'asked', 10, 'user', 'text', 'Count the herons by the weir'),
             ('m2', 'c', 'asked', 11, 'assistant', 'text', 'Rowan counts them'),
             ('m3', 'c', 'work', 20, 'gathr', 'text', 'Task #1 from Mira: Count the herons'),
             ('m4', 'c', 'work', 21, 'assistant', 'text', 'Nine herons');
         ALTER TABLE turns DROP COLUMN placed; DROP TABLE open_checks; ALTER TABLE turns DROP COLUMN session_id;
         PRAGMA user_version = 58;",
    )
    .unwrap();
    let rowids = "SELECT group_concat(rowid || ':' || id) FROM messages";
    let searched =
        "SELECT group_concat(rowid) FROM message_search WHERE message_search MATCH 'herons'";
    let one = |c: &rusqlite::Connection, sql: &str| {
        c.query_row(sql, [], |r| r.get::<_, String>(0)).unwrap()
    };
    let (before, indexed) = (one(&conn, rowids), one(&conn, searched));
    drop(conn);
    let (ok, threads) = gathr(&data.0, &["diag", "threads", "--chat", "c"]);
    assert!(ok, "{threads}");
    let read = |threads: &Value| {
        let turns = threads["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .map(|t| (t["turn"].clone(), t["topic"].clone()))
            .collect::<Vec<_>>()
    };
    let at_58 = read(&threads);
    assert_eq!(
        at_58,
        [
            (json!("asked"), json!("b:asked")),
            (json!("work"), json!("t:1"))
        ]
    );

    let service = TestService::start(&data.0);
    let history = history(&service, "c");
    let conn = rusqlite::Connection::open(&db).unwrap();
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, gathr::db::SCHEMA_VERSION);
    assert_eq!(
        (one(&conn, rowids), one(&conn, searched)),
        (before, indexed)
    );
    let placed: i64 = conn
        .query_row("SELECT count(*) FROM turns WHERE placed IS NULL", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(placed, 2, "an old turn was placed");
    for (turn, topic) in &at_58 {
        assert_eq!(
            &topic_of(&history, turn.as_str().unwrap()),
            topic,
            "{history}"
        );
    }
    let (ok, threads) = gathr(&data.0, &["diag", "threads", "--chat", "c"]);
    assert!(ok, "{threads}");
    assert_eq!(read(&threads), at_58);
}
