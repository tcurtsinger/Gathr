//! Tasks and waits through a real service (0010): a task given by tool runs as its owner's turn
//! under the turn that gave it, its giver hears back, a wait outlasts a restart, and a wait whose
//! look keeps failing is found from its reference (0019).

mod common;

use std::path::Path;

use common::{
    TempDir, TestService, call_tool, gathr, held_recording, recording, sign_in_account, text, until,
};
use serde_json::{Value, json};

struct Team {
    service: TestService,
    chat: String,
    lead: String,
}

/// A service whose engines hold after the message arrives until `go` exists, and a chat with a
/// Supervisor (Lead) and a member (Ann).
fn team(data: &Path, replay: &Path) -> Team {
    let env = data.join("env.jsonl");
    // GitHub's `gh` stands in with no answers: every look fails at once. The real one fails too
    // (no login in CI), but in its own time: on Windows CI three failing looks once took 40 s,
    // past these tests' limit (main at 310556f).
    let answers = data.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let service = TestService::with_engine(
        data,
        &[
            ("GATHR_REPLAY", replay),
            ("GATHR_REPLAY_HOLD", &data.join("go")),
            ("GATHR_REPLAY_ENV", &env),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
            ("GATHR_GH_PATH", &common::fake_gh()),
            ("GATHR_FAKE_GH", &answers),
        ],
    );
    let account = service.add_account("Team");
    sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, ann) = (agent("Lead"), agent("Ann"));
    let members = json!([
        { "agent": lead, "role": "supervisor" },
        { "agent": ann, "role": "member" }
    ]);
    let made = service.send("POST", "/api/chats", json!({ "members": members }));
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = made.json()["id"].as_str().unwrap().to_string();
    Team {
        service,
        chat,
        lead,
    }
}

