//! Scheduled work: words an agent keeps with `schedule`, which come into its chat at their time as a message to it,
//! drawn as the person's, and start its turn as the person's message would. The service looks every 30 seconds and
//! once as it starts (`due`); each run is recorded and its turn queued in one transaction, so a restart neither loses
//! it nor runs it twice. Times are this PC's clock ([`when`]).

pub mod when;

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use crate::Error;
use crate::ids::{now_us, trace_id, ulid, utc};
use crate::store::{self, Start};
use crate::words::Opened;
pub use when::{Ask, When};
use when::{Due, clock, day_time, local};

/// The most a schedule's words may be.
pub const MOST_CHARS: usize = 2000;
/// Why a run was late or skipped.
const AWAY: &str = "Gathr wasn't running or this PC was asleep.";
/// The runs a schedule's page shows, newest first.
const RUNS_SHOWN: i64 = 20;

/// A schedule as saved.
struct Row {
    id: String,
    chat: String,
    agent: String,
    words: String,
    when: When,
    next: Option<i64>,
}

const ROW: &str =
    "SELECT id, chat_id, agent_id, words, minute, days, once_ts, next_ts FROM schedules";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Row> {
    let (minute, days, once): (Option<i64>, Option<String>, Option<i64>) =
        (r.get(4)?, r.get(5)?, r.get(6)?);
    Ok(Row {
        id: r.get(0)?,
        chat: r.get(1)?,
        agent: r.get(2)?,
        words: r.get(3)?,
        when: match (once, minute, days) {
            (Some(at), _, _) => When::Once(at),
            (None, minute, days) => When::Days {
                minute: minute.unwrap_or(0),
                days: days.unwrap_or_default(),
            },
        },
        next: r.get(7)?,
    })
}

fn one(conn: &Connection, id: &str) -> Result<Row, Error> {
    let found = conn
        .prepare_cached(&format!("{ROW} WHERE id = ?1"))?
        .query_row([id], row)
        .optional()?;
    found.ok_or_else(|| Error::Refused(format!("there's no schedule {id}")))
}

/// What `schedule` made: its id, when it runs in words and when it is next due.
pub struct Made {
    pub id: String,
    pub when: String,
    pub next: i64,
}

/// Keeps `words` for `agent` in `chat`, to run `when`, from `now` on: never a time before it.
pub fn add(
    conn: &Connection,
    (chat, agent): (&str, &str),
    words: &str,
    when: &When,
    now: i64,
) -> Result<Made, Error> {
    let words = words.trim();
    if words.is_empty() {
        return Err(Error::Refused("Give the words that will arrive.".into()));
    }
    if words.chars().count() > MOST_CHARS {
        return Err(Error::Refused(format!(
            "The words are {} characters: at most {MOST_CHARS}.",
            words.chars().count()
        )));
    }
    crate::archive::refuse_archived(conn, chat)?;
    let next = when
        .next_after(conn, now)?
        .ok_or_else(|| Error::Refused("That time has passed.".into()))?;
    let id = ulid(now_us());
    let (minute, days, once) = match when {
        When::Days { minute, days } => (Some(*minute), Some(days.as_str()), None),
        When::Once(at) => (None, None, Some(*at)),
    };
    conn.execute(
        "INSERT INTO schedules (id, chat_id, agent_id, words, minute, days, once_ts, next_ts, created_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![id, chat, agent, words, minute, days, once, next, now],
    )?;
    Ok(Made {
        id,
        when: when.words(conn)?,
        next,
    })
}

