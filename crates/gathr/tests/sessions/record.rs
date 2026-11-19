//! Scenario 8b: a turn on a task opens with its questions to the person and their answers, asked or
//! answered since its conversation last ran, and with the chat's lines said since then (0081).

use serde_json::{Value, json};

use super::common::replay::{self, LetGo};
use super::common::{TempDir, call_tool, text};
use super::{
    NEW, Room, give, give_ann, plain, played, quiet, restart, room, say, sent, settled, token_of,
};

const ASKED: &str = "Questions to the person from task #";

fn tool(room: &Room, token: &str, name: &str, args: Value) -> String {
    text(&call_tool(room.service.info.port, token, name, args))
}

fn answer(room: &Room, task: i64, pick: &str) {
    let body = json!({ "answers": [{ "task": task, "pick": [pick] }] });
    let sent = room.service.send("POST", "/api/person-tasks/answer", body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    quiet(room);
}

/// `agent`'s turns on `task`, oldest first, from the `from`th of the chat's turns.
fn turns_on(room: &Room, agent: &str, task: i64, from: usize) -> Vec<Value> {
    let history = quiet(room);
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let mine = turns.into_iter().skip(from);
    mine.filter(|t| t["agent"] == agent && t["task"] == task)
        .collect()
}

fn count(room: &Room) -> usize {
    quiet(room)["turns"].as_array().map_or(0, Vec::len)
}

/// What the engine was sent for each turn, joined.
fn read(room: &Room, turns: &[Value]) -> Vec<String> {
    turns.iter().map(|t| sent(room, t).join("\n")).collect()
}

const COLOUR: &str = "Blue or green?";

/// Ann asks from #1; the lead files it as #3 for her request #2.
fn asked_for_ann(room: &Room) {
    give_ann(room, "Task 1");
    let ann = token_of(room, &room.ann);
    let need =
        json!({ "what": "Which colour?", "choices": [{ "label": "Blue" }, { "label": "Green" }] });
    let routed = tool(room, &ann, "ask_person", need);
    assert!(routed.contains("request #2"), "{routed}");
    quiet(room);
    let own = json!({ "for_task": 2, "what": COLOUR, "choices": [{ "label": "Blue" }, { "label": "Green" }] });
    let filed = tool(room, &room.lead_token(), "ask_person", own);
    assert!(filed.starts_with("Task #3 "), "{filed}");
    quiet(room);
}

/// 8b, a member's question through its request: #1's next turn hears it unanswered, and after the
/// answer her hear-back tells it once.
#[test]
fn a_members_task_hears_the_question_the_lead_filed_for_it_and_its_answer() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    asked_for_ann(&room);
    let before = count(&room);
    restart(&room, 1);
    settled(&room, 1, "blocked");
    let waiting = read(&room, &turns_on(&room, &room.ann, 1, before));
    let unanswered = format!("{ASKED}1:\n- #3 {COLOUR}: not answered yet");
    assert!(
        waiting[0].contains(&text_of(&unanswered)),
        "{unanswered:?} in {waiting:?}"
    );
    assert!(
        waiting[1..].iter().all(|w| !w.contains(ASKED)),
        "told once: {waiting:?}"
    );

    let before = count(&room);
    answer(&room, 3, "Blue");
    restart(&room, 1);
    settled(&room, 1, "blocked");
    // Her hear-back on #1 tells the answer in its request's line; no turn tells it again.
    let after = read(&room, &turns_on(&room, &room.ann, 1, before));
    let told: usize = after
        .iter()
        .map(|a| a.matches("Picked: Blue").count())
        .sum();
    assert_eq!(told, 1, "the answer once in {after:?}");
    assert!(after.iter().all(|a| !a.contains(ASKED)), "{after:?}");
}

