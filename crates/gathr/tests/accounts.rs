//! Named accounts (0002, 0026): each added, named and signed in or out on its own login folder.
//! The replay engine answers `auth login`, `auth logout` and `auth status` in the folder it's given.

mod common;

use common::{TempDir, TestService, gathr, recording, sign_in_account};
use serde_json::{Value, json};

fn accounts(service: &TestService) -> Vec<Value> {
    let answer = service.call("GET", "/api/accounts");
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["accounts"].as_array().cloned().unwrap()
}

#[test]
fn accounts_are_added_and_renamed_each_with_its_own_folder_and_kept_after_a_restart() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    service.add_account("Personal");
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "claude", "name": " Work " }),
    );
    assert_eq!(added.status, 200, "{}", added.body);
    let work = added.json();
    assert_eq!(
        (&work["name"], &work["login"]),
        (&json!("Work"), &json!("signed_out")),
        "{work}"
    );
    let id = work["id"].as_str().unwrap().to_string();

    let refused = [
        (
            json!({ "engine": "claude", "name": "  " }),
            400,
            "a blank name",
        ),
        (
            json!({ "engine": "claude", "name": "work" }),
            409,
            "a name already used",
        ),
        (
            json!({ "engine": "codex", "name": "Codex" }),
            400,
            "an engine Gathr can't run yet",
        ),
    ];
    for (body, status, what) in refused {
        let answer = service.send("POST", "/api/accounts", body);
        assert_eq!(answer.status, status, "{what}: {}", answer.body);
        assert_eq!(
            answer.json()["error"],
            "Add failed",
            "{what}: {}",
            answer.body
        );
    }

    let renamed = service.send(
        "PATCH",
        &format!("/api/accounts/{id}"),
        json!({ "name": "Équipe" }),
    );
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let other_case = json!({ "engine": "claude", "name": "équipe" });
    let taken = service.send("POST", "/api/accounts", other_case);
    assert_eq!(
        taken.status, 409,
        "a name differing only in a non-ASCII letter's case: {}",
        taken.body
    );
    let taken = service.send(
        "PATCH",
        &format!("/api/accounts/{id}"),
        json!({ "name": "PERSONAL" }),
    );
    assert_eq!(
        taken.status, 409,
        "renamed to a name already used: {}",
        taken.body
    );
    let unknown = service.send("PATCH", "/api/accounts/nope", json!({ "name": "X" }));
    assert_eq!(unknown.status, 404, "{}", unknown.body);
    drop(service);

    let service = TestService::with_engine(&data.0, &[]);
    let names: Vec<_> = accounts(&service)
        .iter()
        .map(|a| a["name"].clone())
        .collect();
    assert_eq!(names, [json!("Personal"), json!("Équipe")]);
}

/// SQLite reports every rule a save breaks alike, so only the name rule may read as a taken name.
#[test]
fn a_save_that_fails_for_another_reason_says_why_and_not_that_the_name_is_taken() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.add_account("Work");
    let db = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    for table in ["accounts", "agents"] {
        let trigger = format!(
            "CREATE TRIGGER no_{table} BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'forced'); END"
        );
        db.execute(&trigger, []).unwrap();
    }
    let rename = "CREATE TRIGGER no_rename BEFORE UPDATE ON accounts BEGIN SELECT RAISE(ABORT, 'forced'); END";
    db.execute(rename, []).unwrap();

    let saves = [
        (
            "POST",
            "/api/accounts".to_string(),
            json!({ "engine": "claude", "name": "Home" }),
            "Add failed",
        ),
        (
            "PATCH",
            format!("/api/accounts/{work}"),
            json!({ "name": "Home" }),
            "Rename failed",
        ),
        (
            "POST",
            "/api/agents".to_string(),
            json!({ "name": "Orin", "account": work }),
            "Add failed",
        ),
    ];
    for (method, path, body, error) in saves {
        let failed = service.send(method, &path, body);
        assert_eq!(failed.status, 500, "{method} {path}: {}", failed.body);
        let answer = failed.json();
        assert_eq!(answer["error"], error, "{method} {path}: {answer}");
        assert!(
            !answer["why"].as_str().unwrap().contains("has that name"),
            "{method} {path}: {answer}"
        );
        let (ok, trace) = gathr(&data.0, &["diag", "trace", answer["ref"].as_str().unwrap()]);
        assert!(ok && trace.to_string().contains("forced"), "{trace}");
    }
}

