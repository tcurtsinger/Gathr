//! The person's tasks (F9, 0010) on a real database: only the Supervisor gives them, a member's ask
//! reaches the Supervisor first, and the person's answer travels back to the member who needed it.

mod common;

use common::TempDir;
use gathr::tasks::person::{self, Answer, Ask, Choice, Form};
use gathr::tasks::{self, Finish, Kind, Said, Start, State};
use rusqlite::Connection;

const T: &str = "test-trace";

/// A chat with a Supervisor (`lead`) and a member (`ann`), and a chat of `solo`'s alone.
fn team(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1),
             ('solo', 'Solo', 'solo', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1), ('alone', 'Alone', 1);
         INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1);",
    )
    .unwrap();
    conn
}

fn running(conn: &Connection, chat: &str, id: &str, agent: &str, task: Option<i64>) {
    gathr::store::add_turn(
        conn,
        chat,
        id,
        T,
        (Some(agent), task, None),
        gathr::words::Opened::Person,
    )
    .unwrap();
    conn.execute("UPDATE turns SET status = 'running' WHERE id = ?1", [id])
        .unwrap();
    if let Some(task) = task {
        conn.execute(
            "UPDATE tasks SET state = 'working' WHERE number = ?1",
            [task],
        )
        .unwrap();
    }
}

fn ended(conn: &mut Connection, turn: &str, agent: &str, task: Option<i64>) -> Vec<Start> {
    let m = tasks::Moved {
        chat: "c",
        turn,
        agent: Some(agent),
        task,
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(conn, &m, "done", None).unwrap()
}

fn ask<'a>(chat: &'a str, giver: &'a str, turn: &'a str, what: Option<&'a str>) -> Ask<'a> {
    Ask {
        chat,
        giver,
        given_in: Some(turn),
        parent: None,
        what,
        why: Some("the notice ships today"),
        form: None,
        choices: ["Reconnecting…", "Connection lost"]
            .map(|l| Choice {
                label: l.into(),
                detail: None,
            })
            .to_vec(),
        files: Vec::new(),
        request: None,
    }
}

#[test]
fn a_members_ask_goes_through_the_supervisor_to_the_person_and_the_answer_comes_back() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    // Ann works on a task of Lead's and needs the person.
    let g = tasks::Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Word the notice",
        brief: common::COMPLETE,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: false,
    };
    let (work, _) = tasks::give(&mut conn, &g, T).unwrap();
    running(&conn, "c", "ann-1", "ann", Some(work.number));
    let what = Some("What should the reconnect notice say?");
    let (request, starts) = person::ask(&mut conn, ask("c", "ann", "ann-1", what), T).unwrap();
    assert_eq!(request.kind, Kind::PersonRequest);
    assert_eq!(
        request.owner.as_deref(),
        Some("lead"),
        "only the lead asks the person"
    );
    assert_eq!(request.form, Some(Form::Choice));
    let brief = &starts[0].text;
    assert_eq!(starts[0].agent, "lead");
    assert!(brief.contains("Ann needs the person's answer"), "{brief}");
    assert!(brief.contains("1. Reconnecting… (their pick)"), "{brief}");
    assert!(
        person::open(&conn, None).unwrap().is_empty(),
        "not on the person's list before the check"
    );

    // The lead files it in its own words.
    running(&conn, "c", "lead-1", "lead", Some(request.number));
    let mut filing = ask(
        "c",
        "lead",
        "lead-1",
        Some("Which notice text should ship?"),
    );
    filing.request = Some(request.number);
    let (asked, starts) = person::ask(&mut conn, filing, T).unwrap();
    assert!(starts.is_empty(), "the person's task starts no turn");
    assert_eq!((asked.kind, asked.owner.clone()), (Kind::Person, None));
    assert_eq!(asked.request, Some(request.number));
    let open = person::open(&conn, None).unwrap();
    let listed = (
        open[0].task.number,
        open[0].chat_title.as_str(),
        open[0].for_agent.as_deref(),
    );
    assert_eq!(listed, (asked.number, "Chat", Some("ann")));
    ended(&mut conn, "lead-1", "lead", Some(request.number));
    let held = tasks::get(&conn, request.number).unwrap().unwrap();
    assert_eq!(held.state, State::Held);
    assert_eq!(
        held.reason.as_deref(),
        Some(format!("waiting on #{} (the person)", asked.number).as_str())
    );

    // The person answers: the request is done with it, and Ann hears it at once, then the lead.
    conn.execute("UPDATE turns SET status = 'done' WHERE id = 'ann-1'", [])
        .unwrap();
    let answer = Answer {
        task: asked.number,
        pick: vec!["Connection lost".into()],
        text: Some("and say we retry".into()),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[answer], T).unwrap();
    let (answered, starts) = (sent.answered, sent.starts);
    assert_eq!(answered[0].state, State::Done);
    assert_eq!(
        answered[0].reason.as_deref(),
        Some("Connection lost: and say we retry")
    );
    let (n, r) = (asked.number, request.number);
    let agents: Vec<&str> = starts.iter().map(|s| s.agent.as_str()).collect();
    assert_eq!(agents, ["ann", "lead"], "{starts:?}");
    assert!(
        starts[0].text.contains(&format!(
            "#{r} Lead, What should the reconnect notice say?: done: the person answered #{n} (Which notice text \
             should ship?): Picked: Connection lost. Their words: and say we retry"
        )),
        "{}",
        starts[0].text
    );
    let back = &starts[1];
    assert_eq!(
        (back.task, back.answers.as_deref()),
        (None, Some("lead-1")),
        "under the turn that asked"
    );
    assert!(back.text.contains("Ann has the answer."), "{}", back.text);
}

