//! A hear-back says what is new, and what really happened (0047, 0051, docs/scenarios.md 5e): only
//! the tasks its giver hasn't heard about, a changed outcome once more, and a task given in passing
//! by itself. Trouble is heard at once; the rest when everything given from the same place is in.

use super::*;

fn note(text: &str) -> Said<'_> {
    Said {
        note: Some(text),
        ..Said::default()
    }
}

/// G188: a hear-back repeated every task its giver gave in that turn. It names only what is new.
#[test]
fn a_hear_back_names_only_what_its_giver_has_not_heard() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "done");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    let starts = work(&mut conn, a, Some(Finish::Blocked));
    assert_eq!(to_lead(&starts).len(), 1, "trouble, at once: {starts:?}");
    let starts = work(&mut conn, b, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(said.starts_with("The task you gave is settled:"), "{said}");
    assert!(said.contains(&format!("#{b} Bo")), "{said}");
    assert!(
        !said.contains(&format!("#{a} Ann")),
        "heard already: {said}"
    );

    tasks::restart(&mut conn, Some("lead"), a, T).unwrap();
    let starts = work(&mut conn, a, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(said.starts_with("The task you gave is settled:"), "{said}");
    assert!(said.contains(&format!("#{a} Ann")), "{said}");
    assert!(!said.contains(&format!("#{b} Bo")), "heard already: {said}");
}

/// 5e: an outcome that changes after its giver heard it is heard once more. The same outcome,
/// reached again by a turn nobody asked for, is not. After a restart it is: the giver asked for
/// that run, and hears how it ended.
#[test]
fn an_outcome_that_changes_after_it_was_heard_is_heard_once_more() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "done");
    let (task, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let starts = work(&mut conn, task, Some(Finish::Blocked));
    assert_eq!(to_lead(&starts).len(), 1, "heard blocked: {starts:?}");

    // A turn lands on it and ends with nothing new: blocked as before, and nothing more to hear.
    let starts = work(&mut conn, task, None);
    assert_eq!(state(&conn, task).0, State::Blocked);
    assert!(
        to_lead(&starts).is_empty(),
        "the same outcome twice: {starts:?}"
    );

    // Restarted by its giver, it ends the same way: the giver hears how the run it asked for ended.
    tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    let starts = work(&mut conn, task, Some(Finish::Blocked));
    assert_eq!(to_lead(&starts).len(), 1, "{starts:?}");

    // Its owner reports it done from another turn: its giver hears that.
    let (_, _, starts) = tasks::finish(
        &mut conn,
        "ann",
        task,
        Finish::Done,
        &note("fixed in #9"),
        T,
    )
    .unwrap();
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let line = format!("#{task} Ann, Fix the build: done: fixed in #9");
    assert!(back[0].text.contains(&line), "{}", back[0].text);
}

/// G164: a task given in passing doesn't hold the task whose turn gave it, the hear-back on the
/// others doesn't wait for it, and it is heard back on by itself.
#[test]
fn a_task_given_in_passing_holds_nothing_and_is_heard_back_on_by_itself() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    let giving = begin(&mut conn, lead_task);
    let (needed, _) = give(&mut conn, "ann", &[], Some(&giving));
    let aside = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: Some(&giving),
        parent: None,
        kind: Kind::Work,
        what: "Tidy the notes",
        why: None,
        after: &[],
        pr: None,
        in_passing: true,
    };
    let aside = tasks::give(&mut conn, &aside, T).unwrap().0.number;
    gathr::store::set_turn(&conn, &giving, "done", None).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok, T).unwrap();
    let held = Some(format!("waiting on #{needed} (Ann)"));
    assert_eq!(state(&conn, lead_task), (State::Held, held));

    let starts = work(&mut conn, needed, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "it doesn't wait for #{aside}: {starts:?}");
    assert!(back[0].text.contains(&format!("#{needed} Ann")));
    assert!(
        !back[0].text.contains(&format!("#{aside} Bo")),
        "{}",
        back[0].text
    );

    let starts = work(&mut conn, aside, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(said.starts_with("The task you gave is settled:"), "{said}");
    assert!(said.contains(&format!("#{aside} Bo")), "{said}");
}