#[test]
fn signing_one_account_in_and_out_leaves_the_other_as_it_was() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let first = service.signed_in(&data.0);
    let id = service.add_account("Work");

    let started = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(started.status, 200, "{}", started.body);
    let code = json!({ "code": "made-up-code#made-up-state" });
    service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    // The login file lands just before `auth login` exits and the sign-in ends: wait for both.
    common::account_until(&service, &id, "the sign-in to finish", |a| {
        a["login"] == "signed_in" && a["sign_in"].is_null()
    });
    let folder = data.0.join("accounts").join(&id).join("claude");
    assert!(
        folder.join(".credentials.json").is_file(),
        "not signed in its own folder"
    );

    let out = service.call("POST", &format!("/api/accounts/{first}/sign-out"));
    assert_eq!(out.status, 200, "{}", out.body);
    let login =
        |id: &str| service.call("GET", &format!("/api/accounts/{id}")).json()["login"].clone();
    assert_eq!(login(&first), "signed_out");
    assert_eq!(
        login(&id),
        "signed_in",
        "signing one out signed Work out too"
    );
    let unknown = service.call("POST", "/api/accounts/nope/sign-in");
    assert_eq!(unknown.status, 404, "{}", unknown.body);
}

fn turns_done(history: &Value) -> usize {
    let turns = history["turns"].as_array().cloned().unwrap_or_default();
    turns.iter().filter(|t| t["status"] == "done").count()
}

#[test]
fn a_new_data_folder_has_no_accounts_and_its_chats_wait_for_the_first_one_signed_in() {
    let data = TempDir::new();
    let reply = recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &reply)]);
    assert_eq!(accounts(&service), Vec::<Value>::new());
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.json()["status"], "waiting_sign_in", "{}", sent.body);

    let work = service.add_account("Work");
    sign_in_account(&data.0, &work);
    service.wait_for(&chat, 10, "the message to run on Work", |h| {
        turns_done(h) == 1
    });
}

#[test]
fn removing_an_account_signs_it_out_deletes_its_folder_and_moves_chats_to_the_next() {
    let data = TempDir::new();
    let reply = recording("signed_in");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &reply)]);
    let work = service.add_account("Work");
    let second = service.add_account("Second");
    sign_in_account(&data.0, &work);
    sign_in_account(&data.0, &second);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Say hi" }));
    service.wait_for(&chat, 10, "the first reply", |h| turns_done(h) == 1);

    let removed = service.call("DELETE", &format!("/api/accounts/{work}"));
    assert_eq!(removed.status, 200, "{}", removed.body);
    let names: Vec<Value> = accounts(&service)
        .iter()
        .map(|a| a["name"].clone())
        .collect();
    assert_eq!(names, [json!("Second")]);
    assert!(
        !data.0.join("accounts").join(&work).exists(),
        "the removed account's folder is still there"
    );
    let sql = format!(
        "SELECT error_type FROM event_log WHERE name = 'gathr.engine.sign_out' \
         AND json_extract(attrs, '$.account') = '{work}'"
    );
    let (ok, rows) = gathr(&data.0, &["diag", "query", &sql]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"].as_array().map(Vec::len), Some(1), "{rows}");
    assert!(rows["rows"][0]["error_type"].is_null(), "{rows}");

    service.send("POST", &path, json!({ "text": "Say hi again" }));
    service.wait_for(&chat, 10, "the next reply, on Second", |h| {
        turns_done(h) == 2
    });
}