#[test]
fn each_answer_in_a_send_is_answered_or_refused_on_its_own() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    running(&conn, "c", "lead-1", "lead", None);
    let mut yesno = ask("c", "lead", "lead-1", Some("Ship it today?"));
    (yesno.form, yesno.choices) = (Some(Form::Yesno), Vec::new());
    let (first, _) = person::ask(&mut conn, yesno, T).unwrap();
    let mut todo = ask("c", "lead", "lead-1", Some("Sign the release"));
    todo.choices.clear();
    let (second, _) = person::ask(&mut conn, todo, T).unwrap();
    let (_, _) = person::ask(
        &mut conn,
        ask("c", "lead", "lead-1", Some("Which notice?")),
        T,
    )
    .unwrap();
    assert_eq!(second.form, Some(Form::Done));

    let yes = Answer {
        task: first.number,
        yes: Some(true),
        ..Default::default()
    };
    let empty = Answer {
        task: second.number,
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[yes, empty], T).unwrap();
    assert_eq!(sent.answered.len(), 1);
    assert_eq!(
        sent.refused,
        [(
            second.number,
            format!("Answer #{} with done, or enter text", second.number)
        )]
    );
    assert_eq!(sent.starts.len(), 1, "one hear-back for one Send");
    assert!(
        sent.starts[0].text.contains("Ship it today?: Picked: Yes"),
        "{}",
        sent.starts[0].text
    );
    assert!(!sent.starts[0].text.contains("Sign the release"));
    // The chat shows the answer on the turn its giver hears it in.
    assert_eq!(
        sent.answered[0].heard_in.as_deref(),
        Some(sent.starts[0].turn.as_str())
    );
    let open: Vec<i64> = person::open(&conn, Some("c"))
        .unwrap()
        .iter()
        .map(|o| o.task.number)
        .collect();
    assert_eq!(
        open.len(),
        2,
        "the refused one stays open, with the unsent one: {open:?}"
    );
    assert!(open.contains(&second.number));
    assert!(person::open(&conn, Some("alone")).unwrap().is_empty());

    let again = Answer {
        task: first.number,
        yes: Some(false),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[again], T).unwrap();
    assert_eq!(
        sent.refused,
        [(first.number, format!("#{} is already done", first.number))]
    );
    assert!(sent.starts.is_empty());
}

