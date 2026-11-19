//! A restart in a room at work (0010, 0032): a turn that Quit or a crash cut off carries on by
//! itself at the next start, in one carry-on on the same task, as after an update. Each test stops
//! the work at a checkpoint (an engine that holds) and reads the record there, so nothing passes by
//! timing.
mod common;
#[path = "words/samples.rs"]
mod samples;

use common::update::*;
use common::{gathr, until};
use serde_json::{Value, json};

/// Ann's turn on task #1 is cut by Quit (`kill` false) or a crash: at the next start it carries on
/// once, its task never reads stopped, and its lead hears back once, when the task ends.
fn carries_on_after(kill: bool) {
    let mut room = Room::start();
    let stopped = room.ann_works();

    room.restart(kill);
    let info = room.service.info.clone();
    // The checkpoint: the carry-on runs and holds. Nothing has ended yet.
    let history = carry_on_held(&info, &room.chat, &stopped);
    let cut = turn_by(&history, "id", &stopped).unwrap();
    assert_eq!(
        (&cut["status"], &cut["end"]),
        (&json!("stopped"), &json!("restart"))
    );
    let carry = turn_by(&history, "answers", &stopped).unwrap();
    assert_eq!(carry["task"], 1, "{history}");
    let said = history["messages"].as_array().unwrap().iter();
    let said = said.filter(|m| m["role"] == "gathr" && m["turn"] == carry["id"]);
    let said: Vec<&str> = said.map(|m| m["body"].as_str().unwrap()).collect();
    samples::same("carry_on: restart", said[0]);
    let kept = (json!("carry_on"), json!(gathr::words::CARRY_ON));
    assert_eq!(common::opened(&room.data, &carry["id"]), kept);
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
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

    room.go();
    until(30, "the lead's hear-back to be queued", || {
        (room.lead_heard() == (2, 1)).then_some(())
    });
    room.go_until(&info, "every turn to end", all_ended);
    let states = room.states();
    let done = states.iter().filter(|s| **s == json!("done")).count();
    assert_eq!(done, 1, "the task ended once: {states:?}");
    assert!(!states.contains(&json!("stopped")), "{states:?}");
    assert_eq!(room.lead_heard(), (2, 1), "the lead heard back once");
    assert_eq!(room.carry_ons(&stopped).len(), 1, "still one carry-on");
}

#[test]
fn a_task_cut_off_by_quit_carries_on_by_itself_once() {
    carries_on_after(false);
}

#[test]
fn a_task_cut_off_by_a_crash_carries_on_by_itself_once() {
    carries_on_after(true);
}

/// Stop now saved its carry-on and the turn hadn't ended yet when Gathr quit: the start keeps that
/// one, with its words and its hold, and adds none. It waits for Restart, then runs once.
#[test]
fn a_carry_on_saved_before_the_stop_is_kept_and_not_doubled() {
    let mut room = Room::start();
    let stopped = room.ann_works();
    room.pause(&room.service.info, "/api/pauses/stop");
    until(
        20,
        "Stop now's carry-on, with the turn still running",
        || {
            let history = room.history(&room.service.info);
            let running = turn_by(&history, "id", &stopped)?["status"] == "running";
            let carried = turn_by(&history, "answers", &stopped)?;
            (running && carried["held"] == "pause").then_some(())
        },
    );

    room.restart(false);
    let info = room.service.info.clone();
    held_by_pause(&room, &info, &stopped);
    assert_eq!(room.carry_ons(&stopped).len(), 1, "the restart added one");
    let history = room.history(&info);
    assert_eq!(history["run"]["state"], "stopped", "{history}");
    assert!(
        history.to_string().contains("Stop now ended your turn"),
        "{history}"
    );

    room.pause(&info, "/api/pauses/resume");
    room.go_until(&info, "the carry-on after Restart", |h| {
        turn_by(h, "answers", &stopped).is_some_and(|t| t["status"] == "done")
    });
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
    room.go_until(&info, "everything to end", all_ended);
}

/// A carry-on waiting for its account to sign in still waits after another restart, and its task
/// stays working: once the account is signed in, it runs once.
#[test]
fn a_carry_on_waiting_for_sign_in_outlasts_a_restart() {
    let mut room = Room::start();
    let stopped = room.ann_works();
    let accounts = std::fs::read_dir(room.data.join("accounts")).unwrap();
    let account = accounts.map(|a| a.unwrap().file_name()).next().unwrap();
    let account = account.to_string_lossy().to_string();
    let login = room
        .data
        .join("accounts")
        .join(&account)
        .join("claude/.credentials.json");

    room.stop(false);
    std::fs::remove_file(&login).unwrap();
    room.start_again();
    let waits = |room: &Room| {
        until(30, "the carry-on to wait for sign-in", || {
            let history = room.history(&room.service.info);
            let carried = turn_by(&history, "answers", &stopped)?;
            (carried["status"] == "waiting_sign_in").then_some(())
        })
    };
    waits(&room);
    room.restart(false);
    waits(&room);
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    let state = room.column("SELECT state FROM tasks WHERE number = 1", "state");
    assert_eq!(state, [json!("working")], "{:?}", room.states());

    common::sign_in_account(&room.data, &account);
    room.restart(false);
    let info = room.service.info.clone();
    room.go_until(&info, "the carry-on once signed in", |h| {
        turn_by(h, "answers", &stopped).is_some_and(|t| t["status"] == "done")
    });
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
    room.go_until(&info, "everything to end", all_ended);
}

