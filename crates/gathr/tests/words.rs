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

/// (kind, version, hash of that version's samples). Rows are only ever added: an edited or removed
/// row is a review finding.
const KEPT: &[(&str, i64, &str)] = &[
    (
        "brief",
        1,
        "ae3396d9e478c5ca1271bc733a725070ef1309fc1e2ad11f727fe1f06a253960",
    ),
    (
        "brief",
        2,
        "7f380f7d6e856f440308e8b771f53b3f246dc7d5e9155407538c73c84096ecc1",
    ),
    (
        "review_brief",
        1,
        "e669125e4d03cdde446a93940ef509a65c0a869dbb4186ce78b8284754265bd4",
    ),
    (
        "request_brief",
        1,
        "eb1881cfb324a2c433b6246eae2d321554cccd5ddd541c21ac693bede700310b",
    ),
    (
        "request_brief",
        2,
        "e953eec83c9f42a437e817afd524a85d9329f4bb1aacbdf8fd2193510c8a9b8d",
    ),
    (
        "report_ask",
        1,
        "63f9511fb29f42bb19ac806963220da7f5ae9bab0e58591c0ac271ab437403fe",
    ),
    (
        "hear_back",
        1,
        "8946f889d56b743fea86f1cfc5e61c8b33e3816cbe6ec17319157dc667336fa8",
    ),
    (
        "answered",
        1,
        "bf72ea9b5350d9857006f2df029fb0da378a80782ab02b8cb5374c74ccb33bc7",
    ),
    (
        "answered",
        2,
        "e89abe05b16499c815f0b11335636a900d9704a51b6adfbcd02bd64cbd2b7ea3",
    ),
    (
        "wake",
        1,
        "b95544be05abcd329423d20e52e3a88d044d3f4f1346b316cd4e4804be7c31e9",
    ),
    (
        "wake",
        2,
        "6dbf06ca487691e3b49790ae9e6791047483b7b0138964eefbe67853b7c56545",
    ),
    (
        "silent",
        1,
        "2ae5593c12f7ecbf7cd1b8b051e2168b0bfd02032b937e01642a6398f31a1921",
    ),
    (
        "wait_fired",
        1,
        "1b0a02a5395a3db048b51dd425c2b3ef2b29bfbb06c9ef543ba8b156cdc08b8b",
    ),
    (
        "stale",
        1,
        "048d7662081243f24c4f15effc265badeb27eacfbf53eab592d181a45cbb1c1f",
    ),
    (
        "landing",
        1,
        "dfa3d9ae21cb108ef454cec42c83501aaef1cf67534293f6255686227a8d5039",
    ),
    (
        "carry_on",
        1,
        "aa923edc4ff88aebeb7bf6708fb0908c027f303219d99dbafe68398e69b1bd55",
    ),
    (
        "carry_on",
        2,
        "2f897d21873137a2ae85e73a8c137d8cc60bef6871e1fba990180fb04dc1a5e5",
    ),
    (
        "hear_back",
        2,
        "0d809789abf8f9349065c0aac9cf2e947869f5c78587c3cd47aa68975dccb5f0",
    ),
    (
        "plan_check",
        1,
        "e488746765bf295adadcc51023fc04969c9fc4c156f52c1489bcf66d8c360943",
    ),
    (
        "sent_back",
        1,
        "074479489631eb5291d177966d7c325a7af4b1e9ba82d751008e9ed3e6b47dba",
    ),
    (
        "brief",
        3,
        "3f28e84f1f4269ac6794ee6b5b6bc07916d977a673327545609bd3b8064df2f9",
    ),
    (
        "resized",
        1,
        "c453e3a17f6b08ccb7b60b53a1c9f7c5d9381a0090b2109e98a7a7a8eb6aa9fb",
    ),
    (
        "role",
        1,
        "0cb35e37d095928928ad396191fdd7c8eff8a4c00e1a333cd1c5b57d3bb95354",
    ),
    (
        "liked",
        1,
        "ed00c98abadfd2812437dd5e5472d022166eca2cbe6015318ad2e072f96a8469",
    ),
    (
        "reply",
        1,
        "43ae8394759a1c60c042e051c72d17c82d76e0d3ce375989a0ecabc474125687",
    ),
    (
        "reply",
        2,
        "13d5121c412b139b1987d3114f74b970a7c84dfc9faa026ca222550b0e41006c",
    ),
    (
        "picked",
        1,
        "85ae84d5185f8c0dfe843cff0918cc4beac470ae3aafe9e2c4ce48ae88200db0",
    ),
    (
        "hear_back",
        3,
        "8946f889d56b743fea86f1cfc5e61c8b33e3816cbe6ec17319157dc667336fa8",
    ),
    (
        "hear_back",
        4,
        "7ef9b6ebec5c9c754ee2f8d226d97d436234db9d028e4dab6ca7cd053d563e06",
    ),
    (
        "role",
        2,
        "4d7a91ab2650d7e8842e4655a884a90b4758e6fff2f976174b18a2fa7a3b8df4",
    ),
    (
        "check",
        1,
        "757d03478711bd3a2bedd1c55876542889253142eb006211ff1a30ca20302471",
    ),
    (
        "last_word",
        1,
        "a57edb981d9f7a821156c834bb42398b3cf6d7918fd21ad5e810cae25440f6ae",
    ),
    (
        "check",
        2,
        "72fa2db387d90be17ce050d8b3184cbb66c7eed24bc93a73d33be8d74cf96407",
    ),
    (
        "last_word",
        2,
        "6bed4d345a91141b49f1028daf788459ab4a23e316be45b3b5b360b91fbff28b",
    ),
];
/// Every file that asserts samples.
const SOURCES: &[&str] = &[
    include_str!("tasks/words.rs"),
    include_str!("quiet/words.rs"),
    include_str!("stale.rs"),
    include_str!("merge/lands.rs"),
    include_str!("app_update.rs"),
    include_str!("pauses.rs"),
    include_str!("restart_room.rs"),
    include_str!("plans/write.rs"),
    include_str!("plans/check.rs"),
    include_str!("plans/decide.rs"),
    include_str!("hold/give.rs"),
    include_str!("hold/wait.rs"),
    include_str!("roles.rs"),
    include_str!("likes.rs"),
    include_str!("replies.rs"),
    include_str!("picked.rs"),
];

