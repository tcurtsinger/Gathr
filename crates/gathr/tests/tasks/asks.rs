//! 0061, Stage 3 part G: every message of the person's in a team chat starts an ask or joins one;
//! every turn and task serves the ask it inherits; the lead sizes an ask (or splits a message off),
//! the person changes the size, and a repair whose work is in is said "Fixed" once.

use super::*;
use gathr::asks::{self, Sizing};
use gathr::changes::{self, Measure, count::Count};
use gathr::tasks::person::{self, Answer};
use gathr::words::Opened;

const EXPORT: &str = "The export button does nothing";

/// The person's message in a new turn of `agent`'s, saved as `send` saves them: its id.
fn sent(conn: &mut Connection, (chat, agent): (&str, &str), turn: &str, text: &str) -> String {
    let tx = conn.transaction().unwrap();
    gathr::store::add_turn(
        &tx,
        chat,
        turn,
        T,
        (Some(agent), None, None),
        Opened::Person,
    )
    .unwrap();
    let message = gathr::store::add_message(&tx, chat, turn, "user", "text", text).unwrap();
    tx.commit().unwrap();
    message["id"].as_str().unwrap().to_string()
}

/// The person's message sent into `turn` while it runs: its id.
fn steered(conn: &mut Connection, turn: &str, text: &str) -> String {
    let chat: String = conn
        .query_row("SELECT chat_id FROM turns WHERE id = ?1", [turn], |r| {
            r.get(0)
        })
        .unwrap();
    let tx = conn.transaction().unwrap();
    let message = gathr::store::add_message(&tx, &chat, turn, "user", "text", text).unwrap();
    tx.commit().unwrap();
    message["id"].as_str().unwrap().to_string()
}

fn turn_ask(conn: &Connection, turn: &str) -> Option<String> {
    let sql = "SELECT ask_id FROM turns WHERE id = ?1";
    conn.query_row(sql, [turn], |r| r.get(0)).unwrap()
}

fn task_ask(conn: &Connection, task: i64) -> Option<String> {
    let sql = "SELECT ask_id FROM tasks WHERE number = ?1";
    conn.query_row(sql, [task], |r| r.get(0)).unwrap()
}

fn status(conn: &Connection, turn: &str, status: &str) {
    let sql = "UPDATE turns SET status = ?2 WHERE id = ?1";
    conn.execute(sql, [turn, status]).unwrap();
}

/// The queued turn on `task`.
fn queued_on(conn: &Connection, task: i64) -> String {
    let sql =
        "SELECT id FROM turns WHERE task = ?1 AND status = 'queued' ORDER BY rowid DESC LIMIT 1";
    conn.query_row(sql, [task], |r| r.get(0)).unwrap()
}

/// Each `gathr.ask.<name>` event's attributes, oldest first.
fn ask_events(conn: &Connection, name: &str) -> Vec<serde_json::Value> {
    let sql = "SELECT json(attrs) FROM events WHERE name = ?1 ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([format!("gathr.ask.{name}")], |r| r.get::<_, String>(0));
    let rows = rows.unwrap().map(Result::unwrap);
    rows.map(|a| serde_json::from_str(&a).unwrap()).collect()
}

fn gives(
    conn: &mut Connection,
    (giver, owner): (&str, &str),
    given_in: Option<&str>,
    parent: Option<i64>,
) -> i64 {
    let g = Give {
        chat: "c",
        giver: Some(giver),
        owner,
        given_in,
        parent,
        kind: Kind::Work,
        what: "Fix the export",
        brief: common::COMPLETE,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: true,
    };
    tasks::give(conn, &g, T).unwrap().0.number
}

/// The owner reports `task` done and its running turn `turn` ends.
fn done_in(conn: &mut Connection, task: i64, turn: &str) -> Vec<Start> {
    let owner = tasks::get(conn, task).unwrap().unwrap().owner.unwrap();
    let said = Said {
        note: Some("see the change"),
        ..Said::default()
    };
    finish(conn, &owner, task, Finish::Done, &said).unwrap();
    gathr::store::set_turn(conn, turn, "done", None).unwrap();
    tasks::ended(conn, task, Outcome::Ok, T).unwrap()
}

