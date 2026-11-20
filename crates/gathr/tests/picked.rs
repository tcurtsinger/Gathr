//! Every agent the person picks gets the message (0028) through a real service, on Claude Code's replay
//! stand-in: the message is saved once; the agent it is sent to reads it in its turn, and each other
//! picked agent reads Gathr's copy, in its running turn in the chat or a turn of its own, after one line
//! naming the others; every answer sits in the message's thread. A pick outside a project's team, or
//! one too many joining a chat with no project, refuses the send before anything is saved. Scenario 1j.

mod common;

use std::path::Path;

use common::replay::{escaped, read_in, turns as engine};
use common::{TempDir, TestService, gathr, rows, team, until};
use gathr::http::Response;
use serde_json::{Value, json};

const WORDS: &str = "@Ann @Bo can you two check this?";

fn history(service: &TestService, chat: &str) -> Value {
    service.call("GET", &format!("/api/chats/{chat}")).json()
}

fn post(service: &TestService, chat: &str, body: Value) -> Response {
    service.send("POST", &format!("/api/chats/{chat}/messages"), body)
}

/// A new agent on the account `account`: its id.
fn agent(service: &TestService, name: &str, account: &Value) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// The account the chat's lead runs on.
fn account_of(service: &TestService, agent: &str) -> Value {
    service.call("GET", &format!("/api/agents/{agent}")).json()["account"].clone()
}

/// Bo, on `account`, joins the team chat as a member: Ann's id and Bo's.
fn with_bo(service: &TestService, chat: &str, account: &Value) -> (String, String) {
    let path = format!("/api/chats/{chat}");
    let mut members: Vec<Value> = history(service, chat)["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!({ "agent": m["agent"], "role": m["role"] }))
        .collect();
    let ann = history(service, chat)["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["name"] == "Ann")
        .unwrap()["agent"]
        .as_str()
        .unwrap()
        .to_string();
    let bo = agent(service, "Bo", account);
    members.push(json!({ "agent": bo, "role": "member" }));
    let changed = service.send("PATCH", &path, json!({ "members": members }));
    assert_eq!(changed.status, 200, "{}", changed.body);
    (ann, bo)
}

/// "@Ann @Bo …" with both names picked, sent to `first`.
fn to_both(first: &str, ann: &str, bo: &str) -> Value {
    let picks = json!([{ "at": 0, "len": 4, "agent": ann }, { "at": 5, "len": 3, "agent": bo }]);
    json!({ "text": WORDS, "agent": first, "picks": picks })
}

fn turn_of(history: &Value, turn: &str) -> Value {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["id"] == turn);
    found
        .unwrap_or_else(|| panic!("no turn {turn}: {history}"))
        .clone()
}

fn turn_is(service: &TestService, chat: &str, turn: &str, statuses: &[&str]) -> Value {
    let what = format!("turn {turn} to be {statuses:?}");
    let history = service.wait_for(chat, 30, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["id"] == turn && statuses.iter().any(|s| t["status"] == *s))
    });
    turn_of(&history, turn)
}

/// The send's turn for `agent`, as its answer lists it.
fn sent_to(answer: &Value, agent: &str) -> Value {
    let turns = answer["turns"].as_array().cloned().unwrap_or_default();
    let found = turns.iter().find(|t| t["agent"] == agent);
    found
        .unwrap_or_else(|| panic!("no turn for {agent}: {answer}"))
        .clone()
}

/// The person's message with these words, as history shows it.
fn person_said(history: &Value, words: &str) -> Vec<Value> {
    let messages = history["messages"].as_array().cloned().unwrap_or_default();
    let of = |m: &&Value| m["role"] == "user" && m["body"] == words;
    messages.iter().filter(of).cloned().collect()
}

/// The engine read Gathr's `line`, then the person's words, in `turn`.
fn reads(data: &Path, turn: &str, line: &str) {
    let both = escaped(&format!("{line}\n\n{WORDS}"));
    until(10, &format!("turn {turn} to read {line:?}"), || {
        read_in(data, turn).contains(&both).then_some(())
    });
}

