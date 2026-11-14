//! The giver's hear-back (0047, 0051): what became of the tasks it gave, said when there is news
//! it can act on. Trouble at once; the rest when everything given from the same place is in. And
//! the line in the chat for a task that ends done, which wakes nobody.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::held::{ONLY_ITS_MERGE, released};
use super::records::{OptionalTask, heard_as};
use super::{Finish, Kind, SETTLED, Start, State, Task, read, words};
use crate::Error;
use crate::event::Event;

/// A chat's rounds. A round is the turn the person's word began (their message, or their answer
/// to a question: G158) and every turn that answers it, directly or through another. It is named
/// by that first turn.
pub(super) struct Rounds {
    /// Each of the chat's turns, with the turn it answers.
    answers: HashMap<String, Option<String>>,
    /// The turns the person's answers woke.
    woken: HashSet<String>,
    /// The rounds found so far: a long round is walked once, not once for each of its tasks.
    known: RefCell<HashMap<String, String>>,
}

impl Rounds {
    pub(super) fn of(conn: &Connection, chat: &str) -> Result<Rounds, Error> {
        let mut turns = conn.prepare_cached("SELECT id, answers FROM turns WHERE chat_id = ?1")?;
        let answers = turns
            .query_map([chat], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<Result<_, _>>()?;
        let mut woken = conn.prepare_cached(
            "SELECT heard_in FROM tasks WHERE chat_id = ?1 AND kind = 'person' AND heard_in IS NOT NULL",
        )?;
        let woken = woken
            .query_map([chat], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(Rounds {
            answers,
            woken,
            known: RefCell::default(),
        })
    }

    /// The round `turn` is part of.
    pub(super) fn round(&self, turn: &str) -> String {
        let mut known = self.known.borrow_mut();
        let (mut at, mut passed) = (turn, Vec::new());
        let round = loop {
            if let Some(round) = known.get(at) {
                break round.clone();
            }
            let above = self.answers.get(at).and_then(|a| a.as_deref());
            let above = above.filter(|a| !self.woken.contains(at) && self.answers.contains_key(*a));
            match above {
                // No more steps than there are turns: a loop of links ends.
                Some(above) if passed.len() < self.answers.len() => {
                    passed.push(at);
                    at = above;
                }
                _ => break at.to_string(),
            }
        };
        for turn in passed {
            known.insert(turn.to_string(), round.clone());
        }
        round
    }
}

/// Each task's round, for a chat's list of them.
pub(super) fn rounds(conn: &Connection, chat: &str, tasks: &mut [Task]) -> Result<(), Error> {
    let rounds = Rounds::of(conn, chat)?;
    for t in tasks {
        t.round = t.given_in.as_deref().map(|turn| rounds.round(turn));
    }
    Ok(())
}

/// A task that ends done is a line in the chat when it ends, said by Gathr in no turn: the person
/// reads it, and nobody is woken for it. Not for the person's own tasks and requests, whose answer
/// already shows.
pub(super) fn done_line(conn: &Connection, task: i64) -> Result<(), Error> {
    let t = read(conn, task)?;
    if let Some(text) = done_words(conn, &t)? {
        crate::store::say(conn, &t.chat, &text)?;
    }
    Ok(())
}

/// The words of that line: none for a task that gets no line.
pub fn done_words(conn: &Connection, t: &Task) -> Result<Option<String>, Error> {
    if t.owner.is_none() || !matches!(t.kind, Kind::Work | Kind::Review) {
        return Ok(None);
    }
    Ok(Some(format!("{}\n{}", words::DONE, words::line(conn, t)?)))
}

/// Done and held only until its merge, and its giver reviews that PR: the merge waits on that
/// review, so the giver hears now (found in #100's own review: the fix its reviewer gave waited on
/// the merge while the review waited to hear of the fix). For any other giver the work stands
/// when it has landed, which Gathr sees to (0049).
fn waits_on_its_giver(conn: &Connection, task: i64) -> Result<bool, Error> {
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM tasks t WHERE t.number = ?1 AND {ONLY_ITS_MERGE}
         AND EXISTS (SELECT 1 FROM tasks r WHERE r.chat_id = t.chat_id AND r.pr = t.pr
             AND r.kind = 'review' AND r.owner = t.giver AND r.state != 'cancelled'))"
    );
    Ok(conn.prepare_cached(&sql)?.query_row([task], |r| r.get(0))?)
}

/// One of a group: the task, whether its giver has heard of it, and the turn that gave it.
type Given = (i64, bool, String);

