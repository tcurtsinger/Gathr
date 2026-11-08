//! Named accounts (0002, 0026): each added, named and signed in or out on its own login folder.
//! The replay engine answers `auth login`, `auth logout` and `auth status` in the folder it's given.

mod common;

use common::{TempDir, TestService, sign_in};
use serde_json::{Value, json};

fn accounts(service: &TestService) -> Vec<Value> {
    let answer = service.call("GET", "/api/accounts");
    assert_eq!(answer.status, 200, "{}", answer.body);
    answer.json()["accounts"].as_array().cloned().unwrap()
}

#[test]
fn the_account_signed_in_before_accounts_had_names_carries_over_signed_in() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::with_engine(&data.0, &[]);
    let listed = accounts(&service);
    assert_eq!(listed.len(), 1, "{listed:?}");
    let main = &listed[0];
    assert_eq!(
        (&main["id"], &main["name"], &main["engine"], &main["login"]),
        (
            &json!("main"),
            &json!("Main"),
            &json!("claude"),
            &json!("signed_in")
        ),
        "{main}"
    );
}

#[test]
fn accounts_are_added_and_renamed_each_with_its_own_folder_and_kept_after_a_restart() {
    let data = TempDir::new();
    let service = TestService::with_engine(&data.0, &[]);
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
    assert_ne!(id, "main");

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
        json!({ "name": "Side" }),
    );
    assert_eq!(renamed.status, 200, "{}", renamed.body);
    let taken = service.send(
        "PATCH",
        &format!("/api/accounts/{id}"),
        json!({ "name": "MAIN" }),
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
    assert_eq!(names, [json!("Main"), json!("Side")]);
}

#[test]
fn signing_one_account_in_and_out_leaves_the_other_as_it_was() {
    let data = TempDir::new();
    sign_in(&data.0);
    let service = TestService::with_engine(&data.0, &[]);
    let work = service.send(
        "POST",
        "/api/accounts",
        json!({ "engine": "claude", "name": "Work" }),
    );
    let id = work.json()["id"].as_str().unwrap().to_string();

    let started = service.call("POST", &format!("/api/accounts/{id}/sign-in"));
    assert_eq!(started.status, 200, "{}", started.body);
    let code = json!({ "code": "made-up-code#made-up-state" });
    service.send("POST", &format!("/api/accounts/{id}/sign-in/code"), code);
    let signed_in = common::account_until(&service, &id, "the sign-in to finish", |a| {
        a["login"] == "signed_in"
    });
    assert!(signed_in["sign_in"].is_null(), "{signed_in}");
    let folder = data.0.join("accounts").join(&id).join("claude");
    assert!(
        folder.join(".credentials.json").is_file(),
        "not signed in its own folder"
    );

    let out = service.call("POST", "/api/accounts/main/sign-out");
    assert_eq!(out.status, 200, "{}", out.body);
    let login =
        |id: &str| service.call("GET", &format!("/api/accounts/{id}")).json()["login"].clone();
    assert_eq!(login("main"), "signed_out");
    assert_eq!(
        login(&id),
        "signed_in",
        "signing Main out signed Work out too"
    );
    let unknown = service.call("POST", "/api/accounts/nope/sign-in");
    assert_eq!(unknown.status, 404, "{}", unknown.body);
}
