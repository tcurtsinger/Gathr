//! Questions in the lead's words (0065) on a real database: a member's request reaches the person
//! only in the lead's words, the answer settles it in the same transaction and its member hears at
//! once, agents hear the pick and the words apart, and Discuss takes a question off the person's
//! list until the lead's turn that heard it ends. These are coordination transitions inside one
//! transaction (a turn's end, an answer, a cancel, a restart), so they sit beside
//! `tests/person_tasks.rs` at the SQLite seam.

mod common;
#[path = "questions/discuss.rs"]
mod discuss;

use common::TempDir;
use gathr::Error;
use gathr::tasks::person::{self, Answer, Ask, Choice, File, Form, Sent};
use gathr::tasks::{self, Finish, Moved, Said, Start, State, Task};
use rusqlite::Connection;
use serde_json::{Value, json};

const T: &str = "test-trace";
const NEED: &str = "What should the reconnect notice say?";
const ASKED: &str = "Which notice text should ship?";
const PICKED: &str = "Picked: Connection lost. Their words: and say we retry";

/// Chat `c`: Mira leads, Ann and Bo are members. Mira gave Ann #1 and Bo #2, each on its running
/// turn (`ann-1`, `bo-1`).
fn room(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('mira', 'Mira', 'mira', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1), ('bo', 'Bo', 'bo', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1);
         INSERT INTO chat_members VALUES ('c', 'mira', 'supervisor', 1), ('c', 'ann', 'member', 1),
             ('c', 'bo', 'member', 1);
         INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts) VALUES
             ('c', 'mira', 'ann', 'work', 'Word the notice', 'ready', 1, 1),
             ('c', 'mira', 'bo', 'work', 'Word the banner', 'ready', 1, 1);",
    )
    .unwrap();
    running(&conn, "ann-1", "ann", Some(1));
    running(&conn, "bo-1", "bo", Some(2));
    conn
}

fn running(conn: &Connection, id: &str, agent: &str, task: Option<i64>) {
    let on = (Some(agent), task, None);
    gathr::store::add_turn(conn, "c", id, T, on, gathr::words::Opened::Person).unwrap();
    conn.execute("UPDATE turns SET status = 'running' WHERE id = ?1", [id])
        .unwrap();
    if let Some(task) = task {
        let sql = "UPDATE tasks SET state = 'working' WHERE number = ?1";
        conn.execute(sql, [task]).unwrap();
    }
}

/// The turn moves to `status`, as the service saves it.
fn moved(conn: &mut Connection, turn: &str, status: &str) -> Vec<Start> {
    let (agent, task): (String, Option<i64>) = conn
        .query_row(
            "SELECT agent_id, task FROM turns WHERE id = ?1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    let m = Moved {
        chat: "c",
        turn,
        agent: Some(&agent),
        task,
        trace: T,
        carries_on: None,
    };
    tasks::turn_moved(conn, &m, status, None).unwrap()
}

fn choices(labels: &[&str]) -> Vec<Choice> {
    let detail = |i: usize| (i == 0).then(|| "it says what happened".to_string());
    let made = labels.iter().enumerate().map(|(i, l)| Choice {
        label: l.to_string(),
        detail: detail(i),
    });
    made.collect()
}

/// A member's request from its `turn`, with its own why, choices and a file: its number.
fn need(conn: &mut Connection, member: &str, turn: &str) -> i64 {
    let a = Ask {
        chat: "c",
        giver: member,
        given_in: Some(turn),
        parent: None,
        what: Some(NEED),
        why: Some("the notice ships today"),
        form: None,
        choices: choices(&["Reconnecting…", "Connection lost"]),
        files: vec![file("notice.png")],
        request: None,
    };
    person::ask(conn, a, T).unwrap().0.number
}

fn file(name: &str) -> File {
    File {
        name: name.into(),
        size: 2048,
        path: format!("made-up/{name}"),
    }
}

/// Mira's question for `request` from her `turn`, in her own words.
fn filing<'a>(turn: &'a str, request: Option<i64>, what: Option<&'a str>) -> Ask<'a> {
    Ask {
        chat: "c",
        giver: "mira",
        given_in: Some(turn),
        parent: None,
        what,
        why: Some("Which text ships with 2.1"),
        form: None,
        choices: choices(&["Connection lost", "Reconnecting…"]),
        files: Vec::new(),
        request,
    }
}

