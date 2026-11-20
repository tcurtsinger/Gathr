//! The open check (0080), ported from SerenityOS's room checks: once per message of the person's,
//! when the chat's lead has answered it and no turn of the chat is still to end, every free member
//! who took no part reads the answer in a turn of its own. A thumbs-up on the answer or silence
//! wakes nobody; words wake the lead once, for its last word, with each member's words.

mod words;

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::store::Start;
use crate::team::Role;
use crate::words::Opened;

/// A turn that is still to end.
const BUSY: &str = "('queued', 'waiting_sign_in', 'running')";

/// What a lead's turn opened by these never says to the person, so it answers nothing (0078).
const NOT_AN_ANSWER: &str = "('engine', 'compaction', 'check', 'last_word')";

/// A round under way, as its row keeps it.
struct Round {
    id: i64,
    asked: String,
    answer: String,
    answer_turn: String,
    last_word: Option<String>,
}

/// The person's newest message, the lead's answer to it and who is to check it.
struct Due {
    asked: String,
    answer: String,
    answer_turn: String,
    lead: String,
    checkers: Vec<String>,
}

/// Moves the chat's rounds on and begins the one its newest message of the person's is owed, in one
/// transaction: the turns to start. The service calls it after every write that may have moved the
/// chat, so it does nothing more than once.
pub fn settle(conn: &mut Connection, chat: &str, trace: &str) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let live: bool = tx
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM chats WHERE id = ?1 AND archived_ts IS NULL)",
        )?
        .query_row([chat], |r| r.get(0))?;
    if !live || crate::team::members(&tx, chat)?.len() < 2 {
        return Ok(Vec::new());
    }
    let mut starts = Vec::new();
    for round in under_way(&tx, chat)? {
        starts.extend(advance(&tx, chat, &round, trace)?);
    }
    if let Some(due) = due(&tx, chat)? {
        starts.extend(begin(&tx, chat, &due, trace)?);
    }
    tx.commit()?;
    Ok(starts)
}

/// Whether `turn` is a check whose answer is `message`: its thumbs-up there is its whole check.
pub fn checks(conn: &Connection, turn: &str, message: &str) -> Result<bool, Error> {
    let sql = "SELECT EXISTS (SELECT 1 FROM turns u JOIN open_checks c ON c.answer_turn = u.answers
               WHERE u.id = ?1 AND u.opened_by = 'check' AND c.answer = ?2)";
    Ok(conn
        .prepare_cached(sql)?
        .query_row([turn, message], |r| r.get(0))?)
}

/// The answer a check turn checks, which `react` takes as `answer` (0082).
pub fn answer_of(conn: &Connection, turn: &str) -> Result<String, Error> {
    let sql = "SELECT c.answer FROM turns u JOIN open_checks c ON c.answer_turn = u.answers
               WHERE u.id = ?1 AND u.opened_by = 'check' AND c.answer IS NOT NULL ORDER BY c.id DESC LIMIT 1";
    conn.prepare_cached(sql)?
        .query_row([turn], |r| r.get(0))
        .optional()?
        .ok_or_else(|| {
            Error::Refused(
                "only a check has an answer to agree with: give the message's id.".into(),
            )
        })
}

/// Gathr's calls that only report or place: words followed by nothing else are still the message.
const NOT_A_STEP: [&str; 1] = ["mcp__gathr__react"];

/// A turn's message, as the window draws it (0082, `ui/src/chat/fold.ts`): the words it wrote after
/// its last step, else its last words. Words before a later step are narration, which no agent hears.
fn message_of(tx: &Transaction, turn: &str) -> Result<Option<String>, Error> {
    let mut stmt = tx.prepare_cached(
        "SELECT kind, body FROM messages WHERE turn_id = ?1 AND role = 'assistant' AND to_agent IS NULL
         ORDER BY ts, rowid",
    )?;
    let rows = stmt.query_map([turn], |r| {
        Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
    })?;
    let mut said: Vec<String> = Vec::new();
    let mut last = None;
    for row in rows {
        let (kind, body) = row?;
        let fields = || serde_json::from_str::<serde_json::Value>(&body).unwrap_or_default();
        let step = match kind.as_str() {
            "text" if !body.trim().is_empty() => {
                said.push(body.trim().to_string());
                false
            }
            "tool" => !NOT_A_STEP.contains(&fields()["tool"].as_str().unwrap_or_default()),
            // A thought under a second isn't drawn, so it is no step either.
            "thinking" => {
                let f = fields();
                f["status"] == "running" || f["duration_ms"].as_i64().unwrap_or(0) >= 1000
            }
            _ => false,
        };
        if step {
            last = said.pop().or(last);
            said.clear();
        }
    }
    Ok(match said.is_empty() {
        true => last,
        false => Some(said.join("\n\n")),
    })
}

