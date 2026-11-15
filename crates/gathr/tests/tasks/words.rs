//! Samples of the words Gathr opens task turns with (Stage 3, part A), built through the real calls
//! from fixed made-up input, each with the kind and version its turn records.

use super::*;
use gathr::tasks::Brief;
use gathr::tasks::person::{self, Answer, Ask, Choice, File};
use gathr::words;

/// The start's text is the sample `name`, and its turn records `kind` at `version`.
fn sampled(conn: &Connection, name: &str, start: &Start, (kind, version): (&str, i64)) {
    samples::same(name, &start.text);
    let recorded = samples::opened(conn, &start.turn);
    assert_eq!(recorded, (Some(kind.to_string()), Some(version)), "{name}");
}

fn only<'a>(starts: &'a [Start], agent: &str) -> &'a Start {
    let to: Vec<&Start> = starts.iter().filter(|s| s.agent == agent).collect();
    assert_eq!(to.len(), 1, "one start for {agent}: {starts:?}");
    to[0]
}

fn given(conn: &mut Connection, g: &Give) -> (i64, Vec<Start>) {
    let (task, starts) = tasks::give(conn, g, T).unwrap();
    (task.number, starts)
}

fn work_for(owner: &'static str, kind: Kind, what: &'static str, pr: Option<i64>) -> Give<'static> {
    Give {
        chat: "c",
        giver: Some("lead"),
        owner,
        given_in: None,
        parent: None,
        kind,
        what,
        brief: match kind {
            Kind::Work => common::COMPLETE,
            _ => Brief::default(),
        },
        after: &[],
        pr,
        in_passing: false,
    }
}

#[test]
fn briefs_are_the_kept_samples() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let brief = ("brief", words::BRIEF);
    // Tasks given before 0059 have no done, proof or where, and some no why: their briefs still
    // start them when they are restarted.
    conn.execute_batch(
        "INSERT INTO tasks (chat_id, giver, owner, kind, what, why, state, created_ts, updated_ts) VALUES
             ('c', 'lead', 'ann', 'work', 'Fix the build', NULL, 'cancelled', 1, 1),
             ('c', 'lead', 'ann', 'work', 'Fix the build', 'CI is red', 'cancelled', 1, 1);",
    )
    .unwrap();
    let starts = tasks::restart(&mut conn, Some("lead"), 1, T).unwrap();
    sampled(&conn, "brief: no why", &starts[0], brief);
    let starts = tasks::restart(&mut conn, Some("lead"), 2, T).unwrap();
    sampled(&conn, "brief: why", &starts[0], brief);
    tasks::started(&mut conn, 2, T).unwrap();
    let failed = Outcome::Failed("the engine exited".into());
    tasks::ended(&mut conn, 2, failed, T).unwrap();
    let starts = tasks::restart(&mut conn, Some("lead"), 2, T).unwrap();
    sampled(&conn, "brief: carry", &starts[0], brief);

    let review = ("review_brief", words::REVIEW_BRIEF);
    let mut fix = work_for("ann", Kind::Work, "Fix the label", Some(70));
    fix.brief.why = Some("the label is cut off");
    let (fix, _) = given(&mut conn, &fix);
    tasks::pr_looked(&mut conn, fix, "OPEN", Some("abc1234def5678"), None, T).unwrap();
    let (_, starts) = given(
        &mut conn,
        &work_for("bo", Kind::Review, "Review #70", Some(70)),
    );
    sampled(&conn, "review_brief: head seen, a task", &starts[0], review);
    let no_pr = work_for("bo", Kind::Review, "Review the notes", None);
    let (_, starts) = given(&mut conn, &no_pr);
    sampled(&conn, "review_brief: no PR", &starts[0], review);

    let dir = TempDir::new();
    let mut conn = team(&dir);
    let complete = work_for("ann", Kind::Work, "Fix the build", None);
    let (_, starts) = given(&mut conn, &complete);
    sampled(&conn, "brief: complete", &starts[0], brief);
    let mut sized = work_for("ann", Kind::Work, "Fix the build", None);
    sized.brief.size = Some(("src/build.rs", 40));
    let (_, starts) = given(&mut conn, &sized);
    sampled(&conn, "brief: size", &starts[0], brief);
}

#[test]
fn a_members_requests_brief_its_lead_and_the_answers_come_back_as_kept() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let request = ("request_brief", words::REQUEST_BRIEF);
    let (work, _) = given(
        &mut conn,
        &work_for("ann", Kind::Work, "Word the notice", None),
    );
    turn(&conn, "ann-1", "ann", Some(work), "running");
    let choice = |label: &str, detail: Option<&str>| Choice {
        label: label.into(),
        detail: detail.map(String::from),
    };
    let ask = Ask {
        chat: "c",
        giver: "ann",
        given_in: Some("ann-1"),
        parent: None,
        what: Some("What should the reconnect notice say?"),
        why: Some("the notice ships today"),
        form: None,
        choices: vec![
            choice("Reconnecting…", Some("it comes back by itself")),
            choice("Connection lost", None),
        ],
        files: vec![File {
            name: "notice.png".into(),
            size: 2048,
            path: "made-up/notice.png".into(),
        }],
        request: None,
    };
    let (asked, starts) = person::ask(&mut conn, ask, T).unwrap();
    sampled(&conn, "request_brief: choice", &starts[0], request);
    let words_only = Ask {
        chat: "c",
        giver: "ann",
        given_in: Some("ann-1"),
        parent: None,
        what: Some("Which port should the dev service use?"),
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: None,
    };
    let (_, starts) = person::ask(&mut conn, words_only, T).unwrap();
    sampled(&conn, "request_brief: text", &starts[0], request);

    // The lead files the first as it is, and the person answers it.
    turn(&conn, "lead-1", "lead", Some(asked.number), "running");
    let filing = Ask {
        chat: "c",
        giver: "lead",
        given_in: Some("lead-1"),
        parent: None,
        what: None,
        why: None,
        form: None,
        choices: Vec::new(),
        files: Vec::new(),
        request: Some(asked.number),
    };
    let (filed, _) = person::ask(&mut conn, filing, T).unwrap();
    let moved = tasks::Moved {
        chat: "c",
        turn: "lead-1",
        agent: Some("lead"),
        task: Some(asked.number),
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(&mut conn, &moved, "done", None).unwrap();
    let answer = Answer {
        task: filed.number,
        pick: vec!["Connection lost".into()],
        text: Some("and say we retry".into()),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[answer], T).unwrap();
    let answered = ("answered", words::ANSWERED);
    sampled(
        &conn,
        "answered: a member's request",
        &sent.starts[0],
        answered,
    );
}

