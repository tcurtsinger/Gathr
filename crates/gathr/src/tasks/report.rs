//! An owner's report on its task (0047): kept on the task, taken whatever state the task is in,
//! and applied when the task's running turn ends or, with none running, at once.

use rusqlite::{Connection, params};
use serde::Serialize;

use super::records::{read, state_of};
use super::{Kind, Start, State, Task, cascade, held, refused, turn_running};
use crate::Error;
use crate::ids::now_us;

/// What an owner says of its task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    Done,
    Left,
    Blocked,
}

impl Finish {
    pub fn named(text: &str) -> Option<Finish> {
        match text {
            "done" => Some(Finish::Done),
            "left" => Some(Finish::Left),
            "blocked" => Some(Finish::Blocked),
            _ => None,
        }
    }

    pub(super) fn as_str(self) -> &'static str {
        match self {
            Finish::Done => "done",
            Finish::Left => "left",
            Finish::Blocked => "blocked",
        }
    }
}

/// The owner's finish, with what goes with it.
#[derive(Default)]
pub struct Said<'a> {
    pub note: Option<&'a str>,
    pub on_task: Option<i64>,
    pub pr: Option<i64>,
    pub commit: Option<&'a str>,
    /// For work: a file to look at.
    pub file: Option<&'a str>,
}

/// How Gathr took a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Took {
    /// A turn on the task is running: the report applies when it ends.
    Kept,
    /// Applied at once: the task says where it stands now.
    Applied,
    /// The task was done already: nothing changed.
    Already,
    /// The task was cancelled: nothing changed.
    Cancelled,
}

/// What an owner said of its task, and when.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub how: Finish,
    pub note: Option<String>,
    /// For work: the commit or file to look at. Its PR is the task's own.
    pub see: Option<String>,
    pub at_us: Option<i64>,
}

/// The owner says how its task stands: a record, taken whatever state the task is in (0047).
/// While a turn runs on the task it's kept and applies when that turn ends; else it applies at
/// once. A review's done names the commit reviewed (G73). What's refused is a report that isn't the
/// caller's to make or that says too little, never one for the state the task is in.
pub fn finish(
    conn: &mut Connection,
    agent: &str,
    task: i64,
    how: Finish,
    said: &Said,
    trace: &str,
) -> Result<(Task, Took, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let t = read(&tx, task).map_err(|_| refused(format!("there's no task #{task}")))?;
    if t.owner.as_deref() != Some(agent) {
        return Err(refused(format!("task #{task} isn't yours")));
    }
    let given = |text: Option<&str>| {
        let text = text.map(str::trim).filter(|s| !s.is_empty());
        text.map(String::from)
    };
    let (note, commit, file) = (given(said.note), given(said.commit), given(said.file));
    // A review that is done is recorded again at another commit while its PR is open (G170): the
    // task stays done, and no fresh review task is made only to re-record an approval.
    let again = t.state == State::Done
        && t.kind == Kind::Review
        && how == Finish::Done
        && commit.is_some()
        && commit != t.reviewed
        && tx.query_row(
            "SELECT pr IS NOT NULL AND pr_closed = 0 FROM tasks WHERE number = ?1",
            [task],
            |r| r.get::<_, bool>(0),
        )?;
    match t.state {
        State::Done if !again => return Ok((t, Took::Already, Vec::new())),
        State::Cancelled => return Ok((t, Took::Cancelled, Vec::new())),
        _ => {}
    }
    if let Some(on) = said.on_task
        && (state_of(&tx, on)?.is_none() || on == task)
    {
        return Err(refused(format!(
            "there's no other task #{on} to be blocked on"
        )));
    }
    // A task's PR is what it's about (a review's above all, G73): a report can't move it to another.
    if let (Some(said), Some(its)) = (said.pr, t.pr)
        && said != its
    {
        return Err(refused(format!(
            "task #{task} is about PR #{its}, not #{said}"
        )));
    }
    if said.on_task.is_some() && how != Finish::Blocked {
        return Err(refused("`on_task` goes with blocked"));
    }
    let shown = note.is_some() || commit.is_some() || file.is_some() || said.pr.or(t.pr).is_some();
    let missing = match (how, t.kind) {
        (Finish::Done, Kind::Review) if commit.is_none() => {
            Some("Include the reviewed commit when you finish a review")
        }
        (Finish::Done, Kind::Work) if !shown => {
            Some("Say what to look at when work is done: a PR, a commit, a file or a note")
        }
        (Finish::Left, _) if note.is_none() => Some("Say what is left, in `note`"),
        (Finish::Blocked, _) if note.is_none() && said.on_task.is_none() => {
            Some("Say what blocks it, in `note`, or the task it waits on, in `on_task`")
        }
        _ => None,
    };
    if let Some(missing) = missing {
        return Err(refused(missing));
    }
    // A review's commit is the commit reviewed. Work's commit or file is what to look at.
    let (reviewed, see) = match t.kind {
        Kind::Review => (commit, None),
        _ => (None, file.or(commit)),
    };
    tx.execute(
        "UPDATE tasks SET finish = ?2, finish_note = ?3, finish_see = ?4, finish_ts = ?5, asks = 0,
                          blocked_on = CASE WHEN ?2 = 'blocked' THEN ?6 ELSE NULL END,
                          pr = COALESCE(?7, pr), reviewed = COALESCE(?8, reviewed),
                          review_done = CASE WHEN kind = 'review' THEN (?2 = 'done' AND ?8 IS NOT NULL) ELSE review_done END,
                          outdated = CASE WHEN ?8 IS NULL THEN outdated ELSE 0 END
         WHERE number = ?1",
        params![task, how.as_str(), note, see, now_us(), said.on_task, said.pr, reviewed],
    )?;
    let running = turn_running(&tx, task)?;
    // Left on a task whose turn is still to come: the note is kept, and it keeps its place.
    let in_line = matches!(t.state, State::Waiting | State::Ready) && how == Finish::Left;
    let (took, starts) = if again {
        (Took::Applied, Vec::new())
    } else if running {
        (Took::Kept, Vec::new())
    } else if in_line {
        (Took::Applied, Vec::new())
    } else {
        let mut starts = held::settle(&tx, &t, trace)?;
        starts.extend(cascade(&tx, task, trace)?);
        (Took::Applied, starts)
    };
    let task = read(&tx, task)?;
    tx.commit()?;
    Ok((task, took, starts))
}
