//! What a start does with the work the stop left (0010, 0032).

use rusqlite::Connection;

use super::{State, hear, held, person, ready, set};
use crate::Error;
use crate::event::Event;

/// The service started: each turn the stop cut off is cut, with the steps it left running, and
/// carries on in one carry-on (0010, 0032). A task whose carry-on couldn't be saved, or that was
/// working with no turn, is `stopped`, with why, for a restart; its dependants stay `waiting`. A
/// cut-off turn that gave tasks ends here, so its giver's hear-back is queued for the start to take
/// over. All in one transaction: when it fails, nothing is cut and the next start tries again.
pub fn recover(conn: &mut Connection) -> Result<usize, Error> {
    let mut tx = conn.transaction()?;
    let trace = crate::ids::trace_id();
    let running: Vec<(String, String, Option<i64>)> = {
        let mut stmt =
            tx.prepare("SELECT id, chat_id, task FROM turns WHERE status = 'running'")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (turn, chat, task) in &running {
        // Each in its own savepoint: one that fails leaves nothing, and its task is stopped below.
        let saved = {
            let point = tx.savepoint()?;
            crate::carry::restarted(&point, turn).and_then(|()| Ok(point.commit()?))
        };
        if let Err(e) = saved {
            let event = Event::point("gathr.turn.carry_on")
                .trace(&trace)
                .chat(chat)
                .error("sqlite")
                .attr("gathr.turn.id", turn.as_str())
                .attr("why", e.to_string());
            let event = match task {
                Some(task) => event.task(*task),
                None => event,
            };
            event.insert(&tx)?;
        }
    }
    let working: Vec<i64> = {
        // A task with a carry-on waiting (a turn not yet started that answers a turn on the same
        // task) carries on in it: it isn't stopped (0032).
        let mut stmt = tx.prepare(
            "SELECT number FROM tasks WHERE state = 'working' AND NOT EXISTS
             (SELECT 1 FROM turns u JOIN turns was ON was.id = u.answers
              WHERE u.task = tasks.number AND u.status IN ('queued', 'waiting_sign_in')
                AND was.task = tasks.number)",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let giving: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT u.id FROM turns u JOIN tasks t ON t.given_in = u.id
             WHERE u.status = 'running'",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    crate::store::cut_running(&tx)?;
    for &task in &working {
        let why = "Gathr stopped during its turn";
        set(&tx, task, State::Stopped, Some(why), &trace)?;
    }
    // Background commands ended with their engines: a task held only on one carries on.
    crate::background::clear(&tx)?;
    let holding: Vec<i64> = {
        // A discussed question has no turn to start: the sweep brings it back.
        let mut stmt =
            tx.prepare("SELECT number FROM tasks WHERE state = 'held' AND kind != 'person'")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for task in holding {
        if held::open_on(&tx, task)?.is_empty() {
            let carry = "Gathr stopped and ended your background command";
            ready(&tx, task, Some(carry))?;
        }
    }
    person::links::recover(&tx, &trace)?;
    for turn in &giving {
        hear::hear_backs(&tx, turn, &trace)?;
    }
    tx.commit()?;
    Ok(working.len())
}
