//! H6, H6a, H7 and H8: Approve makes each part a task, Send back wakes the lead, Drop ends the
//! wait, a second plan keeps the first, and a plan keeps its ask's messages, check and tasks.

use super::*;
use gathr::asks::{self, Sizing};

const TEN: i64 = 600_000_000;

/// The chat's unsent versions reach the person, their checks late (H4.12).
fn late(conn: &mut Connection, chat: &str) {
    let now = gathr::ids::now_us();
    plans::overdue(conn, chat, now + TEN, TEN, T).unwrap();
}

/// Each part's task, by the part's number.
fn part_tasks(conn: &Connection, version: i64) -> Vec<i64> {
    let sql = "SELECT task FROM plan_parts WHERE version_id = ?1 ORDER BY n";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([version], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

fn after(conn: &Connection, task: i64) -> Vec<i64> {
    let sql = "SELECT after_task FROM task_after WHERE task = ?1 ORDER BY after_task";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([task], |r| r.get(0)).unwrap();
    rows.map(Result::unwrap).collect()
}

/// Mira's queued turns: each id, what opened it, the turn it answers, its ask and its message.
type Woken = (String, String, Option<String>, Option<String>, String);

fn mira_queued(conn: &Connection) -> Vec<Woken> {
    let sql = "SELECT u.id, u.opened_by, u.answers, u.ask_id, m.body FROM turns u
               JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
               WHERE u.agent_id = 'mira' AND u.status = 'queued' ORDER BY u.rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt
        .query_map([], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })
        .unwrap();
    rows.map(Result::unwrap).collect()
}

fn needs(conn: &Connection, chat: &str) -> serde_json::Value {
    let chats = gathr::store::chats(conn, false).unwrap();
    let found = chats.as_array().unwrap().iter().find(|c| c["id"] == chat);
    found.unwrap()["needs"].clone()
}

fn task_count(conn: &Connection) -> i64 {
    conn.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
        .unwrap()
}

/// The person's message in a new turn of Mira's: the ask that turn then serves.
fn person_says(conn: &mut Connection, turn: &str, text: &str) -> (String, Option<String>) {
    let tx = conn.transaction().unwrap();
    let opened = Opened::Person;
    gathr::store::add_turn(&tx, "c", turn, T, (Some("mira"), None, None), opened).unwrap();
    let message = gathr::store::add_message(&tx, "c", turn, "user", "text", text).unwrap();
    tx.commit().unwrap();
    let ask = conn
        .query_row("SELECT ask_id FROM turns WHERE id = ?1", [turn], |r| {
            r.get(0)
        })
        .unwrap();
    (message["id"].as_str().unwrap().to_string(), ask)
}

/// Bo agrees with the version while no turn runs on its check: the version waits on the person.
fn agreed(conn: &mut Connection, wrote: &Wrote) {
    let (queued, _) = queued_on(conn, wrote.check);
    set_status(conn, &queued, "stopped");
    let message: String = version_field(conn, wrote.version, "message_id");
    let tx = conn.transaction().unwrap();
    gathr::reactions::add(&tx, "c", &message, "bo", T).unwrap();
    plans::agreed(&tx, "c", &message, "bo", T).unwrap().unwrap();
    tx.commit().unwrap();
}

/// Mira splits `message` off from her `turn`.
fn split(conn: &mut Connection, turn: &str, message: &str) -> Result<String, Error> {
    let s = Sizing {
        chat: "c",
        agent: "mira",
        turn: Some(turn),
        size: "new",
        message: Some(message),
        trace: T,
    };
    asks::size(conn, &s)
}

/// H6.1 to H6.3.
#[test]
fn approve_makes_each_part_a_task_word_for_word() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (41, 'c', 'mira', 'ann', 'work', 'Old work', 'done', 1, 1)",
        [],
    )
    .unwrap();
    let after_41 = [41];
    let mut parts = three();
    parts[2].after = &[];
    parts[2].after_tasks = &after_41;
    let wrote = write(&mut conn, Some("T"), &parts, None).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    late(&mut conn, "c");
    let late_line = "Bo didn't finish checking version 1 of the plan \"Tags on notes\".";
    assert_eq!(lines(&conn, "c"), [late_line]);

    let decided = plans::approve(&mut conn, wrote.plan, 1, T).unwrap();
    assert_eq!(decided.summary.state, plans::State::Approved);
    let made = part_tasks(&conn, wrote.version);
    assert_eq!(made.len(), 3, "{made:?}");
    type Row = (
        String,
        String,
        String,
        String,
        String,
        String,
        String,
        i64,
        String,
        String,
        String,
        String,
    );
    let row = |task: i64| -> Row {
        conn.query_row(
            "SELECT owner, what, why, done, proof, place, size, size_lines, giver, given_in, kind, ask_id
             FROM tasks WHERE number = ?1",
            [task],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                    r.get(10)?,
                    r.get(11)?,
                ))
            },
        )
        .unwrap()
    };
    let want = |owner: &str, what: &str| -> Row {
        (
            owner.into(),
            what.into(),
            "Notes need tags".into(),
            "A note carries its tags".into(),
            "tests/tags.rs passes".into(),
            "the project's worktree, on a new branch".into(),
            "src/tags.rs".into(),
            60,
            "mira".into(),
            "T".into(),
            "work".into(),
            ask.clone(),
        )
    };
    assert_eq!(row(made[0]), want("ann", "Add the tags table"));
    assert_eq!(row(made[1]), want("bo", "Show tags in the list"));
    assert_eq!(row(made[2]), want("ann", "Filter by tag"));
    let afters: Vec<Vec<i64>> = made.iter().map(|t| after(&conn, *t)).collect();
    assert_eq!(afters, [vec![], vec![made[0]], vec![41]]);
    let states: Vec<tasks::State> = made.iter().map(|t| task_state(&conn, *t).0).collect();
    let (ready, waiting) = (tasks::State::Ready, tasks::State::Waiting);
    assert_eq!(states, [ready, waiting, ready]);
    for task in [made[0], made[2]] {
        let (turn, _) = queued_on(&conn, task);
        assert_eq!(samples::opened(&conn, &turn).0.as_deref(), Some("brief"));
    }

    let approved = "Approved: version 1 of the plan \"Tags on notes\", 3 tasks.";
    assert_eq!(
        lines(&conn, "c"),
        [late_line, approved],
        "no second check line"
    );
    let line: Option<String> = version_field(&conn, wrote.version, "decided_line");
    let said: String = conn
        .query_row("SELECT id FROM messages WHERE body = ?1", [approved], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(line, Some(said));
    let decided_event = events(&conn, "decided");
    let e = decided_event.last().unwrap();
    let seen = (
        &e["gathr.plan.how"],
        &e["gathr.task.count"],
        &e["gathr.task.started"],
    );
    let want_seen = (
        &serde_json::json!("approved"),
        &serde_json::json!(3),
        &serde_json::json!(2),
    );
    assert_eq!(seen, want_seen, "{e}");
    assert_eq!(task_state(&conn, wrote.check).0, tasks::State::Cancelled);
    assert_eq!(
        mira_queued(&conn),
        Vec::<Woken>::new(),
        "the lead isn't woken"
    );
    assert_eq!(needs(&conn, "c"), false);

    // Each refusal writes nothing.
    let before = task_count(&conn);
    let refused =
        |conn: &mut Connection, plan: i64, n: i64| refusal(plans::approve(conn, plan, n, T));
    assert_eq!(
        refused(&mut conn, wrote.plan, 1),
        "Version 1 is already approved."
    );
    assert_eq!(refused(&mut conn, 99, 1), "there's no plan #99");
    asked(&mut conn, "c", "T2", "Add folders");
    let first = write(
        &mut conn,
        Some("T2"),
        &[part("Ann", "Add folders", &[])],
        None,
    )
    .unwrap();
    let second = write(
        &mut conn,
        Some("T2"),
        &[part("Ann", "Add the folders table", &[])],
        None,
    );
    let second = second.unwrap();
    assert_eq!(second.plan, first.plan);
    assert_eq!(
        refused(&mut conn, first.plan, 1),
        "Version 1 isn't the newest: version 2 is."
    );
    assert_eq!(
        refused(&mut conn, first.plan, 2),
        "Version 2 is still being checked."
    );
    late(&mut conn, "c");
    conn.execute(
        "DELETE FROM chat_members WHERE chat_id = 'c' AND agent_id = 'ann'",
        [],
    )
    .unwrap();
    assert_eq!(
        refused(&mut conn, first.plan, 2),
        "Part 1's owner, Ann, has left this chat: send the plan back."
    );
    conn.execute(
        "INSERT INTO chat_members VALUES ('c', 'ann', 'member', 2)",
        [],
    )
    .unwrap();
    conn.execute_batch(
        "UPDATE chat_members SET role = 'member' WHERE agent_id = 'mira';
         UPDATE chat_members SET role = 'supervisor' WHERE agent_id = 'ann';",
    )
    .unwrap();
    assert_eq!(
        refused(&mut conn, first.plan, 2),
        "Mira no longer leads this chat: drop this plan."
    );
    conn.execute_batch(
        "UPDATE chat_members SET role = 'supervisor' WHERE agent_id = 'mira';
         UPDATE chat_members SET role = 'member' WHERE agent_id = 'ann';",
    )
    .unwrap();
    conn.execute(
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES (50, 'c', 'mira', 'ann', 'work', 'Index', 'ready', 1, 1)",
        [],
    )
    .unwrap();
    asked(&mut conn, "c", "T3", "Add search");
    let after_50 = [50];
    let mut searched = part("Ann", "Search notes", &[]);
    searched.after_tasks = &after_50;
    let third = write(&mut conn, Some("T3"), &[searched], None).unwrap();
    late(&mut conn, "c");
    conn.execute("UPDATE tasks SET state = 'cancelled' WHERE number = 50", [])
        .unwrap();
    assert_eq!(
        refused(&mut conn, third.plan, 1),
        "Part 1 comes after #50, which was cancelled: send the plan back."
    );
    assert_eq!(
        task_count(&conn),
        before + 4,
        "the three checks and #50 only"
    );
    gathr::archive::archive(&mut conn, "c", T).unwrap();
    assert_eq!(refused(&mut conn, first.plan, 2), "the chat is archived");
}

