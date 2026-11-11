//! A task's row: read, moved to a state with its event, and its state alone.

use rusqlite::{Connection, OptionalExtension, params};

use super::person::Form;
use super::{Kind, State, Task};
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

/// Moves a task to `state`, with why, and records it.
pub fn set(
    tx: &Connection,
    task: i64,
    state: State,
    reason: Option<&str>,
    trace: &str,
) -> Result<(), Error> {
    // A task heard while held only on its merge (held.rs) is heard again when it settles, merged,
    // closed or cancelled: its giver's own task is still held on it.
    let chat: String = tx.query_row(
        "UPDATE tasks SET state = ?2, reason = ?3, updated_ts = ?4, stale_ts = NULL,
             heard = CASE WHEN state = 'held' AND ?5 THEN 0 ELSE heard END
         WHERE number = ?1 RETURNING chat_id",
        params![task, state.as_str(), reason, now_us(), state.settled()],
        |r| r.get(0),
    )?;
    let event = Event::point("gathr.task.state").trace(trace).chat(&chat);
    let event = event.task(task).attr("gathr.task.state", state.as_str());
    let event = match state {
        State::Failed => event.error("task_failed"),
        _ => event.ok(),
    };
    event.attr("gathr.task.reason", reason).insert(tx)?;
    if state == State::Cancelled {
        let why = reason.unwrap_or("no reason given");
        tx.execute(
            "UPDATE turns SET stop_why = ?2
             WHERE task = ?1 AND status IN ('queued', 'waiting_sign_in', 'running')",
            params![task, format!("its task #{task} was cancelled: {why}")],
        )?;
    }
    Ok(())
}

pub fn state_of(conn: &Connection, task: i64) -> Result<Option<State>, Error> {
    let found: Option<String> = conn
        .query_row("SELECT state FROM tasks WHERE number = ?1", [task], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(found.as_deref().map(State::named))
}

pub fn read(conn: &Connection, task: i64) -> Result<Task, Error> {
    let mut t = conn.query_row(
        "SELECT number, chat_id, giver, owner, given_in, parent, kind, what, why, state, reason,
                blocked_on, pr, reviewed, outdated, form, choices, files, request, answer, created_ts,
                stale_ts, heard_in
         FROM tasks WHERE number = ?1",
        [task],
        |r| {
            Ok(Task {
                number: r.get(0)?,
                chat: r.get(1)?,
                giver: r.get(2)?,
                owner: r.get(3)?,
                given_in: r.get(4)?,
                parent: r.get(5)?,
                kind: Kind::named(&r.get::<_, String>(6)?).unwrap_or(Kind::Work),
                what: r.get(7)?,
                why: r.get(8)?,
                state: State::named(&r.get::<_, String>(9)?),
                reason: r.get(10)?,
                after: Vec::new(),
                blocked_on: r.get(11)?,
                pr: r.get(12)?,
                reviewed: r.get(13)?,
                outdated: r.get(14)?,
                form: r
                    .get::<_, Option<String>>(15)?
                    .as_deref()
                    .and_then(Form::named),
                choices: json(r, 16)?.unwrap_or_default(),
                files: json(r, 17)?.unwrap_or_default(),
                request: r.get(18)?,
                answer: json(r, 19)?,
                created_us: r.get(20)?,
                stale_us: r.get(21)?,
                heard_in: r.get(22)?,
            })
        },
    )?;
    let mut stmt =
        conn.prepare_cached("SELECT after_task FROM task_after WHERE task = ?1 ORDER BY 1")?;
    t.after = stmt
        .query_map([task], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(t)
}

/// A JSON column, read as `T`; none when it's empty.
fn json<T: serde::de::DeserializeOwned>(
    r: &rusqlite::Row,
    i: usize,
) -> rusqlite::Result<Option<T>> {
    let text: Option<String> = r.get(i)?;
    Ok(text.and_then(|t| serde_json::from_str(&t).ok()))
}

pub trait OptionalTask {
    fn optional_task(self) -> Result<Option<Task>, Error>;
}

impl OptionalTask for Result<Task, Error> {
    fn optional_task(self) -> Result<Option<Task>, Error> {
        match self {
            Ok(t) => Ok(Some(t)),
            Err(Error::Db(rusqlite::Error::QueryReturnedNoRows)) => Ok(None),
            Err(e) => Err(e),
        }
    }
}
