use super::*;

#[test]
fn a_report_ask_after_auto_compaction_restores_the_tasks_full_brief() {
    let data = TempDir::new();
    let frames = std::fs::read_to_string(common::recording("compact_claude")).unwrap();
    let compacted = frames
        .lines()
        .find(|l| l.contains("compact_boundary"))
        .unwrap()
        .to_string();
    let replay = played(&data.0, 12, |i, turn| {
        if i == 0 {
            turn.insert(turn.len() - 1, compacted.clone());
        }
    });
    let room = room(&data.0, &replay, "claude", &[]);
    let task = give_ann(&room, "Restore the brief");
    let turn = turn_on(&room, task, 2);
    let opened = common::replay::read_in(&data.0, turn["id"].as_str().unwrap());
    for words in [
        "Your conversation was compacted.",
        "Task #1 from Lead: Restore the brief",
        "Why: CI is red",
        "Done when: CI on main is green",
        "Proof: the CI run on the PR",
        "Where: the project's worktree, on a new branch",
        "Your last turn on task #1",
    ] {
        assert!(opened.contains(words), "missing {words:?}: {opened}");
    }
}

fn compact_room() -> (TempDir, Room) {
    let data = TempDir::new();
    let frames = std::fs::read_to_string(common::recording("compact_claude")).unwrap();
    let lines: Vec<_> = frames.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.contains(r#"\"content\":\"/compact\""#))
        .unwrap();
    let compacting = data.0.join("compact.jsonl");
    std::fs::write(&compacting, lines[at..].join("\n")).unwrap();
    let room = room(
        &data.0,
        &plain(&data.0),
        "claude",
        &[("GATHR_REPLAY_SLASH", &compacting)],
    );
    (data, room)
}

fn compact_ann(room: &Room) {
    let path = format!("/api/chats/{}/agents/{}/compact", room.chat, room.ann);
    let compacted = room.service.send("POST", &path, json!({}));
    assert_eq!(compacted.status, 200, "{}", compacted.body);
    let history = quiet(room);
    let id = compacted.json()["turn"].as_str().unwrap().to_string();
    let turn = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["id"] == id)
        .unwrap();
    assert_eq!(turn["status"], "done", "{turn}");
}

#[test]
fn a_failed_restoration_marker_read_is_found_from_the_turns_reference() {
    let (data, room) = compact_room();
    let task = give_ann(&room, "Task A");
    compact_ann(&room);
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "CREATE TRIGGER fail_summed BEFORE UPDATE OF summed ON task_sessions
         WHEN OLD.summed = 1 AND NEW.summed = 0
         BEGIN SELECT RAISE(FAIL, 'forced restoration marker read failure'); END;",
    )
    .unwrap();
    restart(&room, task);
    settled(&room, task, "blocked");
    let turn = turn_on(&room, task, 3);
    let (ok, trace) = common::gathr(&data.0, &["diag", "trace", turn["ref"].as_str().unwrap()]);
    assert!(ok, "{trace}");
    let event = trace["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["name"] == "gathr.context.summed")
        .unwrap_or_else(|| panic!("{trace}"));
    assert_eq!(event["error_type"], "sqlite", "{event}");
    assert!(
        event
            .to_string()
            .contains("forced restoration marker read failure"),
        "{event}"
    );
}

#[test]
fn another_tasks_turn_does_not_take_the_compacted_conversations_restoration() {
    let (data, room) = compact_room();
    let a = give_ann(&room, "Task A");
    compact_ann(&room);
    let b = give_ann(&room, "Task B");
    let opened = turn_on(&room, b, 1);
    let opened = common::replay::read_in(&data.0, opened["id"].as_str().unwrap());
    assert!(
        !opened.contains("Your conversation was compacted."),
        "B got A's restoration: {opened}"
    );
    restart(&room, a);
    settled(&room, a, "blocked");
    let opened = turn_on(&room, a, 3);
    let opened = common::replay::read_in(&data.0, opened["id"].as_str().unwrap());
    assert!(
        opened.contains("Your conversation was compacted."),
        "A lost its restoration: {opened}"
    );
    assert!(
        opened.contains(&format!(
            "#{b} Ann blocked (no report: two turns ended without one): Task B"
        )),
        "{opened}"
    );
}

#[test]
fn a_compacted_conversation_gets_its_restoration_after_the_service_restarts() {
    let (data, room) = compact_room();
    let a = give_ann(&room, "Task A");
    compact_ann(&room);
    let Room {
        service,
        data: folder,
        chat,
        lead,
        ann,
    } = room;
    assert!(service.stop().success());
    let service = TestService::with_engine(
        &data.0,
        &[
            ("GATHR_REPLAY", &data.0.join("played.jsonl")),
            ("GATHR_REPLAY_SESSION", Path::new("echo")),
        ],
    );
    let room = Room {
        service,
        data: folder,
        chat,
        lead,
        ann,
    };
    restart(&room, a);
    settled(&room, a, "blocked");
    let opened = turn_on(&room, a, 3);
    let opened = common::replay::read_in(&data.0, opened["id"].as_str().unwrap());
    assert!(
        opened.contains("Your conversation was compacted."),
        "lost at restart: {opened}"
    );
}

