//! 0080: the open check. Once per message of the person's, when the lead has answered it and the
//! chat has gone quiet, every free member who took no part checks the answer in a turn of its own; a
//! thumbs-up or silence wakes nobody, words wake the lead once for its last word, and a plan version
//! reaches the person only after both.

use super::*;
use gathr::open_check;
use serde_json::{Value, json};

/// One part: Ann adds the table.
fn one() -> Vec<Part<'static>> {
    vec![part("Ann", "Add the tags table", &[])]
}

/// The round's turns the settle started, as (agent, kind of words, turn it answers).
fn started(starts: &[Start], conn: &Connection) -> Vec<(String, String, Option<String>)> {
    starts
        .iter()
        .map(|s| {
            let kind = samples::opened(conn, &s.turn).0.unwrap_or_default();
            (s.agent.clone(), kind, s.answers.clone())
        })
        .collect()
}

fn settle(conn: &mut Connection) -> Vec<Start> {
    open_check::settle(conn, "c", T).unwrap()
}

/// `agent`'s words in `turn`.
fn says(conn: &Connection, turn: &str, words: &str) -> String {
    let said = gathr::store::add_message(conn, "c", turn, "assistant", "text", words).unwrap();
    said["id"].as_str().unwrap().to_string()
}

/// A turn of `agent`'s in chat `c`, created now, with `status`.
fn turn_of(conn: &Connection, id: &str, agent: &str, status: &str) {
    gathr::store::add_turn(conn, "c", id, T, (Some(agent), None, None), Opened::Person).unwrap();
    set_status(conn, id, status);
}

fn thumbs_up(conn: &Connection, agent: &str, message: &str) {
    gathr::reactions::add(conn, "c", message, agent, T).unwrap();
}

/// Each `gathr.check.round` event's attributes, oldest first.
fn rounds(conn: &Connection) -> Vec<Value> {
    let sql = "SELECT json(attrs) FROM events WHERE name = 'gathr.check.round' ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).unwrap();
    rows.map(|a| serde_json::from_str(&a.unwrap()).unwrap())
        .collect()
}

fn round(asked: i64, agreed: i64, objected: i64, silent: i64, last_word: bool) -> Value {
    json!({ "gathr.check.asked": asked, "gathr.check.agreed": agreed, "gathr.check.objected": objected,
            "gathr.check.silent": silent, "gathr.check.last_word": last_word })
}

/// The attributes of the newest round's event that the check counts, without its ids.
fn counted(conn: &Connection) -> Vec<Value> {
    let keep = [
        "gathr.check.asked",
        "gathr.check.agreed",
        "gathr.check.objected",
        "gathr.check.silent",
        "gathr.check.last_word",
    ];
    rounds(conn)
        .into_iter()
        .map(|mut e| {
            e.as_object_mut()
                .unwrap()
                .retain(|k, _| keep.contains(&k.as_str()));
            e
        })
        .collect()
}

/// The text of the turn a start queued, with the chat's message ids read as `<id>`.
fn text_of(conn: &Connection, start: &Start) -> String {
    let mut stmt = conn.prepare("SELECT id FROM messages").unwrap();
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    ids.iter()
        .fold(start.text.clone(), |t, id| t.replace(id, "<id>"))
}

