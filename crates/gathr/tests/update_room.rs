//! An update in a room at work (0032, the small Update button): agents stopped in the middle of
//! tasks carry on truthfully on the copy that takes over. Each test stops the work at a checkpoint
//! (an engine that holds) and reads the record there, so nothing passes by timing.
mod common;

use std::time::{Duration, Instant};

use common::update::*;
use common::{TempDir, call_tool, gathr, until};
use gathr::{VERSION, http};
use serde_json::{Value, json};

/// Nobody is told a task stopped when it is carrying on (the small Update button's plan, change
/// 2): an update that stops a member in the middle of a task never sets the task `stopped`, its
/// lead hears nothing until the task ends, and the one carry-on runs once on the new copy.
#[test]
fn an_update_in_the_middle_of_a_task_carries_it_on_and_tells_its_lead_nothing_false() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();

    room.update();
    // Ann's old engine goes on to its end, as a stopped engine does.
    room.go();
    let next = successor(&room.data, room.service.info.pid);

    // The checkpoint: the carry-on runs and holds. Nothing has ended yet.
    let history = carry_on_held(&next, &room.chat, &stopped);
    assert_eq!(history["turns"][1]["status"], "stopped", "{history}");
    let states = room.states();
    assert!(
        !states.contains(&json!("stopped")),
        "the task was told stopped: {states:?}"
    );
    assert_eq!(
        room.lead_heard(),
        (1, 0),
        "the lead heard back before the task ended"
    );
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");

    room.go();
    // The task ends once, and only then its lead hears back: its turn holds, then is let go.
    until(30, "the lead's hear-back to be queued", || {
        (room.lead_heard() == (2, 1)).then_some(())
    });
    room.go();
    until(30, "every turn to end", || {
        let history = get(&next, &format!("/api/chats/{}", room.chat));
        let turns = history["turns"].as_array()?;
        (turns.len() == 4 && turns[2]["status"] == "done" && turns[3]["status"] == "done")
            .then_some(())
    });
    let states = room.states();
    let done = states.iter().filter(|s| **s == json!("done")).count();
    assert_eq!(done, 1, "the task ended once: {states:?}");
    assert_eq!(room.lead_heard(), (2, 1), "the lead heard back once");
    assert_eq!(room.carry_ons(&stopped).len(), 1, "still one carry-on");
    let history = room.history(&next);
    let ended = turn_by(&history, "answers", &stopped).unwrap()["ended"].clone();
    let turns = history["turns"].as_array().unwrap();
    let heard = turns.iter().rfind(|t| t["agent"] == room.lead.as_str());
    let made = heard.unwrap()["created"].as_str().unwrap();
    assert!(
        made >= ended.as_str().unwrap(),
        "the lead heard back at {made}, before the carry-on ended at {ended}"
    );
}

/// The turn in `history` whose id or `answers` is `id`, by `field`.
fn turn_by<'a>(history: &'a Value, field: &str, id: &str) -> Option<&'a Value> {
    let turns = history["turns"].as_array()?;
    turns.iter().find(|t| t[field] == id)
}

/// Whether every turn in the chat has ended.
fn all_ended(history: &Value) -> bool {
    let turns = history["turns"].as_array().unwrap();
    let ended = |t: &Value| matches!(t["status"].as_str(), Some("done" | "stopped" | "failed"));
    turns.iter().all(ended)
}

/// What the owner reported before the update stopped its turn still stands: the stopped turn's
/// end doesn't clear it, and the task settles by it when the carry-on ends.
#[test]
fn a_report_made_before_the_update_still_settles_the_task() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    // Ann's engine is the second started. It leaves work, with a note no default would give.
    let note = "the made-up half is left";
    let report = json!({ "how": "left", "note": note });
    call_tool(
        room.service.info.port,
        &room.token(1),
        "finish_task",
        report,
    );

    room.update();
    room.go();
    let next = successor(&room.data, room.service.info.pid);
    carry_on_held(&next, &room.chat, &stopped);
    let kept = room.column(
        "SELECT finish_note FROM tasks WHERE number = 1",
        "finish_note",
    );
    assert_eq!(kept, [json!(note)], "the report went with the stopped turn");

    // The carry-on ends with nothing more said, so the task settles by the report: left, and Ann
    // gets the work she left as her next turn.
    let history = room.go_until(&next, "the turn the report asks for", |h| {
        h.to_string().contains("you left this work")
    });
    let left = format!("you left this work: {note}");
    assert!(history.to_string().contains(&left), "{history}");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
    // Nothing is left holding, so no engine outlives the test.
    room.go_until(&next, "everything to end", all_ended);
}

