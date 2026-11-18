//! Agents (0026): each runs on its own account with its model and effort, checked against what
//! the account's engine lists, and a saved change applies from the agent's next turn.

mod common;

use std::path::Path;

use common::{TempDir, TestService, gathr, next_live, recording, sign_in_account};
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

/// An agent's face (0074): what it is drawn from, as the API answers it.
fn face(agent: &Value) -> Value {
    json!([agent["face_seed"], agent["gender"], agent["skin"]])
}

#[test]
fn a_new_agents_face_seed_is_its_name_and_a_rename_keeps_it() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let added = service.send(
        "POST",
        "/api/agents",
        json!({ "name": " Tess ", "account": work }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    assert_eq!(face(&added.json()), json!(["Tess", null, null]));
    let tess = added.json()["id"].as_str().unwrap().to_string();

    let path = format!("/api/agents/{tess}");
    let renamed = service.send("PATCH", &path, json!({ "name": "Tessa" }));
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let read = service.call("GET", &path).json();
    assert_eq!(
        (&read["name"], face(&read)),
        (&json!("Tessa"), json!(["Tess", null, null]))
    );

    let shuffled = json!({ "name": "Made-up", "account": work, "face_seed": "k2v9q" });
    let shuffled = service.send("POST", "/api/agents", shuffled);
    assert_eq!(face(&shuffled.json()), json!(["k2v9q", null, null]));
}

#[test]
fn a_faces_gender_skin_tone_and_seed_are_saved_and_bad_ones_refused() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let made = json!({ "name": "Ann", "account": work, "gender": "woman", "skin": 5 });
    let ann = service.send("POST", "/api/agents", made);
    assert_eq!(
        face(&ann.json()),
        json!(["Ann", "woman", 5]),
        "{}",
        ann.body
    );
    let path = format!("/api/agents/{}", ann.json()["id"].as_str().unwrap());

    let changed = json!({ "gender": "man", "skin": 2 });
    let changed = service.send("PATCH", &path, changed);
    assert_eq!(changed.status, 200, "{}", changed.body);
    // Shuffle: a new seed, and nothing else moves.
    let shuffled = service.send("PATCH", &path, json!({ "face_seed": "x7f3p" }));
    assert_eq!(shuffled.status, 200, "{}", shuffled.body);
    let read = service.call("GET", &path).json();
    assert_eq!(
        (&read["name"], face(&read)),
        (&json!("Ann"), json!(["x7f3p", "man", 2]))
    );
    let listed = service.call("GET", "/api/agents").json();
    assert_eq!(face(&listed["agents"][0]), json!(["x7f3p", "man", 2]));
    let cleared = service.send("PATCH", &path, json!({ "gender": null, "skin": null }));
    assert_eq!(face(&cleared.json()), json!(["x7f3p", null, null]));

    let refused = [
        (json!({ "gender": "other" }), "Choose Man or Woman"),
        (json!({ "skin": 7 }), "Choose a skin tone from 1 to 6"),
        (json!({ "skin": 0 }), "Choose a skin tone from 1 to 6"),
        (json!({ "face_seed": " " }), "the face seed is blank"),
    ];
    for (body, why) in refused {
        let answer = service.send("PATCH", &path, body.clone());
        assert_eq!(answer.status, 400, "{body}: {}", answer.body);
        let answer = answer.json();
        assert_eq!(
            (&answer["error"], &answer["why"]),
            (&json!("Save failed"), &json!(why))
        );
        let reference = answer["ref"].as_str().unwrap();
        let (_, trace) = gathr(&data.0, &["diag", "trace", reference]);
        assert!(trace.to_string().contains(why), "{why:?} not in {trace}");
    }
    let made = json!({ "name": "Bo", "account": work, "gender": "other" });
    let answer = service.send("POST", "/api/agents", made);
    assert_eq!(
        (answer.status, &answer.json()["error"]),
        (400, &json!("Add failed"))
    );
}

#[test]
fn a_faces_background_is_the_persons_pick_kept_in_upper_case_and_bad_ones_refused() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let tess = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Tess", "account": work }),
    );
    assert_eq!(tess.json()["background"], json!(null), "{}", tess.body);
    let made = json!({ "name": "Ann", "account": work, "background": "#fca0a7" });
    let ann = service.send("POST", "/api/agents", made);
    assert_eq!(ann.json()["background"], json!("#FCA0A7"), "{}", ann.body);
    let path = format!("/api/agents/{}", ann.json()["id"].as_str().unwrap());

    let changed = service.send("PATCH", &path, json!({ "background": "#7fB5a8" }));
    assert_eq!(changed.status, 200, "{}", changed.body);
    assert_eq!(
        service.call("GET", &path).json()["background"],
        json!("#7FB5A8")
    );
    let listed = service.call("GET", "/api/agents").json();
    assert_eq!(listed["agents"][1]["background"], json!("#7FB5A8"));
    let renamed = service.send("PATCH", &path, json!({ "name": "Anna" }));
    assert_eq!(renamed.json()["background"], json!("#7FB5A8"));

    let why = "Choose a background as #RRGGBB";
    for bad in ["FCA0A7", "#FCA", "#FCA0A", "#FCA0A7 ", "#GGGGGG", "red", ""] {
        let answer = service.send("PATCH", &path, json!({ "background": bad }));
        assert_eq!(answer.status, 400, "{bad:?}: {}", answer.body);
        let answer = answer.json();
        assert_eq!(
            (&answer["error"], &answer["why"]),
            (&json!("Save failed"), &json!(why)),
            "{bad:?}"
        );
        let (_, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
        assert!(trace.to_string().contains(why), "{why:?} not in {trace}");
    }
    let made = json!({ "name": "Bo", "account": work, "background": "blue" });
    let answer = service.send("POST", "/api/agents", made);
    assert_eq!(
        (answer.status, &answer.json()["error"]),
        (400, &json!("Add failed"))
    );
    assert_eq!(
        service.call("GET", &path).json()["background"],
        json!("#7FB5A8")
    );

    let cleared = service.send("PATCH", &path, json!({ "background": null }));
    assert_eq!(
        cleared.json()["background"],
        json!(null),
        "{}",
        cleared.body
    );
}

#[test]
fn a_removed_agent_keeps_its_face_for_its_old_messages() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let made = json!({ "name": "Bo", "account": work, "skin": 3, "face_seed": "p0q8", "background": "#6cd5b3" });
    let bo = service.send("POST", "/api/agents", made).json();
    let deleted = service.call(
        "DELETE",
        &format!("/api/agents/{}", bo["id"].as_str().unwrap()),
    );
    assert_eq!(deleted.status, 200, "{}", deleted.body);
    let listed = service.call("GET", "/api/agents").json();
    let removed = &listed["removed"][0];
    assert_eq!(
        (&removed["id"], &removed["name"], face(removed)),
        (&bo["id"], &json!("Bo"), json!(["p0q8", null, 3]))
    );
    assert_eq!(removed["background"], json!("#6CD5B3"), "{removed}");
}