/// A round begins once per message of the person's, when the lead has answered it and nothing of
/// its ask runs or is owed, and wakes each free member who took no part and nobody else.
#[test]
fn a_round_wakes_each_free_member_who_took_no_part_once_the_lead_has_answered() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    conn.execute_batch(
        "INSERT INTO chat_members VALUES ('c', 'cy', 'member', 4);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES ('dee', 'Dee', 'dee', 'acc', 1);
         INSERT INTO chat_members VALUES ('c', 'dee', 'member', 5);",
    )
    .unwrap();
    // Dee is at work in this chat on something older, so isn't free.
    turn_of(&conn, "D1", "dee", "running");
    conn.execute(
        "UPDATE turns SET created_ts = 0, ask_id = NULL WHERE id = 'D1'",
        [],
    )
    .unwrap();
    let ask = asked(&mut conn, "c", "T2", "Why is the list slow?");
    says(&conn, "T2", "Looking.");
    assert_eq!(settle(&mut conn), [], "the lead is still answering");

    // Ann took part: a turn of hers on the ask since the person wrote.
    turn_of(&conn, "A1", "ann", "done");
    conn.execute("UPDATE turns SET ask_id = ?1 WHERE id = 'A1'", [&ask])
        .unwrap();
    let answer = says(&conn, "T2", "The list reads every note's tags one by one.");
    moved(&mut conn, ("T2", "mira"), None, "done");
    let starts = settle(&mut conn);
    let checkers: Vec<_> = started(&starts, &conn);
    assert_eq!(
        checkers,
        [
            ("bo".into(), "check".into(), Some("T2".into())),
            ("cy".into(), "check".into(), Some("T2".into())),
        ],
        "Bo and Cy, free and no part of it; not Ann, Dee or Mira"
    );
    samples::same("check: an answer", &text_of(&conn, &starts[0]));
    assert!(starts[0].text.contains(&answer), "{}", starts[0].text);
    assert_eq!(
        samples::opened(&conn, &starts[0].turn),
        (Some("check".into()), Some(gathr::words::CHECK))
    );
    assert_eq!(settle(&mut conn), [], "once per message");
}

/// A round needs a lead's answer that ended well, nothing open for the person, and two agents.
#[test]
fn no_round_starts_for_a_failed_answer_a_question_or_a_chat_of_one() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    says(&conn, "T", "Half an answer");
    moved(&mut conn, ("T", "mira"), None, "failed");
    assert_eq!(settle(&mut conn), [], "the answer failed");

    asked(&mut conn, "c", "T2", "Which colour?");
    says(&conn, "T2", "Which colour do you want?");
    let q = gathr::tasks::person::Ask {
        chat: "c",
        giver: "mira",
        given_in: Some("T2"),
        parent: None,
        what: Some("Which colour?"),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    gathr::tasks::person::ask(&mut conn, q, T).unwrap();
    moved(&mut conn, ("T2", "mira"), None, "done");
    assert_eq!(settle(&mut conn), [], "it asked the person");

    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('solo', 'Solo', 1);
         INSERT INTO chat_members VALUES ('solo', 'mira', 'supervisor', 1);",
    )
    .unwrap();
    asked(&mut conn, "solo", "S", "Hello");
    gathr::store::add_message(&conn, "solo", "S", "assistant", "text", "Hi").unwrap();
    set_status(&conn, "S", "done");
    assert_eq!(open_check::settle(&mut conn, "solo", T).unwrap(), []);
    let rounds: i64 = conn
        .query_row("SELECT count(*) FROM open_checks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(rounds, 0);
}

/// Thumbs-ups and silence end the round and wake nobody; its event counts each.
#[test]
fn a_thumbs_up_and_silence_wake_nobody() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let answer = says(&conn, "T", "Done: tags show in the list.");
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    let (ann, bo) = (&starts[0].turn, &starts[1].turn);
    set_status(&conn, ann, "running");
    thumbs_up(&conn, "ann", &answer);
    // Words after a thumbs-up are a closing line, not an objection.
    says(&conn, ann, "Looks right.");
    set_status(&conn, ann, "done");
    assert_eq!(settle(&mut conn), [], "Bo is still checking");
    assert_eq!(rounds(&conn), Vec::<Value>::new());
    set_status(&conn, bo, "done");
    assert_eq!(settle(&mut conn), [], "nobody is woken");
    assert_eq!(counted(&conn), [round(2, 1, 0, 1, false)]);
    let attrs = &rounds(&conn)[0];
    assert_eq!(
        (&attrs["gathr.message.id"], &attrs["gathr.turn.id"]),
        (&json!(answer), &json!("T"))
    );
}

