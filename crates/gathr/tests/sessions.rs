//! Which conversation a turn runs in (0055, scenarios 8h to 8k), through a real service. Every
//! engine is the replay stand-in, started on the session Gathr gives it and naming that session in
//! its frames (`GATHR_REPLAY_SESSION=echo`), so each task's conversation has an id of its own.

mod common;
#[path = "sessions/record.rs"]
mod record;

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use common::{TempDir, TestService, call_tool, rows, sign_in_account, text, until};
use serde_json::{Value, json};

const NEW: &str = "This is a new conversation for task #";
const CLOSED: &str = "the next turn is on another task";
const REFUSED: &str = "it couldn't resume its conversation";
const SOON: &str = "Your conversation is close to its compaction point";

/// `n` turns of `signed_in` for each engine to play from its start; `change` edits turn `i`.
fn played(dir: &Path, n: usize, change: impl Fn(usize, &mut Vec<String>)) -> PathBuf {
    let one = std::fs::read_to_string(common::recording("signed_in")).unwrap();
    let mut all = Vec::new();
    for i in 0..n {
        let mut turn: Vec<String> = one.lines().map(String::from).collect();
        change(i, &mut turn);
        all.extend(turn);
    }
    let path = dir.join("played.jsonl");
    std::fs::write(&path, all.join("\n") + "\n").unwrap();
    path
}

fn plain(dir: &Path) -> PathBuf {
    played(dir, 12, |_, _| {})
}

struct Room {
    service: TestService,
    data: PathBuf,
    chat: String,
    lead: String,
    ann: String,
}

impl Room {
    /// The token of the lead's engine now: it starts again when the chat's roles change.
    fn lead_token(&self) -> String {
        token_of(self, &self.lead)
    }
}

