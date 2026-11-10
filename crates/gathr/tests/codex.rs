//! Codex accounts through the service boundary, with an isolated replay engine.
mod common;
use common::{TempDir, TestService};
use serde_json::json;

#[test]
fn a_codex_account_has_its_own_login_folder_and_survives_a_restart() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
    let added = service.send(
        "POST",
        "/api/accounts",
        json!({"engine":"codex", "name":"Codex test"}),
    );
    assert_eq!(added.status, 200, "Codex account refused: {}", added.body);
    let account = added.json();
    assert_eq!(account["engine"], "codex");
    assert_eq!(account["login"], "signed_out");
    drop(service);
    let service = TestService::with_engine(&data.0, &[]);
    let path = format!("/api/accounts/{}", account["id"].as_str().unwrap());
    assert_eq!(service.call("GET", &path).json()["engine"], "codex");
}

#[test]
fn a_codex_authentication_failure_holds_the_turn_for_its_own_account() {
    let data = TempDir::new();
    let replay = common::replay_engine();
    let recording = common::recording("codex_signed_out");
    let service = TestService::start_with(
        &data.0,
        &[("GATHR_CODEX_PATH", &replay), ("GATHR_REPLAY", &recording)],
    );
    let added = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex", "name":"Codex test"}),
        )
        .json();
    let account = added["id"].as_str().unwrap();
    let folder = data.0.join("accounts").join(account).join("codex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(
        folder.join("auth.json"),
        r#"{"tokens":{"refresh_token":"made-up"}}"#,
    )
    .unwrap();
    let shown = service
        .call("GET", &format!("/api/accounts/{account}"))
        .json();
    assert_eq!(
        shown["login"], "signed_in",
        "Codex's login was not read: {shown}"
    );
    let chat = service.new_chat();
    let sent = service.send(
        "POST",
        &format!("/api/chats/{chat}/messages"),
        json!({"text":"Say hi."}),
    );
    assert_eq!(sent.status, 200, "Codex didn't start: {}", sent.body);
    let history = service.wait_for(&chat, 5, "Codex authentication failure", |h| {
        h["turns"][0]["status"] == "waiting_sign_in"
    });
    assert_eq!(history["turns"][0]["status"], "waiting_sign_in");
}

#[test]
fn browser_sign_in_completes_without_a_pasted_code_and_sign_out_ends_it() {
    let data = TempDir::new();
    let replay = common::replay_engine();
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_CODEX_PATH", &replay), ("BROWSER", &replay)],
    );
    let added = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex", "name":"Codex test"}),
        )
        .json();
    let id = added["id"].as_str().unwrap();
    let started = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(started.status, 200, "{}", started.body);
    assert_eq!(
        started.json()["paste"],
        false,
        "Codex asked for a pasted code: {}",
        started.body
    );
    let account = common::account_until(&service, id, "Codex sign-in to finish", |a| {
        a["sign_in"].is_null()
    });
    assert_eq!(account["login"], "signed_in", "{account}");
    assert!(
        !data.0.join("accounts").join(id).join("claude").exists(),
        "Codex created a Claude folder"
    );
    let ended = service.call("POST", &format!("/api/accounts/{id}/sign-out"));
    assert_eq!(ended.status, 200, "{}", ended.body);
    assert_eq!(
        service.call("GET", &format!("/api/accounts/{id}")).json()["login"],
        "signed_out"
    );
}

