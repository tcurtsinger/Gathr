//! Discuss (0065, F5): the question leaves the person's list, the person's words reach the lead
//! alone, and the question comes back when that turn ends unless a new one takes its place.

use super::*;

/// Mira's standing in the chat's waiting answer.
fn mira_stands(conn: &Connection) -> (&'static str, String) {
    let live = gathr::waiting::Live::default();
    let said = gathr::waiting::answer(conn, "c", &live, gathr::ids::now_us()).unwrap();
    let mira = said.agents.into_iter().find(|a| a.agent == "mira").unwrap();
    (mira.state, mira.words)
}

/// F5.1 to F5.4, scenario 4e.
#[test]
fn discuss_takes_the_question_off_the_list_and_reaches_the_lead_only() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (request, q) = filed(&mut conn);
    let sent = discuss(&mut conn, q, "Does it retry by itself?");
    assert!(sent.refused.is_empty(), "{sent:?}");
    let asked = task(&conn, q);
    assert_eq!(
        (asked.state, asked.reason.as_deref()),
        (State::Held, Some("Discuss: Does it retry by itself?"))
    );
    assert!(listed(&conn).is_empty(), "off the person's list");
    assert_eq!(needs(&conn), false);
    let live = gathr::waiting::Live::default();
    let said = gathr::waiting::answer(&conn, "c", &live, gathr::ids::now_us()).unwrap();
    assert!(
        said.lines.iter().all(|l| l.kind != "question"),
        "{:#?}",
        said.lines
    );
    let stands = mira_stands(&conn);
    assert_ne!(stands.0, "needs_you", "{stands:?}");
    assert_eq!(
        (task(&conn, request).state, task(&conn, 1).state),
        (State::Held, State::Held)
    );
    assert!(to(&sent.starts, "ann").is_empty(), "{sent:?}");
    let mira = to(&sent.starts, "mira");
    assert_eq!((mira.len(), mira[0].task), (1, Some(request)), "{sent:?}");
    assert_eq!(
        mira[0].text,
        "The person wants to discuss:\n- #4 Which notice text should ship?: Does it retry by itself?\n#4 was asked \
         for Ann (#3), who keeps waiting.\n#4 is off the person's list until this turn ends. Reply to them here. To \
         change the question, ask a new one with ask_person: it takes #4's place."
    );
    let mut discussed = task(&conn, q).answer.unwrap()["discussed"].clone();
    let turn = mira[0].turn.as_str();
    // When it was sent (0073); person_tasks.rs checks its value.
    assert!(discussed[0]["at"].take().is_i64(), "{discussed}");
    assert_eq!(
        discussed,
        json!([{ "at": null, "text": "Does it retry by itself?", "turn": turn }])
    );
    let attrs = json!({ "gathr.discuss.how": "asked", "gathr.turn.id": turn });
    assert_eq!(
        events(&conn, "gathr.task.discussed"),
        [(Some("4".to_string()), attrs)]
    );

    // The refusals, each with the question still on the list.
    let open = person::ask(&mut conn, filing("mira-3", None, Some("Ship today?")), T)
        .unwrap()
        .0
        .number;
    let refused = |conn: &mut Connection, a: Answer| {
        let sent = person::answer(conn, &[a], T).unwrap();
        assert_eq!(task(conn, sent.refused[0].0).state, State::Ready);
        sent.refused[0].1.clone()
    };
    let no_words = Answer {
        task: open,
        discuss: true,
        ..Default::default()
    };
    assert_eq!(refused(&mut conn, no_words), "Discuss #5 with a message");
    let with_pick = Answer {
        task: open,
        discuss: true,
        pick: vec!["Connection lost".into()],
        text: Some("and this".into()),
        ..Default::default()
    };
    let alone = "Discuss #5 with a message alone, not a choice";
    assert_eq!(refused(&mut conn, with_pick), alone);
    conn.execute(
        "INSERT INTO tasks (chat_id, kind, what, state, form, choices, created_ts, updated_ts)
         VALUES ('c', 'person', 'Land #7?', 'ready', 'choice', '[{\"label\":\"Let it go on\"},{\"label\":\"Hold it\"}]', 1, 1)",
        [],
    )
    .unwrap();
    let gathrs = Answer {
        task: 6,
        discuss: true,
        text: Some("Why?".into()),
        ..Default::default()
    };
    assert_eq!(
        refused(&mut conn, gathrs),
        "Answer #6 with one of its choices"
    );
    let missing = Answer {
        task: 99,
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[missing], T).unwrap();
    assert_eq!(sent.refused[0].1, "there's no task #99");
    let sql = "SELECT task_id, conversation_id FROM events WHERE name = 'gathr.task.answer' AND error_type = 'refused'
               ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let chats: Vec<(Option<String>, Option<String>)> = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    let c = Some("c".to_string());
    let in_chat = |t: &str| (Some(t.to_string()), c.clone());
    let refusals = [
        in_chat("5"),
        in_chat("5"),
        in_chat("6"),
        (Some("99".into()), None),
    ];
    assert_eq!(chats, refusals, "a refusal is in its task's chat");
}