/// 8b, the lead's own task: its next turn on it hears the question it asked there, and the turn its
/// answer opens doesn't hear it twice.
#[test]
fn the_leads_task_turn_hears_its_question_and_its_answered_turn_no_more() {
    let data = TempDir::new();
    let go = data.0.join("go");
    let _go = LetGo(go.clone());
    // The lead's second turn, its first on the task, waits for `go`.
    let held = replay::turns(&data.0, &[1], 12);
    let room = room(&data.0, &held, "claude", &[("GATHR_REPLAY_HOLD", &go)]);
    let plan = json!({ "to": "Lead", "what": "Plan the work" });
    let task = give(&room, &room.lead_token(), plan);
    room.service
        .wait_for(&room.chat, 30, "the lead's turn on its task", |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns
                .iter()
                .any(|t| t["task"] == task && t["status"] == "running")
        });
    let ask = json!({ "what": COLOUR, "choices": [{ "label": "Blue" }, { "label": "Green" }] });
    let asked = tool(&room, &room.lead_token(), "ask_person", ask);
    let question: i64 = asked
        .strip_prefix("Task #")
        .and_then(|r| r.split(' ').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("not asked: {asked}"));
    let to_ann = json!({ "to": "Ann", "what": "Draft it" });
    let drafted = give(&room, &room.lead_token(), to_ann);
    std::fs::write(&go, "").unwrap();
    // Ann's task ends blocked: the lead hears back on its own task, still waiting on the question.
    settled(&room, drafted, "blocked");
    let on = read(&room, &turns_on(&room, &room.lead, task, 0));
    let unanswered = text_of(&format!(
        "{ASKED}{task}:\n- #{question} {COLOUR}: not answered yet"
    ));
    assert!(!on[0].contains(ASKED), "asked in it: {on:?}");
    assert!(on[1].contains(&unanswered), "{unanswered:?} in {on:?}");

    let before = count(&room);
    answer(&room, question, "Green");
    let woken = read(&room, &turns_on(&room, &room.lead, task, before));
    assert!(
        woken[0].contains("The person answered:"),
        "the answered turn: {woken:?}"
    );
    assert!(woken.iter().all(|w| !w.contains(ASKED)), "{woken:?}");
}

/// 8b, a new conversation: after a refused resume, #1's turn hears every question asked from it,
/// answered before too.
#[test]
fn a_new_conversation_on_a_task_hears_all_its_questions() {
    let data = TempDir::new();
    let missing = [("GATHR_REPLAY_RESUME", std::path::Path::new("missing"))];
    let room = room(&data.0, &plain(&data.0), "claude", &missing);
    asked_for_ann(&room);
    let before = count(&room);
    answer(&room, 3, "Green");
    give_ann(&room, "Task 4");
    restart(&room, 1);
    settled(&room, 1, "blocked");
    // #1's conversation is refused: its turn opens new.
    let all = read(&room, &turns_on(&room, &room.ann, 1, before));
    let new = all.iter().find(|a| a.contains(NEW));
    let new = new.unwrap_or_else(|| panic!("no new conversation: {all:?}"));
    let answered = text_of(&format!("{ASKED}1:\n- #3 {COLOUR}: Picked: Green"));
    assert!(new.contains(&answered), "{answered:?} in {new}");
}

/// 0081 across conversations: Ann on #1, then #2, then #1 again. #1's conversation hears what was
/// said since it last ran, her own words on #2 among them, up to the last 30.
#[test]
fn a_task_back_from_another_hears_what_was_said_since_it_last_ran() {
    let data = TempDir::new();
    let room = room(&data.0, &played(&data.0, 30, |_, _| {}), "claude", &[]);
    give_ann(&room, "Task 1");
    for n in 1..=16 {
        say(&room, &room.lead, &format!("Line {n} before #2"));
    }
    give_ann(&room, "Task 2");
    say(&room, &room.lead, "Line after #2");
    let before = count(&room);
    restart(&room, 1);
    settled(&room, 1, "blocked");
    let back = read(&room, &turns_on(&room, &room.ann, 1, before));
    let heard = back[0]
        .split("New in this chat since your last turn:")
        .nth(1)
        .unwrap_or_else(|| panic!("no lines heard: {}", back[0]));
    for said in ["Line after #2", "Line 16 before #2", "Ann: Hi!"] {
        assert!(heard.contains(said), "{said:?} in {heard}");
    }
    // The person's 16 lines and the lead's 16 replies before #2 are more than 30 with what came after.
    assert!(!heard.contains("Line 1 before #2"), "{heard}");
}

/// Words as the engine's JSON frame holds them.
fn text_of(words: &str) -> String {
    let json = serde_json::to_string(words).unwrap();
    json.trim_matches('"').to_string()
}