/// An account added while another is being removed must not hand the chats back to the one
/// going: its engine is held mid-turn, so the removal waits on its close while Second is added.
#[test]
fn an_account_added_during_a_removal_never_picks_the_account_being_removed() {
    let data = TempDir::new();
    let go = data.0.join("close-done");
    let held = data.0.join("held.jsonl");
    let frames = std::fs::read_to_string(recording("signed_in")).unwrap();
    let (first, rest) = frames.split_once('\n').unwrap();
    std::fs::write(&held, format!("{first}\n{{\"dir\":\"hold\"}}\n{rest}")).unwrap();
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_REPLAY", &held), ("GATHR_REPLAY_HOLD", &go)],
    );
    let work = service.signed_in(&data.0);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_eq!(sent.json()["status"], "running", "{}", sent.body);

    let removed = std::thread::scope(|scope| {
        let removing = scope.spawn(|| service.call("DELETE", &format!("/api/accounts/{work}")));
        // Long enough for the removal to reach the engine's close, which waits on `go`.
        std::thread::sleep(std::time::Duration::from_millis(500));
        let second = service.add_account("Second");
        sign_in_account(&data.0, &second);
        std::fs::write(&go, "").unwrap();
        removing.join().unwrap()
    });
    assert_eq!(removed.status, 200, "{}", removed.body);

    let next = service.new_chat();
    let path = format!("/api/chats/{next}/messages");
    let sent = service.send("POST", &path, json!({ "text": "Say hi" }));
    assert_ne!(
        sent.json()["status"],
        "waiting_sign_in",
        "the chats went back to the removed account: {}",
        sent.body
    );
    service.wait_for(&next, 10, "the reply on Second", |h| turns_done(h) == 1);
}

#[test]
fn removing_the_account_of_a_turn_under_way_stops_it_and_closes_its_engine_gently() {
    let data = TempDir::new();
    let asks = recording("approval");
    let service = TestService::with_engine(&data.0, &[("GATHR_REPLAY", &asks)]);
    let id = service.signed_in(&data.0);
    let chat = service.new_chat();
    let path = format!("/api/chats/{chat}/messages");
    service.send("POST", &path, json!({ "text": "Write note.txt" }));
    service.wait_for(&chat, 10, "the approval", |h| {
        h["approvals"][0]["tool"] == "Write"
    });

    let removed = service.call("DELETE", &format!("/api/accounts/{id}"));
    assert_eq!(removed.status, 200, "{}", removed.body);
    let history = service.call("GET", &format!("/api/chats/{chat}")).json();
    assert_eq!(history["turns"][0]["status"], "stopped", "{history}");
    let sql = "SELECT error_type FROM event_log WHERE name = 'gathr.engine.close'";
    let (ok, rows) = gathr(&data.0, &["diag", "query", sql]);
    assert!(ok, "{rows}");
    assert_eq!(rows["rows"].as_array().map(Vec::len), Some(1), "{rows}");
    assert!(rows["rows"][0]["error_type"].is_null(), "killed: {rows}");
}

#[test]
fn a_remove_whose_sign_out_fails_keeps_the_account_and_answers_a_reference_that_traces() {
    let data = TempDir::new();
    let missing = data.0.join("no-such-claude.exe");
    let service = TestService::start_with(&data.0, &[("GATHR_CLAUDE_PATH", &missing)]);
    let id = service.signed_in(&data.0);

    let removed = service.call("DELETE", &format!("/api/accounts/{id}"));
    assert_eq!(removed.status, 500, "{}", removed.body);
    let answer = removed.json();
    assert_eq!(answer["error"], "Remove failed", "{answer}");
    let reference = answer["ref"].as_str().unwrap();
    let (ok, trace) = gathr(&data.0, &["diag", "trace", reference]);
    assert!(ok, "{trace}");
    let failed = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "gathr.engine.sign_out");
    assert_eq!(
        failed.map(|e| &e["error_type"]),
        Some(&json!("sign_out")),
        "{trace}"
    );
    assert_eq!(accounts(&service).len(), 1, "the account went anyway");
    let login = data
        .0
        .join("accounts")
        .join(&id)
        .join("claude/.credentials.json");
    assert!(login.is_file(), "the login went anyway");

    // Never signed in, so there's no login to end: it goes without Claude Code.
    let spare = service.add_account("Spare");
    let removed = service.call("DELETE", &format!("/api/accounts/{spare}"));
    assert_eq!(removed.status, 200, "{}", removed.body);
    let unknown = service.call("DELETE", "/api/accounts/nope");
    assert_eq!(unknown.status, 404, "{}", unknown.body);
}