/// H6a, H7.3.
#[test]
fn send_back_and_drop_end_the_wait() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &three(), None).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    late(&mut conn, "c");
    assert_eq!(needs(&conn, "c"), true);

    assert_eq!(
        refusal(plans::send_back(&mut conn, wrote.plan, 1, "  ", T)),
        "Say what to change."
    );
    let sent = plans::send_back(&mut conn, wrote.plan, 1, "Split part 2", T).unwrap();
    assert_eq!(sent.summary.state, plans::State::Revising);
    let woken = mira_queued(&conn);
    assert_eq!(woken.len(), 1, "{woken:?}");
    let (turn, opened, answers, of_ask, body) = &woken[0];
    let seen = (opened.as_str(), answers.as_deref(), of_ask.as_deref());
    assert_eq!(seen, ("sent_back", Some("T"), Some(ask.as_str())));
    samples::same("sent_back", body);
    assert!(
        sent.starts.iter().any(|s| &s.turn == turn),
        "{:?}",
        sent.starts
    );
    let opened_message: String = conn
        .query_row(
            "SELECT id FROM messages WHERE turn_id = ?1 AND role = 'gathr'",
            [turn],
            |r| r.get(0),
        )
        .unwrap();
    let line: Option<String> = version_field(&conn, wrote.version, "decided_line");
    assert_eq!(line, Some(opened_message));
    assert_eq!(plan_state(&conn, "c", wrote.plan), plans::State::Revising);
    assert_eq!(needs(&conn, "c"), false);

    asked(&mut conn, "c", "T2", "Add folders");
    let other = write(
        &mut conn,
        Some("T2"),
        &[part("Ann", "Add folders", &[])],
        None,
    )
    .unwrap();
    begin(&mut conn, other.check);
    let said = lines(&conn, "c");
    let checked = events(&conn, "checked").len();
    let dropped = plans::drop(&mut conn, other.plan, 1, T).unwrap();
    assert_eq!(dropped.summary.state, plans::State::Dropped);
    assert_eq!(task_state(&conn, other.check).0, tasks::State::Cancelled);
    let line: Option<String> = version_field(&conn, other.version, "check_line");
    let sent_ts: Option<i64> = version_field(&conn, other.version, "sent_ts");
    assert_eq!((line, sent_ts), (None, None));
    assert_eq!(events(&conn, "checked").len(), checked, "no check event");
    let mut want = said;
    want.push("Dropped: the plan \"Tags on notes\".".into());
    assert_eq!(lines(&conn, "c"), want);
    assert_eq!(mira_queued(&conn).len(), 1, "no turn for Drop");
    let of_other: String = conn
        .query_row(
            "SELECT ask_id FROM plans WHERE id = ?1",
            [other.plan],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(plans::open_state(&conn, &of_other).unwrap(), None);
    assert_eq!(needs(&conn, "c"), false);
}