/// A new copy that fails to start: the copy before comes back, and the carry-on runs there, once.
#[test]
fn an_update_that_fails_to_start_carries_the_task_on_on_the_copy_before() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();

    // The command line stands in for the release's service: it exits at once.
    room.update_to(env!("CARGO_BIN_EXE_gathr"));
    room.go();
    let back = successor(&room.data, room.service.info.pid);
    assert_eq!(get(&back, "/api/app")["version"], "0.0.1");
    carry_on_held(&back, &room.chat, &stopped);
    let states = room.states();
    assert!(!states.contains(&json!("stopped")), "{states:?}");
    assert_eq!(room.lead_heard(), (1, 0), "the lead heard back early");
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");

    let history = room.go_until(&back, "the task to end and its lead to hear", |h| {
        h["turns"].as_array().unwrap().len() == 4 && all_ended(h)
    });
    let done = room
        .states()
        .iter()
        .filter(|s| **s == json!("done"))
        .count();
    assert_eq!(done, 1, "the task ended once: {history}");
    assert_eq!(room.lead_heard(), (2, 1), "the lead heard back once");
    assert_eq!(room.carry_ons(&stopped).len(), 1, "still one carry-on");
}

/// The carry-on of `stopped`, saved and held back by the chat's pause on `next`.
fn held_by_pause(room: &Room, next: &gathr::data::ServiceInfo, stopped: &str) {
    until(30, "the carry-on held by the pause", || {
        let history = room.history(next);
        let carried = turn_by(&history, "answers", stopped)?;
        (carried["status"] == "queued" && carried["held"] == "pause").then_some(())
    });
}

/// A chat the person paused before the update is still paused after it: the carry-on waits for
/// Resume, then runs once.
#[test]
fn a_chat_paused_before_the_update_stays_paused_and_its_carry_on_waits_for_resume() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    room.pause(&room.service.info, "/api/pauses");

    room.update();
    room.go();
    let next = successor(&room.data, room.service.info.pid);
    held_by_pause(&room, &next, &stopped);
    let pauses = get(&next, "/api/pauses");
    assert_eq!(
        pauses["pauses"][0]["target"],
        room.chat.as_str(),
        "{pauses}"
    );
    std::thread::sleep(Duration::from_millis(500));
    held_by_pause(&room, &next, &stopped);

    room.pause(&next, "/api/pauses/resume");
    room.go_until(&next, "the carry-on after Resume", |h| {
        turn_by(h, "answers", &stopped).is_some_and(|t| t["status"] == "done")
    });
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
    // Nothing is left holding, so no engine outlives the test.
    room.go_until(&next, "everything to end", all_ended);
}

/// A chat the person stopped before the update is still stopped after it, and the update adds no
/// carry-on of its own there: Stop now's waits for Restart, then runs once.
#[test]
fn a_chat_stopped_before_the_update_stays_stopped_and_restarts_once() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    room.pause(&room.service.info, "/api/pauses/stop");
    room.go();
    until(20, "Stop now's carry-on", || {
        let history = room.history(&room.service.info);
        let ended = turn_by(&history, "id", &stopped)?["status"] == "stopped";
        let carried = turn_by(&history, "answers", &stopped)?;
        (ended && carried["held"] == "pause").then_some(())
    });

    room.update();
    let next = successor(&room.data, room.service.info.pid);
    held_by_pause(&room, &next, &stopped);
    assert_eq!(room.history(&next)["run"]["state"], "stopped");
    assert_eq!(room.carry_ons(&stopped).len(), 1, "the update added one");
    std::thread::sleep(Duration::from_millis(500));
    held_by_pause(&room, &next, &stopped);

    room.pause(&next, "/api/pauses/resume");
    room.go_until(&next, "the carry-on after Restart", |h| {
        turn_by(h, "answers", &stopped).is_some_and(|t| t["status"] == "done")
    });
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    // Nothing is left holding, so no engine outlives the test.
    room.go_until(&next, "everything to end", all_ended);
}