/// Every schedule whose time has come by `now`, each in a transaction of its own: one due today or less than two
/// hours ago runs, its run recorded and its turn queued together; one due before that records one skipped run. A once that ran leaves
/// the list with its runs; one that was skipped stays, disabled. The turns to queue, and whether any came due.
pub fn due(conn: &mut Connection, now: i64) -> Result<(Vec<Start>, bool), Error> {
    let ids: Vec<String> = conn
        .prepare_cached("SELECT id FROM schedules WHERE next_ts <= ?1 ORDER BY next_ts")?
        .query_map([now], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let mut starts = Vec::new();
    for id in &ids {
        let tx = conn.transaction()?;
        let s = one(&tx, id)?;
        let Some(next) = s.next.filter(|n| *n <= now) else {
            continue;
        };
        let once = matches!(s.when, When::Once(_));
        let ran = match s.when.due(&tx, next, now)? {
            Due::Run { due, late } => {
                let kind = if late { "late" } else { "on_time" };
                starts.push(queue_run(&tx, &s, (due, now), kind)?);
                true
            }
            Due::Skip { due } => {
                tx.execute(
                    "INSERT INTO schedule_runs (id, schedule_id, chat_id, kind, due_ts, ran_ts)
                     VALUES (?1, ?2, ?3, 'skipped', ?4, ?5)",
                    params![ulid(now_us()), s.id, s.chat, due, now],
                )?;
                false
            }
        };
        match (once, ran) {
            (true, true) => tx.execute("DELETE FROM schedules WHERE id = ?1", [&s.id])?,
            _ => tx.execute(
                "UPDATE schedules SET next_ts = ?2 WHERE id = ?1",
                params![s.id, s.when.next_after(&tx, now)?],
            )?,
        };
        tx.commit()?;
    }
    Ok((starts, !ids.is_empty()))
}

/// Run now: a run at once, which doesn't move the next time.
pub fn run_now(conn: &mut Connection, id: &str) -> Result<Start, Error> {
    let tx = conn.transaction()?;
    let s = one(&tx, id)?;
    crate::archive::refuse_archived(&tx, &s.chat)?;
    let now = now_us();
    let start = queue_run(&tx, &s, (now, now), "now")?;
    tx.commit()?;
    Ok(start)
}

/// The run's turn and its message, the schedule's words as the person's, and the run that names both.
fn queue_run(
    tx: &Transaction,
    s: &Row,
    (due, ran): (i64, i64),
    kind: &str,
) -> Result<Start, Error> {
    let (turn, trace) = (ulid(now_us()), trace_id());
    store::add_turn(
        tx,
        &s.chat,
        &turn,
        &trace,
        (Some(&s.agent), None),
        Opened::Scheduled,
    )?;
    let saved = store::add_person_message(tx, &s.chat, &turn, &s.words, None)?;
    let message = saved["id"].as_str().unwrap_or_default().to_string();
    tx.execute(
        "INSERT INTO schedule_runs (id, schedule_id, chat_id, kind, due_ts, ran_ts, turn_id, message_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![ulid(now_us()), s.id, s.chat, kind, due, ran, turn, message],
    )?;
    Ok(Start {
        chat: s.chat.clone(),
        agent: s.agent.clone(),
        answers: None,
        text: crate::replies::read_as(tx, &message, &s.words)?,
        turn,
        trace,
        message: store::message(tx, &message)?,
    })
}

/// Gathr's line before a run's words, for its agent: which schedule, and when it was due.
pub fn line(conn: &Connection, message: &str) -> Result<Option<String>, Error> {
    let found: Option<(String, String, i64, i64)> = conn
        .prepare_cached(
            "SELECT schedule_id, kind, due_ts, ran_ts FROM schedule_runs WHERE message_id = ?1",
        )?
        .query_row([message], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let Some((id, kind, due, ran)) = found else {
        return Ok(None);
    };
    let said = match kind.as_str() {
        "now" => format!("Scheduled run of {id}, started now by the person with Run now."),
        _ => format!(
            "Scheduled run of {id}, due {} on this PC's clock.",
            day_time(conn, due)?
        ),
    };
    let late = match kind.as_str() {
        "late" => format!(
            " It started late, at {}: {AWAY}",
            clock(local(conn, ran)?.minute)
        ),
        _ => String::new(),
    };
    Ok(Some(format!(
        "{said}{late} The words below are what you scheduled in this chat, not a new message from the person."
    )))
}

/// Every schedule, oldest first, each with its chat's title, its project's name, when in words and next.
pub fn list(conn: &Connection) -> Result<Value, Error> {
    let ids: Vec<String> = conn
        .prepare_cached("SELECT id FROM schedules ORDER BY created_ts, id")?
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    let shown: Result<Vec<Value>, Error> =
        ids.iter().map(|id| shown(conn, &one(conn, id)?)).collect();
    Ok(json!({ "schedules": shown? }))
}

fn shown(conn: &Connection, s: &Row) -> Result<Value, Error> {
    let (title, project): (String, Option<String>) = conn.query_row(
        "SELECT c.title, p.name FROM chats c LEFT JOIN projects p ON p.id = c.project_id WHERE c.id = ?1",
        [&s.chat],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(json!({
        "id": s.id,
        "chat": s.chat,
        "chat_title": title,
        "project": project,
        "agent": s.agent,
        "words": s.words,
        "when": s.when.words(conn)?,
        "enabled": s.next.is_some(),
        "next": s.next.map(utc),
    }))
}

/// One schedule with its last runs, newest first: when each ran, its state and one line.
pub fn get(conn: &Connection, id: &str) -> Result<Value, Error> {
    let s = one(conn, id)?;
    let mut shown = shown(conn, &s)?;
    let mut stmt = conn.prepare_cached(
        "SELECT r.id, r.kind, r.due_ts, r.ran_ts, r.message_id, t.status, t.why,
                (SELECT m.body FROM messages m WHERE m.chat_id = r.chat_id AND m.turn_id = r.turn_id
                   AND m.role = 'assistant' AND m.kind = 'text' AND trim(m.body) != ''
                 ORDER BY m.ts DESC LIMIT 1)
         FROM schedule_runs r LEFT JOIN turns t ON t.id = r.turn_id
         WHERE r.schedule_id = ?1 ORDER BY r.ran_ts DESC LIMIT ?2",
    )?;
    type Run = (
        String,
        String,
        i64,
        i64,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    );
    let runs: Vec<Run> = stmt
        .query_map(params![id, RUNS_SHOWN], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
                r.get(7)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut listed = Vec::new();
    for (run, kind, due, ran, message, status, why, said) in runs {
        let first = said.and_then(|s| {
            s.lines()
                .map(str::trim)
                .find(|l| !l.is_empty())
                .map(String::from)
        });
        let (state, line) = match (kind.as_str(), status.as_deref()) {
            ("skipped", _) => ("skipped", Some(AWAY.to_string())),
            (_, Some("done")) if kind == "late" => {
                let at = clock(local(conn, due)?.minute);
                ("late", Some(format!("Due at {at}. {AWAY}")))
            }
            (_, Some("done") | None) => ("done", first.map(|l| crate::text::cut(&l, 160, false))),
            (_, Some("failed")) => ("failed", why),
            (_, Some("stopped")) => ("stopped", why),
            _ => ("running", None),
        };
        let at = if kind == "skipped" { due } else { ran };
        listed.push(
            json!({ "id": run, "at": utc(at), "state": state, "line": line, "message": message }),
        );
    }
    shown["runs"] = json!(listed);
    Ok(shown)
}

/// Enables it from `now` on, never catching up on a time before; or disables it.
pub fn set_enabled(conn: &Connection, id: &str, on: bool, now: i64) -> Result<Value, Error> {
    let s = one(conn, id)?;
    let next = match (on, s.next) {
        (false, _) => None,
        (true, Some(next)) => Some(next),
        (true, None) => {
            crate::archive::refuse_archived(conn, &s.chat)?;
            let next = s.when.next_after(conn, now)?;
            Some(next.ok_or_else(|| {
                Error::Refused("Its time has passed. Ask the agent again.".into())
            })?)
        }
    };
    conn.execute(
        "UPDATE schedules SET next_ts = ?2 WHERE id = ?1",
        params![id, next],
    )?;
    shown(conn, &one(conn, id)?)
}

/// Deletes a schedule; its runs stay with its chat. With `chat`, only one made in that chat.
pub fn delete(conn: &Connection, id: &str, chat: Option<&str>) -> Result<(), Error> {
    let gone = conn.execute(
        "DELETE FROM schedules WHERE id = ?1 AND chat_id = coalesce(?2, chat_id)",
        params![id, chat],
    )?;
    match gone {
        0 => Err(Error::Refused(format!(
            "There's no schedule {id} in this chat."
        ))),
        _ => Ok(()),
    }
}

/// What is scheduled in `chat`, for an agent: each one's id, when, next and words.
pub fn in_chat(conn: &Connection, chat: &str) -> Result<String, Error> {
    let rows: Vec<Row> = conn
        .prepare_cached(&format!("{ROW} WHERE chat_id = ?1 ORDER BY created_ts, id"))?
        .query_map([chat], row)?
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Ok("Nothing is scheduled in this chat.".into());
    }
    let mut lines = vec!["Scheduled in this chat:".to_string()];
    for s in rows {
        let next = match s.next {
            Some(at) => format!("next {}", day_time(conn, at)?),
            None => "disabled".to_string(),
        };
        let words = crate::text::cut(
            &s.words.split_whitespace().collect::<Vec<_>>().join(" "),
            120,
            false,
        );
        lines.push(format!(
            "- {}: {}, {next}: {words}",
            s.id,
            s.when.words(conn)?
        ));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests;