/// The first `name` event's reference, and what `gathr diag trace` finds from it.
fn traced(room: &Room, name: &str) -> (Value, String) {
    let found = events(&room.data, name);
    let event = found.iter().find(|e| e["status"] == "error");
    let event = event
        .unwrap_or_else(|| panic!("no failed {name}: {found:?}"))
        .clone();
    let (ok, trace) = gathr(
        &room.data,
        &["diag", "trace", event["ref"].as_str().unwrap()],
    );
    assert!(ok, "{trace}");
    (event, trace.to_string())
}

/// A carry-on that can't be saved leaves nothing: the turn is cut, its task is stopped as before,
/// for a restart, and the failure is found from its reference.
#[test]
fn a_carry_on_that_cant_be_saved_stops_its_task_and_says_why() {
    let mut room = Room::start();
    let stopped = room.ann_works();
    room.stop(false);
    let db = rusqlite::Connection::open(room.data.join("gathr.db")).unwrap();
    // The carry-on's turn is written; its message, after it, fails.
    db.execute_batch(
        "CREATE TRIGGER made_to_fail BEFORE INSERT ON messages WHEN NEW.role = 'gathr'
           AND (SELECT opened_by FROM turns WHERE id = NEW.turn_id) = 'carry_on'
         BEGIN SELECT RAISE(ABORT, 'made to fail'); END;",
    )
    .unwrap();

    room.start_again();
    let info = room.service.info.clone();
    let history = room.history(&info);
    assert_eq!(
        turn_by(&history, "id", &stopped).unwrap()["status"],
        "stopped"
    );
    assert!(room.carry_ons(&stopped).is_empty(), "a carry-on was saved");
    let sql = "SELECT state || ': ' || reason AS stands FROM tasks WHERE number = 1";
    let stands = room.column(sql, "stands");
    assert_eq!(stands, [json!("stopped: Gathr stopped during its turn")]);
    let (_, trace) = traced(&room, "gathr.turn.carry_on");
    assert!(
        trace.contains("made to fail") && trace.contains("gathr.task.state"),
        "{trace}"
    );

    db.execute_batch("DROP TRIGGER made_to_fail").unwrap();
    room.go_until(&info, "everything to end", all_ended);
}

/// An update saved its carry-on and Gathr crashed before the hand-over cut the turn; then the
/// start's recovery fails as a whole. It cuts nothing, and the carry-on waits with its turn for the
/// next start, which runs it once: never a second. The failure is found from its reference.
#[test]
fn a_recovery_that_fails_cuts_nothing_and_the_next_start_carries_on_once() {
    let mut room = Room::start();
    let stopped = room.ann_works();
    room.stop(true);
    let mut db = gathr::db::open(&room.data.join("gathr.db")).unwrap();
    gathr::carry::save_all(&mut db, "Gathr's update to 9.9.9").unwrap();
    // The recovery fails at its last step, past the cut, which the take-over never does.
    db.execute_batch(&format!(
        "INSERT INTO background (id, turn_id, description, started_ts) VALUES ('made-up', '{stopped}', 'made up', 1);
         CREATE TRIGGER made_to_fail BEFORE DELETE ON background BEGIN SELECT RAISE(ABORT, 'made to fail'); END;"
    ))
    .unwrap();

    room.start_again();
    // The service answers once the start's recovery and take-over are behind it.
    let history = room.history(&room.service.info);
    assert_eq!(
        turn_by(&history, "id", &stopped).unwrap()["status"],
        "running"
    );
    let (_, trace) = traced(&room, "gathr.task.recover");
    assert!(trace.contains("made to fail"), "{trace}");
    std::thread::sleep(std::time::Duration::from_millis(500));
    let history = room.history(&room.service.info);
    let carried = turn_by(&history, "answers", &stopped).unwrap();
    assert_eq!(
        carried["status"], "queued",
        "it ran before its turn was cut: {history}"
    );

    room.stop(false);
    db.execute_batch("DROP TRIGGER made_to_fail").unwrap();
    room.start_again();
    let info = room.service.info.clone();
    let history = carry_on_held(&info, &room.chat, &stopped);
    assert!(
        history
            .to_string()
            .contains("Gathr's update to 9.9.9 ended your turn"),
        "{history}"
    );
    assert_eq!(room.carry_ons(&stopped).len(), 1, "one carry-on");
    room.go_until(&info, "everything to end", all_ended);
    assert_eq!(room.carry_ons(&stopped).len(), 1, "still one carry-on");
    assert!(
        !room.states().contains(&json!("stopped")),
        "{:?}",
        room.states()
    );
}