/// The lowercase hex SHA-256 of a kind's sample texts, in order, each followed by a NUL byte.
fn hash(kind: &str, samples: &[(&str, &str, &str)]) -> String {
    let mut bytes = Vec::new();
    for (_, _, text) in samples.iter().filter(|s| s.0 == kind) {
        bytes.extend_from_slice(text.as_bytes());
        bytes.push(0);
    }
    gathr::fetch::sha256(&bytes)
}

/// Each way the kinds, their samples and the kept hashes disagree, as a message saying what to do.
fn check(
    kinds: &[(&str, Option<i64>)],
    samples: &[(&str, &str, &str)],
    kept: &[(&str, i64, &str)],
    sources: &[&str],
) -> Vec<String> {
    let mut failures = Vec::new();
    for &(kind, version) in kinds {
        let sampled = samples.iter().any(|s| s.0 == kind);
        let rows: Vec<&(&str, i64, &str)> = kept.iter().filter(|k| k.0 == kind).collect();
        let Some(v) = version else {
            if sampled || !rows.is_empty() {
                failures.push(format!(
                    "{kind} has no version, so it takes no samples or kept rows: remove them, or give it a \
                     version in crates/gathr/src/words.rs."
                ));
            }
            continue;
        };
        if !sampled {
            failures.push(format!(
                "{kind} has fixed text and no sample: add one sample for each of its main forms to \
                 tests/words/samples.rs (the list in part A's decision)."
            ));
            continue;
        }
        let (hash, upper) = (hash(kind, samples), kind.to_uppercase());
        let highest = rows.iter().map(|k| k.1).max();
        match (highest, rows.iter().find(|k| k.1 == v)) {
            (Some(w), _) if w > v => failures.push(format!(
                "{kind} is at version {v}, but version {w} is kept: a version only goes up. Set {upper} to {} \
                 and add (\"{kind}\", {}, \"{hash}\") to KEPT.",
                w + 1,
                w + 1
            )),
            (_, None) => failures.push(format!(
                "{kind} is at version {v}, which has no kept row: add (\"{kind}\", {v}, \"{hash}\") to KEPT in \
                 crates/gathr/tests/words.rs."
            )),
            (_, Some(row)) if row.2 != hash => failures.push(format!(
                "{kind}'s samples changed since version {v} was kept: set {upper} to {} in \
                 crates/gathr/src/words.rs and add (\"{kind}\", {}, \"{hash}\") to KEPT in \
                 crates/gathr/tests/words.rs. Never edit a kept row.",
                v + 1,
                v + 1
            )),
            _ => {}
        }
    }
    for (i, (_, name, _)) in samples.iter().enumerate() {
        if samples[..i].iter().any(|s| s.1 == *name) {
            failures.push(format!("two samples are named \"{name}\"."));
        }
        let quoted = format!("\"{name}\"");
        if !sources.iter().any(|source| source.contains(&quoted)) {
            failures.push(format!(
                "sample \"{name}\" is asserted by no test: assert it with samples::same where the real code \
                 builds its text."
            ));
        }
    }
    failures
}