#[test]
fn after_compaction_a_task_gets_its_plan_and_open_tasks_back() {
    let (data, room) = compact_room();
    let task = give_ann(&room, "Build the notes filter");
    let routed = text(&call_tool(
        room.service.info.port,
        &token_of(&room, &room.ann),
        "ask_person",
        json!({"what": "Which colour?", "choices": [{"label":"Blue"}, {"label":"Green"}]}),
    ));
    assert!(routed.contains("request #2"), "{routed}");
    quiet(&room);
    let filed = text(&call_tool(
        room.service.info.port,
        &room.lead_token(),
        "ask_person",
        json!({"for_task":2, "what":"Blue or green?", "choices":[{"label":"Blue"},{"label":"Green"}]}),
    ));
    assert!(filed.starts_with("Task #3 "), "{filed}");
    quiet(&room);
    let answered = room.service.send(
        "POST",
        "/api/person-tasks/answer",
        json!({"answers":[{"task":3,"pick":["Blue"]}]}),
    );
    assert_eq!(answered.status, 200, "{}", answered.body);
    let history = quiet(&room);
    let before = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["task"] == task)
        .count();
    let conn = rusqlite::Connection::open(data.0.join("gathr.db")).unwrap();
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .unwrap();
    conn.execute(
        "INSERT INTO plans (id, chat_id, ask_id, lead, created_ts)
         SELECT 700, chat_id, ask_id, giver, 1 FROM tasks WHERE number = ?1",
        [task],
    )
    .unwrap();
    conn.execute_batch(
        "INSERT INTO plan_versions (id, plan_id, n, message_id, turn_id, title, text, created_ts, decided)
         VALUES (700, 700, 2, 'seed-plan', 'seed-turn', 'Find notes', 'Filter the notes', 1, 'approved');",
    ).unwrap();
    conn.execute(
        "INSERT INTO plan_parts (version_id, n, owner, what, why, done, proof, place, size, size_lines, task)
         SELECT 700, 3, owner, what, 'why', 'done', 'proof', 'place', 'size', 10, number
         FROM tasks WHERE number = ?1", [task],
    ).unwrap();
    compact_ann(&room);
    restart(&room, task);
    settled(&room, task, "blocked");
    let turn = turn_on(&room, task, before + 1);
    let opened = common::replay::read_in(&data.0, turn["id"].as_str().unwrap());
    for words in [
        "Plan: Find notes, version 2, approved",
        "Your task is part 3: Build the notes filter",
        &format!("#{task} Ann ready: Build the notes filter"),
        "Questions to the person from task #1:",
        "Blue or green?: Picked: Blue",
    ] {
        assert!(opened.contains(words), "missing {words:?}: {opened}");
    }
    assert_eq!(opened.matches("Picked: Blue").count(), 1, "{opened}");
    let later = turn_on(&room, task, before + 2);
    let later = common::replay::read_in(&data.0, later["id"].as_str().unwrap());
    assert!(
        !later.contains("Your conversation was compacted."),
        "{later}"
    );

    // The lead's next turn has no task, but still serves this ask and its plan.
    conn.execute(
        "UPDATE plan_versions SET decided = 'sent_back' WHERE id = 700",
        [],
    )
    .unwrap();
    let path = format!("/api/chats/{}/agents/{}/compact", room.chat, room.lead);
    let compacted = room.service.send("POST", &path, json!({}));
    assert_eq!(compacted.status, 200, "{}", compacted.body);
    quiet(&room);
    let sent = text(&call_tool(
        room.service.info.port,
        &token_of(&room, &room.ann),
        "message",
        json!({"to": "Lead", "text": "Please continue"}),
    ));
    assert!(sent.starts_with("Sent to Lead"), "{sent}");
    let history = quiet(&room);
    let lead = history["turns"]
        .as_array()
        .unwrap()
        .iter()
        .rev()
        .find(|t| t["agent"] == room.lead && t["opened"] != "compaction")
        .unwrap();
    assert!(lead["task"].is_null(), "{lead}");
    let opened = common::replay::read_in(&data.0, lead["id"].as_str().unwrap());
    assert!(
        opened.contains("Plan: Find notes, version 2, revising"),
        "{opened}"
    );
    assert!(opened.contains("No open tasks."), "{opened}");
    assert!(!opened.contains("Your task is part"), "{opened}");
}
