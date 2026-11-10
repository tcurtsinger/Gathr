//! No fallback to the person's installed engines (0025, owner task 1284).
mod common;
use common::{TempDir, TestService};
use serde_json::json;

#[test]
fn a_turn_waits_for_gathrs_copy_instead_of_borrowing_from_path() {
    let data = TempDir::new();
    let bin = data.0.join("persons-programs");
    std::fs::create_dir_all(&bin).unwrap();
    for name in ["claude", "codex"] {
        let path = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(common::replay_engine(), path).unwrap();
    }
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let empty = std::path::Path::new("");
    let env = [
        ("PATH", std::path::Path::new(&path)),
        ("GATHR_CLAUDE_PATH", empty),
        ("GATHR_CODEX_PATH", empty),
    ];
    let service = TestService::start_with(&data.0, &env);
    let mut chats = Vec::new();
    for (engine, label) in [("claude", "Claude Code"), ("codex", "Codex")] {
        let info = service
            .call("GET", &format!("/api/engines/{engine}"))
            .json();
        assert!(
            info["found"].is_null(),
            "borrowed the person's {label}: {info}"
        );
        let account = service
            .send(
                "POST",
                "/api/accounts",
                json!({"engine":engine,"name":label}),
            )
            .json();
        let agent = service
            .send(
                "POST",
                "/api/agents",
                json!({"account":account["id"],"name":label}),
            )
            .json();
        let chat = service.new_chat();
        let sent = service.send(
            "POST",
            &format!("/api/chats/{chat}/messages"),
            json!({"text":"Say hi", "agent":agent["id"]}),
        );
        assert_eq!(
            sent.status, 200,
            "missing copy failed instead of waiting: {}",
            sent.body
        );
        let history = service.call("GET", &format!("/api/chats/{chat}")).json();
        assert_eq!(history["turns"][0]["status"], "queued", "{history}");
        assert_eq!(history["turns"][0]["held"], engine, "{history}");
        assert_eq!(
            history["turns"][0]["why"],
            format!("{label} is downloading"),
            "{history}"
        );
        chats.push((chat, engine));
    }
    drop(service);
    let service = TestService::start_with(&data.0, &env);
    for (chat, engine) in chats {
        let history = service.wait_for(&chat, 10, "the engine wait to survive restart", |h| {
            h["turns"][0]["held"] == engine
        });
        assert_eq!(history["turns"][0]["status"], "queued", "{history}");
    }
}
