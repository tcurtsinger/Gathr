//! A pull request's size as Gathr measured it at landing, the person's question when it goes past
//! its project's line, and once it merges, the record of what landed (0054). A row per project and
//! PR; `question` is the person's task while it's asked about.

use rusqlite::{Connection, OptionalExtension, Params, Transaction, params};
use serde_json::json;

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::person::Choice;
use crate::tasks::{self, Start, State, records};

pub mod count;
pub mod report;

use count::{Count, grouped};

const GO_ON: &str = "Let it go on";
const SEND_BACK: &str = "Send it back";

/// A PR's count at one head, and the project's line then.
#[derive(Debug, Clone)]
pub struct Measure {
    pub project: String,
    pub chat: String,
    pub pr: i64,
    pub head: String,
    pub count: Count,
    pub line: i64,
    /// Which line it is: `project`, `fix` or `part` (0064).
    pub line_of: &'static str,
    /// What measured it: `landing`, `merge` or `report`.
    pub by: &'static str,
}

/// Where a PR stands with the person, while it isn't landed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// Gathr's question to the person is open.
    Asked(i64),
    /// The person sent its head back.
    SentBack,
    /// The person cancelled the question at this head.
    Cancelled(i64),
}

/// A row as kept.
struct Row {
    project: String,
    chat: String,
    pr: i64,
    head: String,
    count: Count,
    line: i64,
    question: Option<i64>,
    let_through: Option<i64>,
    sent_back_head: Option<String>,
    line_of: String,
}

/// The rows' key.
const KEY: &str = "project_id = ?1 AND pr = ?2";

fn read(conn: &Connection, filter: &str, p: impl Params) -> Result<Option<Row>, Error> {
    let sql = format!(
        "SELECT project_id, chat_id, pr, head, counted, removed, top, line, question, let_through,
                sent_back_head, line_of FROM changes WHERE {filter}"
    );
    let found = conn
        .query_row(&sql, p, |r| {
            let top: String = r.get(6)?;
            Ok(Row {
                project: r.get(0)?,
                chat: r.get(1)?,
                pr: r.get(2)?,
                head: r.get(3)?,
                count: Count {
                    added: r.get(4)?,
                    removed: r.get(5)?,
                    top: serde_json::from_str(&top).unwrap_or_default(),
                },
                line: r.get(7)?,
                question: r.get(8)?,
                let_through: r.get(9)?,
                sent_back_head: r.get(10)?,
                line_of: r.get(11)?,
            })
        })
        .optional()?;
    Ok(found)
}

/// The PR's head, line and count as last measured: at the same head and line, landing needs no
/// `git check-attr`.
pub fn stored(
    conn: &Connection,
    project: &str,
    pr: i64,
) -> Result<Option<(String, i64, Count)>, Error> {
    let row = read(conn, KEY, params![project, pr])?;
    Ok(row.map(|r| (r.head, r.line, r.count)))
}

/// Keeps the measure, written only at a new head or line, and says what the PR rests on, if
/// anything: a head the person sent back, a question open or cancelled at this head, or the
/// question it files now. A question is withdrawn once the PR isn't over. One transaction, with
/// the turns it starts.
pub fn measured(
    conn: &mut Connection,
    m: &Measure,
    trace: &str,
) -> Result<(Option<String>, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let row = kept(&tx, m, trace)?;
    let mut starts = Vec::new();
    let rests = verdict(&tx, &row, None, &mut starts, trace)?;
    tx.commit()?;
    Ok((rests.map(|(why, _)| why), starts))
}

/// The measure written at a new head or line, and the row as it stands then.
fn kept(tx: &Transaction, m: &Measure, trace: &str) -> Result<Row, Error> {
    let key = params![m.project, m.pr];
    let was = read(tx, KEY, key)?;
    if was.is_none_or(|r| r.head != m.head || r.line != m.line || r.line_of != m.line_of) {
        write(tx, m, trace)?;
    }
    read(tx, KEY, key)?.ok_or(Error::Db(rusqlite::Error::QueryReturnedNoRows))
}