/// The turn Gathr queued on a task.
fn turn_on(conn: &Connection, task: i64) -> String {
    let sql = "SELECT id FROM turns WHERE task = ?1 ORDER BY rowid LIMIT 1";
    conn.query_row(sql, [task], |r| r.get(0)).unwrap()
}

/// A turn of Lead's on no task that has ended, answering `answers`.
fn lead_turn(conn: &Connection, id: &str, answers: Option<&str>) {
    gathr::store::add_turn(conn, "c", id, T, (Some("lead"), None, answers)).unwrap();
    gathr::store::set_turn(conn, id, "done", None).unwrap();
}

/// What Gathr said in the chat in no turn.
fn lines(conn: &Connection) -> Vec<String> {
    let sql = "SELECT body FROM messages WHERE chat_id = 'c' AND turn_id IS NULL AND role = 'gathr'
               ORDER BY rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([], |r| r.get(0)).unwrap();
    rows.collect::<Result<_, _>>().unwrap()
}

/// 5e: a task that ends blocked, failed or stopped is heard at once, by itself, while the others
/// still run. The rest is heard when all of it is in, and only that.
#[test]
fn trouble_is_heard_at_once_while_the_others_still_run() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    lead_turn(&conn, "lead-turn", None);
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    let (c, _) = give(&mut conn, "ann", &[], Some("lead-turn"));

    begin(&mut conn, a);
    let starts =
        tasks::ended(&mut conn, a, Outcome::Failed("the engine exited".into()), T).unwrap();
    let back = to_lead(&starts);
    assert_eq!(
        back.len(),
        1,
        "a failure, while #{b} and #{c} are open: {starts:?}"
    );
    assert!(
        back[0].text.contains(&format!("#{a} Ann")),
        "{}",
        back[0].text
    );
    assert!(
        !back[0].text.contains(&format!("#{b} Bo")),
        "{}",
        back[0].text
    );

    let starts = work(&mut conn, b, Some(Finish::Done));
    assert!(to_lead(&starts).is_empty(), "#{c} is open: {starts:?}");
    let starts = work(&mut conn, c, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(
        said.starts_with("The 2 tasks you gave are settled:"),
        "{said}"
    );
    assert!(
        !said.contains(&format!("#{a} Ann, Fix the build: failed")),
        "{said}"
    );
}

/// 5e: a lead that gave tasks in three turns of one round hears once, when all are in, in a turn
/// that answers the newest of the three. Work from a later message of the person's is another
/// round: it is heard by itself and holds nothing back. The service says which round a task is in.
#[test]
fn work_given_in_three_turns_of_one_round_is_heard_once_when_all_of_it_is_in() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    // The person writes: the round's first turn. A member's message wakes the lead again, and
    // that turn's own follow-up gives more.
    lead_turn(&conn, "r1", None);
    let (a, _) = give(&mut conn, "ann", &[], Some("r1"));
    lead_turn(&conn, "r2", Some(&turn_on(&conn, a)));
    let (b, _) = give(&mut conn, "bo", &[], Some("r2"));
    lead_turn(&conn, "r3", Some("r2"));
    let (c, _) = give(&mut conn, "ann", &[], Some("r3"));
    // The person writes again: another round.
    lead_turn(&conn, "p2", None);
    let (d, _) = give(&mut conn, "bo", &[], Some("p2"));

    let listed = tasks::list(&conn, "c").unwrap();
    let round = |n: i64| listed.iter().find(|t| t.number == n).unwrap().round.clone();
    assert_eq!(
        [round(a), round(b), round(c)],
        [Some("r1".to_string()), Some("r1".into()), Some("r1".into())]
    );
    assert_eq!(round(d), Some("p2".to_string()));

    let starts = work(&mut conn, a, Some(Finish::Done));
    assert!(
        to_lead(&starts).is_empty(),
        "#{b} and #{c} are open: {starts:?}"
    );
    let starts = work(&mut conn, d, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(
        back.len(),
        1,
        "the newer round waits for nothing of the older: {starts:?}"
    );
    assert_eq!(back[0].answers.as_deref(), Some("p2"));
    assert!(
        !back[0].text.contains(&format!("#{a} Ann")),
        "{}",
        back[0].text
    );

    let starts = work(&mut conn, b, Some(Finish::Done));
    assert!(to_lead(&starts).is_empty(), "#{c} is open: {starts:?}");
    let starts = work(&mut conn, c, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "one hear-back for the round: {starts:?}");
    assert_eq!(
        back[0].answers.as_deref(),
        Some("r3"),
        "the newest turn that gave work"
    );
    assert_eq!(back[0].task, None);
    let said = &back[0].text;
    assert!(
        said.starts_with("The 3 tasks you gave are settled:"),
        "{said}"
    );
    assert!(
        !said.contains(&format!("#{d} Bo")),
        "heard with its own round: {said}"
    );
}

