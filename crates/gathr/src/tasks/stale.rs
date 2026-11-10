//! Stale tasks (F11, 0010). A task nothing has moved (no turn, message, wait or answer) for the
//! stale days is marked, and its chat's Supervisor is told once. Once it has been stale for the cancel
//! days, still without a move, it's cancelled, restartable, unless something can still move it: an open wait, PR or
//! turn, a background command, open work it handed on, a pause holding it. The person's own tasks
//! are never cancelled this way.

use std::collections::BTreeMap;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};

use super::records::read;
use super::{Start, cancel_in, words};
use crate::Error;
use crate::event::Event;
use crate::pauses::{self, Pause};

/// After how many days without a move a task is stale, and how many days stale it's cancelled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Days {
    pub stale: i64,
    pub cancel: i64,
}

const DEFAULT: Days = Days {
    stale: 3,
    cancel: 7,
};
const MOST: i64 = 365;

pub fn days(conn: &Connection) -> Result<Days, Error> {
    let saved = |key: &str| -> Result<Option<i64>, Error> {
        let value: Option<String> = conn
            .query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
                r.get(0)
            })
            .optional()?;
        Ok(value.and_then(|v| v.parse().ok()))
    };
    Ok(Days {
        stale: saved("stale_days")?.unwrap_or(DEFAULT.stale),
        cancel: saved("cancel_days")?.unwrap_or(DEFAULT.cancel),
    })
}

pub fn set_days(conn: &Connection, days: Days) -> Result<(), Error> {
    if !(1..=MOST).contains(&days.stale) || !(1..=MOST).contains(&days.cancel) {
        return Err(Error::Refused(format!(
            "Enter a number of days from 1 to {MOST}"
        )));
    }
    for (key, value) in [("stale_days", days.stale), ("cancel_days", days.cancel)] {
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
    }
    Ok(())
}

/// What a sweep did: tasks marked stale, tasks cancelled, and the turns that calls for (each chat's
/// Supervisor told, hear-backs on what was cancelled).
#[derive(Debug, Default)]
pub struct Swept {
    pub marked: Vec<i64>,
    pub cancelled: Vec<i64>,
    pub starts: Vec<Start>,
}

/// An open task in a chat that isn't archived, with its last move and whether anything can still
/// move it. A waiting task whose `after` tasks are still open is never stale: they are.
const OPEN: &str = "
    SELECT t.number, t.chat_id, t.owner, t.stale_ts,
        max(t.updated_ts,
            coalesce((SELECT max(coalesce(u.ended_ts, u.created_ts)) FROM turns u WHERE u.task = t.number), 0),
            coalesce((SELECT max(m.ts) FROM messages m JOIN turns u ON u.id = m.turn_id
                      WHERE u.task = t.number), 0),
            coalesce((SELECT max(coalesce(w.ended_ts, w.created_ts)) FROM waits w WHERE w.task = t.number), 0)),
        t.kind = 'person'
        OR (t.pr IS NOT NULL AND t.pr_closed = 0)
        OR EXISTS (SELECT 1 FROM waits w WHERE w.task = t.number AND w.state IN ('open', 'met'))
        OR EXISTS (SELECT 1 FROM turns u WHERE u.task = t.number
                   AND u.status IN ('queued', 'waiting_sign_in', 'running'))
        OR EXISTS (SELECT 1 FROM background b JOIN turns u ON u.id = b.turn_id WHERE u.task = t.number)
        OR EXISTS (SELECT 1 FROM tasks c WHERE (c.parent = t.number OR c.request = t.number)
                   AND c.state IN ('waiting', 'ready', 'working', 'held'))
    FROM tasks t JOIN chats ch ON ch.id = t.chat_id
    WHERE t.state NOT IN ('done', 'cancelled') AND ch.archived_ts IS NULL
      AND NOT (t.state = 'waiting' AND EXISTS (
          SELECT 1 FROM task_after a JOIN tasks f ON f.number = a.after_task
          WHERE a.task = t.number AND f.state IN ('waiting', 'ready', 'working', 'held')))
    ORDER BY t.number";

type Row = (i64, String, Option<String>, Option<i64>, i64, bool);