fn write(tx: &Transaction, m: &Measure, trace: &str) -> Result<(), Error> {
    // A new head may be asked about again, unless its question is still open.
    tx.execute(
        "INSERT INTO changes (project_id, pr, chat_id, head, counted, removed, top, line, measured_ts, line_of)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT (project_id, pr) DO UPDATE SET head = excluded.head, counted = excluded.counted,
             removed = excluded.removed, top = excluded.top, line = excluded.line,
             measured_ts = excluded.measured_ts, line_of = excluded.line_of,
             question = CASE WHEN changes.head != excluded.head AND NOT EXISTS
                 (SELECT 1 FROM tasks WHERE number = changes.question AND state = 'ready')
                 THEN NULL ELSE changes.question END",
        params![
            m.project,
            m.pr,
            m.chat,
            m.head,
            m.count.added,
            m.count.removed,
            json!(m.count.top).to_string(),
            m.line,
            now_us(),
            m.line_of
        ],
    )?;
    Event::point("gathr.change.measured")
        .trace(trace)
        .chat(&m.chat)
        .ok()
        .attr("gathr.project.id", m.project.as_str())
        .attr("pr", m.pr)
        .attr("head", m.head.as_str())
        .attr("counted", m.count.added)
        .attr("removed", m.count.removed)
        .attr("line", m.line)
        .attr("line_of", m.line_of)
        .attr("by", m.by)
        .insert(tx)?;
    Ok(())
}

/// What the PR rests on, in words, with the question open when it waits on one. A question filed
/// now is given in `given_in` (a report's, 0064); what a withdrawn one releases joins `starts`.
fn verdict(
    tx: &Transaction,
    r: &Row,
    given_in: Option<&str>,
    starts: &mut Vec<Start>,
    trace: &str,
) -> Result<Option<(String, Option<i64>)>, Error> {
    if r.sent_back_head.as_deref() == Some(r.head.as_str()) {
        let why = format!("was sent back by the person at {}", short(&r.head));
        return Ok(Some((why, None)));
    }
    let asked = match r.question {
        Some(q) => records::state_of(tx, q)?.map(|s| (q, s)),
        None => None,
    };
    if !count::over(r.count.added, r.line, r.let_through) {
        if let Some((q, State::Ready)) = asked {
            starts.extend(withdraw(tx, r, q, trace)?);
        }
        return Ok(None);
    }
    let q = match asked {
        Some((q, State::Ready)) => q,
        Some((q, State::Cancelled)) => {
            let why = format!("waits on a new head: the person cancelled #{q}");
            return Ok(Some((why, None)));
        }
        _ => ask(tx, r, given_in, trace)?,
    };
    Ok(Some((
        format!("waits on the person's answer to #{q}"),
        Some(q),
    )))
}

/// The line a row was measured against, in words: "this project's line".
fn line_name(r: &Row) -> &'static str {
    match r.line_of.as_str() {
        "fix" => "a Fix's line",
        "part" => "its part's line",
        _ => "this project's line",
    }
}

/// The PR is no longer over: the person isn't asked about lines that are gone.
fn withdraw(tx: &Transaction, r: &Row, q: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let why = format!(
        "#{} is now +{}, under {} of {}",
        r.pr,
        grouped(r.count.added),
        line_name(r),
        grouped(r.line)
    );
    tasks::cancelled_with(tx, q, &why, trace)?;
    tx.execute(
        "UPDATE changes SET question = NULL WHERE project_id = ?1 AND pr = ?2",
        params![r.project, r.pr],
    )?;
    record(tx, r, "gathr.change.withdrawn", Some(q), trace)?;
    Ok(report::freed(tx, q, trace)?.1)
}

/// Where the row's line comes from, in a sentence; `work` is the task the PR answers.
fn set_by(r: &Row, work: Option<&tasks::Task>) -> String {
    let line = grouped(r.line);
    match r.line_of.as_str() {
        "fix" if r.line == report::FIX_FLOOR => {
            format!("A Fix's line is {line}, the least it can be.")
        }
        "fix" => format!(
            "A Fix's line here is {line}: the most any of this project's last 50 Fixes added."
        ),
        "part" => match work.and_then(|w| w.size_lines) {
            Some(size) => format!("Its part said {} lines; its line is {line}.", grouped(size)),
            None => format!("Its part's line is {line}."),
        },
        _ if r.line == count::FLOOR => {
            format!("This project's line is {line}, the least it can be.")
        }
        _ => format!("This project's line is {line}: the most any of its last 50 changes added."),
    }
}