/// H7.
#[test]
fn a_second_plan_for_an_ask_keeps_the_first() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    let first = write(&mut conn, Some("T"), &three(), None).unwrap();
    late(&mut conn, "c");
    plans::approve(&mut conn, first.plan, 1, T).unwrap();
    let record = |conn: &Connection| -> Vec<String> {
        let sql = "SELECT json_array(v.n, v.title, v.text, v.sent_ts, v.decided, v.decided_line, v.check_line,
                       p.n, p.owner, p.what, p.after_parts, p.after_tasks, p.task)
                   FROM plan_versions v JOIN plan_parts p ON p.version_id = v.id
                   WHERE v.plan_id = ?1 ORDER BY v.n, p.n";
        let mut stmt = conn.prepare(sql).unwrap();
        let rows = stmt.query_map([first.plan], |r| r.get(0)).unwrap();
        rows.map(Result::unwrap).collect()
    };
    let kept = record(&conn);
    let made = part_tasks(&conn, first.version);

    let on = [made[0]];
    let mut next = part("Ann", "Index the tags", &[]);
    next.after_tasks = &on;
    let second = write(&mut conn, Some("T"), &[next], None).unwrap();
    assert_ne!(second.plan, first.plan);
    assert_eq!(second.n, 1);
    late(&mut conn, "c");
    let before = task_count(&conn);
    plans::approve(&mut conn, second.plan, 1, T).unwrap();
    assert_eq!(task_count(&conn), before + 1);
    let given = part_tasks(&conn, second.version);
    assert_eq!(given.len(), 1);
    assert_eq!(after(&conn, given[0]), [made[0]]);
    assert_eq!(record(&conn), kept, "the first plan's record is unchanged");
}