/// Gathr's question on PR `pr` at +3,200 lines, in chat `c` of project `p`: its number.
fn gathrs_question(conn: &mut Connection, pr: i64) -> i64 {
    conn.execute_batch(
        "INSERT OR IGNORE INTO projects (id, name, name_key, folder, created_ts)
             VALUES ('p', 'Site', 'site', 'x', 1);
         UPDATE chats SET project_id = 'p' WHERE id = 'c';",
    )
    .unwrap();
    let top = vec![("src/big.rs".to_string(), 3200)];
    let count = Count {
        added: 3200,
        removed: 0,
        top,
    };
    let m = Measure {
        project: "p".into(),
        chat: "c".into(),
        pr,
        head: "aaaa111122223333".into(),
        count,
        line: 3000,
        by: "landing",
    };
    changes::measured(conn, &m, T).unwrap();
    let sql = "SELECT number FROM tasks WHERE giver IS NULL AND kind = 'person' AND pr = ?1";
    conn.query_row(sql, [pr], |r| r.get(0)).unwrap()
}

/// G1.2 to G1.5, G1.8: each turn and task takes the ask of what it comes from.
#[test]
fn every_turn_and_task_carries_the_ask_it_serves() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let m1 = sent(&mut conn, ("c", "lead"), "p1", EXPORT);
    status(&conn, "p1", "running");
    let one = gives(&mut conn, ("lead", "ann"), Some("p1"), None);
    let brief = queued_on(&conn, one);
    let ann = begin(&mut conn, one);
    assert_eq!(ann, brief, "Ann's turn on #1 is its brief's");
    let two = gives(&mut conn, ("ann", "bo"), Some(&ann), Some(one));
    let bo = begin(&mut conn, two);
    gathr::store::set_turn(&conn, &bo, "done", None).unwrap();
    let report_ask = tasks::ended(&mut conn, two, Outcome::Ok, T).unwrap();
    let report_ask = report_ask
        .iter()
        .find(|s| s.task == Some(two))
        .unwrap()
        .turn
        .clone();
    let bo = begin(&mut conn, two);
    assert_eq!(bo, report_ask);
    done_in(&mut conn, two, &bo);
    gathr::store::set_turn(&conn, &ann, "done", None).unwrap();
    let back = tasks::turn_ended(&mut conn, &ann, T).unwrap();
    let ann_back = back.iter().find(|s| s.agent == "ann").unwrap().turn.clone();
    gathr::store::set_turn(&conn, "p1", "done", None).unwrap();

    let question = person::Ask {
        chat: "c",
        giver: "lead",
        given_in: Some("p1"),
        parent: None,
        what: Some("Which file?"),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    let question = person::ask(&mut conn, question, T).unwrap().0.number;
    conn.execute("UPDATE tasks SET pr = 41 WHERE number = ?1", [one])
        .unwrap();
    let gathrs = gathrs_question(&mut conn, 41);
    let alone = gathrs_question(&mut conn, 42);
    let answer = Answer {
        task: question,
        text: Some("src/export.rs".into()),
        ..Answer::default()
    };
    let sent_back = person::answer_as(&mut conn, &[answer], None, T).unwrap();
    let answered = sent_back.starts[0].turn.clone();
    status(&conn, &answered, "running");
    let after = gives(&mut conn, ("lead", "ann"), Some(&answered), None);
    let listed = tasks::list(&conn, "c").unwrap();
    let round = listed
        .iter()
        .find(|t| t.number == after)
        .unwrap()
        .round
        .clone();
    assert_eq!(
        round.as_deref(),
        Some(answered.as_str()),
        "the answer begins a round"
    );

    let teammate = Start::queue(
        &conn,
        ("c", "bo"),
        (None, Some(&ann)),
        Opened::Teammate,
        "Hi".into(),
    );
    let carried = Start::save(
        &conn,
        ("c", Some("ann")),
        (Some(one), Some(&ann)),
        Opened::CarryOn,
        "Go on",
    );
    let fix = gives(&mut conn, ("bo", "ann"), None, Some(one));
    let fix_brief = queued_on(&conn, fix);
    let wake = Start::queue(
        &conn,
        ("c", "lead"),
        (None, None),
        Opened::Wake,
        "Wake".into(),
    );

    let m1 = Some(m1);
    let turns = [
        ("the person's turn", "p1".to_string(), &m1),
        ("#1's brief", brief, &m1),
        ("the report ask on #2", report_ask, &m1),
        ("Ann's hear-back", ann_back, &m1),
        ("the turn the person's answer queued", answered, &m1),
        ("a teammate answering Ann", teammate.unwrap().turn, &m1),
        ("a carry-on", carried.unwrap().0, &m1),
        ("the brief of a fix given in no turn", fix_brief, &m1),
        ("a wake", wake.unwrap().turn, &None),
    ];
    for (what, turn, want) in turns {
        assert_eq!(&turn_ask(&conn, &turn), want, "{what}");
    }
    let tasks = [
        ("#1", one, &m1),
        ("#2, given in Ann's turn", two, &m1),
        ("the lead's question", question, &m1),
        ("Gathr's question on #1", gathrs, &m1),
        ("a fix given in no turn", fix, &m1),
        ("Gathr's question on no task", alone, &None),
    ];
    for (what, task, want) in tasks {
        assert_eq!(&task_ask(&conn, task), want, "{what}");
    }

    // A message read in a turn of its own takes the ask of the turn it was saved in.
    status(&conn, "p1", "running");
    let more = steered(&mut conn, "p1", "Also the import");
    gathr::store::add_turn(
        &conn,
        "c",
        "read",
        T,
        (Some("lead"), None, None),
        Opened::Person,
    )
    .unwrap();
    gathr::store::move_message(&conn, &more, "read").unwrap();
    assert_eq!(
        turn_ask(&conn, "read"),
        m1,
        "a message moved to its own turn"
    );

    // A teammate's copy takes its sender's ask, not that of the turn it was delivered into.
    let from_ann =
        gathr::store::add_message(&conn, "c", &ann, "assistant", "text", "Bo, look").unwrap();
    conn.execute_batch(
        "INSERT INTO asks (id, chat_id) VALUES ('m9', 'c');
         INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, ask_id)
             VALUES ('bo-m9', 'c', 'x', 'running', 1, 'bo', 'm9');",
    )
    .unwrap();
    let copy = gathr::store::add_message(&conn, "c", "bo-m9", "gathr", "text", "Bo, look").unwrap();
    let copy = copy["id"].as_str().unwrap();
    conn.execute(
        "UPDATE messages SET copy_of = ?2 WHERE id = ?1",
        [copy, from_ann["id"].as_str().unwrap()],
    )
    .unwrap();
    gathr::store::add_turn(
        &conn,
        "c",
        "copy-read",
        T,
        (Some("bo"), None, None),
        Opened::Teammate,
    )
    .unwrap();
    gathr::store::move_message(&conn, copy, "copy-read").unwrap();
    assert_eq!(turn_ask(&conn, "copy-read"), m1, "a teammate's copy, moved");
}