/// A carry-on that never runs leaves no task `working` with no turn: its task cancelled while it
/// waits, or its chat archived.
#[test]
fn a_carry_on_that_never_runs_leaves_no_task_working() {
    for end in ["cancel", "archive"] {
        let room = Room::start();
        let _next = Successor(room.data.clone());
        let stopped = room.ann_works();
        // Paused, so the carry-on is still waiting on the new copy.
        room.pause(&room.service.info, "/api/pauses");
        room.update();
        room.go();
        let next = successor(&room.data, room.service.info.pid);
        held_by_pause(&room, &next, &stopped);

        let path = match end {
            "cancel" => "/api/tasks/1/cancel".to_string(),
            _ => format!("/api/chats/{}/archive", room.chat),
        };
        let ended = http::send(next.port, &next.token, "POST", &path, &json!({})).unwrap();
        assert_eq!(ended.status, 200, "{end}: {}", ended.body);
        if end == "cancel" {
            room.pause(&next, "/api/pauses/resume");
        }
        until(20, "the task and its turns to end", || {
            let state = room.column("SELECT state FROM tasks WHERE number = 1", "state");
            let open = "SELECT id FROM turns WHERE task = 1 \
                        AND status IN ('queued', 'running', 'waiting_sign_in')";
            (state == [json!("cancelled")] && room.column(open, "id").is_empty()).then_some(())
        });
        let ran = format!(
            "SELECT m.id FROM messages m JOIN turns u ON u.id = m.turn_id \
             WHERE u.answers = '{stopped}' AND m.role = 'assistant'"
        );
        assert!(
            room.column(&ran, "id").is_empty(),
            "{end}: the carry-on ran"
        );
    }
}