/// The tasks `giver` gave from where `turn` was. From a task of its own (`from`): everything given
/// in that task's turns. From no task: everything given in the turn's round, of which only what is
/// still open or unheard is read, as the rest is over.
fn group(
    conn: &Connection,
    (chat, giver): (&str, &str),
    turn: &str,
    from: Option<i64>,
) -> Result<Vec<Given>, Error> {
    let row = |r: &rusqlite::Row| Ok((r.get(0)?, r.get(1)?, r.get(2)?));
    // The person's answers come back as they're sent (person::heard), not with these.
    let Some(task) = from else {
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT c.number, c.heard, c.given_in FROM tasks c JOIN turns u ON u.id = c.given_in
             WHERE c.chat_id = ?1 AND c.giver = ?2 AND c.owner IS NOT NULL AND u.task IS NULL
             AND (c.heard = 0 OR c.state NOT IN {SETTLED}) ORDER BY c.number"
        ))?;
        let all: Vec<Given> = stmt
            .query_map([chat, giver], row)?
            .collect::<Result<_, _>>()?;
        let rounds = Rounds::of(conn, chat)?;
        let round = rounds.round(turn);
        return Ok(all
            .into_iter()
            .filter(|given| rounds.round(&given.2) == round)
            .collect());
    };
    let mut stmt = conn.prepare_cached(
        "SELECT c.number, c.heard, c.given_in FROM tasks c JOIN turns u ON u.id = c.given_in
         WHERE u.task = ?1 AND c.giver = ?2 AND c.owner IS NOT NULL ORDER BY c.number",
    )?;
    let rows = stmt.query_map(params![task, giver], row)?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The hear-back of the agent whose turn `turn` is, on what it gave from there: from that turn's
/// task, or with no task from its round. It goes once every turn that gave some of it has ended
/// (the giver is done handing out work), and says only what its giver hasn't heard (G188):
/// - a task that is blocked, failed or stopped, at once, whatever the others do;
/// - a task given in passing, as it settles (G164);
/// - the rest, done or cancelled, once everything in the group is settled.
///
/// It is a turn that answers the newest turn its tasks were given in, on the giver's own task
/// while that is open. With nothing for the giver to do (all of them done, and its own task held
/// with its owner's word that it is done) no turn starts: they count as heard, and that task is
/// looked at again as when any hold ends.
///
/// A task restarted after its hear-back is heard again when it settles, whatever the outcome (its
/// giver asked for that run), and one that settles as something else than was heard is heard once
/// more (0047).
pub fn hear_backs(tx: &Transaction, turn: &str, trace: &str) -> Result<Vec<Start>, Error> {
    let found: Option<(String, Option<String>, Option<i64>)> = tx
        .query_row(
            "SELECT chat_id, agent_id, task FROM turns WHERE id = ?1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((chat, Some(giver), from)) = found else {
        return Ok(Vec::new());
    };
    // Archiving cancelled every task in its chat: nobody is to hear back on them there.
    if crate::archive::archived(tx, &chat)?.is_some() {
        return Ok(Vec::new());
    }
    let group = group(tx, (&chat, &giver), turn, from)?;
    let mut gave: Vec<&str> = group.iter().map(|g| g.2.as_str()).collect();
    gave.sort_unstable();
    gave.dedup();
    for giving in gave {
        let status: String =
            tx.query_row("SELECT status FROM turns WHERE id = ?1", [giving], |r| {
                r.get(0)
            })?;
        if !matches!(status.as_str(), "done" | "failed" | "stopped") {
            return Ok(Vec::new());
        }
    }
    let (mut due, mut rest, mut pending) = (Vec::new(), Vec::new(), false);
    for (number, heard, _) in &group {
        let t = read(tx, *number)?;
        let stands = t.state.settled() || waits_on_its_giver(tx, *number)?;
        let trouble = matches!(t.state, State::Blocked | State::Failed | State::Stopped);
        match (stands, *heard) {
            (false, _) => pending |= !t.in_passing,
            (true, true) => {}
            (true, false) if trouble || t.in_passing => due.push(t),
            (true, false) => rest.push(t),
        }
    }
    if !pending {
        due.extend(rest);
    }
    let Some(newest) = due.iter().map(|t| t.number).max() else {
        return Ok(Vec::new());
    };
    due.sort_by_key(|t| t.number);
    for t in &due {
        tx.execute(
            "UPDATE tasks SET heard = 1, heard_as = ?2 WHERE number = ?1",
            params![t.number, heard_as(t.state, t.reason.as_deref())],
        )?;
    }
    let own = match from {
        Some(task) => read(tx, task).optional_task()?,
        None => None,
    };
    let own = own.filter(|t| !matches!(t.state, State::Done | State::Cancelled));
    let reported_done = |t: &Task| t.report.as_ref().is_some_and(|r| r.how == Finish::Done);
    let nothing_to_do = due.iter().all(|t| t.state == State::Done)
        && own
            .as_ref()
            .is_some_and(|t| t.state == State::Held && reported_done(t));
    let answers = group.iter().find(|g| g.0 == newest).map(|g| g.2.as_str());
    Event::point("gathr.task.hear_back")
        .trace(trace)
        .chat(&chat)
        .ok()
        .attr("gathr.turn.id", answers)
        .attr("gen_ai.agent.id", giver.as_str())
        .attr("gathr.task.count", due.len())
        .attr("gathr.hear_back.turn", !nothing_to_do)
        .insert(tx)?;
    let own = own.map(|t| t.number);
    if let (true, Some(own)) = (nothing_to_do, own) {
        return released(tx, own, trace);
    }
    let text = words::hear_back(tx, &due, own)?;
    let mut start = Start::queue(tx, (&chat, &giver), (own, answers), text)?;
    start.hear_back = true;
    Ok(vec![start])
}