/// A refusal: 400 with `why`, and a reference `gathr diag trace` finds it from (0019).
fn refused(data: &Path, answer: &Response, why: &str) {
    assert_eq!(answer.status, 400, "{}", answer.body);
    let said = answer.json();
    assert_eq!(said["why"], json!(why), "{said}");
    let reference = said["ref"].as_str().expect("a reference");
    let (ok, trace) = gathr(data, &["diag", "trace", reference]);
    let events = trace["events"].as_array().cloned().unwrap_or_default();
    let failed = events.iter().find(|e| e["name"] == "gathr.api.request");
    let failed = failed.map(|e| e["attrs"]["reason"].clone());
    assert!(ok && failed == Some(json!(why)), "{reference}: {trace}");
    // The forced failure: `cargo nextest run -p gathr --test picked outside -- --nocapture`.
    println!("diag trace {reference}: {trace}");
}

#[test]
fn a_picked_agent_outside_a_projects_team_refuses_the_send_and_nothing_is_saved() {
    let data = TempDir::new();
    let (service, _, lead) = team(&data.0, &engine(&data.0, &[], 2), &[]);
    let account = account_of(&service, &lead);
    let ann = agent(&service, "Ann2", &account);
    let bo = agent(&service, "Bo", &account);
    let repos = data.0.join("repos");
    std::fs::create_dir_all(&repos).unwrap();
    let (folder, _) = common::project(&repos);
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let chat = common::project_chat(&service, &folder, members);

    let sent = post(&service, &chat, to_both(&ann, &ann, &bo));
    refused(&data.0, &sent, "a picked agent isn't in this chat");
    let shown = history(&service, &chat);
    assert_eq!(shown["messages"], json!([]), "{shown}");
    assert_eq!(shown["turns"], json!([]), "{shown}");
}