/// A signed-in team chat on `engine` (Lead, Supervisor; Ann, member) whose lead has run a turn.
fn room(data: &Path, recording: &Path, engine: &str, more: &[(&str, &Path)]) -> Room {
    let answers = data.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let replay = common::replay_engine();
    let mut env: Vec<(&str, &Path)> = vec![
        ("GATHR_CLAUDE_PATH", &replay),
        ("GATHR_CODEX_PATH", &replay),
        ("GATHR_REPLAY", recording),
        ("GATHR_REPLAY_SESSION", Path::new("echo")),
        ("GATHR_GH_PATH", Path::new("")),
        ("GATHR_FAKE_GH", &answers),
    ];
    let (args, starts, gh) = (
        data.join("args.jsonl"),
        data.join("env.jsonl"),
        common::fake_gh(),
    );
    env[4].1 = &gh;
    env.extend([
        ("GATHR_REPLAY_ARGS", args.as_path()),
        ("GATHR_REPLAY_ENV", &starts),
    ]);
    env.extend_from_slice(more);
    let service = TestService::start_with(data, &env);
    let account = signed_in(&service, data, engine);
    let agent = |name: &str| new_agent(&service, name, &account);
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members =
        json!([{ "agent": lead, "role": "supervisor" }, { "agent": ann, "role": "member" }]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    let room = Room {
        service,
        data: data.to_path_buf(),
        chat,
        lead,
        ann,
    };
    say(&room, &room.lead, "Plan it");
    common::fix_it(room.service.info.port, &room.lead_token());
    room
}

/// A new account on `engine`, signed in with a made-up login: its id.
fn signed_in(service: &TestService, data: &Path, engine: &str) -> String {
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": engine, "name": engine }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    let account = added.json()["id"].as_str().unwrap().to_string();
    match engine {
        "codex" => {
            let folder = data.join("accounts").join(&account).join("codex");
            std::fs::create_dir_all(&folder).unwrap();
            let login = r#"{"tokens":{"refresh_token":"made-up"}}"#;
            std::fs::write(folder.join("auth.json"), login).unwrap();
        }
        _ => sign_in_account(data, &account),
    }
    account
}

fn new_agent(service: &TestService, name: &str, account: &str) -> String {
    let made = service.send(
        "POST",
        "/api/agents",
        json!({ "name": name, "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    made.json()["id"].as_str().unwrap().to_string()
}

/// Adds `name` to the chat as `role`, on `account`: its id.
fn join(room: &Room, name: &str, role: &str, account: &str) -> String {
    let agent = new_agent(&room.service, name, account);
    let chat = room
        .service
        .call("GET", &format!("/api/chats/{}", room.chat))
        .json();
    let mut members: Vec<Value> = chat["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!({ "agent": m["agent"], "role": m["role"] }))
        .collect();
    members.push(json!({ "agent": agent, "role": role }));
    let path = format!("/api/chats/{}", room.chat);
    let changed = room
        .service
        .send("PATCH", &path, json!({ "members": members }));
    assert_eq!(changed.status, 200, "{}", changed.body);
    agent
}

/// The person's message to `agent`, once every turn has ended.
fn say(room: &Room, agent: &str, said: &str) {
    let path = format!("/api/chats/{}/messages", room.chat);
    let sent = room
        .service
        .send("POST", &path, json!({ "text": said, "agent": agent }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    quiet(room);
}

/// Waits until no turn runs or waits in the queue.
fn quiet(room: &Room) -> Value {
    room.service
        .wait_for(&room.chat, 30, "every turn to end", |h| {
            let turns = h["turns"].as_array().cloned().unwrap_or_default();
            turns
                .iter()
                .all(|t| !matches!(t["status"].as_str(), Some("running" | "queued")))
        })
}

/// `give_task` as the agent whose token it is: the task's number.
fn give(room: &Room, token: &str, ask: Value) -> i64 {
    let given = text(&call_tool(
        room.service.info.port,
        token,
        "give_task",
        common::briefed(ask),
    ));
    let number = given
        .strip_prefix("Task #")
        .and_then(|r| r.split(' ').next());
    number
        .and_then(|n| n.parse().ok())
        .unwrap_or_else(|| panic!("not given: {given}"))
}

/// The lead gives Ann `what`; it ends blocked, with no report, and every turn has ended.
fn give_ann(room: &Room, what: &str) -> i64 {
    let task = give(
        room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": what }),
    );
    settled(room, task, "blocked");
    task
}

fn settled(room: &Room, task: i64, state: &str) {
    until(30, &format!("#{task} to be {state}"), || {
        let tasks = room
            .service
            .call("GET", &format!("/api/chats/{}/tasks", room.chat))
            .json();
        let all = tasks["tasks"].as_array().cloned().unwrap_or_default();
        all.iter()
            .any(|t| t["number"] == task && t["state"] == state)
            .then_some(())
    });
    quiet(room);
}

fn restart(room: &Room, task: i64) {
    let restarted = room
        .service
        .call("POST", &format!("/api/tasks/{task}/restart"));
    assert_eq!(restarted.status, 200, "{}", restarted.body);
}

/// Each engine `agent` started, oldest first: `gathr.engine.start`'s attributes.
fn starts(room: &Room, agent: &str) -> Vec<Value> {
    let sql = "SELECT attrs FROM event_log WHERE name = 'gathr.engine.start' ORDER BY id";
    let all = rows(&room.data, sql)
        .into_iter()
        .map(|r| r["attrs"].clone());
    all.filter(|a| a["gen_ai.agent.id"] == agent && a["process.pid"].is_number())
        .collect()
}

/// The token of `agent`'s newest engine: the last start in its working folder.
fn token_of(room: &Room, agent: &str) -> String {
    let cwd = until(20, "the agent's engine", || {
        starts(room, agent).last().cloned()
    })["cwd"]
        .clone();
    until(20, "the engine's token", || {
        let text = std::fs::read_to_string(room.data.join("env.jsonl")).ok()?;
        let mut lines = text
            .lines()
            .filter_map(|l| serde_json::from_str::<Value>(l).ok());
        let last = lines.rfind(|l| l["cwd"] == cwd)?;
        last["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

/// The arguments of the engine started on `session`.
fn args_with(room: &Room, session: &str) -> Vec<String> {
    let text = std::fs::read_to_string(room.data.join("args.jsonl")).unwrap();
    let mut all = text
        .lines()
        .filter_map(|l| serde_json::from_str::<Vec<String>>(l).ok());
    let found = all.rfind(|a| a.iter().any(|s| s == session));
    found.unwrap_or_else(|| panic!("no engine started on {session}: {text}"))
}

fn flag(args: &[String], name: &str, session: &str) -> bool {
    args.windows(2).any(|w| w[0] == name && w[1] == session)
}

/// `task_sessions` as `[task, agent, session, used, told]`, by task.
fn sessions(room: &Room) -> Vec<Value> {
    let sql = "SELECT task, agent_id, session_id, used, told FROM task_sessions ORDER BY task";
    rows(&room.data, sql)
}

fn session_of(room: &Room, task: i64) -> String {
    let all = sessions(room);
    let row = all.iter().find(|r| r["task"] == task);
    row.and_then(|r| r["session_id"].as_str())
        .unwrap_or_else(|| panic!("#{task}: {all:?}"))
        .to_string()
}

/// The `n`th turn on `task` (from 1).
fn turn_on(room: &Room, task: i64, n: usize) -> Value {
    let history = room
        .service
        .call("GET", &format!("/api/chats/{}", room.chat))
        .json();
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    let on: Vec<&Value> = turns.iter().filter(|t| t["task"] == task).collect();
    on.get(n - 1)
        .map(|t| (*t).clone())
        .unwrap_or_else(|| panic!("turn {n} on #{task}: {history}"))
}

/// What the engine was sent for the turn: each frame as written.
fn sent(room: &Room, turn: &Value) -> Vec<String> {
    let id = turn["id"].as_str().unwrap();
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{id}' ORDER BY id"
    );
    rows(&room.data, &sql)
        .iter()
        .filter_map(|r| r["body"].as_str().map(String::from))
        .collect()
}

fn events(room: &Room, name: &str) -> Vec<Value> {
    let sql = format!("SELECT ref, status, attrs FROM event_log WHERE name = '{name}' ORDER BY id");
    rows(&room.data, &sql)
}

fn closed_for_another_task(room: &Room) -> usize {
    let closes = events(room, "gathr.engine.close");
    closes
        .iter()
        .filter(|e| e["attrs"]["why"] == CLOSED)
        .count()
}

/// Scenario 8h: Ann's second task starts in a conversation of its own, and each task keeps its id.
#[test]
fn a_members_second_task_starts_in_a_new_session() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    give_ann(&room, "Task 1");
    give_ann(&room, "Task 2");

    let started = starts(&room, &room.ann);
    assert_eq!(started.len(), 2, "{started:?}");
    let second = &started[1];
    let shown = (
        &second["resumed"],
        &second["gathr.session.from"],
        &second["gathr.task.id"],
    );
    assert_eq!(shown, (&json!(false), &json!("new"), &json!(2)), "{second}");
    let id = second["session"].as_str().unwrap();
    let args = args_with(&room, id);
    assert!(flag(&args, "--session-id", id), "{args:?}");
    assert_eq!(closed_for_another_task(&room), 1);

    let (first, second) = (session_of(&room, 1), session_of(&room, 2));
    assert_ne!(first, second, "{:?}", sessions(&room));
    assert_eq!(
        (first.as_str(), second.as_str()),
        (started[0]["session"].as_str().unwrap(), id)
    );
}

/// Scenario 8i: a restart of #1 after #2 goes back to #1's conversation, with no opening.
#[test]
fn a_later_turn_on_a_task_resumes_its_own_session() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    give_ann(&room, "Task 1");
    give_ann(&room, "Task 2");
    restart(&room, 1);
    settled(&room, 1, "blocked");

    let started = starts(&room, &room.ann);
    assert_eq!(started.len(), 3, "{started:?}");
    let first = session_of(&room, 1);
    let third = &started[2];
    assert_eq!(
        (&third["session"], &third["resumed"]),
        (&json!(first), &json!(true)),
        "{third}"
    );
    assert_eq!(third["gathr.session.from"], "task", "{third}");
    assert!(flag(&args_with(&room, &first), "--resume", &first));
    assert_eq!(closed_for_another_task(&room), 2);
    let restarted = sent(&room, &turn_on(&room, 1, 3)).join("\n");
    assert!(
        restarted.contains("Task #1 from Lead") && !restarted.contains(NEW),
        "{restarted}"
    );
}

/// A task's own turns and a turn on no task all run in the engine already open.
#[test]
fn turns_on_one_task_and_a_no_task_turn_start_no_new_engine() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    give_ann(&room, "Task 1");
    assert_eq!(
        turn_on(&room, 1, 2)["status"],
        "done",
        "the ask for its report ran"
    );
    say(&room, &room.ann, "Anything else?");
    let started = starts(&room, &room.ann);
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(closed_for_another_task(&room), 0);
}

/// Answer 1: the lead keeps one conversation, its own tasks' turns too.
#[test]
fn the_lead_keeps_one_session_for_its_tasks() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    for what in ["Plan the work", "Check the plan"] {
        let task = give(
            &room,
            &room.lead_token(),
            json!({ "to": "Lead", "what": what }),
        );
        settled(&room, task, "blocked");
    }
    let started = starts(&room, &room.lead);
    assert_eq!(started.len(), 1, "{started:?}");
    assert_eq!(started[0]["gathr.session.for"], "lead", "{}", started[0]);
    assert_eq!(events(&room, "gathr.engine.close"), Vec::<Value>::new());
    assert_eq!(
        sessions(&room),
        Vec::<Value>::new(),
        "the lead's tasks have no rows"
    );
}

/// Answer 2: a new conversation's first turn opens with Ann's five newest other tasks in the chat.
#[test]
fn a_new_sessions_first_turn_lists_its_last_five_tasks() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    for n in 1..=7 {
        give_ann(&room, &format!("Task {n}"));
    }
    give_ann(&room, "Task 8");

    let first = sent(&room, &turn_on(&room, 8, 1));
    let first = first
        .iter()
        .find(|f| f.contains(NEW))
        .unwrap_or_else(|| panic!("no opening: {first:?}"));
    let blocked = "no report: two turns ended without one";
    let listed: Vec<String> = (3..=7)
        .rev()
        .map(|n| format!("- #{n} Ann, Task {n}: {blocked}"))
        .collect();
    // The chat's recent lines come between the opening and the ask (0081); which ones depends on how
    // far the other turns got, so only where they stand is checked here (tests/room.rs has the lines).
    let opening = format!(
        "This is a new conversation for task #8. Your earlier turns in this chat aren't in it, only its recent \
         lines below: read_chat and list_tasks have the rest.\nYour last tasks in this chat:\n{}\n\n\
         Recent lines in this chat:\n",
        listed.join("\n")
    );
    let frame: Value = serde_json::from_str(first).unwrap_or_else(|e| panic!("{e}: {first}"));
    let content = frame["message"]["content"].as_str().unwrap_or_default();
    assert!(
        content.starts_with(&opening) && content.contains("\n\nTask #8 from Lead: Task 8\n"),
        "expected it to start with:\n{opening}\nand end with the ask, got:\n{content}"
    );
    let ask = sent(&room, &turn_on(&room, 8, 2)).join("\n");
    assert!(!ask.contains(NEW), "the ask for a report runs in it: {ask}");
    let opened = events(&room, "gathr.session.opening");
    let last = &opened.last().unwrap()["attrs"];
    assert_eq!(
        (&last["gathr.task.id"], &last["gathr.task.count"]),
        (&json!(8), &json!(5)),
        "{last}"
    );
    let chars = content
        .split("\n\nRecent lines in this chat:")
        .next()
        .unwrap()
        .chars()
        .count();
    assert_eq!(last["gathr.message.chars"], chars, "{last}");
    assert_eq!(opened.len(), 8, "one for each new conversation: {opened:?}");
}

/// Answer 3: Ann's hear-back on a task she gave from #1 goes back to #1's conversation once #1
/// closed; a teammate's message naming #1 runs where Ann is.
#[test]
fn a_hear_back_runs_in_the_session_that_gave_the_task() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    let away = signed_out_account(&room);
    join(&room, "Pia", "member", &away);
    give_ann(&room, "Task 1");
    let ann = token_of(&room, &room.ann);
    let to_pia = json!({ "to": "Pia", "what": "Check the notes", "in_passing": true });
    let pias = give(&room, &ann, to_pia);
    let cancelled = room.service.send(
        "POST",
        "/api/tasks/1/cancel",
        json!({ "why": "done another way" }),
    );
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    give_ann(&room, "Task 3");

    let note = json!({ "to": "Ann", "text": "About #1: it's done another way." });
    text(&call_tool(
        room.service.info.port,
        &room.lead_token(),
        "message",
        note,
    ));
    quiet(&room);
    assert_eq!(
        starts(&room, &room.ann).len(),
        2,
        "the message ran where Ann was"
    );

    sign_in_account(&room.data, &away);
    settled(&room, pias, "blocked");
    let started = until(30, "Ann's hear-back", || {
        let started = starts(&room, &room.ann);
        (started.len() == 3).then_some(started)
    });
    quiet(&room);
    let back = &started[2];
    assert_eq!(
        (&back["session"], &back["resumed"]),
        (&json!(session_of(&room, 1)), &json!(true)),
        "{back}"
    );
    assert_eq!(back["gathr.session.for"], "task", "{back}");
}

/// A second account with no login: its agents' turns wait for a sign-in.
fn signed_out_account(room: &Room) -> String {
    let added = room.service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "claude", "name": "Away" }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    added.json()["id"].as_str().unwrap().to_string()
}

/// Answer 4: Gathr's fix goes back to its PR's work task's conversation, or opens New when that
/// task never ran.
#[test]
fn a_fix_task_continues_the_builders_session_for_its_pr() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    let account = room
        .service
        .call("GET", &format!("/api/agents/{}", room.lead))
        .json()["account"]
        .clone();
    let rev = join(&room, "Rev", "reviewer", account.as_str().unwrap());
    let work = give(
        &room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": "Build it", "pr": 7 }),
    );
    settled(&room, work, "held");
    give_ann(&room, "Something else");
    let fix = reviewed_with_a_wrong_note(&room, &rev, 7);

    let started = starts(&room, &room.ann);
    let last = started.last().unwrap();
    let shown = (
        &last["session"],
        &last["gathr.session.from"],
        &last["gathr.task.id"],
    );
    assert_eq!(
        shown,
        (
            &json!(session_of(&room, work)),
            &json!("work_task"),
            &json!(fix)
        ),
        "{last}"
    );
    assert_eq!(
        session_of(&room, fix),
        session_of(&room, work),
        "the fix's own row"
    );

    let never = give(
        &room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": "Later", "pr": 8, "after": [2] }),
    );
    let fix = reviewed_with_a_wrong_note(&room, &rev, 8);
    assert!(
        sessions(&room).iter().all(|r| r["task"] != never),
        "{:?}",
        sessions(&room)
    );
    let last = starts(&room, &room.ann).last().unwrap().clone();
    let shown = (
        &last["resumed"],
        &last["gathr.session.from"],
        &last["gathr.task.id"],
    );
    assert_eq!(shown, (&json!(false), &json!("new"), &json!(fix)), "{last}");
}

/// N7 for a fix: the conversation it resumes shows that conversation's last fill from its open,
/// before its first call, though the fix has no row of its own yet.
#[test]
fn a_fix_shows_the_fill_of_the_conversation_it_resumes_before_its_first_call() {
    let data = TempDir::new();
    let room = room(&data.0, &plain(&data.0), "claude", &[]);
    let account = room
        .service
        .call("GET", &format!("/api/agents/{}", room.lead))
        .json()["account"]
        .clone();
    let rev = join(&room, "Rev", "reviewer", account.as_str().unwrap());
    let work = give(
        &room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": "Build it", "pr": 7 }),
    );
    settled(&room, work, "held");
    give_ann(&room, "Something else");
    // A fill only the work task's row holds, so what shows can't come from another conversation.
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute(
        "UPDATE task_sessions SET used = 300000 WHERE task = ?1",
        [work],
    )
    .unwrap();
    let updates = context_updates(&room.service, &room.ann);

    reviewed_with_a_wrong_note(&room, &rev, 7);
    let shown = updates.lock().unwrap().clone();
    assert_eq!(
        shown.first().map(|c| &c["used"]),
        Some(&json!(300_000)),
        "the fix's conversation as it opened: {shown:?}"
    );
}

/// Rev reviews `pr` and leaves a wrong note: the fix Gathr gives Ann, once its turns have ended.
fn reviewed_with_a_wrong_note(room: &Room, rev: &str, pr: i64) -> i64 {
    let review =
        json!({ "to": "Rev", "what": format!("Review #{pr}"), "kind": "review", "pr": pr });
    let review = give(room, &room.lead_token(), review);
    settled(room, review, "blocked");
    let notes = json!([{ "kind": "wrong", "file": "src/lib.rs:1", "says": "it fails" }]);
    let left = json!({ "task": review, "how": "left", "note": "one thing fails", "commit": "abc1234",
                       "notes": notes });
    let said = text(&call_tool(
        room.service.info.port,
        &token_of(room, rev),
        "finish_task",
        left,
    ));
    let fix = until(20, &format!("the fix ({said})"), || {
        let tasks = room
            .service
            .call("GET", &format!("/api/chats/{}/tasks", room.chat))
            .json();
        let all = tasks["tasks"].as_array().cloned().unwrap_or_default();
        let found = all
            .iter()
            .find(|t| t["what"] == format!("Fix the review of #{pr}"));
        found.and_then(|t| t["number"].as_i64())
    });
    // On a PR, a turn with no report holds its task until the PR merges.
    settled(room, fix, "held");
    fix
}

/// A Background frame naming one command, as Claude Code sends while it runs.
const BACKGROUND: &str = r#"{"dir":"out","line":"{\"type\":\"system\",\"subtype\":\"background_tasks_changed\",\"tasks\":[{\"task_id\":\"b1\",\"description\":\"a ping\"}],\"session_id\":\"x\"}"}"#;

/// N4: under a background command the engine never closes: a task that would open New takes the
/// open conversation as its own, and a resumed task's row stays as it was.
#[test]
fn no_switch_while_a_background_command_runs() {
    let data = TempDir::new();
    // An engine's third turn starts a command that never ends.
    let recording = played(&data.0, 12, |i, turn| {
        if i == 2 {
            turn.insert(2, BACKGROUND.to_string());
        }
    });
    let room = room(&data.0, &recording, "claude", &[]);
    give_ann(&room, "Task 1");
    give_ann(&room, "Task 2");
    restart(&room, 1);
    settled(&room, 1, "blocked");
    say(&room, &room.ann, "Start the ping");
    let (first, second) = (session_of(&room, 1), session_of(&room, 2));

    restart(&room, 2);
    settled(&room, 2, "blocked");
    let third = give_ann(&room, "Task 3");

    assert_eq!(
        starts(&room, &room.ann).len(),
        3,
        "no engine after the command began"
    );
    assert_eq!(closed_for_another_task(&room), 2);
    assert_eq!(
        session_of(&room, 2),
        second,
        "the resumed task's row is its own"
    );
    assert_eq!(
        session_of(&room, third),
        first,
        "the new task's is the open one"
    );
    let kept = events(&room, "gathr.session.kept");
    let tasks: Vec<&Value> = kept.iter().map(|e| &e["attrs"]["gathr.task.id"]).collect();
    assert!(
        tasks.contains(&&json!(2)) && tasks.contains(&&json!(third)),
        "{kept:?}"
    );
    let attrs = &kept[0]["attrs"];
    assert_eq!(
        (&attrs["session"], &attrs["why"]),
        (&json!(first), &json!("background")),
        "{kept:?}"
    );
}

/// The live `context` updates for `agent`, collected as they come.
fn context_updates(service: &TestService, agent: &str) -> Arc<Mutex<Vec<Value>>> {
    let mut socket = service.live();
    socket.set_read_timeout(None).unwrap();
    let seen = Arc::new(Mutex::new(Vec::new()));
    let (kept, agent) = (seen.clone(), agent.to_string());
    std::thread::spawn(move || {
        while let Some(update) = next_frame(&mut socket) {
            if update["type"] == "context" && update["agent"] == agent.as_str() {
                kept.lock().unwrap().push(update["context"].clone());
            }
        }
    });
    seen
}

/// The next JSON frame on the live socket; none once it closes.
fn next_frame(socket: &mut std::net::TcpStream) -> Option<Value> {
    loop {
        let mut head = [0u8; 2];
        socket.read_exact(&mut head).ok()?;
        let len = match head[1] & 0x7F {
            126 => {
                let mut n = [0u8; 2];
                socket.read_exact(&mut n).ok()?;
                u16::from_be_bytes(n) as usize
            }
            127 => {
                let mut n = [0u8; 8];
                socket.read_exact(&mut n).ok()?;
                u64::from_be_bytes(n) as usize
            }
            n => n as usize,
        };
        let mut body = vec![0u8; len];
        socket.read_exact(&mut body).ok()?;
        if head[0] & 0x0F == 1 {
            return serde_json::from_slice(&body).ok();
        }
    }
}

/// N7: the context shown follows the conversation: none on a new one until its first call, and a
/// task's saved fill and telling back when it resumes, after an idle close too, so it isn't told
/// twice.
#[test]
fn the_context_follows_each_conversation() {
    let data = TempDir::new();
    // Each call fills 225,340: past 75% of a 200K point, so a conversation's second turn is told.
    let full = played(&data.0, 12, |_, turn| {
        for line in turn.iter_mut() {
            *line = line.replace(
                r#"cache_read_input_tokens\":23627"#,
                r#"cache_read_input_tokens\":223627"#,
            );
        }
    });
    let idle = [("GATHR_IDLE_CLOSE_MS", Path::new("3000"))];
    let room = room(&data.0, &full, "claude", &idle);
    let saved = room
        .service
        .send("PATCH", "/api/settings", json!({ "auto_compact": 200_000 }));
    assert_eq!(saved.status, 200, "{}", saved.body);
    let updates = context_updates(&room.service, &room.ann);
    let one = give(
        &room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": "Task 1" }),
    );
    let two = give(
        &room,
        &room.lead_token(),
        json!({ "to": "Ann", "what": "Task 2" }),
    );
    settled(&room, one, "blocked");
    settled(&room, two, "blocked");

    let shown = updates.lock().unwrap().clone();
    let empty = shown.iter().filter(|c| c["used"].is_null()).count();
    assert_eq!(
        empty, 2,
        "none until each new conversation's first call: {shown:?}"
    );
    let rows = sessions(&room);
    let row = rows.iter().find(|r| r["task"] == one).unwrap();
    assert_eq!(
        (&row["used"], &row["told"]),
        (&json!(225_340), &json!(1)),
        "{rows:?}"
    );
    let told = || {
        let soon = events(&room, "gathr.context.soon");
        soon.iter()
            .filter(|e| e["attrs"]["gen_ai.agent.id"] == room.ann.as_str())
            .count()
    };
    assert_eq!(told(), 2, "each of Ann's conversations told once");

    // A fill only #1's row holds, so its return can't show another's.
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    db.execute(
        "UPDATE task_sessions SET used = 300000 WHERE task = ?1",
        [one],
    )
    .unwrap();
    idle_closed(&room, &room.ann);
    let before = updates.lock().unwrap().len();
    restart(&room, one);
    settled(&room, one, "blocked");
    let back = updates.lock().unwrap()[before].clone();
    assert_eq!(back["used"], 300_000, "{back}");
    let restarted = sent(&room, &turn_on(&room, one, 3)).join("\n");
    assert!(!restarted.contains(SOON), "told once: {restarted}");
    assert_eq!(told(), 2);
}

/// Waits until `agent`'s newest engine has closed idle.
fn idle_closed(room: &Room, agent: &str) {
    let last = starts(room, agent).last().unwrap()["process.pid"].clone();
    until(20, "the agent's engine to close idle", || {
        let closes = events(room, "gathr.engine.close");
        closes
            .iter()
            .any(|e| e["attrs"]["process.pid"] == last && e["attrs"]["why"] == "it was idle")
            .then_some(())
    });
}

/// The engine's close reasons, oldest first.
fn closes(room: &Room) -> Vec<Value> {
    let all = events(room, "gathr.engine.close");
    all.iter().map(|e| e["attrs"]["why"].clone()).collect()
}

/// The conversation `agent`'s turns on no task resume (`engine_sessions`).
fn latest_of(room: &Room, agent: &str) -> Option<String> {
    let sql = format!(
        "SELECT session_id FROM engine_sessions WHERE chat_id = '{}' AND agent_key = '{agent}'",
        room.chat
    );
    let found = rows(&room.data, &sql);
    found
        .first()
        .and_then(|r| r["session_id"].as_str().map(String::from))
}

/// N8 on the latest conversation: the lead's turn, and Ann's on no task, resume what the engine
/// refuses. The refused engine closes, and the turn runs once more in a new conversation.
#[test]
fn a_refused_resume_of_the_latest_conversation_starts_new() {
    let data = TempDir::new();
    let more = [
        ("GATHR_REPLAY_RESUME", Path::new("missing")),
        ("GATHR_IDLE_CLOSE_MS", Path::new("3000")),
    ];
    let room = room(&data.0, &plain(&data.0), "claude", &more);
    say(&room, &room.ann, "Hello");
    for agent in [room.lead.clone(), room.ann.clone()] {
        idle_closed(&room, &agent);
        let refused = latest_of(&room, &agent).expect("a latest conversation");
        let before = events(&room, "gathr.engine.resume_failed").len();
        say(&room, &agent, "Anything else?");

        let history = room
            .service
            .call("GET", &format!("/api/chats/{}", room.chat))
            .json();
        let turns = history["turns"].as_array().cloned().unwrap_or_default();
        let turn = turns
            .iter()
            .rfind(|t| t["agent"] == agent.as_str())
            .unwrap();
        assert_eq!(turn["status"], "done", "{turn}");
        let failed = events(&room, "gathr.engine.resume_failed");
        let failed = &failed[before..];
        assert_eq!(failed.len(), 1, "{failed:?}");
        assert_eq!(
            failed[0]["ref"], turn["ref"],
            "on the turn's trace: {failed:?}"
        );
        assert_eq!(
            failed[0]["attrs"]["session"],
            refused.as_str(),
            "{failed:?}"
        );
        let started = starts(&room, &agent);
        let shown: Vec<&Value> = started
            .iter()
            .rev()
            .take(2)
            .map(|s| &s["resumed"])
            .collect();
        assert_eq!(shown, [&json!(false), &json!(true)], "{started:?}");
        let now = latest_of(&room, &agent);
        assert!(now.is_some_and(|s| s != refused), "{refused}");
    }
    let refusals = closes(&room).iter().filter(|w| *w == REFUSED).count();
    assert_eq!(refusals, 2, "{:?}", closes(&room));
}

/// Scenario 8k: the engine refuses the id #1's row holds. The turn goes back once, the row goes, and
/// it runs in a new conversation with #1's brief once.
fn a_refused_resume_starts_new_with_the_brief(engine: &str) {
    let data = TempDir::new();
    let recording = match engine {
        "codex" => common::recording("codex_signed_in"),
        _ => plain(&data.0),
    };
    let missing = [("GATHR_REPLAY_RESUME", Path::new("missing"))];
    let room = room(&data.0, &recording, engine, &missing);
    give_ann(&room, "Task 1");
    give_ann(&room, "Task 2");
    let refused = session_of(&room, 1);
    restart(&room, 1);
    settled(&room, 1, "blocked");

    let failed = events(&room, "gathr.engine.resume_failed");
    assert_eq!(failed.len(), 1, "{failed:?}");
    let restart = turn_on(&room, 1, 3);
    assert_eq!(
        failed[0]["ref"], restart["ref"],
        "on the turn's trace: {failed:?} {restart}"
    );
    let attrs = &failed[0]["attrs"];
    assert_eq!(
        (&attrs["session"], &attrs["gathr.task.id"]),
        (&json!(refused), &json!(1)),
        "{attrs}"
    );
    assert!(
        sessions(&room)
            .iter()
            .all(|r| r["session_id"] != refused.as_str()),
        "{:?}",
        sessions(&room)
    );
    assert_eq!(restart["status"], "done", "{restart}");
    // The refused engine may have taken the ask before it exited: the new conversation's is last.
    let input = sent(&room, &restart);
    let opened = input.iter().rev().find(|f| f.contains(NEW));
    let opened = opened.unwrap_or_else(|| panic!("no opening: {input:?}"));
    assert_eq!(opened.matches("Task #1 from Lead").count(), 1, "{opened}");
    // Codex's driver closes an engine whose request it rejected before the turn ends.
    let why = match engine {
        "codex" => "the engine rejected its request",
        _ => REFUSED,
    };
    let all = closes(&room);
    assert_eq!(all.iter().filter(|w| *w == why).count(), 1, "{all:?}");
    assert_eq!(closed_for_another_task(&room), 2, "{all:?}");
}

#[test]
fn a_refused_resume_starts_new_with_the_brief_claude() {
    a_refused_resume_starts_new_with_the_brief("claude");
}

#[test]
fn a_refused_resume_starts_new_with_the_brief_codex() {
    a_refused_resume_starts_new_with_the_brief("codex");
}