/// The person's message in a new turn of Mira's on an ask with no plan yet: the turn.
fn planned(conn: &mut Connection, turn: &str) -> String {
    gathr::store::add_turn(
        conn,
        "c",
        turn,
        T,
        (Some("mira"), None, None),
        gathr::words::Opened::Person,
    )
    .unwrap();
    let message = gathr::store::add_message(conn, "c", turn, "user", "text", "Add tags").unwrap();
    let id = message["id"].as_str().unwrap().to_string();
    conn.execute(
        "INSERT OR IGNORE INTO asks (id, chat_id) VALUES (?1, 'c')",
        [&id],
    )
    .unwrap();
    let sql = "UPDATE turns SET ask_id = ?2, status = 'running' WHERE id = ?1";
    conn.execute(sql, [turn, &id]).unwrap();
    turn.to_string()
}

fn plan(conn: &mut Connection, turn: &str) -> Result<gathr::plans::Wrote, Error> {
    let part = gathr::plans::Part {
        to: "Ann",
        what: "Add the tags table",
        why: Some("Notes need tags"),
        done: Some("A note carries its tags"),
        proof: Some("tests/tags.rs passes"),
        place: Some("the project's worktree, on a new branch"),
        size: Some("src/tags.rs"),
        lines: Some(60),
        after: &[],
        after_tasks: &[],
    };
    let w = gathr::plans::Writing {
        chat: "c",
        agent: "mira",
        turn: Some(turn),
        title: "Tags on notes",
        text: "Notes carry tags.",
        parts: &[part],
    };
    gathr::plans::write(conn, &w, T)
}

/// F5.4: H3.2 still holds the plan while its question is discussed.
#[test]
fn a_plan_waits_while_its_question_is_discussed() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let turn = planned(&mut conn, "mira-t");
    let ask = filing(&turn, None, Some("Which tags first?"));
    let q = person::ask(&mut conn, ask, T).unwrap().0.number;
    moved(&mut conn, "mira-t", "done");
    let sent = discuss(&mut conn, q, "Does it matter?");
    let answered = &to(&sent.starts, "mira")[0].turn;
    conn.execute(
        "UPDATE turns SET status = 'running' WHERE id = ?1",
        [answered],
    )
    .unwrap();
    assert_eq!(
        refusal(plan(&mut conn, answered)),
        format!(
            "Question #{q} to the person is open for this ask: write the plan once it's answered."
        )
    );
}

/// F5.5, F5.7.
#[test]
fn a_discussed_question_comes_back_when_the_leads_turn_ends() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (request, q) = filed(&mut conn);
    let mut turns = Vec::new();
    for status in ["done", "failed", "stopped"] {
        let sent = discuss(&mut conn, q, &format!("Before {status}?"));
        let turn = to(&sent.starts, "mira")[0].turn.clone();
        moved(&mut conn, &turn, "running");
        assert_eq!(task(&conn, q).state, State::Held, "{status}");
        moved(&mut conn, &turn, status);
        let back = task(&conn, q);
        assert_eq!((back.state, back.reason), (State::Ready, None), "{status}");
        assert_eq!(listed(&conn), [q], "{status}");
        assert_eq!(needs(&conn), true, "{status}");
        let asked = ("needs_you", "Asked you a question".to_string());
        assert_eq!(mira_stands(&conn), asked, "{status}");
        if status == "done" {
            let r = task(&conn, request);
            let waits = (r.state, r.reason.as_deref());
            assert_eq!(waits, (State::Held, Some("waiting on #4 (the person)")));
        }
        turns.push(turn);
    }
    let back = events(&conn, "gathr.task.discussed");
    let how: Vec<&Value> = back.iter().map(|e| &e.1["gathr.discuss.how"]).collect();
    assert_eq!(how, ["asked", "back", "asked", "back", "asked", "back"]);

    answer(&mut conn, q, &["Connection lost"], Some("and say we retry"));
    let settled = format!("the person answered #4 ({ASKED}): {PICKED}");
    assert_eq!(task(&conn, request).reason, Some(settled));
    let kept = &task(&conn, q).answer.unwrap()["discussed"];
    let kept: Vec<&str> = kept
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["turn"].as_str().unwrap())
        .collect();
    assert_eq!(kept, turns, "each Discuss is kept with the answer");
}

