//! Which of Gathr's words opened each turn (Stage 3, part A): each kind's version is held by the kept
//! samples of its text, every turn records its kind and version, and `gathr diag words` counts each
//! kind's outcome. Scenario 11e.

mod common;
#[path = "words/samples.rs"]
mod samples;

use std::path::Path;

use common::{TempDir, call_tool, gathr, nth, team, text};
use gathr::words::Opened;
use serde_json::{Value, json};

/// `gathr --data <data> --text <args>`: what it prints.
fn in_text(data: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_gathr"))
        .arg("--data")
        .arg(data)
        .arg("--text")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

/// Scenario 11e: the person writes, the lead gives Ann a task, Ann's turn ends with no report after
/// a report turned away, and Gathr asks once. Each turn records the words that opened it, and
/// `gathr diag words` counts each kind's turns and outcomes.
#[test]
fn turns_record_the_words_that_opened_them_and_diag_counts_each_kind() {
    let data = TempDir::new();
    // Each engine runs two turns: the first held until `go` exists.
    let first = std::fs::read_to_string(common::held_recording(&data.0, "signed_in")).unwrap();
    let second = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let twice = data.0.join("twice.jsonl");
    std::fs::write(&twice, format!("{}\n{second}", first.trim_end())).unwrap();
    let (service, chat, _) = team(&data.0, &twice, &[]);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let env = data.0.join("env.jsonl");
    let lead = nth(&env, 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let port = service.info.port;
    common::fix_it(port, &lead);
    let ask = common::briefed(json!({ "to": "ann", "what": "Add a retry" }));
    let given = text(&call_tool(port, &lead, "give_task", ask));
    assert!(given.starts_with("Task #1 for ann: ready."), "{given}");
    service.wait_for(&chat, 20, "Ann's turn on #1 to run", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns
            .iter()
            .any(|t| t["task"] == 1 && t["status"] == "running")
    });
    let ann = nth(&env, 1)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let bare = text(&call_tool(
        port,
        &ann,
        "finish_task",
        json!({ "how": "done" }),
    ));
    let refused = "finish_task failed: Say what to look at when work is done";
    assert!(bare.starts_with(refused), "{bare}");
    let reference = bare
        .rsplit_once("(ref ")
        .map(|(_, r)| r.trim_end_matches(')'));
    let reference = reference
        .unwrap_or_else(|| panic!("no reference: {bare}"))
        .to_string();
    std::fs::write(data.0.join("go"), "").unwrap();
    service.wait_for(&chat, 20, "the hear-back", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns.len() == 4 && turns.iter().all(|t| t["status"] == "done")
    });

    let (ok, counted) = gathr(&data.0, &["diag", "words", "--since", "1h"]);
    assert!(ok, "{counted}");
    let row = |kind: &str, version: Value, n: [i64; 4]| {
        json!({ "kind": kind, "version": version, "turns": n[0], "no_tool": n[1], "finish_refused": n[2],
                "no_report": n[3] })
    };
    // The versions come from the code, so raising one changes nothing here.
    let (brief, hear_back, report_ask) = (
        gathr::words::BRIEF,
        gathr::words::HEAR_BACK,
        gathr::words::REPORT_ASK,
    );
    let rows = [
        row("brief", json!(brief), [1, 1, 1, 1]),
        row("hear_back", json!(hear_back), [1, 1, 0, 0]),
        row("person", Value::Null, [1, 1, 0, 0]),
        row("report_ask", json!(report_ask), [1, 1, 0, 1]),
    ];
    assert_eq!(counted["kinds"], json!(rows), "{counted}");

    let (ok, turned) = gathr(&data.0, &["diag", "turn", &reference]);
    assert!(ok, "{turned}");
    let turns = turned["turns"].as_array().unwrap();
    assert_eq!(turns.len(), 1, "{turned}");
    let opened = (&turns[0]["opened_by"], &turns[0]["opened_version"]);
    assert_eq!(opened, (&json!("brief"), &json!(brief)), "{turned}");

    let shown = in_text(&data.0, &["diag", "words", "--since", "1h"]);
    let lines: Vec<&str> = shown.lines().collect();
    assert!(
        lines[0].starts_with("Turns since ")
            && lines[0].ends_with(", by the words that opened them"),
        "{shown}"
    );
    assert_eq!(
        lines[1], "kind        version  turns  no tool  finish refused  no report",
        "{shown}"
    );
    let said = format!("brief       {brief:>7}      1        1               1          1");
    assert_eq!(lines[2], said, "{shown}");
    assert_eq!(
        lines[4], "person            -      1        1               0          0",
        "{shown}"
    );
}

/// A database from before turns recorded their words still reads: `diag turn` leaves the two
/// fields out, and `diag words` says nothing is recorded there.
#[test]
fn diag_reads_a_database_from_before_the_turns_recorded_their_words() {
    let data = TempDir::new();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "ALTER TABLE turns DROP COLUMN opened_by;
         ALTER TABLE turns DROP COLUMN opened_version;
         PRAGMA user_version = 42;
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES ('t', 'c', 'abcdef0123456789', 'done', 1);
         INSERT INTO events (id, ts, name, phase, trace_id) VALUES ('e', 1, 'gathr.turn', 'point', 'abcdef0123456789');",
    )
    .unwrap();
    drop(conn);
    let (ok, turned) = gathr(&data.0, &["diag", "turn", "abcdef01"]);
    assert!(ok, "{turned}");
    assert_eq!(
        turned["turns"],
        json!([{ "id": "t", "status": "done", "why": null }])
    );
    let (ok, counted) = gathr(&data.0, &["diag", "words"]);
    assert!(ok, "{counted}");
    let said = "Nothing is recorded before schema 43. This database is at schema 42.";
    assert_eq!(
        (&counted["kinds"], &counted["schema"], &counted["in_words"]),
        (&json!([]), &json!(42), &json!(said))
    );
}

/// The replayed engine in the scenario above calls no tool, so a turn that did is made here: it
/// isn't counted as one with no tool call.
#[test]
fn a_turn_that_called_a_tool_is_not_counted_as_one_with_none() {
    let data = TempDir::new();
    let conn = gathr::db::open(&data.0.join("gathr.db")).unwrap();
    conn.execute(
        "INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1)",
        [],
    )
    .unwrap();
    for turn in ["tooled", "talked"] {
        gathr::store::add_turn(&conn, "c", turn, turn, (None, None, None), Opened::Person).unwrap();
        gathr::store::set_turn(&conn, turn, "done", None).unwrap();
    }
    let tool = r#"{"name":"Read","status":"ok"}"#;
    gathr::store::add_message(&conn, "c", "tooled", "assistant", "tool", tool).unwrap();
    let counted = gathr::words::counted(&conn, 0).unwrap();
    let person = gathr::words::Counted {
        kind: "person".into(),
        version: None,
        turns: 2,
        no_tool: 1,
        finish_refused: 0,
        no_report: 0,
    };
    assert_eq!(counted, [person]);
}
