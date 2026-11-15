//! A task's giver changes it until its first turn starts (0059). The turn runner reads a task's ask
//! again as it takes its turn to send it, and from then on the task has started.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde_json::{Value, json};

use super::records::{OptionalTask, read};
use super::{
    Brief, Kind, Start, State, Task, back_in_line, brief, in_tx, refused, waiting_on, words,
};
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

/// Marks a task started, once.
pub(super) const STARTED: &str =
    "UPDATE tasks SET started_ts = COALESCE(started_ts, ?2) WHERE number = ?1";

/// A change to a task: only what is given changes.
#[derive(Default)]
pub struct Edit<'a> {
    pub chat: &'a str,
    /// The agent editing it; none is the person.
    pub by: Option<&'a str>,
    pub task: i64,
    pub what: Option<&'a str>,
    pub why: Option<&'a str>,
    pub done: Option<&'a str>,
    pub proof: Option<&'a str>,
    pub place: Option<&'a str>,
    /// An empty one removes the size and its lines.
    pub size: Option<&'a str>,
    pub lines: Option<i64>,
    pub after: Option<&'a [i64]>,
}

/// Edits a task: the task as it is now, the turn it starts when the edit makes it ready, and its
/// queued brief's message when that was rewritten.
pub fn edit(
    conn: &mut Connection,
    e: &Edit,
    trace: &str,
) -> Result<(Task, Vec<Start>, Option<Value>), Error> {
    in_tx(conn, |tx| edit_in(tx, e, trace))
}

fn edit_in(
    tx: &Transaction,
    e: &Edit,
    trace: &str,
) -> Result<(Task, Vec<Start>, Option<Value>), Error> {
    let n = e.task;
    let t = read(tx, n).optional_task()?.filter(|t| t.chat == e.chat);
    let t = t.ok_or_else(|| refused(format!("there's no task #{n} in this chat")))?;
    may_edit(tx, &t, e.by)?;
    crate::archive::refuse_archived(tx, e.chat)?;
    let kept = |given: Option<&str>, was: &Option<String>| match given {
        Some(given) => brief::given(Some(given)).map(String::from),
        None => was.clone(),
    };
    let what = e.what.map_or(t.what.clone(), |w| w.trim().to_string());
    let (why, done) = (kept(e.why, &t.why), kept(e.done, &t.done));
    let (proof, place) = (kept(e.proof, &t.proof), kept(e.place, &t.place));
    let (size, lines) = match e.size {
        Some(size) if brief::given(Some(size)).is_none() => (None, None),
        Some(size) => (Some(size.trim().to_string()), e.lines.or(t.size_lines)),
        None => (t.size.clone(), e.lines.or(t.size_lines)),
    };
    let pair = brief::sized(size.as_deref(), lines);
    let b = Brief {
        why: why.as_deref(),
        done: done.as_deref(),
        proof: proof.as_deref(),
        place: place.as_deref(),
        size: pair.as_ref().ok().copied().flatten(),
    };
    brief::checked(t.kind, &what, &b)?;
    pair?;
    let after = e.after.map(|after| {
        let mut after = after.to_vec();
        after.sort_unstable();
        after.dedup();
        after
    });
    if let Some(after) = &after {
        in_chat(tx, e.chat, after)?;
        may_wait(tx, n, after)?;
    }
    let fields = [
        ("what", what == t.what, json!(t.what)),
        ("why", why == t.why, json!(t.why)),
        ("done", done == t.done, json!(t.done)),
        ("proof", proof == t.proof, json!(t.proof)),
        ("where", place == t.place, json!(t.place)),
        (
            "size",
            (&size, lines) == (&t.size, t.size_lines),
            json!({ "size": t.size, "lines": t.size_lines }),
        ),
        (
            "after",
            after.as_ref().is_none_or(|a| *a == t.after),
            json!(t.after),
        ),
    ];
    let changed: Vec<(&str, Value)> = fields
        .into_iter()
        .filter(|f| !f.1)
        .map(|(name, _, was)| (name, was))
        .collect();
    if changed.is_empty() {
        return Err(refused(format!(
            "Nothing to change: task #{n} already reads so."
        )));
    }
    let names: Vec<&str> = changed.iter().map(|c| c.0).collect();
    tx.execute(
        "UPDATE tasks SET what = ?2, why = ?3, done = ?4, proof = ?5, place = ?6, size = ?7, size_lines = ?8,
                          edited_ts = ?9, edited = ?10, updated_ts = ?9, stale_ts = NULL
         WHERE number = ?1",
        params![n, what, why, done, proof, place, size, lines, now_us(), names.join(",")],
    )?;
    let mut starts = Vec::new();
    if let Some(after) = after.filter(|a| *a != t.after) {
        starts = reordered(tx, &t, &after, trace)?;
    }
    let still = t.state == State::Ready && super::state_of(tx, n)? == Some(State::Ready);
    let rewritten = match still && starts.is_empty() {
        true => rewritten(tx, n)?,
        false => None,
    };
    let task = read(tx, n)?;
    Event::point("gathr.task.edit")
        .trace(trace)
        .chat(e.chat)
        .task(n)
        .ok()
        .attr("gathr.task.fields", names.join(","))
        .attr("gathr.task.editor", e.by)
        .attr("gathr.task.state", task.state.as_str())
        .attr(
            "gathr.task.was",
            Value::Object(changed.into_iter().map(|(k, v)| (k.into(), v)).collect()),
        )
        .attr(
            "gathr.turn.id",
            rewritten.as_ref().map(|(turn, _)| turn.clone()),
        )
        .insert(tx)?;
    Ok((task, starts, rewritten.map(|(_, message)| message)))
}

