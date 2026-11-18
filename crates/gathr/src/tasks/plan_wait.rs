//! A task waits for its plan (0063, I4): building work that hasn't started, on an ask the hold
//! holds, is `waiting` with the reason `PLAN_WAIT` until a change of size or Approve releases it.
//! A started task runs on: its turns are a building task's, never held.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::records::set;
use super::{Start, State, back_in_line, words};
use crate::Error;
use crate::event::Event;
use crate::words::Opened;

/// The reason of a task that waits for its plan, read as a mark.
pub const PLAN_WAIT: &str = "waits for its plan";

/// Whether the task waits for its plan: building work not started, on an ask that is held.
pub fn waits(conn: &Connection, task: i64) -> Result<bool, Error> {
    let ask: Option<Option<String>> = conn
        .prepare_cached(
            "SELECT ask_id FROM tasks
             WHERE number = ?1 AND kind = 'work' AND look_into = 0 AND started_ts IS NULL",
        )?
        .query_row([task], |r| r.get(0))
        .optional()?;
    match ask.flatten() {
        Some(ask) => crate::hold::ask_held(conn, &ask),
        None => Ok(false),
    }
}

/// The ask's hold may have changed (I4.2): held now, its ready tasks that wait drop their queued
/// turns and wait for the plan; not held, what waited for it goes back in line. The tasks moved,
/// and the turns the release starts.
pub fn resized(tx: &Transaction, ask: &str, trace: &str) -> Result<(Vec<i64>, Vec<Start>), Error> {
    let held = crate::hold::ask_held(tx, ask)?;
    let tasks: Vec<i64> = {
        let mut stmt = tx.prepare_cached(
            "SELECT number FROM tasks WHERE ask_id = ?1
             AND ((?2 AND state = 'ready') OR (NOT ?2 AND state = 'waiting' AND reason = ?3))
             ORDER BY number",
        )?;
        let rows = stmt.query_map(params![ask, held, PLAN_WAIT], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let (mut moved, mut starts) = (Vec::new(), Vec::new());
    for n in tasks {
        if !held {
            starts.extend(back_in_line(tx, n, trace)?);
        } else if waits(tx, n)? {
            tx.execute(
                "UPDATE turns SET stop_why = ?2 WHERE task = ?1 AND status IN ('queued', 'waiting_sign_in')",
                params![n, format!("its task #{n} waits for its plan")],
            )?;
            set(tx, n, State::Waiting, Some(PLAN_WAIT), trace)?;
        } else {
            continue;
        }
        moved.push(n);
    }
    if !moved.is_empty() {
        let chat: String = tx.query_row("SELECT chat_id FROM asks WHERE id = ?1", [ask], |r| {
            r.get(0)
        })?;
        Event::point("gathr.hold.wait")
            .trace(trace)
            .chat(&chat)
            .ok()
            .attr("gathr.ask.id", ask)
            .attr("gathr.hold.how", if held { "held" } else { "released" })
            .attr("gathr.task.count", moved.len())
            .insert(tx)?;
    }
    Ok((moved, starts))
}

/// The person's line for what their change of size held or released, in Gathr's own words
/// ("Add the tags table waits for the plan."), never the words an agent is told (0073).
pub fn noted(tx: &Transaction, ask: &str, moved: &[i64]) -> Result<(), Error> {
    if moved.is_empty() {
        return Ok(());
    }
    let mut whats = Vec::new();
    for &n in moved {
        let what = super::read(tx, n)?.what;
        whats.push(what.trim_end_matches(['.', '!', '?']).to_string());
    }
    let tail = match (crate::hold::ask_held(tx, ask)?, whats.len() == 1) {
        (true, true) => "waits for the plan.",
        (true, false) => "wait for the plan.",
        (false, true) => "is back in line.",
        (false, false) => "are back in line.",
    };
    let chat: String = tx.query_row("SELECT chat_id FROM asks WHERE id = ?1", [ask], |r| {
        r.get(0)
    })?;
    let last = whats.pop().unwrap_or_default();
    let what = match whats.is_empty() {
        true => last,
        false => format!("{} and {last}", whats.join(", ")),
    };
    crate::store::say(tx, &chat, &format!("{what} {tail}"))?;
    Ok(())
}

/// The person changed the ask's size (I4.5): to new from a repair, its Supervisor is told while it
/// has open work; to a repair, the agent its message went to, when Gathr held something of it or
/// tasks went back in line. `moved` are the tasks the change held or released.
pub fn told(
    tx: &Transaction,
    ask: &str,
    (size, was): (&str, Option<&str>),
    moved: &[i64],
) -> Result<Option<Start>, Error> {
    let sql = format!(
        "SELECT a.chat_id, a.held_ts IS NOT NULL, m.turn_id, u.agent_id, {}
         FROM asks a LEFT JOIN messages m ON m.id = a.id LEFT JOIN turns u ON u.id = m.turn_id
         WHERE a.id = ?1",
        crate::asks::OPEN
    );
    type Row = (String, bool, Option<String>, Option<String>, bool);
    let (chat, held, turn, sent_to, open): Row = tx.query_row(&sql, [ask], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })?;
    let title = crate::asks::title(tx, ask)?;
    let mut tasks = Vec::new();
    for &n in moved {
        let owner = super::read(tx, n)?.owner;
        tasks.push(format!(
            "#{n} ({})",
            words::owner_name(tx, owner.as_deref())?
        ));
    }
    let tasks = tasks.join(", ");
    let (to, text) = match size {
        "new" if was == Some("repair") && open => {
            let lead = crate::team::members(tx, &chat)?
                .into_iter()
                .find(|m| m.role == crate::team::Role::Supervisor)
                .map(|m| m.agent);
            let text = match moved.is_empty() {
                true => format!(
                    "The person made \"{title}\" new, so it waits for a plan. Write the plan with plan."
                ),
                false => format!(
                    "The person made \"{title}\" new, so it waits for a plan. Its tasks that hadn't started wait \
                     for the plan to be approved: {tasks}. Write the plan with plan, and cancel a task the plan replaces."
                ),
            };
            (lead, text)
        }
        "repair" if held || !moved.is_empty() => {
            let text = match moved.is_empty() {
                true => format!(
                    "The person made \"{title}\" a repair, so it goes ahead without a plan."
                ),
                false => format!(
                    "The person made \"{title}\" a repair, so it goes ahead without a plan. Its tasks that waited \
                     for the plan are back in line: {tasks}."
                ),
            };
            (sent_to, text)
        }
        _ => return Ok(None),
    };
    let Some(to) = to else {
        return Ok(None);
    };
    let start = Start::queue(
        tx,
        (&chat, &to),
        (None, turn.as_deref()),
        Opened::Resized,
        text,
    )?;
    // Else it would serve the ask of the turn it answers: for a message split off a turn it was
    // steered into, the other ask.
    tx.execute(
        "UPDATE turns SET ask_id = ?2 WHERE id = ?1",
        [&start.turn, ask],
    )?;
    Ok(Some(start))
}