/// A fresh team where ask `m1` began in the lead's turn `p1`, now done: m1's id.
fn one_ask(dir: &TempDir) -> (Connection, String) {
    let mut conn = team(dir);
    let m1 = sent(&mut conn, ("c", "lead"), "p1", EXPORT);
    status(&conn, "p1", "done");
    assert_eq!(
        turn_ask(&conn, "p1").as_ref(),
        Some(&m1),
        "the message starts its ask"
    );
    (conn, m1)
}

/// G1a: open work is an agent's task not done or cancelled, or a turn queued or running.
#[test]
fn a_message_joins_the_ask_with_open_work_and_starts_one_when_none_is_open() {
    let task_states = [
        ("waiting", true),
        ("ready", true),
        ("working", true),
        ("held", true),
        ("blocked", true),
        ("failed", true),
        ("stopped", true),
        ("done", false),
        ("cancelled", false),
    ];
    for (state, joins) in task_states {
        let dir = TempDir::new();
        let (mut conn, m1) = one_ask(&dir);
        conn.execute(
            "INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts, ask_id)
             VALUES ('c', 'lead', 'ann', 'work', 'x', ?1, 1, 1, ?2)",
            [state, &m1],
        )
        .unwrap();
        let m2 = sent(&mut conn, ("c", "lead"), "p2", "Also this");
        let want = if joins { &m1 } else { &m2 };
        assert_eq!(turn_ask(&conn, "p2").as_ref(), Some(want), "a task {state}");
    }
    let dir = TempDir::new();
    let (mut conn, m1) = one_ask(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts, ask_id)
         VALUES ('c', 'lead', 'person', 'Which?', 'ready', 1, 1, ?1)",
        [&m1],
    )
    .unwrap();
    let m2 = sent(&mut conn, ("c", "lead"), "p2", "Also this");
    assert_eq!(
        turn_ask(&conn, "p2"),
        Some(m2),
        "only the person's question is open"
    );

    let turn_states = [
        ("queued", true),
        ("waiting_sign_in", true),
        ("running", true),
        ("done", false),
        ("failed", false),
        ("stopped", false),
    ];
    for (state, joins) in turn_states {
        let dir = TempDir::new();
        let (mut conn, m1) = one_ask(&dir);
        conn.execute(
            "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, ask_id)
             VALUES ('t', 'c', 'x', ?1, 1, 'ann', ?2)",
            [state, &m1],
        )
        .unwrap();
        let m2 = sent(&mut conn, ("c", "lead"), "p2", "Also this");
        let want = if joins { &m1 } else { &m2 };
        assert_eq!(turn_ask(&conn, "p2").as_ref(), Some(want), "a turn {state}");
    }

    // Two asks open: a new turn joins the newer; a message into a turn of the older stays there.
    let dir = TempDir::new();
    let (mut conn, m1) = one_ask(&dir);
    let m3 = sent(&mut conn, ("c", "lead"), "p3", "Add tags to notes");
    assert_eq!(
        turn_ask(&conn, "p3").as_ref(),
        Some(&m3),
        "nothing open: a new ask"
    );
    status(&conn, "p1", "running");
    let m4 = sent(&mut conn, ("c", "ann"), "p4", "And colours");
    assert_eq!(
        turn_ask(&conn, "p4").as_ref(),
        Some(&m3),
        "the newest open ask"
    );
    let m5 = steered(&mut conn, "p1", "It started after the update");
    assert_eq!(turn_ask(&conn, "p1").as_ref(), Some(&m1));
    let joined: Vec<(String, bool)> = ask_events(&conn, "message")
        .iter()
        .map(|e| {
            let message = e["gathr.message.id"]
                .as_str()
                .unwrap_or_default()
                .to_string();
            (message, e["gathr.ask.joined"] == true)
        })
        .collect();
    let want = [
        (m1.clone(), false),
        (m3.clone(), false),
        (m4, true),
        (m5.clone(), true),
    ];
    assert_eq!(joined, want);
    let events = ask_events(&conn, "message");
    let into_m1 = events
        .iter()
        .find(|e| e["gathr.message.id"] == m5.as_str())
        .unwrap();
    assert_eq!(into_m1["gathr.ask.id"], m1.as_str(), "{into_m1:?}");
    assert_eq!(into_m1["gathr.turn.id"], "p1", "{into_m1:?}");

    // A running turn that serves no ask takes the ask of the message sent into it.
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let wake = Start::queue(
        &conn,
        ("c", "lead"),
        (None, None),
        Opened::Wake,
        "Wake".into(),
    )
    .unwrap();
    status(&conn, &wake.turn, "running");
    let m1 = steered(&mut conn, &wake.turn, EXPORT);
    assert_eq!(turn_ask(&conn, &wake.turn), Some(m1));

    // A chat with one agent has no asks.
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('solo', 'Solo', 1);
         INSERT INTO chat_members VALUES ('solo', 'lead', 'supervisor', 1);",
    )
    .unwrap();
    let before = ask_events(&conn, "message").len();
    sent(&mut conn, ("solo", "lead"), "s1", EXPORT);
    let asks: i64 = conn
        .query_row(
            "SELECT count(*) FROM asks WHERE chat_id = 'solo'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((asks, turn_ask(&conn, "s1")), (0, None));
    assert_eq!(ask_events(&conn, "message").len(), before, "no event");
}