/// Files Gathr's question to the person, with the numbers and the task the PR answers, given in
/// `given_in` when a report asks it: its number.
fn ask(tx: &Transaction, r: &Row, given_in: Option<&str>, trace: &str) -> Result<i64, Error> {
    let parent = tasks::notes::work_on(tx, &r.chat, r.pr)?.map(|(_, n)| n);
    let work = match parent {
        Some(n) => Some(records::read(tx, n)?),
        None => None,
    };
    let (pr, added, line) = (r.pr, grouped(r.count.added), grouped(r.line));
    let named = line_name(r);
    let what = format!("#{pr} adds {added} lines, over {named} of {line}: let it go on?");
    let set_by = set_by(r, work.as_ref());
    let removed = grouped(r.count.removed);
    let top: Vec<String> = (r.count.top.iter())
        .map(|(path, n)| format!("{path} +{}", grouped(*n)))
        .collect();
    let mut why = vec![
        format!("#{pr} adds {added} lines and removes {removed}. {set_by}"),
        format!("Largest: {}.", top.join(", ")),
    ];
    let sent_back = match work {
        Some(work) => {
            let n = work.number;
            let giver = tasks::owner_name(tx, work.giver.as_deref())?;
            let what = tasks::sentence(&work.what);
            why.push(format!("It answers task #{n} from {giver}: {what}"));
            format!("#{n} is blocked, and {giver} is told.")
        }
        None => {
            why.push("No task in this chat asks for it.".to_string());
            "It doesn't land at this head.".to_string()
        }
    };
    let choice = |label: &str, detail: String| Choice {
        label: label.to_string(),
        detail: Some(detail),
    };
    let go_on = match given_in {
        Some(_) => "Its review starts.",
        None => "It lands once its review and checks pass.",
    };
    let choices = [choice(GO_ON, go_on.into()), choice(SEND_BACK, sent_back)];
    let on = (r.chat.as_str(), pr, parent, given_in);
    let q = tasks::person::ask_as_gathr(tx, on, (&what, &why.join("\n")), &choices, trace)?;
    tx.execute(
        "UPDATE changes SET question = ?3, asked = counted WHERE project_id = ?1 AND pr = ?2",
        params![r.project, r.pr, q],
    )?;
    let event = Event::point("gathr.change.asked")
        .attr("gathr.task.parent", parent)
        .attr("line_of", r.line_of.as_str())
        .attr("gathr.turn.id", given_in);
    record_with(tx, r, event, Some(q), trace)?;
    Ok(q)
}

/// Where the chat's PR stands with the person, while it isn't landed.
pub fn standing(conn: &Connection, chat: &str, pr: i64) -> Result<Option<Standing>, Error> {
    let filter = "chat_id = ?1 AND pr = ?2 AND landed_ts IS NULL";
    let Some(r) = read(conn, filter, params![chat, pr])? else {
        return Ok(None);
    };
    let state = match r.question {
        Some(q) => records::state_of(conn, q)?,
        None => None,
    };
    Ok(match (r.question, state) {
        (Some(q), Some(State::Ready)) => Some(Standing::Asked(q)),
        _ if r.sent_back_head.as_deref() == Some(r.head.as_str()) => Some(Standing::SentBack),
        (Some(q), Some(State::Cancelled)) => Some(Standing::Cancelled(q)),
        _ => None,
    })
}