/// Words wake the lead once, with each member's words, for its last word; then the round ends.
#[test]
fn words_wake_the_lead_once_with_each_members_words() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    says(&conn, "T", "Done: tags show in the list.");
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    let (ann, bo) = (starts[0].turn.clone(), starts[1].turn.clone());
    says(
        &conn,
        &ann,
        "> Mira: Done: tags show in the list.\n\nThe list shows them, the filter doesn't.",
    );
    says(&conn, &bo, "No test covers a note with no tags.");
    set_status(&conn, &ann, "done");
    set_status(&conn, &bo, "done");
    let woken = settle(&mut conn);
    assert_eq!(
        started(&woken, &conn),
        [("mira".into(), "last_word".into(), Some("T".into()))]
    );
    samples::same("last_word: an answer", &text_of(&conn, &woken[0]));
    assert_eq!(settle(&mut conn), [], "woken once");
    assert_eq!(
        rounds(&conn),
        Vec::<Value>::new(),
        "not over until it answers"
    );
    set_status(&conn, &woken[0].turn, "done");
    assert_eq!(settle(&mut conn), []);
    assert_eq!(counted(&conn), [round(2, 0, 2, 0, true)]);
}

/// A plan reaches the person only once its round and the lead's last word have ended; a version the
/// last word writes is the one offered, with no second round.
#[test]
fn a_plan_is_offered_after_its_round_and_the_last_word_the_newest_version() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &one()).unwrap();
    assert_eq!(
        wrote.answer,
        "Wrote version 1 of plan #1, \"Tags on notes\": 1 part. The members who took no part check it once this turn \
         ends; the person sees it after."
    );
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    samples::same("check: a plan", &text_of(&conn, &starts[0]));
    let (ann, bo) = (starts[0].turn.clone(), starts[1].turn.clone());
    let refused = plans::approve(&mut conn, wrote.plan, 1, T);
    assert_eq!(refusal(refused), "Version 1 is still being checked.");

    let message: String = version_field(&conn, wrote.version, "message_id");
    thumbs_up(&conn, "ann", &message);
    set_status(&conn, &ann, "done");
    says(
        &conn,
        &bo,
        "> Mira: 1. Add the tags table (Ann)\n\nAnn has no index on the note column.",
    );
    set_status(&conn, &bo, "done");
    let woken = settle(&mut conn);
    samples::same("last_word: a plan", &text_of(&conn, &woken[0]));
    let last = woken[0].turn.clone();
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert_eq!(sent, None, "the last word runs first");
    assert_eq!(plan_state(&conn, "c", wrote.plan), plans::State::Checking);

    set_status(&conn, &last, "running");
    let mut indexed = one();
    indexed[0].what = "Add the tags table, indexed";
    let newer = write(&mut conn, Some(&last), &indexed).unwrap();
    assert_eq!(
        newer.answer,
        "Wrote version 2 of plan #1, \"Tags on notes\": 1 part. The person sees it when this turn ends."
    );
    moved(&mut conn, (&last, "mira"), None, "done");
    assert_eq!(settle(&mut conn), [], "no second round");
    let (first, second): (Option<i64>, Option<i64>) = (
        version_field(&conn, wrote.version, "sent_ts"),
        version_field(&conn, newer.version, "sent_ts"),
    );
    assert_eq!((first.is_some(), second.is_some()), (false, true));
    assert_eq!(
        plans::open_state(&conn, &ask).unwrap(),
        Some(plans::State::Waiting)
    );
    assert_eq!(
        lines(&conn, "c"),
        Vec::<String>::new(),
        "Gathr says no line"
    );
    assert_eq!(counted(&conn), [round(2, 1, 1, 0, true)]);
}

/// A plan with nobody free to check it reaches the person as the lead's turn ends.
#[test]
fn a_plan_nobody_can_check_reaches_the_person_at_once() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    turn_of(&conn, "A0", "ann", "running");
    turn_of(&conn, "B0", "bo", "queued");
    conn.execute(
        "UPDATE turns SET created_ts = 0 WHERE id IN ('A0', 'B0')",
        [],
    )
    .unwrap();
    let wrote = write(&mut conn, Some("T"), &one()).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    assert_eq!(settle(&mut conn), []);
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert!(sent.is_some());
    assert_eq!(counted(&conn), [round(0, 0, 0, 0, false)]);
}