/// Who may edit the task, and when: its giver, until its first turn starts.
fn may_edit(conn: &Connection, t: &Task, by: Option<&str>) -> Result<(), Error> {
    let n = t.number;
    if matches!(t.kind, Kind::Person | Kind::PersonRequest) {
        return Err(refused(format!(
            "Task #{n} is a question for the person: edit_task changes work and review tasks."
        )));
    }
    if by != t.giver.as_deref() {
        let giver = words::owner_name(conn, t.giver.as_deref())?;
        return Err(refused(format!(
            "Only task #{n}'s giver may edit it, and that's {giver}."
        )));
    }
    if t.started_us.is_some() {
        let owner = words::owner_name(conn, t.owner.as_deref())?;
        return Err(refused(format!(
            "Task #{n} has started, so it can't be edited: message {owner} with the change, or cancel it and give a new task."
        )));
    }
    match t.state {
        State::Waiting | State::Ready | State::Blocked => Ok(()),
        state => Err(refused(format!(
            "Task #{n} is {}: only a waiting, ready or blocked task can be edited.",
            state.as_str()
        ))),
    }
}

/// Refuses a task number that isn't of this chat.
pub(super) fn in_chat<'a>(
    conn: &Connection,
    chat: &str,
    numbers: impl IntoIterator<Item = &'a i64>,
) -> Result<(), Error> {
    for &n in numbers {
        let found: Option<String> = conn
            .query_row("SELECT chat_id FROM tasks WHERE number = ?1", [n], |r| {
                r.get(0)
            })
            .optional()?;
        if found.as_deref() != Some(chat) {
            return Err(refused(format!("there's no task #{n} in this chat")));
        }
    }
    Ok(())
}

/// Refuses an `after` that would make the task wait on itself, at once or through others.
fn may_wait(conn: &Connection, task: i64, after: &[i64]) -> Result<(), Error> {
    if after.contains(&task) {
        return Err(refused(format!("Task #{task} can't wait on itself.")));
    }
    for &a in after {
        let looped: bool = conn.query_row(
            "WITH RECURSIVE up (n) AS (
                 SELECT after_task FROM task_after WHERE task = ?1
                 UNION SELECT a.after_task FROM task_after a JOIN up ON a.task = up.n)
             SELECT EXISTS (SELECT 1 FROM up WHERE n = ?2)",
            [a, task],
            |r| r.get(0),
        )?;
        if looped {
            return Err(refused(format!(
                "#{a} waits on #{task} already, so #{task} can't wait on it."
            )));
        }
    }
    Ok(())
}