/// F5.5 at a start: a lead's turn still running when Gathr stopped returns the question, one
/// still queued keeps it off the list.
#[test]
fn a_restart_returns_a_discussed_question_whose_turn_is_over() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (_, q) = filed(&mut conn);
    let sent = discuss(&mut conn, q, "Does it retry by itself?");
    let turn = &to(&sent.starts, "mira")[0].turn;
    conn.execute("UPDATE turns SET status = 'running' WHERE id = ?1", [turn])
        .unwrap();
    tasks::recover(&mut conn).unwrap();
    assert_eq!(task(&conn, q).state, State::Ready);
    discuss(&mut conn, q, "And now?");
    tasks::recover(&mut conn).unwrap();
    assert_eq!(
        task(&conn, q).state,
        State::Held,
        "its turn is still to come"
    );
    let sql = "UPDATE tasks SET heard_in = NULL WHERE number = ?1";
    conn.execute(sql, [q]).unwrap();
    tasks::recover(&mut conn).unwrap();
    let back = task(&conn, q);
    assert_eq!(
        (back.state, back.reason),
        (State::Ready, None),
        "a held question with no turn is back"
    );
}

/// F5.6.
#[test]
fn the_leads_new_question_in_that_turn_takes_the_old_ones_place() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (request, q) = filed(&mut conn);
    let folded = need(&mut conn, "bo", "bo-1");
    running(&conn, "mira-5", "mira", Some(folded));
    let on = Said {
        on_task: Some(q),
        ..Default::default()
    };
    finish(&mut conn, "mira", folded, Finish::Blocked, &on).unwrap();
    moved(&mut conn, "mira-5", "done");
    let sent = discuss(&mut conn, q, "Does it retry by itself?");
    let turn = to(&sent.starts, "mira")[0].turn.clone();
    moved(&mut conn, &turn, "running");
    let before = events(&conn, "gathr.task.discussed").len();
    let queued = |conn: &Connection| -> i64 {
        let sql = "SELECT count(*) FROM turns WHERE status = 'queued'";
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    };
    let was_queued = queued(&conn);
    let mut again = filing(
        &turn,
        Some(request),
        Some("Should the notice say we retry?"),
    );
    (again.form, again.choices) = (Some(Form::Yesno), Vec::new());
    let (q2, starts) = person::ask(&mut conn, again, T).unwrap();
    assert!(starts.is_empty(), "{starts:?}");
    assert_eq!(q2.request, Some(request));
    let old = task(&conn, q);
    assert_eq!(
        (old.state, old.reason.as_deref()),
        (State::Cancelled, Some("#6 took its place"))
    );
    let heard: bool = conn
        .query_row("SELECT heard FROM tasks WHERE number = ?1", [q], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(heard, "the request's hold reads it");
    assert_eq!(queued(&conn), was_queued, "nobody is told");
    let replaced = events(&conn, "gathr.task.discussed");
    assert_eq!(replaced.len(), before + 1);
    assert_eq!(
        replaced[before],
        (
            Some("4".to_string()),
            json!({ "gathr.discuss.how": "replaced", "gathr.task.question": 6 })
        )
    );
    let fold = task(&conn, folded);
    let follows = (fold.blocked_on, fold.reason.as_deref());
    assert_eq!(
        follows,
        (Some(6), Some("blocked on #6")),
        "the fold follows"
    );

    moved(&mut conn, &turn, "done");
    let r = task(&conn, request);
    let waits = (r.state, r.reason.as_deref());
    assert_eq!(waits, (State::Held, Some("waiting on #6 (the person)")));
    assert_eq!(task(&conn, q).state, State::Cancelled, "not back");

    // 0084: the turn Ann hears the request settle in carries #6's answer, not #4 or its place.
    let yes = Answer {
        task: q2.number,
        yes: Some(true),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[yes], T).unwrap();
    let ann = to(&sent.starts, "ann");
    assert_eq!(ann.len(), 1, "{sent:?}");
    let (lines, _) = gathr::tasks::questions(&conn, &ann[0].turn, None)
        .unwrap()
        .expect("#4 and its place");
    assert!(
        lines.contains("- #4 ") && lines.contains("cancelled: #6 took its place"),
        "{lines}"
    );
    assert!(!lines.contains("- #6 "), "#6's answer told twice: {lines}");
}

/// F5.6 with H3.1: the question in its place isn't a second one, a question from another turn is.
#[test]
fn on_a_planned_ask_only_the_question_in_its_place_is_taken() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let turn = planned(&mut conn, "mira-t");
    let q = person::ask(&mut conn, filing(&turn, None, Some("Which tags first?")), T)
        .unwrap()
        .0
        .number;
    moved(&mut conn, "mira-t", "done");
    let sent = discuss(&mut conn, q, "Does it matter?");
    let answered = to(&sent.starts, "mira")[0].turn.clone();
    moved(&mut conn, &answered, "running");
    let replacing = filing(&answered, None, Some("Which tag comes first?"));
    let q2 = person::ask(&mut conn, replacing, T).unwrap().0.number;
    assert_eq!(task(&conn, q).state, State::Cancelled);
    discuss(&mut conn, q2, "Still?");
    let other = person::ask(&mut conn, filing(&turn, None, Some("Colours too?")), T);
    assert_eq!(
        refusal(other),
        format!(
            "Question #{q2} to the person is still open for this ask. Until its plan is approved, ask one at a time: \
             wait for its answer, or cancel #{q2} first."
        )
    );
}
