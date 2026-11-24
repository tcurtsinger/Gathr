//! The hand-back, after SerenityOS's quiet-room nudge: when a chat goes quiet while an agent's line
//! on the board still says doing, that agent gets its turn back once, with Gathr's words. Once per
//! agent until the person writes in the chat again. Any agent of the chat: a member, the lead, or
//! the only agent of a chat of one.

use rusqlite::Connection;

use crate::Error;
use crate::event::Event;
use crate::store::Start;
use crate::words::Opened;

/// The `doing` lines of the chat's agents whose agent can be handed its turn back: one whose engine
/// runs no background command and that wasn't handed one back since the person last wrote in the chat.
const DOING: &str = "SELECT b.owner, b.text FROM board_lines b
    JOIN chat_members m ON m.chat_id = b.chat_id AND m.agent_id = b.owner
    WHERE b.chat_id = ?1 AND b.state = 'doing'
      AND NOT EXISTS (SELECT 1 FROM background g JOIN turns t ON t.id = g.turn_id WHERE t.agent_id = b.owner)
      AND NOT EXISTS (SELECT 1 FROM turns n WHERE n.chat_id = b.chat_id AND n.agent_id = b.owner
          AND n.opened_by = 'nudge'
          AND n.created_ts > coalesce((SELECT max(ts) FROM messages WHERE chat_id = b.chat_id AND role = 'user'), 0))
    ORDER BY b.owner, b.id";

/// Whether the chat is quiet and nothing in it waits on the person, as the database knows it: not
/// archived, no turn running, queued or waiting for a sign-in, no question open.
const QUIET: &str = "SELECT c.archived_ts IS NULL
    AND NOT EXISTS (SELECT 1 FROM turns t WHERE t.chat_id = c.id AND t.status IN ('queued', 'waiting_sign_in', 'running'))
    AND NOT EXISTS (SELECT 1 FROM questions q WHERE q.chat_id = c.id AND q.answered_ts IS NULL AND q.closed_ts IS NULL)
    FROM chats c WHERE c.id = ?1";

/// Gathr's words to an agent whose `lines` still say doing.
fn said(lines: &[String]) -> String {
    let quoted: Vec<String> = lines.iter().map(|l| format!("\"{l}\"")).collect();
    let (line, it, set) = match lines.len() {
        1 => ("Your line on the board still says doing", "it", "the line"),
        _ => (
            "Your lines on the board still say doing",
            "them",
            "the lines",
        ),
    };
    format!(
        "{line}: {}. Nothing of yours is at work on {it}. Do {it} now. If part of {it} is left for someone \
         else or you're blocked, say so plainly and set {set} to what is true.",
        quoted.join(", ")
    )
}

/// At the moment a turn of `chat` ended: once the chat is quiet, unpaused and waits on nobody
/// (`asks`: an approval waits there, which is in no table), queues one turn for each agent whose
/// line still says doing, in one transaction. The turns to start.
pub fn hand_back(
    conn: &mut Connection,
    chat: &str,
    asks: bool,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let quiet: Option<bool> = {
        let mut stmt = tx.prepare_cached(QUIET)?;
        let mut rows = stmt.query([chat])?;
        rows.next()?.map(|r| r.get(0)).transpose()?
    };
    let paused = crate::pauses::holding(&crate::pauses::list(&tx)?, chat).is_some();
    if asks || paused || quiet != Some(true) {
        return Ok(Vec::new());
    }
    let doing: Vec<(String, String)> = {
        let mut stmt = tx.prepare_cached(DOING)?;
        let rows = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    let mut starts = Vec::new();
    for (agent, lines) in by_owner(doing) {
        let start = Start::queue(&tx, (chat, &agent), None, Opened::Nudge, said(&lines))?;
        Event::point("gathr.turn.nudge")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("gathr.turn.id", start.turn.as_str())
            .attr("gen_ai.agent.id", agent.as_str())
            .attr("lines", lines.len())
            .insert(&tx)?;
        starts.push(start);
    }
    tx.commit()?;
    Ok(starts)
}

/// The lines, ordered by owner, grouped under each owner.
fn by_owner(doing: Vec<(String, String)>) -> Vec<(String, Vec<String>)> {
    let mut owners: Vec<(String, Vec<String>)> = Vec::new();
    for (owner, text) in doing {
        match owners.last_mut() {
            Some((last, lines)) if *last == owner => lines.push(text),
            _ => owners.push((owner, vec![text])),
        }
    }
    owners
}