fn file_for(conn: &mut Connection, turn: &str, request: i64) -> i64 {
    let a = filing(turn, Some(request), Some(ASKED));
    person::ask(conn, a, T).unwrap().0.number
}

/// Ann's request #3, filed as Mira's question #4 in her turn on it; both turns ended.
fn filed(conn: &mut Connection) -> (i64, i64) {
    let request = need(conn, "ann", "ann-1");
    running(conn, "mira-3", "mira", Some(request));
    let q = file_for(conn, "mira-3", request);
    moved(conn, "mira-3", "done");
    moved(conn, "ann-1", "done");
    (request, q)
}

fn answer(conn: &mut Connection, q: i64, pick: &[&str], text: Option<&str>) -> Sent {
    let a = Answer {
        task: q,
        pick: pick.iter().map(|p| p.to_string()).collect(),
        text: text.map(String::from),
        ..Default::default()
    };
    person::answer(conn, &[a], T).unwrap()
}

fn discuss(conn: &mut Connection, q: i64, text: &str) -> Sent {
    let a = Answer {
        task: q,
        discuss: true,
        text: Some(text.into()),
        ..Default::default()
    };
    person::answer(conn, &[a], T).unwrap()
}

fn task(conn: &Connection, n: i64) -> Task {
    tasks::get(conn, n).unwrap().unwrap()
}

fn to<'a>(starts: &'a [Start], agent: &str) -> Vec<&'a Start> {
    starts.iter().filter(|s| s.agent == agent).collect()
}