/// 5e: a builder whose review came back clean gets no turn: its task's reason names only its merge,
/// and Gathr lands it (0049). A second task of the same builder's that it never reported done
/// gets its hear-back turn, as before.
#[test]
fn a_clean_review_gives_its_builder_no_turn_once_the_work_is_reported_done() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = work_on_pr(&mut conn, 12);
    let giving = begin(&mut conn, fix);
    let review = review_by_bo(&mut conn, 12, &giving);
    finish(&mut conn, "ann", fix, Finish::Done, &note("the PR is open")).unwrap();
    gathr::store::set_turn(&conn, &giving, "done", None).unwrap();
    tasks::ended(&mut conn, fix, Outcome::Ok, T).unwrap();
    let held = format!("done, waiting on #{review} (Bo), #12 to merge");
    assert_eq!(state(&conn, fix), (State::Held, Some(held)));

    let starts = reviewed(&mut conn, review);
    assert!(
        to(&starts, "ann").is_empty(),
        "nothing for Ann to do: {starts:?}"
    );
    assert!(to(&starts, "lead").is_empty(), "not landed yet: {starts:?}");
    let held = Some("done, waiting on #12 to merge".to_string());
    assert_eq!(
        state(&conn, fix),
        (State::Held, held),
        "only its merge is left"
    );

    let merged = tasks::pr_looked(&mut conn, fix, "MERGED", None, None, T).unwrap();
    assert_eq!(
        to(&merged, "lead").len(),
        1,
        "Lead hears it landed: {merged:?}"
    );
    assert!(to(&merged, "ann").is_empty(), "{merged:?}");

    // Not reported done: its owner still has something to say about it.
    let other = work_on_pr(&mut conn, 13);
    let giving = begin(&mut conn, other);
    let review = review_by_bo(&mut conn, 13, &giving);
    gathr::store::set_turn(&conn, &giving, "done", None).unwrap();
    tasks::ended(&mut conn, other, Outcome::Ok, T).unwrap();
    let starts = reviewed(&mut conn, review);
    let back: Vec<&Start> = starts.iter().filter(|s| s.agent == "ann").collect();
    assert_eq!(back.len(), 1, "{starts:?}");
    assert_eq!(back[0].task, Some(other), "on its own task");
}

/// 5e: a review that isn't clean is trouble, and its builder hears at once, on its task.
#[test]
fn a_review_that_is_not_clean_wakes_its_builder_on_the_work() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = work_on_pr(&mut conn, 14);
    let giving = begin(&mut conn, fix);
    let review = review_by_bo(&mut conn, 14, &giving);
    finish(&mut conn, "ann", fix, Finish::Done, &note("the PR is open")).unwrap();
    gathr::store::set_turn(&conn, &giving, "done", None).unwrap();
    tasks::ended(&mut conn, fix, Outcome::Ok, T).unwrap();

    let starts = work(&mut conn, review, Some(Finish::Blocked));
    let back: Vec<&Start> = starts.iter().filter(|s| s.agent == "ann").collect();
    assert_eq!(back.len(), 1, "{starts:?}");
    assert_eq!(back[0].task, Some(fix));
    assert!(
        back[0].text.contains("blocked: see the change"),
        "{}",
        back[0].text
    );
}

