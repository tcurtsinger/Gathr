//! Agents (0026): each runs on its own account with its model and effort, checked against what
//! the account's engine lists, and a saved change applies from the agent's next turn.

mod common;

use std::path::Path;

use common::{TempDir, TestService, gathr, recording, sign_in_account};
use serde_json::{Value, json};

/// The arguments of the engine's last start.
fn last_run(args_file: &Path) -> Vec<String> {
    let runs = std::fs::read_to_string(args_file).unwrap_or_default();
    let last = runs.lines().last().expect("the engine never started");
    serde_json::from_str(last).unwrap()
}

fn has(args: &[String], arg: &str) -> bool {
    args.iter().any(|a| a == arg)
}

fn turns_done(history: &Value) -> usize {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.iter().filter(|t| t["status"] == "done").count()
}

#[test]
fn a_turn_runs_on_its_agents_account_model_and_effort_and_a_change_applies_from_the_next() {
    let data = TempDir::new();
    let args = data.0.join("engine-args.jsonl");
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &recording("signed_in")),
            ("GATHR_REPLAY_ARGS", &args),
        ],
    );
    let (work, second) = (service.add_account("Work"), service.add_account("Second"));
    sign_in_account(&data.0, &work);
    sign_in_account(&data.0, &second);
    let body = json!({ "name": "Orin", "account": second, "model": "opus", "effort": "high" });
    let orin = service.send("POST", "/api/agents", body);
    assert_eq!(orin.status, 200, "{}", orin.body);
    let orin = orin.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let send = |done: usize, what: &str| {
        let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": orin }));
        assert_eq!(sent.status, 200, "{what}: {}", sent.body);
        service.wait_for(&chat, 10, what, |h| turns_done(h) == done);
        last_run(&args)
    };

    let first = send(1, "the first turn");
    assert!(
        has(&first, "--model=opus") && has(&first, "--effort=high"),
        "{first:?}"
    );
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(history["agent"], orin.as_str(), "{history}");
    let started = "SELECT json_extract(attrs, '$.\"gathr.account\"') AS account FROM event_log \
                   WHERE name = 'gathr.engine.start' AND error_type IS NULL";
    let (_, rows) = gathr(&data.0, &["diag", "query", started]);
    assert_eq!(
        rows["rows"][0]["account"],
        second.as_str(),
        "not on its account: {rows}"
    );

    let low = json!({ "effort": "low" });
    let changed = service.send("PATCH", &format!("/api/agents/{orin}"), low);
    assert_eq!(changed.status, 200, "{}", changed.body);
    let next = send(2, "the turn after the change");
    assert!(
        has(&next, "--effort=low"),
        "the change didn't apply: {next:?}"
    );
    assert!(
        has(&next, "--resume"),
        "the same account's session wasn't resumed: {next:?}"
    );

    let moved = json!({ "account": work });
    let moved = service.send("PATCH", &format!("/api/agents/{orin}"), moved);
    let moved = moved.json();
    assert_eq!(
        (&moved["model"], &moved["effort"]),
        (&Value::Null, &Value::Null),
        "{moved}"
    );
    let on_work = send(3, "the turn on the new account");
    assert!(
        !on_work.iter().any(|a| a.starts_with("--model")),
        "{on_work:?}"
    );
    assert!(
        !has(&on_work, "--resume"),
        "a session from another login folder was resumed: {on_work:?}"
    );
}

/// The next JSON sent on the live socket (the service's frames are unmasked and unfragmented).
fn next_live(socket: &mut std::net::TcpStream) -> Value {
    use std::io::Read;
    loop {
        let mut head = [0u8; 2];
        socket
            .read_exact(&mut head)
            .expect("the live socket went quiet");
        let len = match head[1] & 0x7F {
            126 => {
                let mut n = [0u8; 2];
                socket.read_exact(&mut n).unwrap();
                u16::from_be_bytes(n) as usize
            }
            127 => {
                let mut n = [0u8; 8];
                socket.read_exact(&mut n).unwrap();
                u64::from_be_bytes(n) as usize
            }
            n => n as usize,
        };
        let mut body = vec![0u8; len];
        socket.read_exact(&mut body).unwrap();
        if head[0] & 0x0F == 1 {
            return serde_json::from_slice(&body).unwrap();
        }
    }
}