fn refusal<T: std::fmt::Debug>(result: Result<T, Error>) -> String {
    match result {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// Each `name` event's task and attributes, oldest first.
fn events(conn: &Connection, name: &str) -> Vec<(Option<String>, Value)> {
    let sql = "SELECT task_id, json(attrs) FROM events WHERE name = ?1 ORDER BY ts, rowid";
    let mut stmt = conn.prepare(sql).unwrap();
    let rows = stmt.query_map([name], |r| {
        Ok((r.get::<_, Option<String>>(0)?, r.get::<_, String>(1)?))
    });
    let rows = rows.unwrap().map(Result::unwrap);
    rows.map(|(t, a)| (t, serde_json::from_str(&a).unwrap()))
        .collect()
}

fn listed(conn: &Connection) -> Vec<i64> {
    let open = person::open(conn, None).unwrap();
    open.iter().map(|o| o.task.number).collect()
}

fn needs(conn: &Connection) -> Value {
    let chats = gathr::store::chats(conn, false).unwrap();
    chats.as_array().unwrap()[0]["needs"].clone()
}

fn finish(
    conn: &mut Connection,
    agent: &str,
    n: i64,
    how: Finish,
    said: &Said,
) -> Result<(), Error> {
    tasks::finish(conn, agent, n, how, said, T).map(|_| ())
}

/// F1, scenario 4a, G191.
#[test]
fn the_lead_files_a_request_only_in_its_own_words() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let a = Ask {
        chat: "c",
        giver: "ann",
        given_in: Some("ann-1"),
        parent: None,
        what: Some(NEED),
        why: Some("the notice ships today"),
        form: None,
        choices: choices(&["Reconnecting…", "Connection lost"]),
        files: vec![file("notice.png")],
        request: None,
    };
    let (request, starts) = person::ask(&mut conn, a, T).unwrap();
    let brief: Vec<&str> = starts[0].text.lines().collect();
    assert_eq!(
        brief[brief.len() - 2..],
        [
            "To ask the person, use ask_person with for_task 3 and your own `what`, your pick first. If an open \
             question of yours already asks it, use finish_task blocked with on_task set to that question: this \
             request comes back to you with its answer. To answer it yourself, use finish_task done with the answer \
             in note.",
            "If you ask the person, their answer reaches Ann by itself: after ask_person, end your turn with no finish_task."
        ]
    );
    running(&conn, "mira-3", "mira", Some(request.number));
    let count = |conn: &Connection| -> i64 {
        conn.query_row("SELECT count(*) FROM tasks", [], |r| r.get(0))
            .unwrap()
    };
    let copied = person::ask(&mut conn, filing("mira-3", Some(3), None), T);
    assert_eq!(
        refusal(copied),
        "Write `what` in your own words: request #3 isn't sent to the person as it is"
    );
    assert_eq!(count(&conn), 3, "nothing added");

    let (q, _) = person::ask(&mut conn, filing("mira-3", Some(3), Some(ASKED)), T).unwrap();
    let labels: Vec<&str> = q.choices.iter().map(|c| c.label.as_str()).collect();
    assert_eq!(
        (q.what.as_str(), q.why.as_deref(), q.form, labels),
        (
            ASKED,
            Some("Which text ships with 2.1"),
            Some(Form::Choice),
            vec!["Connection lost", "Reconnecting…"]
        ),
        "hers, not #3's"
    );
    let names = |t: &Task| t.files.iter().map(|f| f.name.clone()).collect::<Vec<_>>();
    assert_eq!(
        names(&q),
        ["notice.png"],
        "the lead can't reach Ann's worktree"
    );
    let mut own = filing("mira-3", Some(3), Some("Which notice image ships?"));
    own.files = vec![file("mine.png")];
    let (q, _) = person::ask(&mut conn, own, T).unwrap();
    assert_eq!(names(&q), ["mine.png"]);

    let mut bare = filing("ann-1", None, None);
    bare.giver = "ann";
    assert_eq!(
        refusal(person::ask(&mut conn, bare, T)),
        "say `what` the person is asked"
    );
}

/// F2.1, check objection 1.
#[test]
fn a_lead_cant_report_blocked_on_a_request_its_question_waits_for() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let request = need(&mut conn, "ann", "ann-1");
    running(&conn, "mira-3", "mira", Some(request));
    let q = file_for(&mut conn, "mira-3", request);
    let on = Said {
        on_task: Some(q),
        ..Default::default()
    };
    let said = "#3 waits on #4, the person's question, and its answer reaches Ann by itself. End your turn with \
                no finish_task.";
    let blocked = finish(&mut conn, "mira", request, Finish::Blocked, &on);
    assert_eq!(refusal(blocked), said);
    let mut starts = moved(&mut conn, "mira-3", "done");
    starts.extend(moved(&mut conn, "ann-1", "done"));
    let r = task(&conn, request);
    assert_eq!(
        (r.state, r.reason.as_deref()),
        (State::Held, Some("waiting on #4 (the person)"))
    );
    let work = task(&conn, 1);
    assert_eq!(
        (work.state, work.reason.as_deref()),
        (State::Held, Some("waiting on #3 (Mira)"))
    );
    assert!(to(&starts, "ann").is_empty(), "{starts:?}");

    discuss(&mut conn, q, "Does it retry by itself?");
    let blocked = finish(&mut conn, "mira", request, Finish::Blocked, &on);
    assert_eq!(refusal(blocked), said, "discussed, it still waits");
    let left = Said {
        note: Some("asked the person"),
        ..Default::default()
    };
    finish(&mut conn, "mira", request, Finish::Left, &left).unwrap();
    assert_eq!(task(&conn, request).state, State::Held);
}

/// F3, scenario 4c, G189, G105.
#[test]
fn one_answer_settles_the_request_at_once_and_its_member_hears() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (request, q) = filed(&mut conn);
    conn.execute(
        "INSERT INTO waits (id, chat_id, agent_id, task, kind, target, then_do, state, until_ts, created_ts)
         VALUES ('w', 'c', 'mira', ?1, 'pr', '7', 'carry on', 'open', 99, 1)",
        [request],
    )
    .unwrap();
    let sent = answer(&mut conn, q, &["Connection lost"], Some("and say we retry"));

    let asked = task(&conn, q);
    let asked = (asked.state, asked.reason.as_deref());
    assert_eq!(
        asked,
        (State::Done, Some("Connection lost: and say we retry"))
    );
    let r = task(&conn, request);
    let settled = format!("the person answered #4 ({ASKED}): {PICKED}");
    assert_eq!(
        (r.state, r.reason.as_deref()),
        (State::Done, Some(settled.as_str()))
    );
    let wait: (String, String) = conn
        .query_row("SELECT state, line FROM waits WHERE id = 'w'", [], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .unwrap();
    assert_eq!(
        wait,
        ("cancelled".into(), "its request was answered".into())
    );

    let ann = to(&sent.starts, "ann");
    assert_eq!((ann.len(), ann[0].task), (1, Some(1)), "{:?}", sent.starts);
    let line = format!("- #3 Mira, {NEED}: done: {settled}");
    assert!(ann[0].text.lines().any(|l| l == line), "{}", ann[0].text);
    let mira = to(&sent.starts, "mira");
    assert_eq!(
        (mira.len(), mira[0].task, mira[0].answers.as_deref()),
        (1, None, Some("mira-3")),
        "{:?}",
        sent.starts
    );
    assert_eq!(
        mira[0].text,
        format!(
            "The person answered:\n- #4 {ASKED}: {PICKED}\n#4 was asked for Ann (#3): Ann has the answer.\nContinue."
        )
    );
    let settled = events(&conn, "gathr.task.request");
    let attrs = json!({ "gathr.task.question": 4, "gen_ai.agent.id": "ann", "gathr.request.how": "answered" });
    assert_eq!(settled, [(Some("3".to_string()), attrs)]);
    let rowid = |turn: &str| -> i64 {
        conn.query_row("SELECT rowid FROM turns WHERE id = ?1", [turn], |r| {
            r.get(0)
        })
        .unwrap()
    };
    assert!(
        rowid(&ann[0].turn) < rowid(&mira[0].turn),
        "Ann's turn first"
    );
}

