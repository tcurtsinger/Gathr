//! An owner's report on its task (0047): kept on the task, taken whatever state the task is in,
//! and applied when the task's running turn ends or, with none running, at once.

use rusqlite::{Connection, params};
use serde::Serialize;

use super::records::{read, state_of};
use super::{Kind, NoteSaid, Start, State, Task, cascade, held, notes, refused, turn_running};
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
    /// For a review that is done: its reviewer checked that the PR's tests and check settings
    /// still test the code (0049).
    pub signed_off: bool,
    /// For a review: what it found, one note each (0053).
    pub notes: &'a [NoteSaid<'a>],
    /// For a review that is done: nothing in the change goes past what was asked.
    pub nothing_unasked: bool,
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
    if said.signed_off && (t.kind != Kind::Review || how != Finish::Done) {
        return Err(refused("`signed_off` goes with a review that is done"));
    }
    if (!said.notes.is_empty() || said.nothing_unasked) && t.kind != Kind::Review {
        return Err(refused("`notes` and `nothing_unasked` go with a review"));
    }
    if said.notes.iter().any(|n| n.says.trim().is_empty()) {
        return Err(refused("Each note says what it is, in `says`"));
    }
    let asks_work = notes::ask_for_work(said.notes);
    // A review that is done is recorded again at another commit while its PR is open (G170): the
    // task stays done, and no fresh review task is made only to re-record an approval. The same
    // review recorded again with its sign-off is taken too (0049).
    let again = t.state == State::Done
        && t.kind == Kind::Review
        && how == Finish::Done
        && commit.is_some()
        && (commit != t.reviewed || (said.signed_off && !t.signed_off))
        && tx.query_row(
            "SELECT pr IS NOT NULL AND pr_closed = 0 FROM tasks WHERE number = ?1",
            [task],
            |r| r.get::<_, bool>(0),
        )?;
    // A done review's notes are its own (0053): what it found later is a new review's.
    if t.state == State::Done && !again && t.kind == Kind::Review && asks_work {
        let at = t.reviewed.as_deref().unwrap_or_default();
        return Err(refused(format!(
            "Review #{task} is done at {}. New notes need a new review: ask its giver.",
            at.get(..7).unwrap_or(at)
        )));
    }
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
        // As the merge tool asks (0043): fewer characters could name another commit, and one that
        // happened to begin like the PR's head would read as a review of it.
        (_, Kind::Review) if commit.as_deref().is_some_and(|c| !names_a_commit(c)) => {
            Some("Give 7 to 40 characters of the commit you reviewed, as git names it")
        }
        (Finish::Done, Kind::Review) if !said.nothing_unasked || asks_work => Some(
            "A done review says nothing_unasked: true and has no wrong or unasked notes. Report those as left.",
        ),
        (_, Kind::Review) if !said.notes.is_empty() && commit.is_none() => {
            Some("Include the reviewed commit with your notes")
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
    // A review's commit is the commit reviewed. Work's commit or file is what to look at. A review
    // recorded at a commit that isn't the PR's head as last seen reads out of date until the look
    // judges it, and any earlier verdict is dropped: it was for another commit.
    let (reviewed, see) = match t.kind {
        Kind::Review => (commit, None),
        _ => (None, file.or(commit)),
    };
    let now = now_us();
    tx.execute(
        "UPDATE tasks SET finish = ?2, finish_note = ?3, finish_see = ?4, finish_ts = ?5, asks = 0,
                          blocked_on = CASE WHEN ?2 = 'blocked' THEN ?6 ELSE NULL END,
                          pr = COALESCE(?7, pr), reviewed = COALESCE(?8, reviewed),
                          review_done = CASE WHEN kind = 'review' THEN (?2 = 'done' AND ?8 IS NOT NULL) ELSE review_done END,
                          signed_off = CASE WHEN kind = 'review' THEN (?2 = 'done' AND ?8 IS NOT NULL
                              AND (?9 OR (signed_off AND review_done AND reviewed = ?8))) ELSE signed_off END,
                          outdated = CASE WHEN ?8 IS NULL THEN outdated WHEN pr_head IS NULL THEN 0
                              WHEN substr(pr_head, 1, length(?8)) = ?8 OR substr(?8, 1, length(pr_head)) = pr_head
                              THEN 0 ELSE 1 END,
                          judged = CASE WHEN ?8 IS NULL THEN judged ELSE NULL END
         WHERE number = ?1",
        params![
            task,
            how.as_str(),
            note,
            see,
            now,
            said.on_task,
            said.pr,
            reviewed,
            said.signed_off
        ],
    )?;
    if let Some(reviewed) = reviewed.as_deref()
        && (!said.notes.is_empty() || how == Finish::Done)
    {
        let nothing_unasked = said.nothing_unasked;
        notes::record(&tx, &t, reviewed, said.notes, now, nothing_unasked, trace)?;
    }
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

/// Whether the text is how git names a commit, in full or cut short: 7 to 40 hex digits.
fn names_a_commit(text: &str) -> bool {
    (7..=40).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}