/// 5e: a task that ends done is a line in the chat when it ends, said by Gathr in no turn, once.
/// Trouble has no such line: its giver's turn says it.
#[test]
fn a_task_that_ends_done_is_a_line_in_the_chat_and_trouble_is_not() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    lead_turn(&conn, "lead-turn", None);
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    assert!(lines(&conn).is_empty());

    work(&mut conn, a, Some(Finish::Done));
    let said = lines(&conn);
    assert_eq!(said.len(), 1, "{said:?}");
    let line = format!("- #{a} Ann, Fix the build: done: see the change");
    assert!(said[0].ends_with(&line), "{}", said[0]);

    work(&mut conn, b, Some(Finish::Blocked));
    assert_eq!(
        lines(&conn).len(),
        1,
        "blocked isn't an end: {:?}",
        lines(&conn)
    );
    // Reported done after all, from another turn: now it ended.
    finish(&mut conn, "bo", b, Finish::Done, &note("fixed in #9")).unwrap();
    let said = lines(&conn);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[1].contains(&format!("#{b} Bo")), "{}", said[1]);
}

/// G158: what a lead hands on after the person's answer is a new round, as after their message.
#[test]
fn the_persons_answer_begins_a_new_round() {
    use gathr::tasks::person::{self, Answer, Ask};
    let dir = TempDir::new();
    let mut conn = team(&dir);
    lead_turn(&conn, "r1", None);
    let (a, _) = give(&mut conn, "ann", &[], Some("r1"));
    let ask = Ask {
        chat: "c",
        giver: "lead",
        given_in: Some("r1"),
        parent: None,
        what: Some("Ship the notice today?"),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    let (asked, _) = person::ask(&mut conn, ask, T).unwrap();
    let answer = Answer {
        task: asked.number,
        text: Some("Yes".into()),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[answer], T).unwrap();
    let woken = &to_lead(&sent.starts)[0].turn;
    assert_eq!(
        to_lead(&sent.starts).len(),
        1,
        "an answer wakes at once, with #{a} open"
    );
    gathr::store::set_turn(&conn, woken, "done", None).unwrap();
    let (b, _) = give(&mut conn, "bo", &[], Some(woken));

    let listed = tasks::list(&conn, "c").unwrap();
    let round = |n: i64| listed.iter().find(|t| t.number == n).unwrap().round.clone();
    assert_eq!(round(a).as_deref(), Some("r1"));
    assert_eq!(round(b).as_deref(), Some(woken.as_str()));
    let starts = work(&mut conn, a, Some(Finish::Done));
    assert_eq!(
        to_lead(&starts).len(),
        1,
        "#{b} is another round's: {starts:?}"
    );
}

/// Found in real use: a chat reads in the order its turns began, and a line said in no turn has
/// none, so on a reload every such line sat at the foot of the chat. It reads after the block of
/// whoever spoke last before it.
#[test]
fn a_line_said_in_no_turn_reads_where_it_was_said() {
    let dir = TempDir::new();
    let conn = team(&dir);
    let say = |turn: &str, text: &str| {
        gathr::store::add_message(&conn, "c", turn, "assistant", "text", text).unwrap();
    };
    for (id, agent) in [("first", "ann"), ("second", "bo")] {
        gathr::store::add_turn(&conn, "c", id, T, (Some(agent), None, None)).unwrap();
        gathr::store::set_turn(&conn, id, "running", None).unwrap();
    }
    say("first", "Ann starts");
    say("second", "Bo starts");
    say("first", "Ann ends");
    gathr::store::say(&conn, "c", "A task is done:\n- #1 Ann, Fix the build: done").unwrap();
    say("second", "Bo goes on");

    let history = gathr::store::history(&conn, "c").unwrap();
    let read: Vec<&str> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["body"].as_str().unwrap().lines().next().unwrap())
        .collect();
    let order = [
        "Ann starts",
        "Ann ends",
        "A task is done:",
        "Bo starts",
        "Bo goes on",
    ];
    assert_eq!(read, order);
}
