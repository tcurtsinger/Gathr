//! A building task's PR, counted when its task reports it done and before its review is given
//! (0064), against the task's own line: a Fix's, or its plan part's. Past it, 0054's question is
//! asked in the task's turn, so the task waits on the person's answer.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Measure, kept, read, record, record_with, verdict};
use crate::Error;
use crate::event::Event;
use crate::tasks::{self, Finish, Kind, Start, State, Task, held, records, review};

/// The least a Fix's line can be.
pub const FIX_FLOOR: i64 = 1_000;
/// Below this many landed Fixes, the project's Fixes set no line.
const FIX_FEWEST: i64 = 10;
/// The landed Fixes a Fix's line reads, newest first.
const FIX_LAST: i64 = 50;
/// A part is asked about past this many times its size, and `PART_OVER` lines over it.
const PART_TIMES: i64 = 3;
const PART_OVER: i64 = 300;

/// What the report's handler found when it counted, before the task's write.
#[derive(Debug, Clone, Default)]
pub enum Counted {
    /// Not a report that is counted: it goes as it always did.
    #[default]
    NotCounted,
    /// The count failed or took too long, and was recorded: the report goes as if not counted.
    Failed,
    /// The PR's count, and how long it took in milliseconds.
    Measured(Measure, i64),
}

/// A report to count: the task's PR in its project, against the task's line.
#[derive(Debug, Clone)]
pub struct ToCount {
    pub project: String,
    pub chat: String,
    pub task: i64,
    pub pr: i64,
    pub line: i64,
    pub line_of: &'static str,
}

/// What a counted report stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stop {
    /// Gathr's question to the person is open: filed now, or already.
    Asked(i64),
    /// The PR rests at this head on the person's word: the task is blocked with why.
    Rested(String),
}

/// Whether the report is counted: done, on the caller's own building task not yet done or
/// cancelled, first naming its PR or reporting it after a send-back, with no review given for it
/// yet, in a project's chat, with a line.
pub fn to_count(
    conn: &Connection,
    agent: &str,
    task: i64,
    how: Finish,
    pr: Option<i64>,
) -> Result<Option<ToCount>, Error> {
    let Some(t) = tasks::get(conn, task)? else {
        return Ok(None);
    };
    let open = !matches!(t.state, State::Done | State::Cancelled);
    let building = t.kind == Kind::Work && !t.look_into;
    if how != Finish::Done || t.owner.as_deref() != Some(agent) || !building || !open {
        return Ok(None);
    }
    let Some(pr) = pr.or(t.pr) else {
        return Ok(None);
    };
    let project: Option<String> = conn
        .query_row(
            "SELECT project_id FROM chats WHERE id = ?1",
            [&t.chat],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let Some(project) = project else {
        return Ok(None);
    };
    // A report can't move a task to another PR: it is refused.
    if t.pr.is_some_and(|its| its != pr)
        || (t.pr.is_some() && !sent_back_to(conn, &t.chat, pr, task)?)
        || review::covered(conn, &t.chat, pr, task)?.is_some()
    {
        return Ok(None);
    }
    let Some((line, line_of)) = task_line(conn, task)? else {
        return Ok(None);
    };
    Ok(Some(ToCount {
        project,
        chat: t.chat,
        task,
        pr,
        line,
        line_of,
    }))
}

/// Whether the chat's PR, not landed, was sent back to `task`: by "Send it back", or by the
/// person cancelling Gathr's question about it.
pub(crate) fn sent_back_to(
    conn: &Connection,
    chat: &str,
    pr: i64,
    task: i64,
) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM changes WHERE chat_id = ?1 AND pr = ?2 AND sent_back_task = ?3
             AND landed_ts IS NULL)",
        params![chat, pr, task],
        |r| r.get(0),
    )?)
}