#[test]
fn the_chats_only_agent_asks_the_person_itself_and_hears_when_its_ask_is_cancelled() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    running(&conn, "alone", "solo-1", "solo", None);
    let mut a = ask("alone", "solo", "solo-1", Some("Pick the design?"));
    a.choices.clear();
    let (asked, _) = person::ask(&mut conn, a, T).unwrap();
    assert_eq!((asked.kind, asked.form), (Kind::Person, Some(Form::Text)));
    conn.execute("UPDATE turns SET status = 'done' WHERE id = 'solo-1'", [])
        .unwrap();
    let starts = tasks::cancel(&mut conn, None, asked.number, "not needed", T).unwrap();
    assert_eq!(starts.len(), 1, "{starts:?}");
    assert!(
        starts[0]
            .text
            .contains("Pick the design?: cancelled (not needed)"),
        "{}",
        starts[0].text
    );
    let agents_task = conn
        .query_row(
            "SELECT count(*) FROM tasks WHERE kind != 'person'",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap();
    assert_eq!(agents_task, 0);
    let wrong = Answer {
        task: asked.number,
        text: Some("late".into()),
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[wrong], T).unwrap();
    let why = format!("#{} is already cancelled", asked.number);
    assert_eq!(sent.refused, [(asked.number, why)]);
}

/// G183, live: a member's request was cancelled after the person had answered it, and the member's
/// work, blocked on the request, was cancelled with it. The answer settles the request as done at
/// once (0065), so what waited on it carries on with the answer.
#[test]
fn work_blocked_on_a_request_the_person_answers_carries_on_with_the_answer() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let g = tasks::Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Word the notice",
        brief: common::COMPLETE,
        after: &[],
        pr: None,
        in_passing: false,
        look_into: false,
    };
    let (work, _) = tasks::give(&mut conn, &g, T).unwrap();
    running(&conn, "c", "ann-1", "ann", Some(work.number));
    let what = Some("What should the reconnect notice say?");
    let (request, _) = person::ask(&mut conn, ask("c", "ann", "ann-1", what), T).unwrap();
    // Ann says her work waits on the request, and her turn ends.
    let on = Said {
        on_task: Some(request.number),
        ..Default::default()
    };
    tasks::finish(&mut conn, "ann", work.number, Finish::Blocked, &on, T).unwrap();
    ended(&mut conn, "ann-1", "ann", Some(work.number));
    assert_eq!(
        tasks::get(&conn, work.number).unwrap().unwrap().state,
        State::Blocked
    );
    // The lead files it, and the person answers.
    running(&conn, "c", "lead-1", "lead", Some(request.number));
    let mut filing = ask(
        "c",
        "lead",
        "lead-1",
        Some("Which notice text should ship?"),
    );
    filing.request = Some(request.number);
    let (asked, _) = person::ask(&mut conn, filing, T).unwrap();
    ended(&mut conn, "lead-1", "lead", Some(request.number));
    let answer = Answer {
        task: asked.number,
        pick: vec!["Connection lost".into()],
        ..Default::default()
    };
    let starts = person::answer(&mut conn, &[answer], T).unwrap().starts;
    let settled = tasks::get(&conn, request.number).unwrap().unwrap();
    let answered = format!(
        "the person answered #{} (Which notice text should ship?): Picked: Connection lost",
        asked.number
    );
    assert_eq!(
        (settled.state, settled.reason.as_deref()),
        (State::Done, Some(answered.as_str())),
        "done, with the answer"
    );
    let carried = tasks::get(&conn, work.number).unwrap().unwrap();
    assert_eq!(carried.state, State::Ready, "Ann's work carries on");
    let to_ann = starts
        .iter()
        .find(|s| s.agent == "ann" && s.task == Some(work.number));
    let to_ann = to_ann.expect("Ann's work starts again");
    let carry = format!(
        "the task blocking this one, #{}, is done: {answered}",
        request.number
    );
    assert!(to_ann.text.contains(&carry), "{}", to_ann.text);
}
