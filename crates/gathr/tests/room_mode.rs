//! A room chat's turns (0065), through a real service on Claude Code's replay stand-in: who gets a
//! turn, and that Gathr says nothing itself; a turn with nothing to add posts nothing; a like starts
//! no turn; the cap on hand-offs pauses the room; Stop now holds the person's message queued there;
//! a REPLY sits under the message it answers; NO_REPLY posts nothing, and a check that reacts and
//! writes it closes; the check takes the lead's main message, never its reply; only a turn's last
//! words are its message, heard, waking and placed, its earlier words kept as notes. Each
//! engine plays its turns from the start of the recording, so an agent's nth turn says the nth words
//! given.

mod common;

use std::path::{Path, PathBuf};

use common::replay::escaped;
use common::{TempDir, TestService, rows, sign_in_account};
use serde_json::{Value, json};

/// Claude Code's stand-in saying each of `turns` in turn instead of "Hi!".
fn saying(dir: &Path, turns: &[&str]) -> PathBuf {
    saying_held(dir, turns, None)
}

/// As `saying`, with the turn at `held` held once its message is read, until the test lets it go.
fn saying_held(dir: &Path, turns: &[&str], held: Option<usize>) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let frames = frames.trim_end();
    let said: Vec<String> = turns
        .iter()
        .enumerate()
        .map(|(at, words)| {
            let turn = frames.replace("Hi!", &escaped(&escaped(words)));
            match (held == Some(at), turn.split_once('\n')) {
                (true, Some((read, rest))) => {
                    format!("{read}\n{{\"dir\":\"hold\",\"line\":\"take\"}}\n{rest}")
                }
                _ => turn,
            }
        })
        .collect();
    let file = dir.join("saying.jsonl");
    std::fs::write(&file, said.join("\n") + "\n").unwrap();
    file
}