fn sizing<'a>(
    agent: &'a str,
    turn: Option<&'a str>,
    size: &'a str,
    message: Option<&'a str>,
) -> Sizing<'a> {
    Sizing {
        chat: "c",
        agent,
        turn,
        size,
        message,
        trace: T,
    }
}

fn ask_row(conn: &Connection, ask: &str) -> (Option<String>, Option<String>, Option<String>) {
    let sql = "SELECT size, sized_by, sized_in FROM asks WHERE id = ?1";
    conn.query_row(sql, [ask], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
}

/// G2.6 to G2.9, and G1.2's hear-back: the lead sizes the ask its turn serves, or splits a
/// message off as an ask of its own; a split moves only the turn the message opened.
#[test]
fn the_lead_sizes_the_ask_its_turn_serves_and_splits_off_a_message() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let m1 = sent(&mut conn, ("c", "lead"), "lead-t", EXPORT);
    status(&conn, "lead-t", "running");
    let said = asks::size(&mut conn, &sizing("lead", Some("lead-t"), "repair", None));
    assert_eq!(said.unwrap().0, format!("Sized \"{EXPORT}\" as repair."));
    let row = ask_row(&conn, &m1);
    assert_eq!(
        row,
        (
            Some("repair".into()),
            Some("lead".into()),
            Some("lead-t".into())
        )
    );
    let again = asks::size(&mut conn, &sizing("lead", Some("lead-t"), "repair", None));
    assert_eq!(again.unwrap().0, format!("\"{EXPORT}\" is already repair."));
    let resized = ask_events(&conn, "resized");
    assert_eq!(resized.len(), 1, "{resized:?}");
    let first = &resized[0];
    let seen = (
        &first["gathr.ask.id"],
        &first["gathr.ask.size"],
        &first["gathr.ask.was"],
        &first["gathr.ask.by"],
        &first["gathr.ask.correction"],
        &first["gathr.turn.id"],
    );
    let want = (
        &json(&m1),
        &json("repair"),
        &serde_json::Value::Null,
        &json("lead"),
        &json(false),
        &json("lead-t"),
    );
    assert_eq!(seen, want);

    // Steered into the lead's running turn: the turn stays, the tasks it gave after the message go.
    let one = gives(&mut conn, ("lead", "ann"), Some("lead-t"), None);
    let m2 = steered(&mut conn, "lead-t", "Add tags to notes");
    let two = gives(&mut conn, ("lead", "bo"), Some("lead-t"), None);
    let (one_brief, two_brief) = (queued_on(&conn, one), queued_on(&conn, two));
    let split = asks::size(&mut conn, &sizing("lead", Some("lead-t"), "new", Some(&m2)));
    assert_eq!(
        split.unwrap().0,
        format!("Made message {m2} an ask of its own and sized \"Add tags to notes\" as new.")
    );
    assert_eq!(ask_row(&conn, &m2).0.as_deref(), Some("new"));
    assert_eq!(
        turn_ask(&conn, "lead-t").as_ref(),
        Some(&m1),
        "the turn stays"
    );
    assert_eq!(
        task_ask(&conn, one).as_ref(),
        Some(&m1),
        "given before the message"
    );
    assert_eq!(task_ask(&conn, two).as_ref(), Some(&m2), "given after it");
    let briefs = (turn_ask(&conn, &one_brief), turn_ask(&conn, &two_brief));
    assert_eq!(
        briefs,
        (Some(m1.clone()), Some(m2.clone())),
        "a moved task's turns move"
    );
    let event = ask_events(&conn, "resized").pop().unwrap();
    let seen = (
        &event["gathr.ask.split"],
        &event["gathr.ask.from"],
        &event["gathr.ask.was"],
        &event["gathr.turn.moved"],
        &event["gathr.task.count"],
    );
    assert_eq!(
        seen,
        (
            &json(true),
            &json(&m1),
            &json("repair"),
            &json(false),
            &json(1)
        )
    );

    // Steered into a member's task turn: it serves its task's ask and stays.
    let ann = begin(&mut conn, one);
    let m4 = steered(&mut conn, &ann, "Check the import too");
    assert_eq!(turn_ask(&conn, &ann).as_ref(), Some(&m1), "joined");
    asks::size(&mut conn, &sizing("lead", Some("lead-t"), "new", Some(&m4))).unwrap();
    assert_eq!(
        (turn_ask(&conn, &ann), task_ask(&conn, one)),
        (Some(m1.clone()), Some(m1.clone()))
    );

    // Both tasks done, the lead's hear-back answers its turn of m1 and serves m2, the newest it
    // reports; m1 isn't fixed while it is queued.
    done_in(&mut conn, one, &ann);
    let bo = begin(&mut conn, two);
    done_in(&mut conn, two, &bo);
    gathr::store::set_turn(&conn, "lead-t", "done", None).unwrap();
    let back = tasks::turn_ended(&mut conn, "lead-t", T).unwrap();
    let back = back.iter().find(|s| s.agent == "lead").unwrap();
    assert_eq!(back.answers.as_deref(), Some("lead-t"));
    assert_eq!(
        turn_ask(&conn, &back.turn).as_ref(),
        Some(&m2),
        "the hear-back"
    );
    asks::fixed(&mut conn, "c", T).unwrap();
    assert_eq!(
        fixed_lines(&conn),
        Vec::<String>::new(),
        "m1 waits on the hear-back"
    );

    // Its own turn: the turn and the tasks it gave go with the message.
    status(&conn, &back.turn, "running");
    let m5 = sent(&mut conn, ("c", "lead"), "p5", "Rename the menu");
    assert_eq!(
        turn_ask(&conn, "p5").as_ref(),
        Some(&m2),
        "joined the open ask"
    );
    status(&conn, "p5", "running");
    let three = gives(&mut conn, ("lead", "ann"), Some("p5"), None);
    let three_brief = queued_on(&conn, three);
    asks::size(
        &mut conn,
        &sizing("lead", Some(&back.turn), "new", Some(&m5)),
    )
    .unwrap();
    let moved = (turn_ask(&conn, "p5"), task_ask(&conn, three));
    assert_eq!(moved, (Some(m5.clone()), Some(m5.clone())));
    assert_eq!(
        turn_ask(&conn, &three_brief),
        Some(m5),
        "#3's brief moves too"
    );
    let event = ask_events(&conn, "resized").pop().unwrap();
    assert_eq!(
        (&event["gathr.turn.moved"], &event["gathr.task.count"]),
        (&json(true), &json(1))
    );

    // A message that started its own ask sizes that ask.
    asks::size(
        &mut conn,
        &sizing("lead", Some(&back.turn), "new", Some(&m1)),
    )
    .unwrap();
    assert_eq!(ask_row(&conn, &m1).0.as_deref(), Some("new"));

    // Read later in a turn of its own, a message split off brings its own ask, not m1.
    let read = (Some("ann"), None, None);
    gathr::store::add_turn(&conn, "c", "read4", T, read, Opened::Person).unwrap();
    gathr::store::move_message(&conn, &m4, "read4").unwrap();
    assert_eq!(turn_ask(&conn, "read4"), Some(m4), "a split message, moved");
}