#[test]
fn the_persons_answer_to_the_agents_own_question_comes_back_as_kept() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-1", "lead", None, "running");
    let ask = Ask {
        chat: "c",
        giver: "lead",
        given_in: Some("lead-1"),
        parent: None,
        what: Some("Which name should the app use?"),
        why: None,
        form: None,
        choices: ["Gathr", "Gather"]
            .map(|l| Choice {
                label: l.into(),
                detail: None,
            })
            .to_vec(),
        files: Vec::new(),
        request: None,
    };
    let (asked, _) = person::ask(&mut conn, ask, T).unwrap();
    gathr::store::set_turn(&conn, "lead-1", "done", None).unwrap();
    let answer = Answer {
        task: asked.number,
        pick: vec!["Gathr".into()],
        text: Some("keep it short".into()),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[answer], T).unwrap();
    let answered = ("answered", words::ANSWERED);
    sampled(
        &conn,
        "answered: the agent's own question",
        &sent.starts[0],
        answered,
    );
}

#[test]
fn the_ask_for_a_report_and_hear_backs_are_the_kept_samples() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "done");
    let (task, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let starts = work(&mut conn, task, None);
    let ask = only(&starts, "ann");
    sampled(
        &conn,
        "report_ask: no report",
        ask,
        ("report_ask", words::REPORT_ASK),
    );

    let id = begin(&mut conn, task);
    let said = Said {
        file: Some("notes.md"),
        ..Said::default()
    };
    finish(&mut conn, "ann", task, Finish::Done, &said).unwrap();
    gathr::store::set_turn(&conn, &id, "done", None).unwrap();
    let starts = tasks::ended(&mut conn, task, Outcome::Ok, T).unwrap();
    let heard = ("hear_back", words::HEAR_BACK);
    sampled(
        &conn,
        "hear_back: one settled, done",
        only(&starts, "lead"),
        heard,
    );
}

#[test]
fn hear_backs_of_several_tasks_and_of_one_that_stands_are_the_kept_samples() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let heard = ("hear_back", words::HEAR_BACK);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    work(&mut conn, a, Some(Finish::Done));
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    gathr::store::set_turn(&conn, "lead-turn", "done", None).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok, T).unwrap();
    tasks::turn_ended(&mut conn, "lead-turn", T).unwrap();
    let starts = tasks::cancel(&mut conn, Some("lead"), b, "not needed", T).unwrap();
    sampled(
        &conn,
        "hear_back: several settled",
        only(&starts, "lead"),
        heard,
    );

    // Lead reviews #90 and gave the fix on it from that review: the fix, done and held on the
    // merge, stands, as the merge waits on Lead's review.
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (review, _) = given(
        &mut conn,
        &work_for("lead", Kind::Review, "Review #90", Some(90)),
    );
    tasks::started(&mut conn, review, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(review), "running");
    let mut fix = work_for("ann", Kind::Work, "Fix the note", Some(90));
    fix.given_in = Some("lead-turn");
    let (fix, _) = given(&mut conn, &fix);
    tasks::ended(&mut conn, review, Outcome::Ok, T).unwrap();
    gathr::store::set_turn(&conn, "lead-turn", "done", None).unwrap();
    let starts = work(&mut conn, fix, Some(Finish::Done));
    sampled(&conn, "hear_back: one stands", only(&starts, "lead"), heard);
}

#[test]
fn a_wait_that_fires_or_expires_wakes_its_agent_with_the_kept_samples() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fired = ("wait_fired", words::WAIT_FIRED);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let wait = Leave {
        chat: "c",
        agent: "lead",
        task: Some(lead_task),
        left_in: Some("lead-turn"),
        cwd: None,
        then_do: "merge it",
        parts: vec![part("ci", "#54")],
    };
    let wait = waits::leave(&mut conn, &wait, T).unwrap();
    let line = "CI on #54 finished: 4 passed";
    let starts = waits::met(&mut conn, &wait.id, line, T).unwrap();
    sampled(&conn, "wait_fired: on a task", &starts[0], fired);

    let wait = wait_on(&mut conn, vec![part("check", "test -f done")]).unwrap();
    let starts = waits::met(&mut conn, &wait.id, "your check came back yes", T).unwrap();
    sampled(&conn, "wait_fired: no task", &starts[0], fired);

    let wait = wait_on(&mut conn, vec![part("check", "test -f done")]).unwrap();
    waits::looked(&conn, &wait.id, None, None, Some("bash didn't start")).unwrap();
    let late = gathr::ids::now_us() + waits::WAIT_FOR_US;
    let starts = waits::expire(&mut conn, late, T).unwrap();
    sampled(&conn, "wait_fired: expired", &starts[0], fired);
}