/// The task's new `after`, and its place in line: a ready task that now waits drops its queued
/// turn, and a waiting or blocked one goes back in line, as `back_in_line` puts it.
fn reordered(tx: &Transaction, t: &Task, after: &[i64], trace: &str) -> Result<Vec<Start>, Error> {
    let n = t.number;
    tx.execute("DELETE FROM task_after WHERE task = ?1", [n])?;
    for &a in after {
        tx.execute(
            "INSERT INTO task_after (task, after_task) VALUES (?1, ?2)",
            [n, a],
        )?;
    }
    let waiting = waiting_on(tx, n)?;
    if t.state == State::Ready {
        if waiting.is_empty() {
            return Ok(Vec::new());
        }
        // Marked as a cancel marks them, so the runner drops them: an edit that clears `after`
        // again queues one turn, not two.
        let on: Vec<String> = waiting.iter().map(|w| format!("#{w}")).collect();
        let why = format!("its task #{n} was edited to wait on {}", on.join(", "));
        tx.execute(
            "UPDATE turns SET stop_why = ?2 WHERE task = ?1 AND status IN ('queued', 'waiting_sign_in')",
            params![n, why],
        )?;
    }
    back_in_line(tx, n, trace)
}

/// Rewrites the brief of the task's queued turn from its row as it is now, with the carry it was
/// queued with: the turn and its message, when the text changed.
fn rewritten(tx: &Transaction, task: i64) -> Result<Option<(String, Value)>, Error> {
    let queued: Option<(String, String, String)> = tx
        .query_row(
            "SELECT u.id, m.id, m.body FROM turns u JOIN messages m ON m.turn_id = u.id AND m.role = 'gathr'
             WHERE u.task = ?1 AND u.status IN ('queued', 'waiting_sign_in')
             ORDER BY u.created_ts DESC, u.rowid DESC, m.ts LIMIT 1",
            [task],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((turn, message, body)) = queued else {
        return Ok(None);
    };
    let t = read(tx, task)?;
    let giver = words::owner_name(tx, t.giver.as_deref())?;
    let text = words::brief(tx, &t, &giver, t.carry.as_deref())?;
    if text == body {
        return Ok(None);
    }
    crate::store::set_message_body(tx, &message, &text)?;
    tx.execute(
        "UPDATE turns SET opened_version = ?2 WHERE id = ?1",
        params![turn, brief::opened(t.kind).version()],
    )?;
    Ok(Some((turn, crate::store::message(tx, &message)?)))
}

/// The turn's ask as saved, when its task hasn't started: every message the person or Gathr put in
/// it, in order. Its task starts here, in the same write, so no edit falls between this read and
/// the send. None, with nothing written, for a turn on no task or on a task that has started.
pub fn claim_ask(conn: &mut Connection, turn: &str) -> Result<Option<String>, Error> {
    in_tx(conn, |tx| {
        let unstarted: Option<i64> = tx
            .query_row(
                "SELECT t.number FROM turns u JOIN tasks t ON t.number = u.task
                 WHERE u.id = ?1 AND t.started_ts IS NULL",
                [turn],
                |r| r.get(0),
            )
            .optional()?;
        let Some(task) = unstarted else {
            return Ok(None);
        };
        let sql = format!("SELECT {} FROM turns t WHERE t.id = ?1", crate::store::ASK);
        let ask: Option<String> = tx.query_row(&sql, [turn], |r| r.get(0))?;
        tx.execute(STARTED, params![task, now_us()])?;
        Ok(ask)
    })
}