/// F4, scenario 4b.
#[test]
fn agents_hear_the_pick_and_the_words_apart() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    running(&conn, "mira-1", "mira", None);
    let answered =
        |conn: &mut Connection, form: Form, labels: &[&str], a: Answer, by: Option<&str>| {
            let request = need(conn, "ann", "ann-1");
            let mut ask = filing("mira-1", Some(request), Some(ASKED));
            ask.form = Some(form);
            ask.choices = choices(labels);
            let q = person::ask(conn, ask, T).unwrap().0.number;
            let sent = person::answer_as(conn, &[Answer { task: q, ..a }], by, T).unwrap();
            if let Some((_, why)) = sent.refused.first() {
                return (q, why.clone(), String::new());
            }
            let prefix = format!("the person answered #{q} ({ASKED}): ");
            let reason = task(conn, request).reason.unwrap_or_default();
            let heard = reason.strip_prefix(&prefix).unwrap_or(&reason).to_string();
            (q, heard, task(conn, q).reason.unwrap_or_default())
        };
    let sent = |mut a: Value| -> Answer {
        a["task"] = json!(0);
        serde_json::from_value(a).unwrap()
    };
    let cases = [
        (
            Form::Choice,
            "Short",
            json!({ "pick": ["Short"] }),
            "Picked: Short",
            "Short",
        ),
        (
            Form::Choice,
            "Short",
            json!({ "pick": ["Short"], "text": "and say we retry" }),
            "Picked: Short. Their words: and say we retry",
            "Short: and say we retry",
        ),
        (
            Form::Choice,
            "Short",
            json!({ "text": "none of these" }),
            "Their words: none of these",
            "none of these",
        ),
        (
            Form::Choices,
            "Short",
            json!({ "pick": ["Short", "Long"] }),
            "Picked: Short, Long",
            "Short, Long",
        ),
        (
            Form::Yesno,
            "",
            json!({ "yes": true }),
            "Picked: Yes",
            "Yes",
        ),
        (
            Form::Done,
            "",
            json!({ "done": true }),
            "Picked: Done",
            "Done",
        ),
        (
            Form::Choice,
            "Retry: later",
            json!({ "pick": ["Retry: later"] }),
            "Picked: Retry: later",
            "Retry: later",
        ),
    ];
    for (form, first, a, heard, merged) in cases {
        let labels = if first.is_empty() {
            vec![]
        } else {
            vec![first, "Long"]
        };
        let (_, got, line) = answered(&mut conn, form, &labels, sent(a), None);
        assert_eq!((got.as_str(), line.as_str()), (heard, merged), "{form:?}");
    }
    let two = ["Short", "Long"];
    let short = || sent(json!({ "pick": ["Short"] }));
    let (_, got, line) = answered(&mut conn, Form::Choice, &two, short(), Some("bo"));
    let by = "(answered by Bo for the person)";
    assert_eq!(
        (got, line),
        (format!("Picked: Short {by}"), format!("Short {by}"))
    );
    let (q, why, _) = answered(&mut conn, Form::Choice, &two, short(), Some("ann"));
    let own = "is its own question to the person: an agent never answers its own question to the person, the \
               person does";
    assert_eq!(why, format!("#{q} {own}"));
}