/// A building task's line, and which it is: on a repair, the Fix's line; on a new or unsized ask
/// with lines, its part's line. None for any other task, or one with no ask.
pub fn task_line(conn: &Connection, task: i64) -> Result<Option<(i64, &'static str)>, Error> {
    let found: Option<(Option<String>, Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT a.size, t.size_lines, c.project_id FROM tasks t JOIN asks a ON a.id = t.ask_id
             JOIN chats c ON c.id = t.chat_id WHERE t.number = ?1",
            [task],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    Ok(match found {
        // A repair's own lines are the lead's word, never its line.
        Some((Some(size), _, project)) if size == "repair" => {
            Some((fix_line(conn, project.as_deref())?, "fix"))
        }
        Some((_, Some(n), _)) => Some(((PART_TIMES * n).max(n + PART_OVER), "part")),
        _ => None,
    })
}

/// The most any of the project's last 50 landed Fixes added, never under the floor.
fn fix_line(conn: &Connection, project: Option<&str>) -> Result<i64, Error> {
    let (n, most): (i64, Option<i64>) = conn.query_row(
        "SELECT count(*), max(counted) FROM (SELECT counted FROM changes
           WHERE project_id = ?1 AND size = 'repair' AND landed_ts IS NOT NULL
           ORDER BY landed_ts DESC LIMIT ?2)",
        params![project, FIX_LAST],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    Ok(match n < FIX_FEWEST {
        true => FIX_FLOOR,
        false => most.unwrap_or(0).max(FIX_FLOOR),
    })
}

/// The line of the chat's PR's work task, which landing measures against when it's lower.
pub fn landing_line(
    conn: &Connection,
    chat: &str,
    pr: i64,
) -> Result<Option<(i64, &'static str)>, Error> {
    match tasks::notes::work_on(conn, chat, pr)? {
        Some((_, work)) => task_line(conn, work),
        None => Ok(None),
    }
}

/// The count of `work`'s done report, in the report's transaction before the task settles: kept as
/// landing keeps one, and past its line 0054's question, given in the turn its review would be
/// given in. A head that rests on the person's word turns the report into a block that says why.
pub(crate) fn reported(
    tx: &Transaction,
    m: &Measure,
    work: &Task,
    turn: Option<&str>,
    ms: i64,
    trace: &str,
) -> Result<(Option<Stop>, Vec<Start>), Error> {
    let row = kept(tx, m, trace)?;
    let given_in = review::given_in(tx, work.number, turn)?;
    let mut starts = Vec::new();
    let stop = match verdict(tx, &row, given_in.as_deref(), &mut starts, trace)? {
        None => None,
        Some((_, Some(q))) => Some(Stop::Asked(q)),
        Some((why, None)) => {
            let (pr, n) = (m.pr, work.number);
            let why = format!("#{pr} {why}. Push a smaller change, then report #{n} done.");
            tx.execute(
                "UPDATE tasks SET finish = 'blocked', finish_note = ?2 WHERE number = ?1",
                params![n, why],
            )?;
            Some(Stop::Rested(why))
        }
    };
    let outcome = match stop {
        None => "under",
        Some(Stop::Asked(_)) => "asked",
        Some(Stop::Rested(_)) => "rested",
    };
    Event::point("gathr.change.report")
        .trace(trace)
        .chat(&m.chat)
        .task(work.number)
        .ok()
        .attr("gathr.task.id", work.number)
        .attr("pr", m.pr)
        .attr("head", m.head.as_str())
        .attr("counted", m.count.added)
        .attr("line", m.line)
        .attr("line_of", m.line_of)
        .attr("outcome", outcome)
        .attr("duration_ms", ms)
        .insert(tx)?;
    Ok((stop, starts))
}

/// Gathr's question `q` asked at a report has ended: its task hears nothing, and the work it holds
/// settles on what else it has open. The work, for one asked at a report.
pub(super) fn freed(
    tx: &Transaction,
    q: i64,
    trace: &str,
) -> Result<(Option<i64>, Vec<Start>), Error> {
    let t = records::read(tx, q)?;
    if t.given_in.is_none() {
        return Ok((None, Vec::new()));
    }
    tx.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [q])?;
    let Some(work) = t.parent else {
        return Ok((None, Vec::new()));
    };
    Ok((Some(work), held::released(tx, work, trace)?))
}

/// "Let it go on" for a question asked at a report: the work settles, and its review is given.
pub(super) fn went_on(tx: &Transaction, q: i64, pr: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let (work, mut starts) = freed(tx, q, trace)?;
    if let Some(work) = work {
        let w = records::read(tx, work)?;
        if let Some(how) = w.report.as_ref().map(|r| r.how) {
            starts.extend(review::give(tx, &w, (how, pr), None, trace)?.1);
        }
    }
    Ok(starts)
}

/// `task` was cancelled (`tasks::cancel_in`). Gathr's question asked at a report, cancelled by the
/// person, rests its PR at this head and blocks the work, whose giver hears; a work task cancelled
/// takes its open question with it.
pub(crate) fn cancelled(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    if let Some(r) = read(tx, "question = ?1 AND landed_ts IS NULL", [task])? {
        let q = records::read(tx, task)?;
        if q.given_in.is_none() {
            return Ok(Vec::new());
        }
        tx.execute(
            "UPDATE changes SET sent_back_task = ?3 WHERE project_id = ?1 AND pr = ?2",
            params![r.project, r.pr, q.parent],
        )?;
        let event = Event::point("gathr.change.answered").attr("pick", "cancelled");
        record_with(tx, &r, event, Some(task), trace)?;
        let Some(work) = q.parent else {
            return Ok(Vec::new());
        };
        if matches!(
            records::state_of(tx, work)?,
            None | Some(State::Done | State::Cancelled)
        ) {
            return Ok(Vec::new());
        }
        let pr = r.pr;
        let why = format!(
            "The person cancelled #{task}, Gathr's question about #{pr}'s size, so #{pr} doesn't land at \
             this head. Narrow #{work}'s ask and restart it, or cancel it."
        );
        records::set(tx, work, State::Blocked, Some(&why), trace)?;
        return tasks::followed(tx, work, trace);
    }
    let open: Option<i64> = tx
        .query_row(
            "SELECT c.question FROM changes c JOIN tasks q ON q.number = c.question
             WHERE q.parent = ?1 AND q.state = 'ready' AND q.given_in IS NOT NULL AND c.landed_ts IS NULL",
            [task],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(q) = open
        && let Some(r) = read(tx, "question = ?1", [q])?
    {
        tasks::cancelled_with(tx, q, &format!("#{task} was cancelled"), trace)?;
        tx.execute("UPDATE tasks SET heard = 1 WHERE number = ?1", [q])?;
        tx.execute(
            "UPDATE changes SET question = NULL WHERE project_id = ?1 AND pr = ?2",
            params![r.project, r.pr],
        )?;
        record(tx, &r, "gathr.change.withdrawn", Some(q), trace)?;
    }
    Ok(Vec::new())
}