/// H8.2.
#[test]
fn a_message_while_a_plan_is_open_joins_its_ask() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &three(), None).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    agreed(&mut conn, &wrote);
    assert_eq!(plan_state(&conn, "c", wrote.plan), plans::State::Waiting);

    let (_, joined) = person_says(&mut conn, "T2", "Also tag old notes");
    assert_eq!(joined, Some(ask.clone()), "it joins the plan's ask");
    set_status(&conn, "T2", "done");
    plans::drop(&mut conn, wrote.plan, 1, T).unwrap();
    let (message, own) = person_says(&mut conn, "T3", "Add folders");
    assert_eq!(
        own,
        Some(message),
        "after Drop, a message starts its own ask"
    );
}

/// H8.3.
#[test]
fn a_split_never_moves_a_plan_its_check_or_its_tasks() {
    let dir = TempDir::new();
    let (mut conn, ask) = room(&dir);
    let wrote = write(&mut conn, Some("T"), &three(), None).unwrap();
    moved(&mut conn, ("T", "mira"), None, "done");
    agreed(&mut conn, &wrote);
    let (m2, joined) = person_says(&mut conn, "T2", "Also tag old notes");
    assert_eq!(joined, Some(ask.clone()));
    set_status(&conn, "T2", "running");
    let mut tagged = three();
    tagged.push(part("Bo", "Tag old notes", &[1]));
    write(&mut conn, Some("T2"), &tagged, None).unwrap();
    let asks_before: i64 = conn
        .query_row("SELECT count(*) FROM asks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        refusal(split(&mut conn, "T2", &m2)),
        format!(
            "Message {m2}'s turn wrote plan #{}, which belongs to its ask: it can't be split off.",
            wrote.plan
        )
    );
    let asks_after: i64 = conn
        .query_row("SELECT count(*) FROM asks", [], |r| r.get(0))
        .unwrap();
    assert_eq!(asks_after, asks_before);

    // Steered into the turn that then gives a task and writes a version: the task moves, not the check.
    let m5 = asked(&mut conn, "c", "T5", "Add search");
    let steered =
        gathr::store::add_message(&conn, "c", "T5", "user", "text", "Also search titles").unwrap();
    let m6 = steered["id"].as_str().unwrap().to_string();
    let g = tasks::Give {
        chat: "c",
        giver: Some("mira"),
        owner: "ann",
        given_in: Some("T5"),
        parent: None,
        kind: tasks::Kind::Work,
        what: "Index titles",
        brief: tasks::Brief {
            why: Some("Search reads titles"),
            done: Some("Titles are indexed"),
            proof: Some("tests/search.rs passes"),
            place: Some("no code"),
            size: None,
        },
        after: &[],
        pr: None,
        in_passing: false,
    };
    let (work, _) = tasks::give(&mut conn, &g, T).unwrap();
    let search = write(
        &mut conn,
        Some("T5"),
        &[part("Ann", "Search notes", &[])],
        None,
    )
    .unwrap();
    split(&mut conn, "T5", &m6).unwrap();
    let ask_of = |conn: &Connection, task: i64| -> String {
        let sql = "SELECT ask_id FROM tasks WHERE number = ?1";
        conn.query_row(sql, [task], |r| r.get(0)).unwrap()
    };
    assert_eq!(
        ask_of(&conn, work.number),
        m6,
        "the task given after the message moves"
    );
    assert_eq!(
        ask_of(&conn, search.check),
        m5,
        "the check stays with its plan"
    );

    // Steered into the turn that wrote a version that is then approved: no part's task moves.
    let m7 = asked(&mut conn, "c", "T7", "Add export");
    let export = write(&mut conn, Some("T7"), &three(), None).unwrap();
    let steered =
        gathr::store::add_message(&conn, "c", "T7", "user", "text", "As CSV too").unwrap();
    let m8 = steered["id"].as_str().unwrap().to_string();
    late(&mut conn, "c");
    plans::approve(&mut conn, export.plan, 1, T).unwrap();
    split(&mut conn, "T7", &m8).unwrap();
    for task in part_tasks(&conn, export.version) {
        assert_eq!(ask_of(&conn, task), m7, "#{task}");
        let sql = "SELECT ask_id FROM turns WHERE task = ?1";
        let mut stmt = conn.prepare(sql).unwrap();
        let turns: Vec<String> = stmt
            .query_map([task], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(turns.iter().all(|a| a == &m7), "#{task}'s turns: {turns:?}");
    }
    let moved: i64 = conn
        .query_row(
            "SELECT json_extract(attrs, '$.\"gathr.task.count\"') FROM events
             WHERE name = 'gathr.ask.resized' ORDER BY ts DESC, rowid DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(moved, 0);
}
