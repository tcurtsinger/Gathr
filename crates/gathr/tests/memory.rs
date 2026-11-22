//! Memory through a real service and the replay engine: an agent's `note` lands in the right file
//! with who wrote it, the window lists it, and the agent's next engine is briefed with it.

mod common;

use std::path::{Path, PathBuf};

use common::{TempDir, TestService, call_tool, held_recording, sign_in_account, text, until};
use gathr::folders::{beside, real};
use serde_json::{Value, json};

/// Each engine start's arguments, as the replay engine appended them.
fn starts(args: &Path) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(args).unwrap_or_default();
    let lines = text.lines().map(|l| serde_json::from_str(l).unwrap());
    lines.collect()
}

/// The briefing of the `n`th engine start, read while that engine still runs.
fn briefing(args: &Path, n: usize) -> String {
    let start = until(10, "the engine's start", || starts(args).into_iter().nth(n));
    let at = start
        .iter()
        .position(|a| a == "--append-system-prompt-file");
    std::fs::read_to_string(&start[at.unwrap() + 1]).unwrap()
}

fn token(env: &Path, n: usize) -> String {
    until(10, "the engine's token", || {
        let text = std::fs::read_to_string(env).ok()?;
        let line: Value = serde_json::from_str(text.lines().nth(n)?).ok()?;
        line["GATHR_AGENT_TOKEN"].as_str().map(String::from)
    })
}

#[test]
fn an_agents_note_lands_in_its_file_and_briefs_its_next_engine() {
    let data = TempDir::new();
    let (go, args, env) = (
        data.0.join("go"),
        data.0.join("args.jsonl"),
        data.0.join("env.jsonl"),
    );
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &held_recording(&data.0, "signed_in")),
            ("GATHR_REPLAY_HOLD", &go),
            ("GATHR_REPLAY_ARGS", &args),
            ("GATHR_REPLAY_ENV", &env),
        ],
    );
    let account = service.add_account("Ann's");
    sign_in_account(&data.0, &account);
    let added = service.send(
        "POST",
        "/api/agents",
        json!({ "name": "Ann", "account": account }),
    );
    let ann = added.json()["id"].as_str().unwrap().to_string();
    let (first, second) = (service.new_chat(), service.new_chat());
    let say = |chat: &str| {
        let path = format!("/api/chats/{chat}/messages");
        let sent = service.send("POST", &path, json!({ "text": "Hi", "agent": ann }));
        assert_eq!(sent.status, 200, "{}", sent.body);
    };
    say(&first);
    let before = briefing(&args, 0);
    assert!(before.contains("call note with one short line"), "{before}");
    assert!(!before.contains("What you know about"), "{before}");
    let port = service.info.port;
    let engine = token(&env, 0);
    let note = |about: &str, line: &str| {
        let asked = json!({ "about": about, "text": line });
        call_tool(port, &engine, "note", asked)
    };

    let said = note("person", "Prefers one pull request per ask");
    assert_eq!(
        text(&said),
        "Noted about you:\nPrefers one pull request per ask"
    );
    let refused = note("project", "The slow tests need the seed data");
    assert_eq!(refused["isError"], true, "{refused}");
    assert!(
        text(&refused).contains("This chat has no project."),
        "{refused}"
    );
    let said = note("work", "Run the type check before the tests");
    assert_eq!(
        text(&said),
        "Noted for own work:\nRun the type check before the tests"
    );

    let memory: PathBuf = real(&beside(&data.0)).join("Memory");
    let you = std::fs::read_to_string(memory.join("You.md")).unwrap();
    assert!(
        you.starts_with("- Prefers one pull request per ask <!-- Ann · "),
        "{you}"
    );
    let own = std::fs::read_to_string(memory.join("Agents").join("Ann.md")).unwrap();
    assert!(own.contains("Run the type check before the tests <!-- Ann · "));
    assert!(!memory.join("Projects").exists());
    let listed = service.call("GET", "/api/memory/you").json();
    assert_eq!(
        listed["notes"][0]["text"],
        "Prefers one pull request per ask"
    );
    assert_eq!(listed["notes"][0]["by"], "Ann");
    let headers = [("Authorization", format!("Bearer {engine}"))];
    let headers: Vec<(&str, &str)> = headers.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let as_engine = service.get("/api/memory", &headers);
    assert_eq!(as_engine.status, 401, "{}", as_engine.body);

    // Ann's next chat closes this engine and starts another, briefed with what she noted.
    std::fs::write(&go, "").unwrap();
    say(&second);
    let after = briefing(&args, 1);
    assert!(
        after.contains("What you know about the person:\n- Prefers one pull request per ask"),
        "{after}"
    );
    assert!(
        after.contains("Your own notes from earlier work:\n- Run the type check before the tests"),
        "{after}"
    );
    assert!(!after.contains("<!--"), "{after}");
}