/// Ten minutes after a version, it reaches the person though its round still runs, once.
#[test]
fn a_version_still_being_checked_after_ten_minutes_reaches_the_person() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &one()).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    set_status(&conn, &starts[0].turn, "done");
    set_status(&conn, &starts[1].turn, "running");
    let written: i64 = version_field(&conn, wrote.version, "created_ts");
    let ten = 600_000_000;
    assert_eq!(
        plans::overdue(&mut conn, "c", written + ten - 1_000_000, ten, T).unwrap(),
        0
    );
    assert_eq!(
        plans::overdue(&mut conn, "c", written + ten, ten, T).unwrap(),
        1
    );
    let late = "Bo didn't finish checking version 1 of the plan \"Tags on notes\".";
    assert_eq!(lines(&conn, "c"), [late]);
    let sent: Option<i64> = version_field(&conn, wrote.version, "sent_ts");
    assert!(sent.is_some());
    let checked = events(&conn, "checked");
    assert_eq!(
        checked
            .last()
            .map(|e| (&e["gathr.plan.outcome"], &e["gathr.plan.sent"])),
        Some((&json!("late"), &json!(true)))
    );
    assert_eq!(
        plans::overdue(&mut conn, "c", written + 2 * ten, ten, T).unwrap(),
        0
    );
    set_status(&conn, &starts[1].turn, "done");
    assert_eq!(settle(&mut conn), []);
    assert_eq!(lines(&conn, "c"), [late], "the line isn't said twice");
}

fn topic(conn: &Connection, turn: &str) -> Option<String> {
    gathr::topics::of_turn(conn, "c", turn).unwrap()
}

/// Where each turn of a round sits (0075): in the thread of the answer it checks, the lead's last
/// word too, unless the last word writes a version, which is a main message of its own.
#[test]
fn a_rounds_turns_sit_in_the_thread_of_the_answer() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    says(&conn, "T", "Done: tags show in the list.");
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    for s in &starts {
        says(&conn, &s.turn, "The filter is missing.");
        set_status(&conn, &s.turn, "done");
    }
    let last = settle(&mut conn)[0].turn.clone();
    let all = [&starts[0].turn, &starts[1].turn, &last];
    let topics: Vec<_> = all.iter().map(|t| topic(&conn, t)).collect();
    assert_eq!(topics, vec![Some("b:T".to_string()); 3]);
    let facts = gathr::topics::Topics::of_chat(&conn, "c").unwrap();
    let facts: Vec<_> = all.iter().map(|t| facts.turn(t).unwrap().fact).collect();
    assert_eq!(facts, ["check", "check", "last_word"]);

    // A last word that writes the next version: the version is its own main message.
    asked(&mut conn, "c", "U", "Plan the export");
    write(&mut conn, Some("U"), &one()).unwrap();
    moved(&mut conn, ("U", "mira"), None, "done");
    for s in settle(&mut conn) {
        says(&conn, &s.turn, "Part 1 is the wrong member's.");
        set_status(&conn, &s.turn, "done");
    }
    let last = settle(&mut conn)[0].turn.clone();
    set_status(&conn, &last, "running");
    let mut changed = one();
    changed[0].to = "Bo";
    write(&mut conn, Some(&last), &changed).unwrap();
    assert_eq!(topic(&conn, &last), Some(format!("b:{last}")));
}