/// The token of the `n`th engine started, from the replay engine's `GATHR_REPLAY_ENV` file.
fn token(data: &Path, n: usize) -> String {
    until(10, "the engine's token", || {
        let text = std::fs::read_to_string(data.join("env.jsonl")).ok()?;
        let line: Value = serde_json::from_str(text.lines().nth(n)?).ok()?;
        line["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

/// Sends the lead a message and returns its engine's token once the turn holds.
fn lead_turn(team: &Team, data: &Path, n: usize) -> String {
    let path = format!("/api/chats/{}/messages", team.chat);
    let sent = team
        .service
        .send("POST", &path, json!({ "text": "Plan it" }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    token(data, n)
}

/// `held_recording`, then a second turn on the same engine: the giver's hear-back.
fn twice(dir: &Path) -> std::path::PathBuf {
    let first = std::fs::read_to_string(held_recording(dir)).unwrap();
    let second = std::fs::read_to_string(recording("signed_in")).unwrap();
    let both = dir.join("twice.jsonl");
    std::fs::write(
        &both,
        format!(
            "{}
{second}",
            first.trim_end()
        ),
    )
    .unwrap();
    both
}

fn tasks(team: &Team) -> Value {
    let listed = team
        .service
        .call("GET", &format!("/api/chats/{}/tasks", team.chat));
    assert_eq!(listed.status, 200, "{}", listed.body);
    listed.json()
}

#[test]
fn a_task_given_by_tool_runs_under_the_turn_that_gave_it_and_its_giver_hears_back() {
    let data = TempDir::new();
    let team = team(&data.0, &twice(&data.0));
    let mut live = team.service.live();
    let lead_token = lead_turn(&team, &data.0, 0);
    let port = team.service.info.port;
    let ask = json!({ "to": "ann", "what": "Write the notes", "why": "for the review" });
    let given = text(&call_tool(port, &lead_token, "give_task", ask));
    assert!(given.starts_with("Task #1 for ann: ready."), "{given}");
    assert!(given.contains("Open PRs"), "G74: {given}");
    let refused = call_tool(
        port,
        &lead_token,
        "give_task",
        json!({ "to": "Zed", "what": "x" }),
    );
    let refused = text(&refused);
    assert!(
        refused.starts_with(
            "give_task failed: no agent Zed in this chat: its agents are Lead, Ann (ref "
        ),
        "S68: {refused}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = team.service.wait_for(&team.chat, 20, "the hear-back", |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        turns.len() == 3 && turns.iter().all(|t| t["status"] == "done")
    });
    let turns = history["turns"].as_array().unwrap();
    let (giving, owned, heard) = (&turns[0], &turns[1], &turns[2]);
    assert_eq!(owned["task"], 1, "Ann's turn works on the task: {history}");
    let listed = tasks(&team);
    let task = &listed["tasks"][0];
    assert_eq!(task["state"], "done", "{listed}");
    assert_eq!(task["given_in"], giving["id"], "the thread link: {listed}");
    assert_eq!(heard["agent"], team.lead.as_str());
    assert_eq!(
        owned["answers"], giving["id"],
        "Ann's reply's thread: {history}"
    );
    assert_eq!(
        heard["answers"], giving["id"],
        "the hear-back's thread: {history}"
    );
    // Summary follows the task live, in the shape `GET /api/chats/<id>/tasks` answers.
    let states: Vec<Value> = std::iter::from_fn(|| Some(common::next_live(&mut live)))
        .filter(|u| u["type"] == "tasks" && u["chat"] == team.chat.as_str())
        .map(|u| u["tasks"][0]["state"].clone())
        .take_while(|state| state != "done")
        .collect();
    assert_eq!(states.first(), Some(&json!("ready")), "{states:?}");
    let said: Vec<&Value> = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|m| m["role"] == "gathr")
        .collect();
    assert_eq!(
        said.len(),
        2,
        "the task and the hear-back are Gathr's: {history}"
    );
    assert!(
        said[0]["body"]
            .as_str()
            .unwrap()
            .starts_with("Task #1 from Lead: Write the notes")
    );
    let back = said[1]["body"].as_str().unwrap();
    assert!(back.contains("#1 Ann, Write the notes: done"), "{back}");
}

/// The engine's working folder for the `n`th start.
fn cwd(data: &Path, n: usize) -> std::path::PathBuf {
    let text = std::fs::read_to_string(data.join("env.jsonl")).unwrap();
    let line: Value = serde_json::from_str(text.lines().nth(n).unwrap()).unwrap();
    line["cwd"].as_str().unwrap().into()
}

#[test]
fn only_the_lead_asks_the_person_and_each_answer_is_taken_or_refused_with_a_reference() {
    let data = TempDir::new();
    let team = team(&data.0, &twice(&data.0));
    let port = team.service.info.port;
    let lead_token = lead_turn(&team, &data.0, 0);
    std::fs::write(cwd(&data.0, 0).join("notice.md"), "# Notice\n").unwrap();
    let ask = json!({ "what": "Which notice?", "files": ["notice.md"],
                      "choices": [{ "label": "Short" }, { "label": "Long" }] });
    let asked = text(&call_tool(port, &lead_token, "ask_person", ask));
    assert!(
        asked.starts_with("Task #1 is on the person's Tasks list."),
        "{asked}"
    );
    let outside = data.0.join("env.jsonl").display().to_string();
    let escape = json!({ "what": "Read this", "files": [outside] });
    let refused = text(&call_tool(port, &lead_token, "ask_person", escape));
    assert!(refused.contains("isn't a file in your folder"), "{refused}");

    // Ann's ask reaches the lead first, never the person's list.
    let gave = json!({ "to": "Ann", "what": "Word the banner" });
    text(&call_tool(port, &lead_token, "give_task", gave));
    let ann_token = token(&data.0, 1);
    let need = json!({ "what": "Ship it today?", "form": "yesno" });
    let routed = text(&call_tool(port, &ann_token, "ask_person", need));
    assert!(
        routed.starts_with("Lead will review request #3 as the Supervisor"),
        "{routed}"
    );
    let open = team.service.call("GET", "/api/person-tasks").json();
    let numbers: Vec<&Value> = open["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| &t["number"])
        .collect();
    assert_eq!(numbers, [&json!(1)], "{open}");
    assert_eq!(open["tasks"][0]["chat_title"], "Plan it", "{open}");
    let file = team.service.call("GET", "/api/tasks/1/files/0");
    assert_eq!((file.status, file.body.as_str()), (200, "# Notice\n"));
    let headers = file.headers.to_lowercase();
    assert!(
        headers.contains("content-security-policy: sandbox"),
        "{headers}"
    );

    // One Send: the person's own is answered, the lead's request is refused with a reference.
    let mut live = team.service.live();
    let answers =
        json!({ "answers": [{ "task": 1, "pick": ["Long"] }, { "task": 3, "text": "yes" }] });
    let sent = team
        .service
        .send("POST", "/api/person-tasks/answer", answers);
    assert_eq!(sent.status, 200, "{}", sent.body);
    let sent = sent.json();
    assert_eq!(sent["answered"][0]["reason"], "Long", "{sent}");
    assert_eq!(
        sent["refused"][0]["why"], "#3 isn't yours to answer",
        "{sent}"
    );
    let reference = sent["refused"][0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    assert!(
        trace.to_string().contains("#3 isn't yours to answer"),
        "{trace}"
    );
    let listed = std::iter::from_fn(|| Some(common::next_live(&mut live)))
        .find(|u| u["type"] == "person_tasks")
        .unwrap();
    assert_eq!(listed["tasks"], json!([]), "{listed}");

    // The lead hears the answer once its turn ends, under the turn that asked.
    std::fs::write(data.0.join("go"), "").unwrap();
    let said = |h: &Value| -> Option<String> {
        let messages = h["messages"].as_array()?;
        let m = messages.iter().find(|m| {
            m["body"]
                .as_str()
                .is_some_and(|b| b.starts_with("The person answered:"))
        })?;
        m["body"].as_str().map(String::from)
    };
    let history = team
        .service
        .wait_for(&team.chat, 20, "the answer's hear-back", |h| {
            said(h).is_some()
        });
    assert!(
        said(&history).unwrap().contains("- #1 Which notice?: Long"),
        "{history}"
    );
}

#[test]
fn a_wait_outlasts_a_restart_and_wakes_its_agent_where_it_left_it() {
    let data = TempDir::new();
    let replay = recording("signed_in");
    let flag = data.0.join("flag");
    let check = format!(
        "test -f '{}'",
        flag.display().to_string().replace('\\', "/")
    );
    let team = team(&data.0, &held_recording(&data.0));
    let lead_token = lead_turn(&team, &data.0, 0);
    let ask = json!({ "check": check, "then": "merge it" });
    let left = text(&call_tool(team.service.info.port, &lead_token, "wait", ask));
    assert!(left.starts_with("Waiting ("), "{left}");
    std::fs::write(data.0.join("go"), "").unwrap();
    team.service
        .wait_for(&team.chat, 20, "the lead's turn", |h| {
            h["turns"][0]["status"] == "done"
        });
    let Team { service, chat, .. } = team;
    service.stop();

    let again = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &replay),
            ("GATHR_WAIT_EVERY_MS", Path::new("100")),
        ],
    );
    let waits = again
        .call("GET", &format!("/api/chats/{chat}/tasks"))
        .json();
    assert_eq!(waits["waits"][0]["state"], "open", "{waits}");
    std::fs::write(&flag, "").unwrap();
    let history = again.wait_for(&chat, 20, "the wake-up", |h| {
        h["turns"]
            .as_array()
            .is_some_and(|t| t.len() == 2 && t[1]["status"] == "done")
    });
    let turns = &history["turns"];
    assert_eq!(
        turns[1]["answers"], turns[0]["id"],
        "the wake-up's thread: {history}"
    );
    let woken = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["role"] == "gathr")
        .unwrap()["body"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        woken.starts_with("Your wait ended: Check passed. You planned to do this: merge it."),
        "{woken}"
    );
}

/// On Windows a check's child that outlived it kept its output pipe open, and the service didn't
/// exit until that child did: 290 s in CI.
#[test]
fn shutdown_ends_a_running_check_with_everything_it_started_and_does_not_wait_for_them() {
    let data = TempDir::new();
    let dir = data.0.to_string_lossy().replace('\\', "/");
    let check = format!("touch '{dir}/started'; (sleep 8; touch '{dir}/late') & wait");
    let team = team(&data.0, &held_recording(&data.0));
    let lead_token = lead_turn(&team, &data.0, 0);
    std::fs::write(data.0.join("go"), "").unwrap();
    team.service
        .wait_for(&team.chat, 20, "the lead's turn", |h| {
            h["turns"][0]["status"] == "done"
        });
    let ask = json!({ "check": check, "then": "go on" });
    let left = text(&call_tool(team.service.info.port, &lead_token, "wait", ask));
    assert!(left.starts_with("Waiting ("), "{left}");
    until(20, "the check running", || {
        data.0.join("started").exists().then_some(())
    });

    let asked = std::time::Instant::now();
    team.service.stop();
    let took = asked.elapsed();
    assert!(took.as_secs() < 5, "the service took {took:?} to stop");
    std::thread::sleep(std::time::Duration::from_secs(10) - took);
    assert!(
        !data.0.join("late").exists(),
        "what the check started was ended with it"
    );
}

#[test]
fn a_wait_whose_looks_keep_failing_is_found_from_its_reference() {
    let data = TempDir::new();
    let team = team(&data.0, &held_recording(&data.0));
    let lead_token = lead_turn(&team, &data.0, 0);
    // The engine's folder isn't a GitHub repository, so every look at its CI fails.
    let ask = json!({ "ci": "#1", "then": "merge it" });
    call_tool(team.service.info.port, &lead_token, "wait", ask);
    let failing = until(30, "the wait to show as failing", || {
        let (ok, rows) = gathr(
            &data.0,
            &[
                "diag",
                "query",
                "SELECT ref, attrs FROM event_log WHERE name = 'gathr.wait.failing'",
            ],
        );
        assert!(ok, "{rows}");
        rows["rows"].as_array()?.first().cloned()
    });
    let reference = failing["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let text = trace.to_string();
    assert!(
        text.contains("gathr.wait.failing") && text.contains("gh"),
        "{trace}"
    );
    let (ok, health) = gathr(&data.0, &["diag", "health"]);
    assert!(ok, "{health}");
    let shown = &health["coordination"]["waits"][0];
    assert_eq!(shown["kind"], "ci", "{health}");
    let listed = tasks(&team);
    let wait = &listed["waits"][0];
    assert!(wait["failures"].as_i64().unwrap() >= 3, "{listed}");
    assert!(
        wait["last_error"].as_str().unwrap().contains("gh"),
        "{listed}"
    );
    std::fs::write(data.0.join("go"), "").unwrap();
}

/// S72: a task cancelled while its turn runs (its PR closed, or its giver or the person cancelled
/// it) stops that turn, which says why, instead of finishing work nobody needs.
#[test]
fn a_turn_whose_task_is_cancelled_stops_and_says_why() {
    let data = TempDir::new();
    let stall = [("GATHR_STALL_AFTER_MS", Path::new("200"))];
    let (service, chat, _) = common::team(&data.0, &held_recording(&data.0), &stall);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead = common::first_token(&data.0);
    let ask = json!({ "to": "Ann", "what": "Review #7", "kind": "review", "pr": 7 });
    let given = text(&call_tool(service.info.port, &lead, "give_task", ask));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    service.wait_for(&chat, 10, "Ann's review running", |h| {
        h["turns"][1]["task"] == 1 && h["turns"][1]["status"] == "running"
    });

    let why = json!({ "why": "#7 was closed without merging" });
    let cancelled = service.send("POST", "/api/tasks/1/cancel", why);
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.turn.stop'";
    until(10, "the review's turn to be stopped", || {
        let (_, rows) = gathr(&data.0, &["diag", "query", sql]);
        (rows["rows"][0]["n"] == 1).then_some(())
    });
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "both turns ended", |h| {
        h["turns"][0]["status"] == "done" && h["turns"][1]["status"] == "stopped"
    });
    assert_eq!(
        history["turns"][1]["why"], "its task #1 was cancelled: #7 was closed without merging",
        "{history}"
    );
}

/// Cancel then Restart before the running turn is looked at: that turn still stops, so only the
/// restart's turn does the work, never both.
#[test]
fn a_turn_whose_task_is_cancelled_then_restarted_still_stops() {
    let data = TempDir::new();
    let stall = [("GATHR_STALL_AFTER_MS", Path::new("2000"))];
    let (service, chat, _) = common::team(&data.0, &twice(&data.0), &stall);
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({ "text": "Plan it" }),
    );
    assert_eq!(sent.status, 200, "{}", sent.body);
    let lead = common::first_token(&data.0);
    let ask = json!({ "to": "Ann", "what": "Notes" });
    text(&call_tool(service.info.port, &lead, "give_task", ask));
    service.wait_for(&chat, 10, "Ann's turn running", |h| {
        h["turns"][1]["task"] == 1 && h["turns"][1]["status"] == "running"
    });

    let why = json!({ "why": "not needed" });
    let cancelled = service.send("POST", "/api/tasks/1/cancel", why);
    assert_eq!(cancelled.status, 200, "{}", cancelled.body);
    let restarted = service.call("POST", "/api/tasks/1/restart");
    assert_eq!(restarted.status, 200, "{}", restarted.body);
    let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.turn.stop'";
    until(10, "the cancelled turn to be stopped", || {
        let (_, rows) = gathr(&data.0, &["diag", "query", sql]);
        (rows["rows"][0]["n"] == 1).then_some(())
    });
    std::fs::write(data.0.join("go"), "").unwrap();
    let history = service.wait_for(&chat, 10, "the cancelled turn ended", |h| {
        !matches!(h["turns"][1]["status"].as_str(), Some("running" | "queued"))
    });
    assert_eq!(
        (&history["turns"][1]["status"], &history["turns"][1]["why"]),
        (
            &json!("stopped"),
            &json!("its task #1 was cancelled: not needed")
        ),
        "{history}"
    );
    let on_it = history["turns"].as_array().unwrap().iter();
    assert_eq!(on_it.filter(|t| t["task"] == 1).count(), 2, "{history}");
}

/// The signed-in turn held until the test lets it go, twice, for an agent's two turns. Each hold
/// takes `go`, so one `go` lets one held turn on, whichever it is.
fn one_at_a_time(dir: &Path) -> std::path::PathBuf {
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    let held = format!(
        "{first}\n{{\"dir\":\"hold\",\"line\":\"take\"}}\n{}",
        rest.trim_end()
    );
    let path = dir.join("one-at-a-time.jsonl");
    std::fs::write(&path, format!("{held}\n{held}\n")).unwrap();
    path
}

/// Waits until the `n`th turn on `task` runs: until then, its engine has no turn to call tools in.
fn running_on(team: &Team, task: i64, n: usize) {
    let what = format!("turn {n} on task #{task}");
    team.service.wait_for(&team.chat, 20, &what, |h| {
        let turns = h["turns"].as_array().cloned().unwrap_or_default();
        let mine: Vec<&Value> = turns.iter().filter(|t| t["task"] == task).collect();
        mine.len() == n && mine[n - 1]["status"] == "running"
    });
}

/// Adds Rev to the team as its Reviewer.
fn with_reviewer(team: &Team) {
    let lead = team
        .service
        .call("GET", &format!("/api/agents/{}", team.lead));
    let account = lead.json()["account"].clone();
    let made = team.service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Rev", "account": account }),
    );
    assert_eq!(made.status, 200, "{}", made.body);
    let chat = team
        .service
        .call("GET", &format!("/api/chats/{}", team.chat));
    let mut members: Vec<Value> = chat.json()["members"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| json!({ "agent": m["agent"], "role": m["role"] }))
        .collect();
    members.push(json!({ "agent": made.json()["id"], "role": "reviewer" }));
    let path = format!("/api/chats/{}", team.chat);
    let changed = team
        .service
        .send("PATCH", &path, json!({ "members": members }));
    assert_eq!(changed.status, 200, "{}", changed.body);
}