#[test]
fn each_kinds_samples_match_the_hash_kept_for_its_version() {
    let mut kinds: Vec<(&str, Option<i64>)> = Opened::ALL
        .iter()
        .map(|o| (o.as_str(), o.version()))
        .collect();
    // A team's role lines open no turn, but their words are versioned the same way (0066).
    kinds.push(("role", Some(gathr::words::ROLE)));
    let failures = check(&kinds, samples::SAMPLES, KEPT, SOURCES);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn a_version_check_names_each_way_the_samples_and_their_hashes_disagree() {
    // The SHA-256 of "x\0" and of "x\0y\0", worked out apart from Gathr.
    let x = "14f825b2bbc32dd8d196367fa8776873069c12a8954d8da7513aa7704ddd09eb";
    let xy = "7055937302e30e9db629cb0b9f15a885c9b667a84842b5bb9ad716caa128cc5b";
    let one = [("a", "a: one", "x")];
    let asserted = ["samples::same(\"a: one\", built)"];
    let a1 = [("a", Some(1))];
    assert_eq!(
        check(&a1, &one, &[("a", 1, x)], &asserted),
        Vec::<String>::new()
    );

    let changed = check(&a1, &one, &[("a", 1, "0123")], &asserted);
    let said = format!(
        "a's samples changed since version 1 was kept: set A to 2 in crates/gathr/src/words.rs and add \
         (\"a\", 2, \"{x}\") to KEPT in crates/gathr/tests/words.rs. Never edit a kept row."
    );
    assert_eq!(changed, [said]);

    let unkept = check(&a1, &one, &[], &asserted);
    let said = format!(
        "a is at version 1, which has no kept row: add (\"a\", 1, \"{x}\") to KEPT in crates/gathr/tests/words.rs."
    );
    assert_eq!(unkept, [said]);

    let lowered = check(&a1, &one, &[("a", 1, x), ("a", 2, "0123")], &asserted);
    let said = format!(
        "a is at version 1, but version 2 is kept: a version only goes up. Set A to 3 and add (\"a\", 3, \"{x}\") to KEPT."
    );
    assert_eq!(lowered, [said]);

    let unsampled = check(&a1, &[], &[], &asserted);
    let said = "a has fixed text and no sample: add one sample for each of its main forms to tests/words/samples.rs \
                (the list in part A's decision).";
    assert_eq!(unsampled, [said]);

    let p = [("p", None)];
    let sampled = check(&p, &[("p", "a: one", "x")], &[], &asserted);
    let said = "p has no version, so it takes no samples or kept rows: remove them, or give it a version in \
                crates/gathr/src/words.rs.";
    assert_eq!(sampled, [said]);

    let unasserted = check(
        &a1,
        &one,
        &[("a", 1, x)],
        &["samples::same(\"a: two\", built)"],
    );
    let said = "sample \"a: one\" is asserted by no test: assert it with samples::same where the real code builds its text.";
    assert_eq!(unasserted, [said]);

    let twice = [("a", "a: one", "x"), ("a", "a: one", "y")];
    let named_twice = check(&a1, &twice, &[("a", 1, xy)], &asserted);
    assert_eq!(named_twice, ["two samples are named \"a: one\"."]);
}

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