/// A window that didn't send a turn still learns whose it is, so its Retry goes to that agent.
#[test]
fn live_turn_updates_say_which_agent_the_turn_was_sent_to() {
    turn_updates_name_their_agent(false);
}

/// The agent comes from the turn the service runs, so a failed status write can't send a Retry
/// to no agent (the window would run it as another agent, on the default account).
#[test]
fn live_turn_updates_name_their_agent_even_when_the_status_write_fails() {
    turn_updates_name_their_agent(true);
}

fn turn_updates_name_their_agent(break_status_writes: bool) {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &recording("signed_in"))]);
    let work = service.add_account("Work");
    sign_in_account(&data.0, &work);
    let orin = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Orin", "account": work }),
    );
    let orin = orin.json()["id"].as_str().unwrap().to_string();
    let chat = service.new_chat();
    if break_status_writes {
        let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
        let trigger = "CREATE TRIGGER no_status BEFORE UPDATE OF status ON turns \
                       BEGIN SELECT RAISE(ABORT, 'forced'); END";
        db.execute(trigger, []).unwrap();
    }
    let mut live = service.live();

    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi", "agent": orin }));
    assert_eq!(sent.status, 200, "{}", sent.body);
    let mut statuses = Vec::new();
    while statuses.last().map(String::as_str) != Some("done") {
        let update = next_live(&mut live);
        if update["type"] != "turn" {
            continue;
        }
        assert_eq!(
            update["agent"],
            orin.as_str(),
            "a turn update without its agent: {update}"
        );
        statuses.push(update["status"].as_str().unwrap_or_default().to_string());
    }
    assert!(statuses.contains(&"running".to_string()), "{statuses:?}");
    if break_status_writes {
        let sql =
            "SELECT ref FROM event_log WHERE name = 'gathr.turn.status' AND error_type = 'sqlite'";
        let (_, rows) = gathr(&data.0, &["diag", "query", sql]);
        assert!(
            rows["rows"][0]["ref"].is_string(),
            "the failed write wasn't recorded: {rows}"
        );
    }
}

#[test]
fn saves_are_checked_against_the_engine_and_an_account_in_use_stays() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let agent = |name: &str, model: &str, effort: &str| json!({ "name": name, "account": work, "model": model, "effort": effort });
    let added = service.send("POST", "/api/agents", agent("Orin", "sonnet", "max"));
    assert_eq!(added.status, 200, "{}", added.body);
    let orin = added.json()["id"].as_str().unwrap().to_string();

    let refused = [
        (
            agent("Cass", "nosuchmodel", "high"),
            400,
            "a model the engine doesn't list",
        ),
        (
            agent("Cass", "opus", "extreme"),
            400,
            "an effort the model doesn't list",
        ),
        (agent("orin", "opus", "high"), 409, "a name already used"),
        (
            json!({ "name": "Cass", "account": "nope" }),
            400,
            "no such account",
        ),
        (json!({ "name": " ", "account": work }), 400, "a blank name"),
    ];
    for (body, status, what) in refused {
        let answer = service.send("POST", "/api/agents", body);
        assert_eq!(answer.status, status, "{what}: {}", answer.body);
        let answer = answer.json();
        assert_eq!(answer["error"], "Add failed", "{what}: {answer}");
        // A reference explains itself (0019): its trace holds the answer's why.
        let reference = answer["ref"].as_str().unwrap();
        let (_, trace) = gathr(&data.0, &["diag", "trace", reference]);
        let why = answer["why"].as_str().unwrap();
        assert!(
            trace.to_string().contains(why),
            "{what}: {why:?} not in {trace}"
        );
    }

    let kept = service.call("DELETE", &format!("/api/accounts/{work}"));
    assert_eq!(kept.status, 409, "{}", kept.body);
    assert_eq!(kept.json()["why"], "agents use it: Orin", "{}", kept.body);
    let reference = kept.json()["ref"].as_str().unwrap().to_string();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", &reference]);
    assert!(ok, "{trace}");
    assert!(
        trace.to_string().contains("agents use it: Orin"),
        "the refusal's reference doesn't trace to why: {trace}"
    );

    let deleted = service.call("DELETE", &format!("/api/agents/{orin}"));
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    let removed = service.call("DELETE", &format!("/api/accounts/{work}"));
    assert_eq!(removed.status, 200, "{}", removed.body);
}