fn json(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).unwrap()
}

fn refused(conn: &mut Connection, s: &Sizing) -> String {
    match asks::size(conn, s) {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// G2.5: each refusal says what to do, in order, and writes nothing.
#[test]
fn size_is_refused_saying_what_to_do() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    conn.execute_batch(
        "INSERT INTO chats (id, title, created_ts) VALUES ('solo', 'Solo', 1), ('gone', 'Gone', 1);
         INSERT INTO chat_members VALUES ('solo', 'lead', 'supervisor', 1),
             ('gone', 'lead', 'supervisor', 1), ('gone', 'ann', 'member', 1);",
    )
    .unwrap();
    let before = sent(&mut conn, ("solo", "lead"), "s1", "Before the team");
    conn.execute(
        "INSERT INTO chat_members VALUES ('solo', 'ann', 'member', 2)",
        [],
    )
    .unwrap();
    let m1 = sent(&mut conn, ("c", "lead"), "p1", EXPORT);
    status(&conn, "p1", "running");
    let elsewhere = sent(&mut conn, ("other", "lead"), "o1", "Elsewhere");
    let gathrs = gathr::store::add_message(&conn, "c", "p1", "gathr", "text", "Task #1").unwrap();
    let gathrs = gathrs["id"].as_str().unwrap().to_string();
    let wake = Start::queue(
        &conn,
        ("c", "lead"),
        (None, None),
        Opened::Wake,
        "Wake".into(),
    )
    .unwrap();
    gathr::archive::archive(&mut conn, "gone", T).unwrap();
    let mut solo = sizing("lead", Some("s1"), "new", None);
    solo.chat = "solo";
    let mut archived = sizing("lead", Some("p1"), "new", None);
    archived.chat = "gone";
    let mut old = sizing("lead", Some("p1"), "new", Some(&before));
    old.chat = "solo";

    let cases = [
        (sizing("lead", Some("p1"), "big", None), "`big` isn't a size: give repair or new.".to_string()),
        (archived, "the chat is archived".into()),
        (sizing("ann", Some("p1"), "new", None), "Only Lead, the chat's Supervisor, sizes asks.".into()),
        (sizing("lead", None, "new", None), "you have no turn running in this chat".into()),
        (
            sizing("lead", Some("p1"), "new", Some(&gathrs)),
            format!("There's no message {gathrs} of the person's in this chat."),
        ),
        (
            sizing("lead", Some("p1"), "new", Some(&elsewhere)),
            format!("There's no message {elsewhere} of the person's in this chat."),
        ),
        (
            sizing("lead", Some(&wake.turn), "new", None),
            "This turn serves none of the person's asks: name the person's message to size its ask.".into(),
        ),
    ];
    for (s, want) in cases {
        assert_eq!(refused(&mut conn, &s), want);
    }
    let solo_said = refused(&mut conn, &solo);
    conn.execute(
        "DELETE FROM chat_members WHERE chat_id = 'solo' AND agent_id = 'ann'",
        [],
    )
    .unwrap();
    let one_agent = refused(&mut conn, &solo);
    assert_eq!(
        one_agent,
        "This chat has one agent, so its asks have no size."
    );
    conn.execute(
        "INSERT INTO chat_members VALUES ('solo', 'ann', 'member', 2)",
        [],
    )
    .unwrap();
    let want = format!(
        "Message {before} serves no ask: it came before asks, or before this chat had two agents."
    );
    assert_eq!(refused(&mut conn, &old), want);
    assert_eq!(
        solo_said,
        "This turn serves none of the person's asks: name the person's message to size its ask."
    );

    asks::resize(&mut conn, &m1, "repair", T).unwrap();
    let person_said = ask_events(&conn, "resized").len();
    let want = "The person made this ask repair: only the person changes it now.";
    assert_eq!(
        refused(&mut conn, &sizing("lead", Some("p1"), "new", None)),
        want
    );
    assert_eq!(person_said, 1, "only the person's resize is recorded");
    assert_eq!(ask_events(&conn, "resized").len(), 1, "no refusal writes");
}

/// G3: the person's resize; only repair to new is a correction, and where the line sits is kept.
#[test]
fn the_person_resizes_and_only_repair_to_new_is_a_correction() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let m1 = sent(&mut conn, ("c", "lead"), "p1", EXPORT);
    status(&conn, "p1", "done");
    for size in ["new", "repair", "new", "new"] {
        let ask = asks::resize(&mut conn, &m1, size, T).unwrap().0;
        assert_eq!(
            (ask.size.as_deref(), ask.by.as_deref()),
            (Some(size), Some("person"))
        );
        assert_eq!(ask.title, EXPORT);
    }
    let corrections: Vec<(serde_json::Value, serde_json::Value)> = ask_events(&conn, "resized")
        .iter()
        .map(|e| (e["gathr.ask.by"].clone(), e["gathr.ask.correction"].clone()))
        .collect();
    let person = json("person");
    let want = [
        (person.clone(), json(false)),
        (person.clone(), json(false)),
        (person, json(true)),
    ];
    assert_eq!(corrections, want, "the same size again records nothing");
    let refusal = |r: Result<(asks::Ask, Vec<Start>), Error>| match r {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    };
    assert_eq!(
        refusal(asks::resize(&mut conn, "nope", "new", T)),
        "there's no ask nope"
    );
    assert_eq!(
        refusal(asks::resize(&mut conn, &m1, "big", T)),
        "`big` isn't a size: repair or new"
    );

    let m2 = sent(&mut conn, ("c", "lead"), "p2", "Add tags");
    asks::size(&mut conn, &sizing("lead", Some("p2"), "repair", None)).unwrap();
    let ask = asks::resize(&mut conn, &m2, "new", T).unwrap().0;
    assert_eq!(
        ask.sized_in.as_deref(),
        Some("p2"),
        "the line stays where the lead put it"
    );
    assert_eq!(
        ask_row(&conn, &m2),
        (Some("new".into()), Some("person".into()), Some("p2".into()))
    );
}

