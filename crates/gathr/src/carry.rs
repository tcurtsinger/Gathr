//! Carry-ons for an update (0032): a queued turn for every agent the update is about to stop, saved
//! all at once before any engine is told to stop. So each is there whatever an engine does next,
//! and when one can't be saved, none is and nothing is stopped.

use rusqlite::{Connection, params};

use crate::Error;
use crate::ids::now_us;
use crate::tasks::Start;

/// What every carry-on ends with.
pub const CHECK_FIRST: &str = "Check what is already done before repeating work, then continue.";

/// What a stop ended for one agent, as its carry-on says it.
pub fn ended(turn: bool, commands: usize) -> String {
    match (turn, commands) {
        (true, 0) => "your turn".to_string(),
        (true, n) => format!("your turn and {n} background command(s)"),
        (false, n) => format!("{n} background command(s) of yours"),
    }
}

/// The carry-on's words: who ended what, and to check first.
pub fn said(by: &str, ended: &str) -> String {
    format!("{by} ended {ended}. {CHECK_FIRST}")
}

/// An agent's place: its chat, and the agent (none in a chat sent to no agent).
type Place = (String, Option<String>);

/// A carry-on saved for a turn that ran then: its own turn, and the turn it answers.
pub struct Saved {
    carry_on: String,
    answers: String,
}

/// Saves a carry-on for every turn that runs, and for every agent with no turn running whose
/// engine runs background commands: each on the same task, answering the turn it carries on from.
/// One transaction, so all of them or none. How many were saved, and those for running turns.
pub fn save_all(conn: &mut Connection, by: &str) -> Result<(usize, Vec<Saved>), Error> {
    let tx = conn.transaction()?;
    // Each place's background commands: how many, and the turn and task of the newest.
    let commands: Vec<(Place, usize, String, Option<i64>)> = {
        let mut stmt = tx.prepare(
            "SELECT s.chat_id, s.agent_id, count(*), b.turn_id, s.task, max(b.started_ts)
             FROM background b JOIN turns s ON s.id = b.turn_id GROUP BY s.chat_id, s.agent_id",
        )?;
        let rows = stmt.query_map([], |r| {
            let count: i64 = r.get(2)?;
            Ok(((r.get(0)?, r.get(1)?), count as usize, r.get(3)?, r.get(4)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let running: Vec<(Place, String, Option<i64>)> = {
        let mut stmt =
            tx.prepare("SELECT chat_id, agent_id, id, task FROM turns WHERE status = 'running'")?;
        let rows = stmt.query_map([], |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?, r.get(3)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let save = |place: &Place, turn: &str, task: Option<i64>, ended: String| {
        let on = (task, Some(turn));
        Start::save(&tx, (&place.0, place.1.as_deref()), on, &said(by, &ended))
    };
    let mut for_turns = Vec::new();
    for (place, turn, task) in &running {
        let count = commands.iter().find(|c| c.0 == *place).map_or(0, |c| c.1);
        let (carry_on, ..) = save(place, turn, *task, ended(true, count))?;
        for_turns.push(Saved {
            carry_on,
            answers: turn.clone(),
        });
    }
    let mut saved = for_turns.len();
    for (place, count, turn, task) in &commands {
        if !running.iter().any(|r| r.0 == *place) {
            save(place, turn, *task, ended(false, *count))?;
            saved += 1;
        }
    }
    tx.commit()?;
    Ok((saved, for_turns))
}

/// Before the hand-over: a carry-on stands only for a turn the update stopped, or one still running
/// that the hand-over cuts off. A turn that ended by itself after its carry-on was saved (done, or
/// failed) settled its task as any turn does, so its carry-on is ended unstarted: it would only
/// repeat the work, or work on a task that failed. How many were ended.
pub fn drop_unneeded(conn: &mut Connection, saved: &[Saved]) -> Result<usize, Error> {
    let tx = conn.transaction()?;
    let mut dropped = 0;
    for s in saved {
        dropped += tx.execute(
            "UPDATE turns SET status = 'stopped', ended_ts = ?3,
                 why = 'its turn ended by itself before Gathr''s update stopped it'
             WHERE id = ?1 AND status = 'queued' AND EXISTS
             (SELECT 1 FROM turns was WHERE was.id = ?2 AND was.status NOT IN ('running', 'stopped'))",
            params![s.carry_on, s.answers, now_us()],
        )?;
    }
    tx.commit()?;
    Ok(dropped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_carry_on_says_who_ended_what() {
        let by = "Gathr's update to 9.9.9";
        assert_eq!(
            said(by, &ended(true, 0)),
            format!("{by} ended your turn. {CHECK_FIRST}")
        );
        assert_eq!(ended(true, 2), "your turn and 2 background command(s)");
        assert_eq!(ended(false, 1), "1 background command(s) of yours");
    }
}