#[test]
fn in_a_chat_with_no_project_two_outsiders_join_in_one_write_and_one_too_many_refuses() {
    let data = TempDir::new();
    let (service, _, lead) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    let account = account_of(&service, &lead);
    let [ann, bo, cy, dee, eve] =
        ["Ann2", "Bo", "Cy", "Dee", "Eve"].map(|n| agent(&service, n, &account));
    let made = service.send("POST", "/api/chats", json!({}));
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let first = post(
        &service,
        &chat,
        json!({ "text": "Start here", "agent": lead }),
    );
    let first = first.json()["turn"].as_str().unwrap().to_string();
    turn_is(&service, &chat, &first, &["done"]);

    let words = "@Ann2 @Bo join us";
    let picks = json!([{ "at": 0, "len": 5, "agent": ann }, { "at": 6, "len": 3, "agent": bo }]);
    let sent = post(
        &service,
        &chat,
        json!({ "text": words, "agent": ann, "picks": picks }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    assert_eq!(
        sent.json()["turns"].as_array().map(Vec::len),
        Some(2),
        "{}",
        sent.body
    );
    let members: Vec<Value> = history(&service, &chat)["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!([m["agent"], m["role"]]))
        .collect();
    assert_eq!(
        members,
        [
            json!([lead, "supervisor"]),
            json!([ann, "member"]),
            json!([bo, "member"])
        ]
    );
    let changes = rows(
        &data.0,
        &format!(
            "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.chat.members' AND conversation_id = '{chat}'"
        ),
    );
    assert_eq!(
        changes,
        [json!({ "n": 2 })],
        "the lead's seat with the first message, then one change for both joiners"
    );

    let more = "@Cy @Dee @Eve too";
    let picks = json!([
        { "at": 0, "len": 3, "agent": cy },
        { "at": 4, "len": 4, "agent": dee },
        { "at": 9, "len": 4, "agent": eve },
    ]);
    let sent = post(
        &service,
        &chat,
        json!({ "text": more, "agent": cy, "picks": picks }),
    );
    refused(&data.0, &sent, "A chat needs 1 to 5 agents");
    let shown = history(&service, &chat);
    assert!(person_said(&shown, more).is_empty(), "{shown}");
    assert_eq!(
        shown["members"].as_array().map(Vec::len),
        Some(3),
        "{shown}"
    );
}

/// A copy's turn that waits for a sign-in is the person's: it outlives a restart and reads the same words.
#[test]
fn with_bo_signed_out_anns_turn_runs_and_bos_waits_for_sign_in_over_a_restart() {
    let data = TempDir::new();
    let (service, chat, _) = team(&data.0, &engine(&data.0, &[], 3), &[]);
    // Bo is on an account nobody signed in to.
    let away = json!(service.add_account("Away"));
    let (ann, bo) = with_bo(&service, &chat, &away);

    let sent = post(&service, &chat, to_both(&ann, &ann, &bo));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let answer = sent.json();
    let anns = sent_to(&answer, &ann)["turn"].as_str().unwrap().to_string();
    let bos = sent_to(&answer, &bo)["turn"].as_str().unwrap().to_string();
    turn_is(&service, &chat, &anns, &["done"]);
    turn_is(&service, &chat, &bos, &["waiting_sign_in"]);
    service.stop();

    common::sign_in_account(&data.0, away.as_str().unwrap());
    let replay = data.0.join("turns.jsonl");
    let again = TestService::with_engine(&data.0, &[("GATHR_REPLAY", replay.as_path())]);
    turn_is(&again, &chat, &bos, &["done"]);
    reads(&data.0, &bos, "The person sent this to you and Ann.");
}

/// A turn sent again after its engine refused the login reads what it read the first time: Gathr's
/// line before the words, here a reply's naming who else got it.
#[test]
fn a_reply_sent_again_after_a_refused_login_reads_gathrs_line_again() {
    let data = TempDir::new();
    // Each engine's first turn answers; its second meets a refused login.
    let frames = |name: &str| std::fs::read_to_string(common::recording(name)).unwrap();
    let replay = data.0.join("refused.jsonl");
    let both = format!(
        "{}\n{}",
        frames("signed_in").trim_end(),
        frames("signed_out")
    );
    std::fs::write(&replay, both).unwrap();
    let (service, chat, lead) = team(&data.0, &replay, &[]);
    let (ann, bo) = with_bo(&service, &chat, &account_of(&service, &lead));
    let asked = post(
        &service,
        &chat,
        json!({ "text": "Plan the river walk", "agent": ann }),
    );
    let asked = asked.json()["turn"].as_str().unwrap().to_string();
    turn_is(&service, &chat, &asked, &["done"]);
    let shown = history(&service, &chat);
    let mut theirs = shown["messages"].as_array().unwrap().iter();
    let to = theirs
        .rfind(|m| m["turn"] == asked.as_str() && m["role"] == "assistant")
        .unwrap()["id"]
        .clone();

    let words = "@Bo does it pass the old mill?";
    let picks = json!([{ "at": 0, "len": 3, "agent": bo }]);
    let body = json!({ "text": words, "agent": ann, "picks": picks, "reply_to": to });
    let sent = post(&service, &chat, body);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let turn = sent_to(&sent.json(), &ann)["turn"]
        .as_str()
        .unwrap()
        .to_string();
    turn_is(&service, &chat, &turn, &["waiting_sign_in"]);
    let account = account_of(&service, &lead);
    common::sign_in_account(&data.0, account.as_str().unwrap());
    turn_is(&service, &chat, &turn, &["done"]);

    let line = "They sent it to you and Bo.";
    let both = escaped(&format!("{line}\n\n{words}"));
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{turn}' ORDER BY id"
    );
    let read = rows(&data.0, &sql);
    let with_line = read
        .iter()
        .filter(|r| r["body"].as_str().unwrap_or_default().contains(&both));
    assert_eq!(with_line.count(), 2, "each attempt read the line: {read:?}");
}