/// Gathr's "Fixed" lines in chat `c`.
fn fixed_lines(conn: &Connection) -> Vec<String> {
    let sql = "SELECT body FROM messages WHERE chat_id = 'c' AND turn_id IS NULL AND role = 'gathr'
               AND body LIKE 'Fixed:%' ORDER BY ts";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// Ask `m1` (`p1`, done) sized `size`, with #1 given in `p1` and its brief turn done: m1, #1.
fn sized_with_a_task(dir: &TempDir, size: Option<&str>) -> (Connection, String, i64) {
    let (mut conn, m1) = one_ask(dir);
    let one = gives(&mut conn, ("lead", "ann"), Some("p1"), None);
    conn.execute("UPDATE turns SET status = 'done' WHERE task = ?1", [one])
        .unwrap();
    conn.execute(
        "UPDATE asks SET size = ?2 WHERE id = ?1",
        rusqlite::params![m1, size],
    )
    .unwrap();
    (conn, m1, one)
}

/// G4: a repair whose work is in, and nothing of it queued or running, is said fixed once.
#[test]
fn a_repair_whose_work_is_in_is_said_fixed_once() {
    type Setup = fn(&Connection, &str, i64);
    let cases: [(&str, Option<&str>, Setup); 8] = [
        ("unsized", None, |c, _, n| done(c, n)),
        ("new", Some("new"), |c, _, n| done(c, n)),
        ("#1 held", Some("repair"), |c, _, n| {
            c.execute("UPDATE tasks SET state = 'held' WHERE number = ?1", [n])
                .unwrap();
        }),
        ("a question open", Some("repair"), |c, m1, n| {
            done(c, n);
            c.execute(
                "INSERT INTO tasks (chat_id, giver, kind, what, state, created_ts, updated_ts, ask_id)
                 VALUES ('c', 'lead', 'person', 'Which?', 'ready', 1, 1, ?1)",
                [m1],
            )
            .unwrap();
        }),
        ("the hear-back queued", Some("repair"), |c, m1, n| {
            done(c, n);
            c.execute(
                "INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, answers, ask_id)
                 VALUES ('back', 'c', 'x', 'queued', 1, 'lead', 'p1', ?1)",
                [m1],
            )
            .unwrap();
        }),
        (
            "a turn of another ask answering one of m1's",
            Some("repair"),
            |c, _, n| {
                done(c, n);
                c.execute_batch(
                "INSERT INTO asks (id, chat_id) VALUES ('m9', 'c');
                 INSERT INTO turns (id, chat_id, trace_id, status, created_ts, agent_id, answers, ask_id)
                 VALUES ('back', 'c', 'x', 'queued', 1, 'lead', 'p1', 'm9');",
            )
            .unwrap();
            },
        ),
        ("only #1 cancelled", Some("repair"), |c, _, n| {
            c.execute(
                "UPDATE tasks SET state = 'cancelled' WHERE number = ?1",
                [n],
            )
            .unwrap();
        }),
        ("#1 failed", Some("repair"), |c, _, n| {
            c.execute("UPDATE tasks SET state = 'failed' WHERE number = ?1", [n])
                .unwrap();
        }),
    ];
    for (what, size, setup) in cases {
        let dir = TempDir::new();
        let (mut conn, m1, one) = sized_with_a_task(&dir, size);
        setup(&conn, &m1, one);
        asks::fixed(&mut conn, "c", T).unwrap();
        assert_eq!(fixed_lines(&conn), Vec::<String>::new(), "{what}");
    }

    let dir = TempDir::new();
    let (mut conn, m1, one) = sized_with_a_task(&dir, Some("repair"));
    let two = gives(&mut conn, ("lead", "bo"), Some("p1"), None);
    conn.execute_batch(&format!(
        "UPDATE turns SET status = 'done' WHERE task = {two};
         UPDATE tasks SET state = 'done', pr = 41 WHERE number = {one};
         UPDATE tasks SET state = 'done', finish = 'done', finish_note = 'see it',
             finish_see = 'src/export.rs', finish_ts = 1 WHERE number = {two};"
    ))
    .unwrap();
    asks::fixed(&mut conn, "c", T).unwrap();
    let proof = common::COMPLETE.proof.unwrap();
    let want = format!(
        "Fixed: {EXPORT}\n#{one} Fix the export: PR #41. Proof: {proof}\n#{two} Fix the export: src/export.rs. Proof: {proof}"
    );
    assert_eq!(fixed_lines(&conn), [want]);
    let said: String = conn
        .query_row(
            "SELECT id FROM messages WHERE body LIKE 'Fixed:%' AND turn_id IS NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let marked: Option<String> = conn
        .query_row("SELECT fixed FROM asks WHERE id = ?1", [&m1], |r| r.get(0))
        .unwrap();
    assert_eq!(marked, Some(said.clone()));
    let events = ask_events(&conn, "fixed");
    assert_eq!(events.len(), 1, "{events:?}");
    let seen = (
        &events[0]["gathr.ask.id"],
        &events[0]["gathr.message.id"],
        &events[0]["gathr.task.count"],
    );
    assert_eq!(seen, (&json(&m1), &json(&said), &json(2)));
    asks::fixed(&mut conn, "c", T).unwrap();
    assert_eq!(fixed_lines(&conn).len(), 1, "said once");

    // New, all done, then made a repair: said at the next look.
    let m5 = sent(&mut conn, ("c", "lead"), "p5", "Rename the menu");
    status(&conn, "p5", "done");
    let five = gives(&mut conn, ("lead", "ann"), Some("p5"), None);
    conn.execute("UPDATE turns SET status = 'done' WHERE task = ?1", [five])
        .unwrap();
    done(&conn, five);
    asks::resize(&mut conn, &m5, "new", T).unwrap();
    asks::fixed(&mut conn, "c", T).unwrap();
    assert_eq!(fixed_lines(&conn).len(), 1, "a new ask isn't fixed");
    asks::resize(&mut conn, &m5, "repair", T).unwrap();
    asks::fixed(&mut conn, "c", T).unwrap();
    let lines = fixed_lines(&conn);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert!(
        lines[1].starts_with("Fixed: Rename the menu\n"),
        "{lines:?}"
    );

    // Two due at once, the older with the larger id (ids of one millisecond): oldest first.
    conn.execute_batch(
        "INSERT INTO asks (id, chat_id, size) VALUES ('m-z', 'c', 'repair'), ('m-a', 'c', 'repair');
         INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts, ask_id)
             VALUES ('c', 'lead', 'ann', 'work', 'x', 'done', 1, 1, 'm-z'),
                    ('c', 'lead', 'ann', 'work', 'y', 'done', 1, 1, 'm-a');",
    )
    .unwrap();
    asks::fixed(&mut conn, "c", T).unwrap();
    let said: Vec<_> = ask_events(&conn, "fixed")
        .into_iter()
        .map(|e| e["gathr.ask.id"].clone())
        .collect();
    assert_eq!(said[said.len() - 2..], [json("m-z"), json("m-a")]);
}

fn done(conn: &Connection, task: i64) {
    conn.execute("UPDATE tasks SET state = 'done' WHERE number = ?1", [task])
        .unwrap();
}
