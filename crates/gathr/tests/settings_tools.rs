//! Settings tools for agents (0036 C5a): an agent reads and changes Gathr's settings when the person
//! asks, each change recorded with who made it. Never an agent's: answering its own approval or its
//! own question to the person, or changing which pushes ask.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, gathr, held_recording, nth, team, text, until};
use serde_json::{Value, json};

/// Lets the held engines go when the test ends, passing or not.
struct Release(PathBuf);

impl Drop for Release {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "go");
    }
}

/// A team chat with both engines held in a turn: the service, the chat, Lead's and Ann's ids and
/// their engines' tokens.
struct Team {
    // First, so it drops first: the engines are let go before the service stops.
    _release: Release,
    service: TestService,
    chat: String,
    lead: (String, String),
    ann: (String, String),
}

fn started(data: &Path) -> Team {
    let (service, chat, lead) = team(data, &held_recording(data), &[]);
    let release = Release(data.join("go"));
    let members = service.send("GET", &format!("/api/chats/{chat}"), json!(null));
    let ann = members.json()["members"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "member")
        .map(|m| m["agent"].as_str().unwrap().to_string())
        .unwrap();
    let path = format!("/api/chats/{chat}/messages");
    let mut tokens = Vec::new();
    for (n, agent) in [&lead, &ann].into_iter().enumerate() {
        let sent = service.send("POST", &path, json!({ "text": "Hold on", "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
        let started = nth(&data.join("env.jsonl"), n);
        tokens.push(started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string());
    }
    Team {
        _release: release,
        service,
        chat,
        lead: (lead, tokens[0].clone()),
        ann: (ann, tokens[1].clone()),
    }
}

fn tool(t: &Team, token: &str, name: &str, arguments: Value) -> (bool, String) {
    let result = call_tool(t.service.info.port, token, name, arguments);
    (result["isError"] != true, text(&result))
}

fn query(data: &Path, sql: &str) -> Vec<Value> {
    let (ok, answer) = gathr(data, &["diag", "query", sql]);
    assert!(ok, "{answer}");
    answer["rows"].as_array().unwrap().clone()
}

/// A refusal says why and carries the reference that finds it.
fn refused(said: &(bool, String), why: &str) {
    assert!(!said.0, "it went through: {}", said.1);
    assert!(said.1.contains(why), "{}", said.1);
    assert!(said.1.contains("(ref "), "no reference: {}", said.1);
}

#[test]
fn an_agent_reads_the_settings_and_changes_them_as_itself() {
    let data = TempDir::new();
    let t = started(&data.0);
    let (ann, ann_token) = &t.ann;

    let (ok, read) = tool(&t, ann_token, "settings", json!({}));
    assert!(ok, "{read}");
    let read: Value = serde_json::from_str(&read).unwrap();
    assert_eq!(read["general"]["stale_days"], 3, "{read}");
    let names: Vec<&str> = read["agents"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|a| a["name"].as_str())
        .collect();
    assert_eq!(names, ["Ann", "Lead"], "{read}");

    let (ok, said) = tool(&t, ann_token, "set_settings", json!({ "stale_days": 5 }));
    assert!(ok, "{said}");
    let shown = t.service.call("GET", "/api/settings").json();
    assert_eq!(shown["stale_days"], 5, "{shown}");
    let (ok, said) = tool(
        &t,
        ann_token,
        "set_agent",
        json!({ "agent": "Lead", "approvals": "edits" }),
    );
    assert!(ok, "{said}");
    let lead = t.service.call("GET", &format!("/api/agents/{}", t.lead.0));
    assert_eq!(lead.json()["approvals"], "edits", "{}", lead.body);

    let sql = "SELECT name, json_extract(attrs, '$.by') AS by FROM event_log
               WHERE name IN ('gathr.settings.change', 'gathr.agent.change') AND status = 'ok'
               AND json_extract(attrs, '$.by') IS NOT NULL ORDER BY time";
    let rows = query(&data.0, sql);
    let by: Vec<(&str, &str)> = rows
        .iter()
        .map(|r| (r["name"].as_str().unwrap(), r["by"].as_str().unwrap()))
        .collect();
    assert_eq!(
        by,
        [
            ("gathr.settings.change", ann.as_str()),
            ("gathr.agent.change", ann.as_str())
        ],
        "{rows:?}"
    );
}

#[test]
fn an_agent_never_answers_its_own_approval_nor_its_own_question_nor_changes_which_pushes_ask() {
    let data = TempDir::new();
    let t = started(&data.0);
    let ((lead, lead_token), (_, ann_token)) = (&t.lead, &t.ann);

    // Which pushes ask: Full skips the push's ask, so no agent moves anyone into it or out of it.
    let full = json!({ "agent": "Lead", "approvals": "full" });
    refused(&tool(&t, ann_token, "set_agent", full), "which pushes ask");
    let full = json!({ "agent": "Ann", "approvals": "full" });
    refused(&tool(&t, lead_token, "set_agent", full), "which pushes ask");

    // Ann's own push waits on an approval: Ann can't answer it; Lead can.
    let (port, token) = (t.service.info.port, ann_token.clone());
    let pushing = std::thread::spawn(move || {
        let auth = format!("Bearer {token}");
        let headers = [
            ("Authorization", auth.as_str()),
            ("gathr-remote", "elsewhere"),
            ("gathr-url", "https://example.invalid/elsewhere.git"),
        ];
        let refs = "refs/heads/x 1111111111111111111111111111111111111111 refs/heads/x 0000000000000000000000000000000000000000";
        gathr::http::post(port, "/hook/push", &headers, refs).unwrap()
    });
    let history = t.service.wait_for(&t.chat, 20, "Ann's push to ask", |h| {
        h["approvals"].as_array().is_some_and(|a| !a.is_empty())
    });
    let request = history["approvals"][0]["request_id"].as_str().unwrap();
    let answer = json!({ "request_id": request, "allow": false });
    refused(
        &tool(&t, ann_token, "answer_approval", answer.clone()),
        "its own approval",
    );
    let (ok, said) = tool(&t, lead_token, "answer_approval", answer);
    assert!(ok, "{said}");
    assert_eq!(pushing.join().unwrap().body, "deny");

    // Lead's own question to the person: Lead can't answer it; the answer stays the person's.
    let asked = json!({ "what": "Ship it?", "form": "yesno" });
    let (ok, said) = tool(&t, lead_token, "ask_person", asked);
    assert!(ok, "{said}");
    let question = until(10, "the question", || {
        let open = t.service.call("GET", "/api/person-tasks").json();
        open["tasks"][0]["number"].as_i64()
    });
    let answer = json!({ "task": question, "yes": true });
    refused(
        &tool(&t, lead_token, "answer_person", answer),
        "its own question",
    );
    let open = t.service.call("GET", "/api/person-tasks").json();
    assert_eq!(open["tasks"][0]["number"], question, "{open}");
    assert_eq!(open["tasks"][0]["giver"], lead.as_str(), "{open}");

    // Another agent may answer it for the person, and the answer says who did.
    let answer = json!({ "task": question, "yes": true });
    let (ok, said) = tool(&t, ann_token, "answer_person", answer);
    assert!(ok, "{said}");
    let tasks = t
        .service
        .call("GET", &format!("/api/chats/{}/tasks", t.chat))
        .json();
    let answered = tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|task| task["number"] == question)
        .unwrap();
    assert_eq!(answered["state"], "done", "{answered}");
    let reason = answered["reason"].as_str().unwrap_or_default();
    assert!(
        reason.contains("answered by Ann for the person"),
        "{answered}"
    );
}