/// Marks, clears and cancels by the clock `now`, a day being `day_us` long.
pub fn sweep(
    conn: &mut Connection,
    now: i64,
    day_us: i64,
    paused: &[Pause],
    trace: &str,
) -> Result<Swept, Error> {
    let days = days(conn)?;
    let tx = conn.transaction()?;
    let rows: Vec<Row> = {
        let mut stmt = tx.prepare(OPEN)?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut swept = Swept::default();
    let mut told: BTreeMap<String, Vec<i64>> = BTreeMap::new();
    for (task, chat, owner, stale, moved, movable) in rows {
        if stale.is_some_and(|since| moved > since) {
            tx.execute("UPDATE tasks SET stale_ts = NULL WHERE number = ?1", [task])?;
            continue;
        }
        let quiet = now - moved;
        if quiet < days.stale * day_us {
            continue;
        }
        if stale.is_none() {
            tx.execute(
                "UPDATE tasks SET stale_ts = ?2 WHERE number = ?1",
                [task, moved],
            )?;
            let event = Event::point("gathr.task.stale").trace(trace).chat(&chat);
            let event = event.task(task).ok().attr("gathr.task.moved", moved);
            event.insert(&tx)?;
            swept.marked.push(task);
            told.entry(chat.clone()).or_default().push(task);
        }
        let held = pauses::holding(paused, &chat, owner.as_deref()).is_some();
        if quiet >= (days.stale + days.cancel) * day_us && !movable && !held {
            let why = format!("stale since {}", &crate::ids::utc(moved)[..10]);
            swept.starts.extend(cancel_in(&tx, task, &why, trace)?);
            swept.cancelled.push(task);
        }
    }
    for (chat, marked) in told {
        if let Some(start) = tell_lead(&tx, &chat, &marked, days)? {
            swept.starts.push(start);
        }
    }
    tx.commit()?;
    Ok(swept)
}

/// One turn for the chat's Supervisor (members list it first) naming what went stale.
fn tell_lead(
    tx: &Transaction,
    chat: &str,
    marked: &[i64],
    days: Days,
) -> Result<Option<Start>, Error> {
    let Some(lead) = crate::team::members(tx, chat)?.into_iter().next() else {
        return Ok(None);
    };
    let mut lines = Vec::new();
    let mut has_agent = false;
    for &n in marked {
        let t = read(tx, n)?;
        let whose = match &t.owner {
            Some(owner) => {
                has_agent = true;
                format!(
                    "assigned to {}, {}",
                    words::name_of(tx, owner)?,
                    t.state.as_str()
                )
            }
            None => "waiting for your answer".to_string(),
        };
        lines.push(format!("#{n} {} ({whose})", t.what));
    }
    let action = if marked.len() == 1 {
        "Finish, restart or cancel it"
    } else {
        "Finish, restart or cancel them"
    };
    let day = if days.stale == 1 { "day" } else { "days" };
    let mut text = format!(
        "No progress in {} {day}: {}. {action}",
        days.stale,
        lines.join("\n")
    );
    if has_agent {
        let day = if days.cancel == 1 { "day" } else { "days" };
        text.push_str(&format!(
            "; agent tasks auto-cancel after {} more {day} without progress unless work is ongoing or paused.",
            days.cancel
        ));
    } else {
        text.push('.');
    }
    Start::queue(tx, (chat, &lead.agent), (None, None), text).map(Some)
}

/// An agent's stale task, for the person's list: the person's own are among their open tasks.
#[derive(Debug, Serialize)]
pub struct Stale {
    #[serde(flatten)]
    pub task: super::Task,
    pub chat_title: String,
}

/// Agents' stale tasks across chats, the longest unmoved first.
pub fn open(conn: &Connection) -> Result<Vec<Stale>, Error> {
    let found: Vec<(i64, String)> = {
        let mut stmt = conn.prepare_cached(
            "SELECT t.number, c.title FROM tasks t JOIN chats c ON c.id = t.chat_id
             WHERE t.stale_ts IS NOT NULL AND t.owner IS NOT NULL
               AND t.state NOT IN ('done', 'cancelled') AND c.archived_ts IS NULL
             ORDER BY t.stale_ts, t.number",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    found
        .into_iter()
        .map(|(n, chat_title)| {
            Ok(Stale {
                task: read(conn, n)?,
                chat_title,
            })
        })
        .collect()
}