/// The person answered `task`, in `person::answer_as`'s transaction. "Let it go on" lets the count
/// asked about through, and asks again at once when the PR's count is past that already. "Send it
/// back" rests the PR at its head and blocks the work it answers, whose giver hears.
pub(crate) fn answered(
    tx: &Transaction,
    task: i64,
    pick: Option<&str>,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let filter = "question = ?1 AND landed_ts IS NULL";
    let Some(r) = read(tx, filter, [task])? else {
        return Ok(Vec::new());
    };
    let key = params![r.project, r.pr];
    let mut starts = Vec::new();
    let picked = match pick {
        Some(GO_ON) => {
            tx.execute(
                "UPDATE changes SET let_through = asked, question = NULL WHERE project_id = ?1 AND pr = ?2",
                key,
            )?;
            "let_through"
        }
        _ => {
            let parent = records::read(tx, task)?.parent;
            tx.execute(
                "UPDATE changes SET sent_back_head = head, sent_back_task = ?3, question = NULL
                 WHERE project_id = ?1 AND pr = ?2",
                params![r.project, r.pr, parent],
            )?;
            if let Some(work) = parent {
                starts.extend(send_back(tx, &r, work, trace)?);
            }
            "sent_back"
        }
    };
    let event = Event::point("gathr.change.answered").attr("pick", picked);
    record_with(tx, &r, event, Some(task), trace)?;
    if picked == "let_through" {
        starts.extend(report::went_on(tx, task, r.pr, trace)?);
        let now = read(tx, KEY, key)?;
        if let Some(now) = now.filter(|n| count::over(n.count.added, n.line, n.let_through)) {
            ask(tx, &now, None, trace)?;
        }
    }
    Ok(starts)
}

/// The work the PR answers is blocked with why, and its giver hears.
fn send_back(tx: &Transaction, r: &Row, work: i64, trace: &str) -> Result<Vec<Start>, Error> {
    if matches!(
        records::state_of(tx, work)?,
        None | Some(State::Done | State::Cancelled)
    ) {
        return Ok(Vec::new());
    }
    let why = format!(
        "The person sent #{} back as bigger than asked: +{} lines; {} is {}. \
         Narrow #{work}'s ask and restart it, or cancel it.",
        r.pr,
        grouped(r.count.added),
        line_name(r),
        grouped(r.line)
    );
    records::set(tx, work, State::Blocked, Some(&why), trace)?;
    tasks::followed(tx, work, trace)
}

/// The chat's PR merged or was closed (`state` as GitHub says it), at `head` when known; `ours` when
/// Gathr merged it. An open question is cancelled. Closed, the row goes; merged, it becomes the
/// record, once. One transaction, with the turns it starts.
pub fn ended(
    conn: &mut Connection,
    chat: &str,
    pr: i64,
    state: &str,
    head: Option<&str>,
    ours: bool,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let merged = match state.to_ascii_uppercase().as_str() {
        "MERGED" => true,
        "CLOSED" => false,
        _ => return Ok(Vec::new()),
    };
    let tx = conn.transaction()?;
    let filter = "chat_id = ?1 AND pr = ?2 AND landed_ts IS NULL";
    let Some(r) = read(&tx, filter, params![chat, pr])? else {
        return Ok(Vec::new());
    };
    let open = match r.question {
        Some(q) => (records::state_of(&tx, q)? == Some(State::Ready)).then_some(q),
        None => None,
    };
    let mut starts = Vec::new();
    if let Some(q) = open {
        let why = match merged {
            true => format!("#{pr} merged"),
            false => format!("#{pr} was closed"),
        };
        tasks::cancelled_with(&tx, q, &why, trace)?;
        starts = report::freed(&tx, q, trace)?.1;
    }
    if !merged {
        tx.execute(
            "DELETE FROM changes WHERE project_id = ?1 AND pr = ?2",
            params![r.project, r.pr],
        )?;
        record(&tx, &r, "gathr.change.closed", None, trace)?;
        tx.commit()?;
        return Ok(starts);
    }
    if open.is_some() {
        record(&tx, &r, "gathr.change.merged_unasked", open, trace)?;
    }
    let head = head.unwrap_or(&r.head);
    if r.sent_back_head.as_deref() == Some(head) {
        record(&tx, &r, "gathr.change.merged_sent_back", None, trace)?;
    }
    landed(&tx, &r, ours, trace)?;
    tx.commit()?;
    Ok(starts)
}

/// Gathr pushed the PR's merge: the merge GitHub shows next is Gathr's, whichever look sees it
/// first.
pub fn pushed(conn: &Connection, project: &str, pr: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE changes SET by_gathr = 1 WHERE project_id = ?1 AND pr = ?2 AND landed_ts IS NULL",
        params![project, pr],
    )?;
    Ok(())
}