/// F3.4, check objection 6.
#[test]
fn a_settled_request_is_heard_at_once_beside_a_pending_one() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let first = need(&mut conn, "ann", "ann-1");
    let second = need(&mut conn, "ann", "ann-1");
    running(&conn, "mira-1", "mira", None);
    let q = file_for(&mut conn, "mira-1", first);
    file_for(&mut conn, "mira-1", second);
    moved(&mut conn, "mira-1", "done");
    moved(&mut conn, "ann-1", "done");
    let sent = answer(&mut conn, q, &["Connection lost"], None);
    let ann = to(&sent.starts, "ann");
    assert_eq!(ann.len(), 1, "heard now, not after #{second}: {sent:?}");
    let line = format!("- #{first} Mira, {NEED}: done: the person answered #{q}");
    assert!(ann[0].text.contains(&line), "{}", ann[0].text);
    assert!(
        !ann[0].text.contains(&format!("#{second} ")),
        "{}",
        ann[0].text
    );
}

/// F6, scenario 4c's "until", G146. The hold passes on `main` before 0065: kept for truth map 4.5.
#[test]
fn the_asking_task_stays_held_until_the_answer_and_hears_it() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let request = need(&mut conn, "ann", "ann-1");
    let starts = moved(&mut conn, "ann-1", "done");
    let held = |conn: &Connection| {
        let work = task(conn, 1);
        (work.state, work.reason)
    };
    let waiting = (State::Held, Some("waiting on #3 (Mira)".to_string()));
    assert_eq!(held(&conn), waiting);
    assert!(to(&starts, "ann").is_empty(), "no report ask: {starts:?}");
    running(&conn, "mira-3", "mira", Some(request));
    let q = file_for(&mut conn, "mira-3", request);
    moved(&mut conn, "mira-3", "done");
    assert_eq!(held(&conn), waiting);
    let sent = answer(&mut conn, q, &["Connection lost"], None);
    let ann = to(&sent.starts, "ann");
    assert_eq!(ann.len(), 1, "{sent:?}");
    assert_eq!(ann[0].task, Some(1), "on her task");

    // From a turn on no task: the hear-back answers that turn.
    running(&conn, "ann-2", "ann", None);
    let request = need(&mut conn, "ann", "ann-2");
    running(&conn, "mira-5", "mira", Some(request));
    let q = file_for(&mut conn, "mira-5", request);
    moved(&mut conn, "mira-5", "done");
    moved(&mut conn, "ann-2", "done");
    let sent = answer(&mut conn, q, &["Connection lost"], None);
    let ann = to(&sent.starts, "ann");
    assert_eq!(ann.len(), 1, "{sent:?}");
    assert_eq!(
        (ann[0].task, ann[0].answers.as_deref()),
        (None, Some("ann-2"))
    );
}

/// F2.2, F2.3.
#[test]
fn a_second_members_request_waits_quietly_on_the_first_question() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (_, q) = filed(&mut conn);
    let folded = need(&mut conn, "bo", "bo-1");
    running(&conn, "mira-5", "mira", Some(folded));
    let on = Said {
        on_task: Some(q),
        ..Default::default()
    };
    finish(&mut conn, "mira", folded, Finish::Blocked, &on).unwrap();
    let mut starts = moved(&mut conn, "mira-5", "done");
    starts.extend(moved(&mut conn, "bo-1", "done"));
    assert_eq!(task(&conn, folded).state, State::Blocked);
    assert!(to(&starts, "bo").is_empty(), "no turn for Bo: {starts:?}");
    let work = task(&conn, 2);
    let waits = (work.state, work.reason.as_deref());
    assert_eq!(waits, (State::Held, Some("waiting on #5 (Mira)")));

    let sent = answer(&mut conn, q, &["Connection lost"], Some("and say we retry"));
    let mut bo = to(&sent.starts, "bo").len();
    let ann = to(&sent.starts, "ann");
    let settled = format!("- #3 Mira, {NEED}: done: the person answered #4 ({ASKED}): {PICKED}");
    assert!(ann[0].text.lines().any(|l| l == settled), "{}", ann[0].text);
    let carried = sent
        .starts
        .iter()
        .find(|s| s.agent == "mira" && s.task == Some(folded));
    let carried = carried.unwrap_or_else(|| panic!("Mira's turn on #5: {:?}", sent.starts));
    let carry = format!(
        "Continue from here: the task blocking this one, #4, is done: {PICKED}. Finish #5 done with this answer in \
         note: Bo hears it then. Check"
    );
    assert!(carried.text.contains(&carry), "{}", carried.text);

    moved(&mut conn, &carried.turn, "running");
    let note = Said {
        note: Some("Connection lost, and say we retry"),
        ..Default::default()
    };
    finish(&mut conn, "mira", folded, Finish::Done, &note).unwrap();
    let starts = moved(&mut conn, &carried.turn, "done");
    let told = to(&starts, "bo");
    bo += told.len();
    assert_eq!(bo, 1, "Bo hears once: {starts:?}");
    let line = format!("- #5 Mira, {NEED}: done: Connection lost, and say we retry");
    assert!(told[0].text.lines().any(|l| l == line), "{}", told[0].text);
}