fn under_way(tx: &Transaction, chat: &str) -> Result<Vec<Round>, Error> {
    let mut stmt = tx.prepare_cached(
        "SELECT id, asked, answer, answer_turn, last_word FROM open_checks
         WHERE chat_id = ?1 AND ended_ts IS NULL ORDER BY id",
    )?;
    let rows = stmt.query_map([chat], |r| {
        Ok(Round {
            id: r.get(0)?,
            asked: r.get(1)?,
            answer: r.get(2)?,
            answer_turn: r.get(3)?,
            last_word: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The chat's newest message of the person's: its id and its turn.
fn newest_asked(tx: &Transaction, chat: &str) -> Result<Option<(String, String)>, Error> {
    let sql = "SELECT m.id, m.turn_id FROM messages m
               WHERE m.chat_id = ?1 AND m.role = 'user' AND m.turn_id IS NOT NULL
               ORDER BY m.ts DESC, m.rowid DESC LIMIT 1";
    let row = tx
        .prepare_cached(sql)?
        .query_row([chat], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    Ok(row)
}

/// The round owed the person's newest message: none while any turn of the chat is still to end,
/// while the lead's newest answer to it hasn't ended well, or once it had its round.
fn due(tx: &Transaction, chat: &str) -> Result<Option<Due>, Error> {
    let Some((asked, asked_turn)) = newest_asked(tx, chat)? else {
        return Ok(None);
    };
    let had: bool = tx
        .prepare_cached(
            "SELECT EXISTS (SELECT 1 FROM open_checks WHERE chat_id = ?1 AND asked = ?2)",
        )?
        .query_row([chat, &asked], |r| r.get(0))?;
    let open =
        format!("SELECT EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1 AND status IN {BUSY})");
    let open: bool = tx.prepare_cached(&open)?.query_row([chat], |r| r.get(0))?;
    let members = crate::team::members(tx, chat)?;
    let Some(lead) = members.iter().find(|m| m.role == Role::Supervisor) else {
        return Ok(None);
    };
    if had || open {
        return Ok(None);
    }
    let since: i64 = tx.query_row(
        "SELECT created_ts FROM turns WHERE id = ?1",
        [&asked_turn],
        |r| r.get(0),
    )?;
    let sql = format!(
        "SELECT id, status FROM turns WHERE chat_id = ?1 AND agent_id = ?2
           AND (opened_by IS NULL OR opened_by NOT IN {NOT_AN_ANSWER}) AND created_ts >= ?3
         ORDER BY created_ts DESC, rowid DESC LIMIT 1"
    );
    let newest: Option<(String, String)> = tx
        .prepare_cached(&sql)?
        .query_row(params![chat, lead.agent, since], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .optional()?;
    let Some((answer_turn, "done")) = newest.as_ref().map(|(t, s)| (t.clone(), s.as_str())) else {
        return Ok(None);
    };
    let Some(answer) = answer_in(tx, &answer_turn)? else {
        return Ok(None);
    };
    let sql = format!(
        "SELECT EXISTS (SELECT 1 FROM turns WHERE chat_id = ?1 AND agent_id = ?2
                        AND (status IN {BUSY} OR created_ts >= ?3))"
    );
    let mut part = tx.prepare_cached(&sql)?;
    let mut checkers = Vec::new();
    for m in members
        .iter()
        .filter(|m| m.agent != lead.agent && !nobody_checks())
    {
        let took_part: bool = part.query_row(params![chat, m.agent, since], |r| r.get(0))?;
        if !took_part {
            checkers.push(m.agent.clone());
        }
    }
    Ok(Some(Due {
        asked,
        answer,
        answer_turn,
        lead: lead.agent.clone(),
        checkers,
    }))
}

/// Service tests of other parts run with nobody to check (`GATHR_OPEN_CHECK=off`, read only in a
/// debug build): rounds still end, with no turns of their own.
fn nobody_checks() -> bool {
    cfg!(debug_assertions) && std::env::var_os("GATHR_OPEN_CHECK").is_some_and(|v| v == "off")
}

/// The message of `turn` that is its answer: its last words.
fn answer_in(tx: &Transaction, turn: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT id FROM messages WHERE turn_id = ?1 AND role = 'assistant' AND kind = 'text'
                 AND trim(body) != '' ORDER BY ts DESC, rowid DESC LIMIT 1";
    Ok(tx
        .prepare_cached(sql)?
        .query_row([turn], |r| r.get(0))
        .optional()?)
}

/// Records the round and queues each checker's turn, answering the answer's turn so it sits in its
/// thread; with nobody to check it, the round ends there.
fn begin(tx: &Transaction, chat: &str, due: &Due, trace: &str) -> Result<Vec<Start>, Error> {
    tx.execute(
        "INSERT INTO open_checks (chat_id, asked, answer, answer_turn, began_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![chat, due.asked, due.answer, due.answer_turn, now_us()],
    )?;
    let round = Round {
        id: tx.last_insert_rowid(),
        asked: due.asked.clone(),
        answer: due.answer.clone(),
        answer_turn: due.answer_turn.clone(),
        last_word: None,
    };
    if due.checkers.is_empty() {
        end(tx, chat, &round, &Counts::default(), trace)?;
        return Ok(Vec::new());
    }
    let text = words::check(tx, (&due.asked, &due.answer), &due.lead)?;
    let on = Some(due.answer_turn.as_str());
    due.checkers
        .iter()
        .map(|agent| Start::queue(tx, (chat, agent), on, Opened::Check, text.clone()))
        .collect()
}

/// How a round's checks ended.
#[derive(Default)]
struct Counts {
    asked: usize,
    agreed: usize,
    silent: usize,
    /// Each member who wrote words and gave no thumbs-up, with its message.
    objected: Vec<(String, String)>,
}

/// Once every check of the round has ended: the lead's last word when a member wrote words, unless
/// the person has written again or decided the plan since; once that has ended too, the round ends.
fn advance(tx: &Transaction, chat: &str, round: &Round, trace: &str) -> Result<Vec<Start>, Error> {
    let sql = format!(
        "SELECT u.agent_id, u.status IN {BUSY},
                EXISTS (SELECT 1 FROM reactions r WHERE r.message_id = ?3 AND r.agent_id = u.agent_id),
                u.id
         FROM turns u WHERE u.chat_id = ?1 AND u.opened_by = 'check' AND u.answers = ?2 ORDER BY u.rowid"
    );
    type Row = (String, bool, bool, String);
    let checks: Vec<Row> = {
        let mut stmt = tx.prepare_cached(&sql)?;
        let rows = stmt.query_map(params![chat, round.answer_turn, round.answer], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    if checks.iter().any(|c| c.1) {
        return Ok(Vec::new());
    }
    let mut counts = Counts {
        asked: checks.len(),
        ..Counts::default()
    };
    for (agent, _, agreed, turn) in checks {
        match (agreed, message_of(tx, &turn)?) {
            (true, _) => counts.agreed += 1,
            (false, Some(said)) => counts.objected.push((agent, said)),
            (false, None) => counts.silent += 1,
        }
    }
    let Some(last) = &round.last_word else {
        if !counts.objected.is_empty() && !moot(tx, chat, round)? {
            let lead: String = tx.query_row(
                "SELECT agent_id FROM turns WHERE id = ?1",
                [&round.answer_turn],
                |r| r.get(0),
            )?;
            let text = words::last_word(tx, &counts.objected)?;
            let on = Some(round.answer_turn.as_str());
            let start = Start::queue(tx, (chat, &lead), on, Opened::LastWord, text)?;
            tx.execute(
                "UPDATE open_checks SET last_word = ?2 WHERE id = ?1",
                params![round.id, start.turn],
            )?;
            return Ok(vec![start]);
        }
        end(tx, chat, round, &counts, trace)?;
        return Ok(Vec::new());
    };
    let sql = format!("SELECT status IN {BUSY} FROM turns WHERE id = ?1");
    let running: bool = tx.prepare_cached(&sql)?.query_row([last], |r| r.get(0))?;
    if !running {
        end(tx, chat, round, &counts, trace)?;
    }
    Ok(Vec::new())
}

/// The person has written again: a last word would answer nothing they still wait on.
fn moot(tx: &Transaction, chat: &str, round: &Round) -> Result<bool, Error> {
    let newest = newest_asked(tx, chat)?.map(|(id, _)| id);
    Ok(newest.as_deref() != Some(round.asked.as_str()))
}

/// Ends the round, with one event that says how it went.
fn end(
    tx: &Transaction,
    chat: &str,
    round: &Round,
    counts: &Counts,
    trace: &str,
) -> Result<(), Error> {
    tx.execute(
        "UPDATE open_checks SET ended_ts = ?2 WHERE id = ?1",
        params![round.id, now_us()],
    )?;
    Event::point("gathr.check.round")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("gathr.message.id", round.answer.as_str())
        .attr("gathr.turn.id", round.answer_turn.as_str())
        .attr("gathr.check.asked", counts.asked)
        .attr("gathr.check.agreed", counts.agreed)
        .attr("gathr.check.objected", counts.objected.len())
        .attr("gathr.check.silent", counts.silent)
        .attr("gathr.check.last_word", round.last_word.is_some())
        .insert(tx)?;
    Ok(())
}