/// Claude Code's stand-in writing, in each of `turns`, its blocks of words with a command run
/// between each block and the next.
fn blocks(dir: &Path, turns: &[&[&str]]) -> PathBuf {
    let frames = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.trim_end().lines().collect();
    let out = |frame: Value| json!({ "dir": "out", "line": frame.to_string() }).to_string();
    let mut all = Vec::new();
    for (at, said) in turns.iter().enumerate() {
        let (last, notes) = said.split_last().unwrap();
        all.extend(lines[..4].iter().map(|l| l.to_string()));
        for (n, note) in notes.iter().enumerate() {
            let id = format!("toolu_{at}_{n}");
            let content = json!([{ "type": "text", "text": note }]);
            let message = json!({ "id": format!("msg_{at}_{n}"), "type": "message", "role": "assistant",
                "content": content });
            all.push(out(json!({ "type": "assistant", "message": message,
                "parent_tool_use_id": null, "session_id": "default" })));
            let input = json!({ "command": "git status --short", "description": "Show changes" });
            let content = json!([{ "type": "tool_use", "id": id, "name": "Bash", "input": input }]);
            let message = json!({ "id": format!("msg_{at}_{n}_t"), "type": "message",
                "role": "assistant", "content": content });
            all.push(out(json!({ "type": "assistant", "message": message,
                "parent_tool_use_id": null, "session_id": "default" })));
            let result = json!([{ "tool_use_id": id, "type": "tool_result", "content": "ok",
                "is_error": false }]);
            all.push(out(
                json!({ "type": "user", "message": { "role": "user", "content": result },
                "parent_tool_use_id": null, "session_id": "default" }),
            ));
        }
        let last = escaped(&escaped(last));
        all.extend(lines[4..].iter().map(|l| l.replace("Hi!", &last)));
    }
    let file = dir.join("blocks.jsonl");
    std::fs::write(&file, all.join("\n") + "\n").unwrap();
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

#[test]
fn the_check_takes_the_leads_main_message_never_its_reply_in_a_thread() {
    let data = TempDir::new();
    // The lead's words wake Ann, hers wake the lead, whose second turn replies to her.
    let first = "@Ann @Lead check the map.";
    let replay = saying(&data.0, &[first, "REPLY\nThanks, I'll merge."]);
    let room = room(&data.0, &replay);
    room.say("Plan the walk", None);
    room.quiet(3);

    // The lead's first words answer the person; its last are a reply to Ann, and aren't checked.
    let checked = || {
        let found = rows(
            &room.data,
            "SELECT answer FROM open_checks WHERE ended_ts IS NOT NULL",
        );
        (!found.is_empty()).then_some(found)
    };
    let checked = common::until(10, "the round", checked);
    let main = rows(
        &room.data,
        &format!(
            "SELECT m.id FROM messages m JOIN turns t ON t.id = m.turn_id WHERE t.agent_id = '{}'
             AND m.role = 'assistant' AND m.kind = 'text' AND m.reply_to IS NULL",
            room.lead
        ),
    );
    assert_eq!(main.len(), 1, "{main:?}");
    assert_eq!(checked, [json!({ "answer": main[0]["id"] })]);
}

#[test]
fn no_reply_posts_nothing_and_ends_done_and_words_after_it_post_without_it() {
    let data = TempDir::new();
    let replay = saying(&data.0, &["NO_REPLY.", "NO_REPLY\nThe map holds."]);
    let room = room(&data.0, &replay);
    let mut live = room.service.live();
    room.say("Anything to add?", None);
    room.quiet(1);
    room.say("And now?", None);
    let mut deltas = Vec::new();
    let posted = loop {
        let event = common::next_live(&mut live);
        match (event["type"].as_str(), &event["message"]) {
            (Some("delta"), _) => deltas.push(event["text"].clone()),
            (Some("message"), m) if m["role"] == "assistant" && m["kind"] == "text" => {
                break m.clone();
            }
            _ => {}
        }
    };
    let history = room.quiet(2);

    // The first turn ended done with nothing posted, live or kept; the second posted its words
    // after NO_REPLY as a main message, and NO_REPLY never streamed.
    assert_eq!(history["turns"][0]["status"], "done", "{history}");
    assert_eq!(posted["body"], "The map holds.", "{posted}");
    let said = agents_words(&history);
    let want = [(json!(room.lead), json!("The map holds."), Value::Null)];
    assert_eq!(said, want, "{history}");
    assert!(
        deltas
            .iter()
            .all(|d| !d.as_str().unwrap().contains("NO_REPLY")),
        "{deltas:?}"
    );
}

#[test]
fn a_check_that_reacts_and_writes_no_reply_closes_and_posts_nothing() {
    let data = TempDir::new();
    // Ann's first turn is a warm-up, so her check is her second: a thumbs-up, then NO_REPLY.
    let replay = saying_held(&data.0, &["The map is ready.", "NO_REPLY"], Some(1));
    let on = [("GATHR_OPEN_CHECK", Path::new("on"))];
    let (service, chat, lead) = common::team(&data.0, &replay, &on);
    let _go = common::replay::LetGo(data.0.join("go"));
    let history = |service: &TestService| service.call("GET", &format!("/api/chats/{chat}")).json();
    let members = history(&service)["members"].as_array().unwrap().clone();
    let ann = members.iter().find(|m| m["name"] == "Ann").unwrap()["agent"].clone();
    let say = |words: &str, agent: Value| {
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send("POST", &path, json!({ "text": words, "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
    };
    let ended = |n: usize| {
        move |h: &Value| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns.len() == n && turns.iter().all(|t| t["status"] == "done")
        }
    };
    say("Warm up", ann.clone());
    service.wait_for(&chat, 30, "Ann's warm-up", ended(1));
    say("Draw the map", Value::Null);
    let checking = |h: &Value| h["turns"].as_array().is_some_and(|t| t.len() == 3);
    service.wait_for(&chat, 30, "Ann's check", checking);
    let token = common::nth(&data.0.join("env.jsonl"), 0)["GATHR_AGENT_TOKEN"].clone();
    let ask = json!({ "message": "answer" });
    let given = common::call_tool(service.info.port, token.as_str().unwrap(), "react", ask);
    assert!(
        common::text(&given)
            .ends_with("That is your whole check: end your turn with only NO_REPLY."),
        "{given}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 30, "every turn done", ended(3));

    let closed = || {
        let ended = rows(
            &data.0,
            "SELECT count(*) AS n FROM open_checks WHERE ended_ts IS NOT NULL",
        );
        (ended[0]["n"] == 1).then_some(())
    };
    common::until(10, "the check closed", closed);
    let history = history(&service);
    assert_eq!(history["turns"].as_array().unwrap().len(), 3, "{history}");
    let said = agents_words(&history);
    let want = [
        (ann.clone(), json!("The map is ready."), Value::Null),
        (json!(lead), json!("The map is ready."), Value::Null),
    ];
    assert_eq!(said, want, "{history}");
}

#[test]
fn only_a_turns_last_words_are_heard_and_wake_whom_they_name() {
    let data = TempDir::new();
    // Every engine plays the same turn: a note naming Bo, a command, then words naming Ann.
    let turn: &[&str] = &["Looking at @Bo's map first.", "@Ann the map is ready."];
    let room = room(&data.0, &blocks(&data.0, &[turn]));
    room.say("Plan the walk", None);
    let history = room.quiet(2);

    // The lead's last words woke Ann; its note named Bo, who was woken by nothing.
    assert_eq!(room.turns_of(&history, &room.ann), [json!("teammate")]);
    assert!(room.turns_of(&history, &room.bo).is_empty(), "{history}");
    let turns = history["turns"].as_array().unwrap().iter();
    let ann = turns
        .into_iter()
        .find(|t| t["agent"] == room.ann.as_str())
        .unwrap();
    let read = common::replay::read_in(&room.data, ann["id"].as_str().unwrap());
    assert!(read.contains("the map is ready"), "{read}");
    assert!(!read.contains("Looking at"), "{read}");
    // The note is kept, as words of the lead's turn.
    let kept = agents_words(&history).into_iter().map(|(_, body, _)| body);
    assert!(
        kept.into_iter().any(|b| b == "Looking at @Bo's map first."),
        "{history}"
    );
}

#[test]
fn a_turn_whose_last_words_start_with_reply_is_a_reply_whatever_came_before() {
    let data = TempDir::new();
    // The lead, woken by the person, posts a main message; Ann's same turn answers it.
    let turn: &[&str] = &["Looking into it.", "REPLY\n@Ann the river bends."];
    let room = room(&data.0, &blocks(&data.0, &[turn]));
    room.say("Plan the walk", None);
    let history = room.quiet(2);

    let said = agents_words(&history);
    let main = history["messages"].as_array().unwrap().iter();
    let mut main =
        main.filter(|m| m["role"] == "assistant" && m["body"] == "@Ann the river bends.");
    let main = main.next().unwrap()["id"].clone();
    let note = json!("Looking into it.");
    let want = [
        (json!(room.lead), note.clone(), Value::Null),
        (
            json!(room.lead),
            json!("@Ann the river bends."),
            Value::Null,
        ),
        (json!(room.ann), note, Value::Null),
        (json!(room.ann), json!("@Ann the river bends."), main),
    ];
    assert_eq!(said, want, "{history}");
}

#[test]
fn the_assistant_takes_words_that_name_nobody_and_brings_in_whom_it_names() {
    let data = TempDir::new();
    let room = room(
        &data.0,
        &saying(&data.0, &["@Bo the map is yours.", "Done."]),
    );
    let set = room
        .service
        .send("PATCH", "/api/settings", json!({ "assistant": room.ann }));
    assert_eq!(set.status, 200, "{}", set.body);
    let made = room.service.send("POST", "/api/chats", json!({}));
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let room = Room { chat, ..room };
    room.say("Plan the walk", None);
    let history = room.quiet(2);

    // Ann, the assistant, took the person's words and leads; her words brought Bo in for a turn.
    assert_eq!(room.turns_of(&history, &room.ann), [json!("person")]);
    assert_eq!(room.turns_of(&history, &room.bo), [json!("teammate")]);
    let members = history["members"].as_array().unwrap().iter();
    let seats: Vec<(Value, Value)> = members
        .map(|m| (m["agent"].clone(), m["role"].clone()))
        .collect();
    let want = [
        (json!(room.ann), json!("supervisor")),
        (json!(room.bo), json!("member")),
    ];
    assert_eq!(seats, want, "{history}");
    assert!(room.turns_of(&history, &room.lead).is_empty(), "{history}");
}