/// F2.2.3: a cancelled question cancels the folded request, and its member hears that once.
#[test]
fn a_folded_request_is_cancelled_with_its_question_and_its_member_hears() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let (_, q) = filed(&mut conn);
    let folded = need(&mut conn, "bo", "bo-1");
    running(&conn, "mira-5", "mira", Some(folded));
    let on = Said {
        on_task: Some(q),
        ..Default::default()
    };
    finish(&mut conn, "mira", folded, Finish::Blocked, &on).unwrap();
    moved(&mut conn, "mira-5", "done");
    moved(&mut conn, "bo-1", "done");
    let starts = tasks::cancel(&mut conn, None, q, "not needed", T).unwrap();
    let r = task(&conn, folded);
    let why = "The task blocking this one, #4, was cancelled";
    assert_eq!(
        (r.state, r.reason.as_deref()),
        (State::Cancelled, Some(why))
    );
    let bo = to(&starts, "bo");
    assert_eq!(bo.len(), 1, "{starts:?}");
    let line = format!("- #5 Mira, {NEED}: cancelled: {why}");
    assert!(bo[0].text.lines().any(|l| l == line), "{}", bo[0].text);
}

/// F9.1, truth map 4.2. Passes before 0065: kept for 4.2, which had no test.
#[test]
fn the_leads_pick_stays_first() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    running(&conn, "mira-1", "mira", None);
    for labels in [["Short", "Long", "Medium"], ["Long", "Medium", "Short"]] {
        let mut ask = filing("mira-1", None, Some(ASKED));
        ask.choices = choices(&labels);
        let q = person::ask(&mut conn, ask, T).unwrap().0.number;
        let open = person::open(&conn, None).unwrap();
        let found = open.iter().find(|o| o.task.number == q).unwrap();
        let got: Vec<&str> = found
            .task
            .choices
            .iter()
            .map(|c| c.label.as_str())
            .collect();
        assert_eq!(got, labels);
    }
}

/// F4.2, check objection 7: a member's task blocked on its own request hears the answer once.
#[test]
fn a_task_blocked_on_its_request_hears_the_answer_once() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let request = need(&mut conn, "ann", "ann-1");
    let on = Said {
        on_task: Some(request),
        ..Default::default()
    };
    finish(&mut conn, "ann", 1, Finish::Blocked, &on).unwrap();
    moved(&mut conn, "ann-1", "done");
    running(&conn, "mira-3", "mira", Some(request));
    let q = file_for(&mut conn, "mira-3", request);
    moved(&mut conn, "mira-3", "done");
    let sent = answer(&mut conn, q, &["Connection lost"], Some("and say we retry"));
    assert_eq!(task(&conn, 1).state, State::Ready);
    let ann = to(&sent.starts, "ann");
    assert_eq!((ann.len(), ann[0].task), (1, Some(1)), "{sent:?}");
    let carry = format!(
        "Continue from here: the task blocking this one, #3, is done: the person answered #4 ({ASKED}): {PICKED}. \
         Check"
    );
    assert!(ann[0].text.contains(&carry), "{}", ann[0].text);
    let heard: bool = conn
        .query_row(
            "SELECT heard FROM tasks WHERE number = ?1",
            [request],
            |r| r.get(0),
        )
        .unwrap();
    assert!(heard, "the readied brief carried it");
    // 0084: the brief's opening doesn't list #4 again, even as a new conversation's.
    let listed = gathr::tasks::questions(&conn, &ann[0].turn, None).unwrap();
    assert_eq!(listed, None, "the answer told twice");
}