/// Fills the record of a merged PR: who built and reviewed it on what, the unasked notes, the
/// commit first reviewed, whether Gathr merged it, and the size of the ask its work answered.
fn landed(tx: &Transaction, r: &Row, ours: bool, trace: &str) -> Result<(), Error> {
    let work = tasks::notes::work_on(tx, &r.chat, r.pr)?;
    let size: Option<String> = match &work {
        Some((_, n)) => tx
            .query_row(
                "SELECT a.size FROM tasks t JOIN asks a ON a.id = t.ask_id WHERE t.number = ?1",
                [n],
                |row| row.get(0),
            )
            .optional()?
            .flatten(),
        None => None,
    };
    let builder = work.map(|(owner, _)| owner);
    let reviewer: Option<String> = tx
        .query_row(
            "SELECT owner FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND kind = 'review'
             AND state != 'cancelled' ORDER BY number DESC LIMIT 1",
            params![r.chat, r.pr],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let (b_engine, b_model) = runs_on(tx, builder.as_deref())?;
    let (r_engine, r_model) = runs_on(tx, reviewer.as_deref())?;
    let (unasked, first_reviewed, by_gathr): (i64, Option<String>, bool) = tx.query_row(
        "SELECT (SELECT count(*) FROM review_notes n JOIN tasks t ON t.number = n.task
                 WHERE n.kind = 'unasked' AND t.chat_id = ?1 AND t.pr = ?2),
                (SELECT first_reviewed FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND kind = 'review'
                 AND first_reviewed IS NOT NULL ORDER BY number LIMIT 1),
                ?4 OR coalesce((SELECT by_gathr FROM changes WHERE project_id = ?3 AND pr = ?2), 0)",
        params![r.chat, r.pr, r.project, ours],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    tx.execute(
        "UPDATE changes SET builder_id = ?3, builder_engine = ?4, builder_model = ?5, reviewer_id = ?6,
             reviewer_engine = ?7, reviewer_model = ?8, unasked = ?9, first_reviewed = ?10, by_gathr = ?11,
             landed_ts = ?12, size = ?13
         WHERE project_id = ?1 AND pr = ?2",
        params![
            r.project,
            r.pr,
            builder,
            b_engine,
            b_model,
            reviewer,
            r_engine,
            r_model,
            unasked,
            first_reviewed,
            by_gathr,
            now_us(),
            size
        ],
    )?;
    let event = Event::point("gathr.change.landed")
        .attr("gathr.ask.size", size)
        .attr("gathr.change.builder", builder)
        .attr("gathr.change.builder_engine", b_engine)
        .attr("gathr.change.builder_model", b_model)
        .attr("gathr.change.reviewer", reviewer)
        .attr("gathr.change.reviewer_engine", r_engine)
        .attr("gathr.change.reviewer_model", r_model)
        .attr("unasked", unasked)
        .attr("first_reviewed", first_reviewed)
        .attr("by_gathr", by_gathr);
    record_with(tx, r, event, None, trace)
}

/// An agent's engine and model, as set now.
fn runs_on(
    conn: &Connection,
    agent: Option<&str>,
) -> Result<(Option<String>, Option<String>), Error> {
    let found = conn
        .query_row(
            "SELECT c.engine, a.model FROM agents a JOIN accounts c ON c.id = a.account_id WHERE a.id = ?1",
            [agent],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    Ok(found.map_or((None, None), |(engine, model)| (Some(engine), model)))
}

fn record(
    tx: &Transaction,
    r: &Row,
    name: &str,
    question: Option<i64>,
    trace: &str,
) -> Result<(), Error> {
    record_with(tx, r, Event::point(name), question, trace)
}

/// Records the event for the PR, with its count and line, on its question when there is one.
fn record_with(
    tx: &Transaction,
    r: &Row,
    event: Event,
    question: Option<i64>,
    trace: &str,
) -> Result<(), Error> {
    let event = event.trace(trace).chat(&r.chat).ok();
    let event = match question {
        Some(q) => event.task(q),
        None => event,
    };
    event
        .attr("gathr.project.id", r.project.as_str())
        .attr("pr", r.pr)
        .attr("head", r.head.as_str())
        .attr("counted", r.count.added)
        .attr("line", r.line)
        .insert(tx)?;
    Ok(())
}

/// A commit as people read it: its first 7 characters.
fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}