// Found live (#85, #86): Ann's work was reviewed and held on its merge, and Lead, who gave it,
// never heard, so nobody saw it was ready to merge.
#[test]
fn work_held_on_its_merge_after_its_review_wakes_its_giver() {
    let data = TempDir::new();
    let team = team(&data.0, &one_at_a_time(&data.0));
    with_reviewer(&team);
    let port = team.service.info.port;
    let lead = lead_turn(&team, &data.0, 0);
    let fix = json!({ "to": "Ann", "what": "Fix the summary", "pr": 85 });
    let given = text(&call_tool(port, &lead, "give_task", fix));
    assert!(given.starts_with("Task #1 for Ann: ready."), "{given}");
    // A task given mid-turn starts its owner's turn at once, so several turns hold together and
    // any of them may take a `go`: each makes its calls first, then all are let go.
    running_on(&team, 1, 1);
    let ann = token(&data.0, 1);
    let review = json!({ "to": "Rev", "what": "Review #85", "kind": "review", "pr": 85 });
    let given = text(&call_tool(port, &ann, "give_task", review));
    assert!(given.starts_with("Task #2 for Rev: ready."), "{given}");
    call_tool(port, &ann, "finish_task", json!({ "how": "done" }));
    running_on(&team, 2, 1);
    let rev = token(&data.0, 2);
    let done = json!({ "how": "done", "commit": "abc1234" });
    call_tool(port, &rev, "finish_task", done);
    // Ann's turn on the review says nothing more: with only the merge open her task stays held on it.
    let go = data.0.join("go");
    let history = until(20, "Lead hears back", || {
        let history = team
            .service
            .call("GET", &format!("/api/chats/{}", team.chat))
            .json();
        let turns = history["turns"].as_array().cloned().unwrap_or_default();
        let lead = team.lead.as_str();
        if turns
            .iter()
            .any(|t| t["agent"] == lead && !t["answers"].is_null())
        {
            return Some(history);
        }
        if !go.exists() {
            std::fs::write(&go, "").unwrap();
        }
        None
    });
    let listed = tasks(&team);
    let held = &listed["tasks"][1];
    assert_eq!(held["state"], "held", "{listed}");
    assert_eq!(held["reason"], "done, waiting on #85 to merge", "{listed}");
    let back = history["messages"]
        .as_array()
        .unwrap()
        .iter()
        .rfind(|m| m["role"] == "gathr")
        .and_then(|m| m["body"].as_str())
        .unwrap();
    let line = "#1 Ann, Fix the summary: held: done, waiting on #85 to merge";
    assert!(
        back.starts_with("Where the task you gave stands:"),
        "{back}"
    );
    assert!(back.contains(line), "{back}");
    let sql = format!(
        "SELECT ref FROM event_log WHERE name = 'gathr.task.hear_back'
         AND json_extract(attrs, '$.\"gen_ai.agent.id\"') = '{}'",
        team.lead
    );
    let (ok, rows) = gathr(&data.0, &["diag", "query", &sql]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"].as_array().unwrap().len(), 1, "{rows}");
    let reference = rows["rows"][0]["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let trace = trace.to_string();
    // On the trace of whichever turn settled the round last: Ann's, Rev's or Lead's own.
    assert!(trace.contains("gathr.task.hear_back"), "{trace}");
    // The PR's proof: `cargo test --test coordination work_held -- --nocapture`.
    println!("diag trace {reference}: {trace}");
    std::fs::write(&go, "").unwrap();
}

/// 0043 C12 (owner, task 1450): a wait for a PR to be up to date with its base is its own kind,
/// and a look that can't read the PR fails with a reference, as other PR looks do.
#[test]
fn an_up_to_date_wait_whose_looks_fail_is_found_from_its_reference() {
    let data = TempDir::new();
    let team = team(&data.0, &held_recording(&data.0));
    let lead_token = lead_turn(&team, &data.0, 0);
    let port = team.service.info.port;
    let alone = call_tool(
        port,
        &lead_token,
        "wait",
        json!({ "up_to_date": true, "then": "x" }),
    );
    assert_eq!(alone["isError"], true, "up_to_date without a PR: {alone}");
    // The engine's folder isn't a GitHub repository, so every look at the PR fails.
    let ask = json!({ "pr": 1, "up_to_date": true, "then": "merge it" });
    let left = call_tool(port, &lead_token, "wait", ask);
    assert!(text(&left).starts_with("Waiting"), "{left}");
    assert_eq!(
        tasks(&team)["waits"][0]["kind"],
        "up_to_date",
        "{}",
        tasks(&team)
    );
    let failing = until(30, "the wait to show as failing", || {
        let query = "SELECT ref FROM event_log WHERE name = 'gathr.wait.failing'";
        let (ok, rows) = gathr(&data.0, &["diag", "query", query]);
        assert!(ok, "{rows}");
        rows["rows"].as_array()?.first().cloned()
    });
    let (ok, trace) = gathr(
        &data.0,
        &["diag", "trace", failing["ref"].as_str().unwrap()],
    );
    assert!(ok && trace.to_string().contains("gh"), "{trace}");
    std::fs::write(data.0.join("go"), "").unwrap();
}

/// Found on #120's Windows CI: `give_task` in a chat whose folder isn't a git repository waited
/// 30 s on `gh` listing PRs there, past the caller's limit. Gathr doesn't ask GitHub there.
#[test]
fn give_task_outside_a_repository_never_asks_github_for_its_prs() {
    let data = TempDir::new();
    let answers = data.0.join("gh.json");
    std::fs::write(&answers, "[]").unwrap();
    let gh = [
        ("GATHR_GH_PATH", common::fake_gh()),
        ("GATHR_FAKE_GH", answers.clone()),
    ];
    let more: Vec<(&str, &Path)> = gh.iter().map(|(k, v)| (*k, v.as_path())).collect();
    let (service, chat, lead) = common::team(&data.0, &held_recording(&data.0), &more);
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Plan it", "agent": lead }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let token = common::first_token(&data.0);
    let given = call_tool(
        service.info.port,
        &token,
        "give_task",
        json!({ "to": "Ann", "what": "Fix it" }),
    );
    assert!(
        text(&given).starts_with("Task #1 for Ann"),
        "{}",
        text(&given)
    );
    let calls = data.0.join("gh.json.calls");
    assert!(
        !calls.exists(),
        "gh was asked in a folder that isn't a repository: {:?}",
        std::fs::read_to_string(&calls)
    );
    std::fs::write(data.0.join("go"), "").unwrap();
}