/// 0084: tasks blocked on the lead's own question each get its answer in their brief. The answer stays
/// linked to the turn the lead heard it in, and each turn that carries it doesn't list it again.
#[test]
fn briefs_readied_by_an_answer_leave_it_linked_where_it_was_heard() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES ('c', 'mira', 'mira', 'work', 'Plan the release', 'ready', 1, 1)",
        [],
    )
    .unwrap();
    running(&conn, "mira-3", "mira", Some(3));
    let q = person::ask(&mut conn, filing("mira-3", None, Some(ASKED)), T)
        .unwrap()
        .0
        .number;
    let on = Said {
        on_task: Some(q),
        ..Default::default()
    };
    finish(&mut conn, "ann", 1, Finish::Blocked, &on).unwrap();
    finish(&mut conn, "mira", 3, Finish::Blocked, &on).unwrap();
    for turn in ["ann-1", "mira-3"] {
        moved(&mut conn, turn, "done");
    }
    let sent = answer(&mut conn, q, &["Connection lost"], None);

    let heard: String = conn
        .query_row(
            "SELECT id FROM turns WHERE opened_by = 'answered' AND agent_id = 'mira'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(task(&conn, q).heard_in, Some(heard), "{sent:?}");
    let mira = to(&sent.starts, "mira");
    assert!(!mira.is_empty(), "{sent:?}");
    for start in mira {
        let listed = gathr::tasks::questions(&conn, &start.turn, None).unwrap();
        assert_eq!(listed, None, "told #{q} twice in {}", start.turn);
    }
}

/// 0084: two answers settle one request in one batch. The request's hear-back carries the first,
/// which settled it, so its opening lists the second alone.
#[test]
fn a_second_answer_for_a_settled_request_still_shows() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    let request = need(&mut conn, "ann", "ann-1");
    running(&conn, "mira-3", "mira", Some(request));
    let first = file_for(&mut conn, "mira-3", request);
    let also = filing(
        "mira-3",
        Some(request),
        Some("Should it say when we retry?"),
    );
    let second = person::ask(&mut conn, also, T).unwrap().0.number;
    moved(&mut conn, "mira-3", "done");
    moved(&mut conn, "ann-1", "done");
    let pick = |n: i64| Answer {
        task: n,
        pick: vec!["Connection lost".to_string()],
        ..Default::default()
    };
    let sent = person::answer(&mut conn, &[pick(first), pick(second)], T).unwrap();
    assert_eq!(task(&conn, second).state, State::Done, "{sent:?}");

    let ann = to(&sent.starts, "ann");
    assert_eq!(ann.len(), 1, "{sent:?}");
    let (listed, count) = gathr::tasks::questions(&conn, &ann[0].turn, None)
        .unwrap()
        .expect("the second answer");
    assert_eq!(count, 1, "{listed}");
    assert!(listed.contains(&format!("- #{second} ")), "{listed}");
}

/// 0084 on an upgrade: an answered turn still queued when 0062 runs keeps its answer out of its
/// opening, as one queued after it does.
#[test]
fn an_answered_turn_queued_before_the_upgrade_is_not_told_its_answer_twice() {
    let dir = TempDir::new();
    let mut conn = room(&dir);
    conn.execute(
        "INSERT INTO tasks (chat_id, giver, owner, kind, what, state, created_ts, updated_ts)
         VALUES ('c', 'mira', 'mira', 'work', 'Plan the release', 'ready', 1, 1)",
        [],
    )
    .unwrap();
    running(&conn, "mira-3", "mira", Some(3));
    let q = person::ask(&mut conn, filing("mira-3", None, Some(ASKED)), T)
        .unwrap()
        .0
        .number;
    moved(&mut conn, "mira-3", "done");
    answer(&mut conn, q, &["Connection lost"], None);
    let (heard, on): (String, Option<i64>) = conn
        .query_row(
            "SELECT id, task FROM turns WHERE opened_by = 'answered' AND agent_id = 'mira'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(on, Some(3), "the answered turn is on Mira's task");
    common::old::back_to(&conn, 61);
    drop(conn);

    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    let status: String = conn
        .query_row("SELECT status FROM turns WHERE id = ?1", [&heard], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "queued");
    let listed = gathr::tasks::questions(&conn, &heard, None).unwrap();
    assert_eq!(listed, None, "told #{q} twice in {heard}");
}