/// A plan checked the old way (0062) still reads: its check task, its findings and Gathr's line.
#[test]
fn a_plan_checked_the_old_way_still_reads() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &one()).unwrap();
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, finish, finish_ts, created_ts, updated_ts,
                            given_in, checks, in_passing)
         VALUES (5, 'c', 'mira', 'bo', 'review', 'Check plan v1: Tags on notes', 'blocked', 'left', 1, 1, 1, 'T', ?1, 1)",
        [wrote.version],
    )
    .unwrap();
    let message: String = version_field(&conn, wrote.version, "message_id");
    conn.execute(
        "INSERT INTO review_notes (task, reviewed, kind, file, says, created_ts) VALUES (5, ?1, 'wrong', NULL, 'Misses a caller', 1)",
        [&message],
    )
    .unwrap();
    let line = gathr::store::say(
        &conn,
        "c",
        "Bo has 1 finding on version 1 of the plan \"Tags on notes\".",
    )
    .unwrap();
    conn.execute(
        "UPDATE plan_versions SET check_line = ?2 WHERE id = ?1",
        params![wrote.version, line["id"].as_str()],
    )
    .unwrap();
    let plan = plans::get(&conn, wrote.plan).unwrap().unwrap();
    let check = &plan["versions"][0]["check"];
    assert_eq!(
        (
            &check["outcome"],
            &check["notes"][0]["says"],
            &plan["state"]
        ),
        (
            &json!("findings"),
            &json!("Misses a caller"),
            &json!("revising")
        )
    );
    let topics = gathr::topics::Topics::of_chat(&conn, "c").unwrap();
    assert_eq!(topics.task(5).map(|t| t.key), Some("b:T".to_string()));
    let said = topics.line(&topics.src.lines[0]);
    assert_eq!(said.key, "b:T");
}

/// An upgrade starts no round for a message the person sent before it: migration 0060 counts each
/// as had, so a quiet team chat isn't checked days after its answer.
#[test]
fn an_upgrade_starts_no_round_for_an_older_message() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    says(&conn, "T", "Done: tags show in the list.");
    moved(&mut conn, ("T", "mira"), None, "done");
    conn.execute_batch("DROP TABLE open_checks; PRAGMA user_version = 59;")
        .unwrap();
    drop(conn);
    let mut conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    assert_eq!(settle(&mut conn), []);
    let rounds: i64 = conn
        .query_row(
            "SELECT count(*) FROM open_checks WHERE answer IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(rounds, 1, "the old message counts as had");
}

/// A check that names the lead with @Name leaves the lead to its last word, which carries those
/// words (0081 meets 0080): the mention wakes only the other member it names.
#[test]
fn a_check_that_names_the_lead_wakes_it_once_by_its_last_word() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    says(&conn, "T", "Done: tags show in the list.");
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    let ann = starts[0].turn.clone();
    says(
        &conn,
        &ann,
        "@Mira the filter doesn't show them. @Bo, you wrote the filter.",
    );
    set_status(&conn, &ann, "done");
    let named: Vec<String> = gathr::handoffs::mentions(&conn, &ann, T)
        .unwrap()
        .named
        .into_iter()
        .map(|n| n.to)
        .collect();
    assert_eq!(named, ["bo"], "Mira hears it in her last word");
}

/// What a round's turn is handed in its own words, it doesn't hear again as the chat's lines: a check
/// its answer and the person's message, the last word the members' checks.
#[test]
fn a_rounds_turns_hear_nothing_twice() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let answer = "Done: tags show in the list.";
    says(&conn, "T", answer);
    moved(&mut conn, ("T", "mira"), None, "done");
    let starts = settle(&mut conn);
    let (ann, bo) = (starts[0].turn.clone(), starts[1].turn.clone());
    assert!(starts[0].text.contains(answer), "{}", starts[0].text);
    let asked: String = conn
        .query_row(
            "SELECT m.body FROM messages m JOIN open_checks c ON c.asked = m.id",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let heard = |conn: &Connection, turn: &str, agent: &str| {
        let heard = gathr::room::catch_up(conn, "c", turn, agent, true).unwrap();
        heard.map(|h| h.text).unwrap_or_default()
    };
    let lines = heard(&conn, &ann, "ann");
    assert!(!lines.contains(answer), "the answer, twice: {lines}");
    assert!(!lines.contains(&asked), "the ask, twice: {lines}");

    let objection = "No test covers a note with no tags.";
    says(&conn, &ann, objection);
    set_status(&conn, &ann, "done");
    set_status(&conn, &bo, "done");
    let woken = settle(&mut conn);
    assert!(woken[0].text.contains(objection), "{}", woken[0].text);
    let lines = heard(&conn, &woken[0].turn, "mira");
    assert!(!lines.contains(objection), "the check, twice: {lines}");
}
