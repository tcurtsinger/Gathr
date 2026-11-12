//! What a task is held on, how a turn that ended well settles it, and the giver's hear-back.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::records::OptionalTask;
use super::{Finish, Start, State, Task, cascade, read, ready, set, words};
use crate::Error;
use crate::event::Event;

/// `left` twice in a row by turns that ran no tool and left nothing open is `blocked` (0010).
const IDLE_LEFTS: i64 = 2;

/// Tasks `t` its owner called done that are held only until their PR merges, read from the reason
/// `settle` writes. They count as done for the tasks `after` them: a review of that PR may be what
/// the merge waits on (0010). Never null, so `NOT` keeps every other task.
pub(super) const ONLY_ITS_MERGE: &str = "(t.state = 'held' AND t.pr IS NOT NULL
    AND t.reason IS 'done, waiting on #' || t.pr || ' to merge')";

/// The task is held only on its merge and no review of its PR is open: nobody on the team is on it
/// but whoever merges, so its giver hears where it stands (backlog "Stall": #85 and #86 sat
/// reviewed and unmerged while the giver, the one to see them merged, heard nothing). A review
/// that gave this task doesn't count: it waits on this hear-back (found in #100's own review).
fn only_its_merge_left(conn: &Connection, task: i64) -> Result<bool, Error> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM tasks t WHERE t.number = ?1 AND {ONLY_ITS_MERGE}
         AND NOT EXISTS (SELECT 1 FROM tasks r WHERE r.chat_id = t.chat_id AND r.pr = t.pr
             AND r.kind = 'review' AND r.state NOT IN {}
             AND NOT EXISTS (SELECT 1 FROM turns u WHERE u.id = t.given_in AND u.task = r.number)))",
        super::SETTLED
    );
    Ok(conn.query_row(&sql, [task], |r| r.get(0))?)
}

/// A review settled: the givers of the work held only on the same PR's merge may hear back now.
pub(super) fn after_review(
    tx: &Transaction,
    review: i64,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let turns: Vec<String> = {
        let sql = format!(
            "SELECT DISTINCT t.given_in FROM tasks r JOIN tasks t ON t.chat_id = r.chat_id
             AND t.pr = r.pr AND t.kind = 'work' AND t.heard = 0 AND t.given_in IS NOT NULL
             WHERE r.number = ?1 AND r.kind = 'review' AND {ONLY_ITS_MERGE}"
        );
        let mut stmt = tx.prepare_cached(&sql)?;
        let rows = stmt.query_map([review], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut starts = Vec::new();
    for turn in turns {
        starts.extend(hear_backs(tx, &turn, trace)?);
    }
    Ok(starts)
}

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
        let why = "2 turns left this task unfinished without progress or open work";
        set(tx, task, State::Blocked, Some(why), trace)?;
        return Ok(Vec::new());
    }
    let carry = match &note {
        Some(left) => format!("you left this work: {left}"),
        None => "work remains".to_string(),
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
    // Archiving cancelled every task in its chat: nobody is to hear back on them there.
    if !matches!(status.as_str(), "done" | "failed" | "stopped")
        || crate::archive::archived(tx, &chat)?.is_some()
    {
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
        let mut pending = false;
        for t in &settled {
            pending |= !t.state.settled() && !only_its_merge_left(tx, t.number)?;
        }
        if pending {
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
        let mut start = Start::queue(tx, (&chat, &giver), (own, Some(turn)), text)?;
        start.hear_back = true;
        starts.push(start);
    }
    Ok(starts)
}

/// Commands that ended: the turn that started each, and that turn's task.
pub type Ended = Vec<(String, Option<i64>)>;

/// What the engine lists as running in the background now, saved; with `release`, each task held on
/// a command that ended and left nothing open settles (G174). Released when the engine is at work on
/// another turn: Claude Code says a command ended inside that turn and begins none for the task.
/// With the engine idle, the turn it begins on the task settles it, as any turn on it does.
pub fn background_changed(
    conn: &mut Connection,
    engine: (&str, Option<&str>),
    listed: &[(String, String)],
    turn: Option<&str>,
    release: bool,
    trace: &str,
) -> Result<(Ended, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let ended = crate::background::set(&tx, engine, listed, turn)?;
    let mut starts = Vec::new();
    if release {
        let mut tasks: Vec<i64> = ended.iter().filter_map(|(_, task)| *task).collect();
        tasks.dedup();
        for task in tasks {
            starts.extend(released(&tx, task, trace)?);
        }
    }
    tx.commit()?;
    Ok((ended, starts))
}

/// A held task's hold may be over, or narrower: settled as its owner last finished it, else as the
/// turn that held it said ("done, waiting on …", "left, waiting on …"). Still held on something
/// else, its reason names only what's left, and what follows it moves on that (#156's review: work
/// held on its merge alone counts as done for a review of it). The owner's finish is kept until
/// the hold ends.
pub fn release(conn: &mut Connection, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let starts = released(&tx, task, trace)?;
    tx.commit()?;
    Ok(starts)
}

fn released(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let Some(t) = read(tx, task).optional_task()? else {
        return Ok(Vec::new());
    };
    if t.state != State::Held {
        return Ok(Vec::new());
    }
    let (finish, note): (Option<String>, Option<String>) = tx.query_row(
        "SELECT finish, finish_note FROM tasks WHERE number = ?1",
        [task],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let ends = open_on(tx, task)?.is_empty();
    if ends {
        tx.execute(
            "UPDATE tasks SET finish = NULL, finish_note = NULL WHERE number = ?1",
            [task],
        )?;
    }
    let said = match t.reason.as_deref() {
        Some(r) if r.starts_with("left,") => Finish::Left,
        _ => Finish::Done,
    };
    // While something is still open, the hold is only renamed: the owner's finish waits for its end.
    let (finish, note) = match ends {
        true => (
            finish.as_deref().and_then(Finish::named).unwrap_or(said),
            note,
        ),
        false => (said, None),
    };
    let mut starts = settle(tx, &t, Some(finish), note, 1, trace)?;
    starts.extend(cascade(tx, task, trace)?);
    Ok(starts)
}
