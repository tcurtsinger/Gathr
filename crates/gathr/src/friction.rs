//! Friction: what got in an agent's way, as it reported it (charter promise 11, part 11.4;
//! docs/decisions/0048-gathr-records-friction.md). Gathr keeps each report with who made it, when,
//! in which chat and on which task, so nobody copies it out of an answer by hand.

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

/// The most a report holds: what happened, in a sentence or two.
pub const MOST: usize = 500;
/// The most its place holds: a tool, a screen, a command or a file.
const PLACE_MOST: usize = 200;

/// What an agent reports, and where it stood when it did.
pub struct Report<'a> {
    pub chat: &'a str,
    pub agent: Option<&'a str>,
    pub turn: Option<&'a str>,
    pub task: Option<i64>,
    pub what: &'a str,
    pub place: Option<&'a str>,
}

/// A kept report, as the window and `gathr diag friction` show it.
#[derive(Debug, Serialize)]
pub struct Friction {
    pub id: i64,
    pub time: String,
    pub chat: String,
    /// None once the chat is deleted: the report outlives it.
    pub chat_title: Option<String>,
    pub agent: Option<String>,
    pub agent_name: Option<String>,
    pub task: Option<i64>,
    pub what: String,
    #[serde(rename = "where")]
    pub place: Option<String>,
}

fn fits(field: &str, text: &str, most: usize) -> Result<(), Error> {
    let long = text.chars().count();
    match long > most {
        true => Err(Error::Refused(format!(
            "`{field}` is {long} characters: cut {} to fit {most}",
            long - most
        ))),
        false => Ok(()),
    }
}

/// Keeps a report, with its event: its number.
pub fn report(conn: &mut Connection, r: &Report, trace: &str) -> Result<i64, Error> {
    let what = r.what.trim();
    if what.is_empty() {
        return Err(Error::Refused("Say what got in your way, in `what`".into()));
    }
    fits("what", what, MOST)?;
    let place = r.place.map(str::trim).filter(|p| !p.is_empty());
    if let Some(place) = place {
        fits("where", place, PLACE_MOST)?;
    }
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO friction (ts, chat_id, agent_id, turn_id, task, what, place)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![now_us(), r.chat, r.agent, r.turn, r.task, what, place],
    )?;
    let id = tx.last_insert_rowid();
    let event = Event::point("gathr.friction.report")
        .trace(trace)
        .chat(r.chat);
    let event = match r.task {
        Some(task) => event.task(task),
        None => event,
    };
    event
        .ok()
        .attr("gathr.friction.id", id)
        .attr("gen_ai.agent.id", r.agent)
        .insert(&tx)?;
    tx.commit()?;
    Ok(id)
}

/// The reports made since `since_us`, newest first, at most `limit`.
pub fn list(conn: &Connection, since_us: i64, limit: i64) -> Result<Vec<Friction>, Error> {
    let mut stmt = conn.prepare(
        "SELECT f.id, f.ts, f.chat_id, c.title, f.agent_id, a.name, f.task, f.what, f.place
         FROM friction f LEFT JOIN chats c ON c.id = f.chat_id LEFT JOIN agents a ON a.id = f.agent_id
         WHERE f.ts >= ?1 ORDER BY f.id DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![since_us, limit], |r| {
        Ok(Friction {
            id: r.get(0)?,
            time: crate::ids::utc(r.get(1)?),
            chat: r.get(2)?,
            chat_title: r.get(3)?,
            agent: r.get(4)?,
            agent_name: r.get(5)?,
            task: r.get(6)?,
            what: r.get(7)?,
            place: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}