#[test]
fn codex_models_include_every_page_and_default_efforts() {
    let data = TempDir::new();
    let replay = common::replay_engine();
    let service = TestService::with_engine(&data.0, &[("GATHR_CODEX_PATH", &replay)]);
    let a = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex", "name":"Codex test"}),
        )
        .json();
    let id = a["id"].as_str().unwrap();
    let models = service.call("GET", &format!("/api/accounts/{id}/models"));
    assert_eq!(models.status, 200, "{}", models.body);
    let listed = models.json();
    assert_eq!(listed["version"], "0.166.0", "wrong engine: {listed}");
    assert_eq!(
        listed["models"].as_array().unwrap().len(),
        14,
        "missing pagination or default row: {listed}"
    );
    assert_eq!(listed["models"][0]["id"], "default");
    assert_eq!(
        listed["models"][0]["efforts"],
        json!(["low", "medium", "high", "xhigh", "max", "ultra"])
    );
    assert!(
        listed["models"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["id"] == "latest:sol"
                && m["resolved"] == "gpt-6.1-sol"
                && m["latest"] == true)
    );
    let agent = service.send(
        "POST",
        "/api/agents",
        json!({"name":"Codex agent", "account":id, "effort":"ultra"}),
    );
    assert_eq!(agent.status, 200, "default effort refused: {}", agent.body);
    let refused = service.send(
        "PATCH",
        &format!("/api/agents/{}", agent.json()["id"].as_str().unwrap()),
        json!({"effort":"unknown"}),
    );
    assert_eq!(
        refused.status, 400,
        "unknown effort accepted: {}",
        refused.body
    );
}

#[test]
fn device_failure_is_traced_and_switching_kind_cancels_the_previous_attempt() {
    let data = TempDir::new();
    let replay = common::replay_engine();
    let service = TestService::with_engine(
        &data.0,
        &[("GATHR_CODEX_PATH", &replay), ("BROWSER", &replay)],
    );
    let a = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex","name":"Codex test"}),
        )
        .json();
    let id = a["id"].as_str().unwrap();
    let folder = data.0.join("accounts").join(id).join("codex");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("hold-login"), "").unwrap();
    let route = format!("/api/accounts/{id}/sign-in");
    assert_eq!(
        service.send("POST", &route, json!({"device":true})).json()["user_code"],
        "ABCD-1234"
    );
    let browser = service.call("POST", &route);
    assert_eq!(
        browser.status, 200,
        "switch to browser failed: {}",
        browser.body
    );
    let shown = service.call("GET", &format!("/api/accounts/{id}")).json();
    assert_eq!(
        shown["sign_in"]["device"], false,
        "device attempt was not replaced: {shown}"
    );
    assert!(
        shown["sign_in"]["user_code"].is_null(),
        "old code survived switching: {shown}"
    );
    assert_eq!(service.call("DELETE", &route).status, 200);
    std::fs::remove_file(folder.join("hold-login")).unwrap();
    std::fs::write(folder.join("fail-login"), "").unwrap();
    assert_eq!(
        service.send("POST", &route, json!({"device":true})).status,
        200
    );
    let failed = common::account_until(&service, id, "device sign-in failure", |a| {
        a["sign_in"]["state"] == "failed"
    });
    assert_eq!(
        failed["sign_in"]["device"], true,
        "device retry missing: {failed}"
    );
    let reference = failed["sign_in"]["ref"].as_str().unwrap();
    let trace = service.call("GET", &format!("/api/trace/{reference}"));
    assert!(
        trace.body.contains("The test sign-in was refused"),
        "reference lost the reason: {}",
        trace.body
    );
    let diag = common::gathr(&data.0, &["diag", "health"]);
    assert!(diag.0, "{}", diag.1);
}

#[test]
fn the_browser_opened_for_sign_in_outlives_its_launcher() {
    let data = TempDir::new();
    let replay = common::replay_engine();
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_CODEX_PATH", &replay),
            ("BROWSER", &replay),
            ("GATHR_REPLAY_BROWSER_WAIT", &data.0),
        ],
    );
    let a = service
        .send(
            "POST",
            "/api/accounts",
            json!({"engine":"codex","name":"Codex test"}),
        )
        .json();
    let id = a["id"].as_str().unwrap();
    let signed = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(signed.status, 200, "{}", signed.body);
    let started_deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !data.0.join("browser-started").exists() {
        assert!(
            std::time::Instant::now() < started_deadline,
            "the browser was not launched"
        );
        std::thread::yield_now();
    }
    std::fs::write(data.0.join("browser-go"), "").unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !data.0.join("browser-finished").exists() {
        assert!(
            std::time::Instant::now() < deadline,
            "sign-in killed the person's browser with its launcher"
        );
        std::thread::yield_now();
    }
}
