//! A task held on a background command (G174): its hold ends when the command does, whenever that is
//! seen, and its owner can finish it from any turn.

use super::*;

/// Ann's task, held on a command her turn on it started and that still runs.
fn held_on_a_command(conn: &mut Connection) -> i64 {
    let (task, _) = give(conn, "ann", &[], None);
    let (on, command) = (format!("t{task}"), format!("b{task}"));
    turn(conn, &on, "ann", Some(task), "running");
    tasks::started(conn, task, T).unwrap();
    let running = [(command, "cargo test".to_string())];
    gathr::background::set(conn, ("c", Some("ann")), &running, Some(&on)).unwrap();
    tasks::finish(conn, "ann", task, Finish::Done, &Said::default()).unwrap();
    tasks::ended(conn, task, Outcome::Ok { tools: 3 }, T).unwrap();
    let (held, why) = state(conn, task);
    assert_eq!(held, State::Held, "{why:?}");
    assert!(why.unwrap().contains("background command"));
    task
}

/// Live, #139: the command ended while Ann's engine ran a turn on another task, so Claude Code
/// said so inside that turn and began none of its own for the held task.
#[test]
fn a_held_task_settles_when_its_command_ends_during_another_turn() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = held_on_a_command(&mut conn);
    turn(&conn, "t2", "ann", None, "running");
    let (ended, _) =
        tasks::background_changed(&mut conn, ("c", Some("ann")), &[], Some("t2"), true, T).unwrap();
    assert_eq!(ended, [(format!("t{task}"), Some(task))]);
    assert_eq!(state(&conn, task).0, State::Done);
}

/// A turn the owner left goes on once the command ends: another turn on it.
#[test]
fn a_task_left_on_a_command_goes_on_when_it_ends() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    turn(&conn, "t1", "ann", Some(task), "running");
    tasks::started(&mut conn, task, T).unwrap();
    let running = [("b1".to_string(), "cargo test".to_string())];
    gathr::background::set(&conn, ("c", Some("ann")), &running, Some("t1")).unwrap();
    tasks::finish(&mut conn, "ann", task, Finish::Left, &Said::default()).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok { tools: 3 }, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Held);
    let (_, starts) =
        tasks::background_changed(&mut conn, ("c", Some("ann")), &[], None, true, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Ready);
    assert_eq!(starts.len(), 1, "{starts:?}");
    assert_eq!(starts[0].task, Some(task));
}

/// With nothing to release it (the engine will begin a turn of its own on the task), the hold stays
/// for that turn to settle.
#[test]
fn a_command_that_ends_with_the_engine_idle_leaves_its_task_to_the_turn_the_engine_begins() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = held_on_a_command(&mut conn);
    tasks::background_changed(&mut conn, ("c", Some("ann")), &[], None, false, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Held);
}

/// G124, G174: the owner finishes a held task from a turn on another task. The finish is kept, and
/// applies once the hold ends: at once when nothing is left open.
#[test]
fn its_owner_finishes_a_held_task_from_any_turn() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let task = held_on_a_command(&mut conn);
    let said = Said {
        note: Some("PR #9"),
        ..Said::default()
    };
    tasks::finish(&mut conn, "ann", task, Finish::Done, &said).unwrap();
    tasks::release(&mut conn, task, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Held, "the command still runs");
    tasks::background_changed(&mut conn, ("c", Some("ann")), &[], None, true, T).unwrap();
    assert_eq!(state(&conn, task), (State::Done, Some("PR #9".to_string())));

    // A hold whose command was forgotten already (#139 sat so) settles as its owner finishes it.
    let task = held_on_a_command(&mut conn);
    gathr::background::set(&conn, ("c", Some("ann")), &[], None).unwrap();
    assert_eq!(state(&conn, task).0, State::Held);
    tasks::finish(&mut conn, "ann", task, Finish::Done, &said).unwrap();
    tasks::release(&mut conn, task, T).unwrap();
    assert_eq!(state(&conn, task), (State::Done, Some("PR #9".to_string())));
    let theirs = tasks::finish(&mut conn, "bo", task, Finish::Done, &said);
    assert_eq!(refusal(theirs), format!("task #{task} isn't yours"));
}
