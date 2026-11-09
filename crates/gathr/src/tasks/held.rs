//! What a task is held on, how a turn that ended well settles it, and the giver's hear-back.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Finish, Start, State, Task, ready, set, words};
use crate::Error;
use crate::event::Event;

/// `left` twice in a row by turns that ran no tool and left nothing open is `blocked` (0010).
const IDLE_LEFTS: i64 = 2;

/// What the task still has open, each in a few words: its waits, the tasks given in its turns that
/// aren't settled, and its PR.
pub(super) fn open_on(conn: &Connection, task: i64) -> Result<Vec<String>, Error> {
    let mut open = Vec::new();
    let mut waits = conn.prepare_cached(
        "SELECT kind, target, (SELECT count(*) FROM waits p WHERE p.part_of = w.id)
         FROM waits w WHERE w.task = ?1 AND w.state IN ('open', 'met') AND w.part_of IS NULL
         ORDER BY w.created_ts",
    )?;
    let rows = waits.query_map([task], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (kind, target, parts) = row?;
        open.push(words::wait_name(conn, &kind, target.as_deref(), parts)?);
    }
    // Settled isn't enough: the giver isn't done before it has heard back on them.
    let mut given = conn.prepare_cached(&format!(
        "SELECT c.number, c.owner FROM tasks c JOIN turns t ON t.id = c.given_in
         WHERE t.task = ?1 AND (c.state NOT IN {} OR c.heard = 0) ORDER BY c.number",
        super::SETTLED
    ))?;
    let rows = given.query_map([task], |r| Ok((r.get::<_, i64>(0)?, r.get(1)?)))?;
    for row in rows {
        let (number, owner): (i64, Option<String>) = row?;
        open.push(format!(
            "#{number} ({})",
            words::owner_name(conn, owner.as_deref())?
        ));
    }
    let mut commands = conn.prepare_cached(
        "SELECT b.description FROM background b JOIN turns t ON t.id = b.turn_id
         WHERE t.task = ?1 ORDER BY b.started_ts",
    )?;
    for described in commands.query_map([task], |r| r.get::<_, String>(0))? {
        open.push(format!("a background command ({})", described?));
    }
    // A review's PR is what it reviewed, not work its owner still owes.
    let pr: Option<Option<i64>> = conn
        .query_row(
            "SELECT pr FROM tasks WHERE number = ?1 AND kind = 'work'",
            [task],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(pr) = pr.flatten() {
        open.push(format!("#{pr} to merge"));
    }
    Ok(open)
}

/// A turn on the task ended well: `blocked` as its owner said; `held` while something it started
/// is open, saying on what and whether its owner called it done (G75); else `done`, or for `left`
/// another turn.
pub fn settle(
    tx: &Transaction,
    t: &Task,
    finish: Option<Finish>,
    note: Option<String>,
    tools: i64,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let task = t.number;
    let open = open_on(tx, task)?;
    let idle = |n: i64| {
        tx.execute(
            "UPDATE tasks SET idle_lefts = ?2 WHERE number = ?1",
            params![task, n],
        )
    };
    if finish == Some(Finish::Blocked) {
        idle(0)?;
        let why = note.unwrap_or_else(|| "its owner can't go on".into());
        set(tx, task, State::Blocked, Some(&why), trace)?;
        return Ok(Vec::new());
    }
    if !open.is_empty() {
        idle(0)?;
        let said = match finish {
            Some(Finish::Left) => "left",
            _ => "done",
        };
        let why = format!("{said}, waiting on {}", open.join(", "));
        set(tx, task, State::Held, Some(&why), trace)?;
        return Ok(Vec::new());
    }
    if finish != Some(Finish::Left) {
        idle(0)?;
        set(tx, task, State::Done, note.as_deref(), trace)?;
        return Ok(Vec::new());
    }
    let lefts: i64 = tx.query_row(
        "SELECT idle_lefts FROM tasks WHERE number = ?1",
        [task],
        |r| r.get(0),
    )?;
    let lefts = if tools == 0 { lefts + 1 } else { 0 };
    idle(lefts)?;
    if lefts >= IDLE_LEFTS {
        let why = "left twice by turns that did nothing and left nothing open";
        set(tx, task, State::Blocked, Some(why), trace)?;
        return Ok(Vec::new());
    }
    let carry = match &note {
        Some(left) => format!("you left part of it: {left}"),
        None => "you left part of it".to_string(),
    };
    Ok(vec![ready(tx, task, Some(&carry))?])
}

/// The hear-backs on the tasks given in `turn`, one per giver: once, when that turn has ended (so
/// the giver is done handing out work) and every task it gave there is settled. Each says what
/// became of every task, on the giver's own task from that turn while it's still open. A task
/// restarted after its hear-back makes its group heard again when it settles.
pub fn hear_backs(tx: &Transaction, turn: &str, trace: &str) -> Result<Vec<Start>, Error> {
    let found: Option<(String, String, Option<i64>)> = tx
        .query_row(
            "SELECT u.chat_id, u.status, t.number FROM turns u LEFT JOIN tasks t
             ON t.number = u.task AND t.state NOT IN ('done', 'cancelled') WHERE u.id = ?1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((chat, status, own)) = found else {
        return Ok(Vec::new());
    };
    if !matches!(status.as_str(), "done" | "failed" | "stopped") {
        return Ok(Vec::new());
    }
    let givers: Vec<String> = {
        let mut stmt = tx.prepare_cached(
            "SELECT DISTINCT giver FROM tasks WHERE given_in = ?1 AND giver IS NOT NULL
             AND heard = 0 AND owner IS NOT NULL",
        )?;
        let rows = stmt.query_map([turn], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut starts = Vec::new();
    for giver in givers {
        let group: Vec<i64> = {
            let mut stmt = tx.prepare_cached(
                // The person's answers come back as they're sent (person::heard), not with these.
                "SELECT number FROM tasks WHERE given_in = ?1 AND giver = ?2 AND owner IS NOT NULL
                 ORDER BY number",
            )?;
            let rows = stmt.query_map(params![turn, giver], |r| r.get(0))?;
            rows.collect::<Result<_, _>>()?
        };
        let settled = group
            .into_iter()
            .map(|n| super::read(tx, n))
            .collect::<Result<Vec<_>, _>>()?;
        if settled.iter().any(|t| !t.state.settled()) {
            continue;
        }
        tx.execute(
            "UPDATE tasks SET heard = 1 WHERE given_in = ?1 AND giver = ?2 AND owner IS NOT NULL",
            params![turn, giver],
        )?;
        Event::point("gathr.task.hear_back")
            .trace(trace)
            .chat(&chat)
            .ok()
            .attr("gathr.turn.id", turn)
            .attr("gen_ai.agent.id", giver.as_str())
            .attr("gathr.task.count", settled.len())
            .insert(tx)?;
        let text = words::hear_back(tx, &settled)?;
        starts.push(Start::queue(tx, (&chat, &giver), (own, Some(turn)), text)?);
    }
    Ok(starts)
}