/// Update gates by its answer: with the worker that stops things held back, Ann's turn still runs,
/// and a message to her and one to the idle lead each become a turn of their own, held, never part
/// of the turn under way nor written to its engine. Each then runs once on the new copy.
#[test]
fn what_is_sent_once_update_answered_waits_and_runs_once_on_the_new_copy() {
    let hold = TempDir::new();
    let switch = hold.0.join("switch");
    let room = Room::start_with(&[("GATHR_APP_SWITCH_HOLD", &switch)]);
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    room.update();

    let path = format!("/api/chats/{}/messages", room.chat);
    let extra = |text: &str, agent: Option<&str>| {
        let sent = room
            .service
            .send("POST", &path, json!({ "text": text, "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
        let sent = sent.json();
        assert_eq!(sent["status"], "held", "{sent}");
        assert!(sent["steered"].is_null(), "{sent}");
        sent["turn"].as_str().unwrap().to_string()
    };
    let to_ann = extra("Made-up extra for Ann", Some(&room.ann));
    let to_lead = extra("Made-up extra for the lead", None);
    let history = room.history(&room.service.info);
    let status = |turn: &str| turn_by(&history, "id", turn).unwrap()["status"].clone();
    assert_eq!(
        status(&stopped),
        "running",
        "the old turn was stopped already"
    );
    for turn in [&to_ann, &to_lead] {
        let held = &turn_by(&history, "id", turn).unwrap()["held"];
        assert_eq!(held, "update", "{history}");
    }
    let written =
        "SELECT id FROM engine_raw WHERE direction = 'in' AND body LIKE '%Made-up extra%'";
    assert!(
        room.column(written, "id").is_empty(),
        "written to an engine"
    );

    std::fs::write(&switch, "").unwrap();
    room.go();
    let next = successor(&room.data, room.service.info.pid);
    let history = room.go_until(&next, "everything to run", |h| {
        let done = |turn: &str| turn_by(h, "id", turn).is_some_and(|t| t["status"] == "done");
        done(&to_ann) && done(&to_lead) && all_ended(h)
    });
    for text in ["Made-up extra for Ann", "Made-up extra for the lead"] {
        let said = format!("SELECT id FROM messages WHERE role = 'user' AND body = '{text}'");
        assert_eq!(room.column(&said, "id").len(), 1, "{text}: {history}");
    }
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    for turn in [&to_ann, &to_lead] {
        let replies = format!(
            "SELECT id FROM messages WHERE turn_id = '{turn}' AND role = 'assistant' AND kind = 'text'"
        );
        assert_eq!(
            room.column(&replies, "id").len(),
            1,
            "answered once: {history}"
        );
    }
}

/// #172's review: a turn that ends by itself after its carry-on was saved, before the update
/// stopped it, settles its task as any turn does. Its carry-on is ended unstarted, so the finished
/// work isn't asked for again on the new copy.
#[test]
fn a_turn_that_ends_by_itself_before_the_stop_gets_no_carry_on() {
    let hold = TempDir::new();
    let switch = hold.0.join("switch");
    let room = Room::start_with(&[("GATHR_APP_SWITCH_HOLD", &switch)]);
    let _next = Successor(room.data.clone());
    let finished = room.ann_works();
    // Update answered and saved the carry-on; the worker that stops things is held back.
    room.update();
    assert_eq!(room.carry_ons(&finished).len(), 1, "the carry-on was saved");
    // Ann's turn goes on to its end by itself: its task is done.
    room.go();
    until(20, "Ann's turn to end by itself", || {
        let history = room.history(&room.service.info);
        (turn_by(&history, "id", &finished)?["status"] == "done").then_some(())
    });

    std::fs::write(&switch, "").unwrap();
    let next = successor(&room.data, room.service.info.pid);
    let history = room.go_until(&next, "the lead to hear back", |h| {
        h["turns"].as_array().unwrap().len() == 4 && all_ended(h)
    });
    let carry_on = turn_by(&history, "answers", &finished).unwrap();
    assert_eq!(carry_on["status"], "stopped", "{history}");
    let why = carry_on["why"].as_str().unwrap_or_default();
    assert!(why.contains("ended by itself"), "{history}");
    let unsaid = "SELECT id FROM messages WHERE role = 'assistant' AND turn_id = '{}'";
    let turn = carry_on["id"].as_str().unwrap();
    let said = room.column(&unsaid.replace("{}", turn), "id");
    assert!(said.is_empty(), "the carry-on ran: {history}");
    let done = room
        .states()
        .iter()
        .filter(|s| **s == json!("done"))
        .count();
    assert_eq!(done, 1, "the task ended once: {history}");
    assert_eq!(room.lead_heard(), (2, 1), "the lead heard back once");
    let unplaced = format!("SELECT id FROM turns WHERE id = '{turn}' AND position IS NULL");
    assert_eq!(
        room.column(&unplaced, "id").len(),
        1,
        "the carry-on started"
    );
}

/// A carry-on that can't be saved: nothing is stopped, the turn under way goes on, and Update
/// fails with a reference that traces to why.
#[test]
fn an_update_that_cant_save_its_carry_ons_stops_nothing_and_says_why() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    let db = rusqlite::Connection::open(room.data.join("gathr.db")).unwrap();
    db.execute_batch(
        "CREATE TRIGGER made_to_fail BEFORE INSERT ON turns WHEN NEW.answers IS NOT NULL
         BEGIN SELECT RAISE(ABORT, 'made to fail'); END;",
    )
    .unwrap();
    publish(
        &room.feed,
        &room.dir.0,
        VERSION,
        env!("CARGO_BIN_EXE_gathrd"),
        "new ui",
    );
    check_now(&room.service);
    ready(&room.service);

    let failed = room
        .service
        .send("POST", "/api/app/update", json!({ "install": true }));
    assert_eq!(failed.status, 500, "{}", failed.body);
    let failed = failed.json();
    let why = failed["why"].as_str().unwrap_or_default();
    assert!(why.contains("nothing was stopped"), "{failed}");
    let (ok, trace) = gathr(
        &room.data,
        &["diag", "trace", failed["ref"].as_str().unwrap()],
    );
    let traced = trace.to_string();
    assert!(
        ok && traced.contains("gathr.app.install") && traced.contains("made to fail"),
        "{trace}"
    );
    let history = room.history(&room.service.info);
    let running = turn_by(&history, "id", &stopped).unwrap();
    assert_eq!(running["status"], "running", "{history}");
    assert!(room.carry_ons(&stopped).is_empty(), "a carry-on was saved");
    assert_eq!(get(&room.service.info, "/api/app")["state"], "ready");

    // The gate was lifted with it: the turn goes on to its end as any turn does.
    db.execute_batch("DROP TRIGGER made_to_fail").unwrap();
    room.go_until(&room.service.info, "the turn to end by itself", |h| {
        turn_by(h, "id", &stopped).is_some_and(|t| t["status"] == "done")
    });
}

/// A turn that never ends doesn't hold the update: its carry-on is saved, the hand-over goes on
/// within the deadline, what still ran is recorded with a reference, and the task, cut off, is
/// never marked stopped.
#[test]
fn a_turn_that_never_ends_doesnt_hold_the_update() {
    let room = Room::start();
    let _next = Successor(room.data.clone());
    let stopped = room.ann_works();
    let began = Instant::now();
    room.update();
    let next = successor(&room.data, room.service.info.pid);
    let took = began.elapsed();
    assert!(took < Duration::from_secs(60), "the update took {took:?}");

    let held = events(&room.data, "gathr.app.hold");
    assert_eq!(held.len(), 1, "{held:?}");
    assert_eq!(held[0]["error_type"], "busy", "{held:?}");
    let (ok, trace) = gathr(
        &room.data,
        &["diag", "trace", held[0]["ref"].as_str().unwrap()],
    );
    assert!(
        ok && trace.to_string().contains("gathr.app.hold"),
        "{trace}"
    );

    carry_on_held(&next, &room.chat, &stopped);
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
    room.go_until(&next, "the carry-on to end", |h| {
        turn_by(h, "answers", &stopped).is_some_and(|t| t["status"] == "done")
    });
    // Nothing is left holding, so no engine outlives the test.
    room.go_until(&next, "everything to end", all_ended);
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
}
