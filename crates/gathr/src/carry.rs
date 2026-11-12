//! Carry-ons for an update (0032): a queued turn for every agent the update is about to stop, saved
//! all at once before any engine is told to stop. So each is there whatever an engine does next,
//! and when one can't be saved, none is and nothing is stopped.

use rusqlite::Connection;

use crate::Error;
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

/// Saves a carry-on for every turn that runs, and for every agent with no turn running whose
/// engine runs background commands: each on the same task, answering the turn it carries on from.
/// One transaction, so all of them or none. How many were saved.
pub fn save_all(conn: &mut Connection, by: &str) -> Result<usize, Error> {
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
    let mut saved = 0;
    let mut save = |place: &Place, turn: &str, task: Option<i64>, ended: String| {
        let agent = place.1.as_deref();
        saved += 1;
        Start::save(
            &tx,
            (&place.0, agent),
            (task, Some(turn)),
            &said(by, &ended),
        )
    };
    for (place, turn, task) in &running {
        let count = commands.iter().find(|c| c.0 == *place).map_or(0, |c| c.1);
        save(place, turn, *task, ended(true, count))?;
    }
    for (place, count, turn, task) in &commands {
        if !running.iter().any(|r| r.0 == *place) {
            save(place, turn, *task, ended(false, *count))?;
        }
    }
    tx.commit()?;
    Ok(saved)
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
