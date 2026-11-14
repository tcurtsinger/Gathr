//! A review asks both questions (0053): what fails against the ask, and what in the change the ask
//! doesn't need. Its notes are kept at the commit reviewed; wrong or unasked ones give the PR's
//! builder a fix, and a done review's suggestions go to the lead with its done line.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use super::{Give, Kind, Start, Task, give_in, words};
use crate::Error;
use crate::event::Event;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteKind {
    Wrong,
    Unasked,
    Suggestion,
}

impl NoteKind {
    pub fn named(text: &str) -> Option<NoteKind> {
        match text {
            "wrong" => Some(NoteKind::Wrong),
            "unasked" => Some(NoteKind::Unasked),
            "suggestion" => Some(NoteKind::Suggestion),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            NoteKind::Wrong => "wrong",
            NoteKind::Unasked => "unasked",
            NoteKind::Suggestion => "suggestion",
        }
    }

    fn label(self) -> &'static str {
        match self {
            NoteKind::Wrong => "Wrong",
            NoteKind::Unasked => "Unasked",
            NoteKind::Suggestion => "Suggestion",
        }
    }
}

/// A note as its reviewer gives it.
#[derive(Debug, Clone, Copy)]
pub struct NoteSaid<'a> {
    pub kind: NoteKind,
    pub file: Option<&'a str>,
    pub says: &'a str,
}

/// A note as a review's newest report keeps it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub kind: NoteKind,
    pub file: Option<String>,
    pub says: String,
}

impl Note {
    /// "<Kind>: <file>: <says>", or without the file when it names none.
    fn line(&self) -> String {
        match &self.file {
            Some(file) => format!("{}: {file}: {}", self.kind.label(), self.says),
            None => format!("{}: {}", self.kind.label(), self.says),
        }
    }
}

/// Whether any note asks for work: a wrong or an unasked one.
pub(super) fn ask_for_work(notes: &[NoteSaid]) -> bool {
    notes.iter().any(|n| n.kind != NoteKind::Suggestion)
}

/// Keeps a report's notes at the commit reviewed, as of the report's own time, with its event.
pub(super) fn record(
    tx: &Transaction,
    review: &Task,
    reviewed: &str,
    notes: &[NoteSaid],
    at_us: i64,
    nothing_unasked: bool,
    trace: &str,
) -> Result<(), Error> {
    let mut insert = tx.prepare_cached(
        "INSERT INTO review_notes (task, reviewed, kind, file, says, created_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for n in notes {
        let file = n.file.map(str::trim).filter(|f| !f.is_empty());
        let row = params![
            review.number,
            reviewed,
            n.kind.as_str(),
            file,
            n.says.trim(),
            at_us
        ];
        insert.execute(row)?;
    }
    let count = |kind: NoteKind| notes.iter().filter(|n| n.kind == kind).count();
    Event::point("gathr.review.notes")
        .trace(trace)
        .chat(&review.chat)
        .task(review.number)
        .ok()
        .attr("gathr.review.commit", reviewed)
        .attr("gathr.review.wrong", count(NoteKind::Wrong))
        .attr("gathr.review.unasked", count(NoteKind::Unasked))
        .attr("gathr.review.suggestions", count(NoteKind::Suggestion))
        .attr("gathr.review.nothing_unasked", nothing_unasked)
        .insert(tx)?;
    Ok(())
}

/// The notes of the review's newest report: those kept as of its time.
pub(super) fn of(conn: &Connection, task: i64) -> Result<Vec<Note>, Error> {
    Ok(newest(conn, task, false)?
        .into_iter()
        .map(|(_, n)| n)
        .collect())
}

/// The newest report's notes with their rows; with `work`, only the wrong and unasked ones.
fn newest(conn: &Connection, task: i64, work: bool) -> Result<Vec<(i64, Note)>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT n.id, n.kind, n.file, n.says FROM review_notes n JOIN tasks t ON t.number = n.task
         WHERE n.task = ?1 AND n.created_ts = t.finish_ts AND (NOT ?2 OR n.kind != 'suggestion')
         ORDER BY n.id",
    )?;
    let rows = stmt.query_map(params![task, work], |r| {
        let kind: String = r.get(1)?;
        let note = Note {
            kind: NoteKind::named(&kind).unwrap_or(NoteKind::Suggestion),
            file: r.get(2)?,
            says: r.get(3)?,
        };
        Ok((r.get(0)?, note))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The fix Gathr gave, by number and its owner's name, and its start.
pub(super) type Fixed = (Option<(i64, String)>, Vec<Start>);

/// A review settles left with nothing open: its newest report's wrong and unasked notes go to the
/// PR's builder as a fix in passing, given by the reviewer (0053). None is given when the PR has no
/// work task, when the reviewer gave one on it in that turn, or for unasked notes alone once a fix
/// carried unasked notes on that PR; each is an event. A refused give fails nothing. The fix's
/// number and owner, and its start, as `Fixed`.
pub(super) fn fix(tx: &Transaction, review: &Task, trace: &str) -> Result<Fixed, Error> {
    let notes = newest(tx, review.number, true)?;
    if notes.is_empty() {
        return Ok((None, Vec::new()));
    }
    // As the report left it: a report made outside a turn settles with the task read before it.
    let review = &super::records::read(tx, review.number)?;
    let outcome = |said: &str, fix: Option<i64>| {
        Event::point("gathr.review.fix")
            .trace(trace)
            .chat(&review.chat)
            .task(review.number)
            .ok()
            .attr("gathr.review.outcome", said)
            .attr("gathr.task.fix", fix)
            .insert(tx)
    };
    let (Some(pr), Some(reviewer)) = (review.pr, review.owner.as_deref()) else {
        outcome("no_work_task", None)?;
        return Ok((None, Vec::new()));
    };
    let Some((builder, _)) = work_on(tx, &review.chat, pr)? else {
        outcome("no_work_task", None)?;
        return Ok((None, Vec::new()));
    };
    let turn: Option<String> = tx
        .query_row(
            "SELECT id FROM turns WHERE task = ?1 AND status IN ('done', 'failed', 'stopped')
             ORDER BY coalesce(ended_ts, 0) DESC, rowid DESC LIMIT 1",
            [review.number],
            |r| r.get(0),
        )
        .optional()?;
    // Gathr's own fix is given by the reviewer too, in an earlier turn: never the reviewer's own.
    let gave_one: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND kind = 'work'
             AND giver = ?3 AND given_in = ?4
             AND number NOT IN (SELECT fix FROM review_notes WHERE fix IS NOT NULL))",
        params![review.chat, pr, reviewer, turn],
        |r| r.get(0),
    )?;
    if gave_one {
        outcome("reviewer_gave_one", None)?;
        return Ok((None, Vec::new()));
    }
    let wrong = notes.iter().any(|(_, n)| n.kind == NoteKind::Wrong);
    let asked: bool = tx.query_row(
        "SELECT EXISTS (SELECT 1 FROM review_notes n JOIN tasks r ON r.number = n.task
             WHERE n.kind = 'unasked' AND n.fix IS NOT NULL AND r.chat_id = ?1 AND r.pr = ?2)",
        params![review.chat, pr],
        |r| r.get(0),
    )?;
    if !wrong && asked {
        outcome("asked_once", None)?;
        return Ok((None, Vec::new()));
    }
    let reviewed = review.reviewed.as_deref().unwrap_or_default();
    let mut why = vec![format!(
        "Review #{} by {}, at {}, left these notes on PR #{pr}:",
        review.number,
        words::name_of(tx, reviewer)?,
        short(reviewed)
    )];
    why.extend(notes.iter().map(|(_, n)| format!("- {}", n.line())));
    why.push(format!(
        "Make what is wrong work and take out what the ask doesn't need, on PR #{pr}. Then report this task done."
    ));
    let (what, why) = (format!("Fix the review of #{pr}"), why.join("\n"));
    let g = Give {
        chat: &review.chat,
        giver: Some(reviewer),
        owner: &builder,
        given_in: turn.as_deref(),
        parent: Some(review.number),
        kind: Kind::Work,
        what: &what,
        why: Some(&why),
        after: &[],
        pr: Some(pr),
        in_passing: true,
    };
    let (given, starts) = match give_in(tx, &g, trace) {
        Ok(given) => given,
        Err(Error::Refused(why)) => {
            outcome(&format!("refused: {why}"), None)?;
            return Ok((None, Vec::new()));
        }
        Err(e) => return Err(e),
    };
    for (id, _) in &notes {
        tx.execute(
            "UPDATE review_notes SET fix = ?2 WHERE id = ?1",
            [*id, given.number],
        )?;
    }
    outcome("given", Some(given.number))?;
    Ok((Some((given.number, words::name_of(tx, &builder)?)), starts))
}

/// The earliest work task in the chat on `pr` that isn't cancelled: its owner, and the task.
fn work_on(conn: &Connection, chat: &str, pr: i64) -> Result<Option<(String, i64)>, Error> {
    Ok(conn
        .query_row(
            "SELECT owner, number FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND kind = 'work'
             AND state != 'cancelled' AND owner IS NOT NULL ORDER BY number LIMIT 1",
            params![chat, pr],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}

/// A done review's reason: its note, then a line for each suggestion of its newest report.
pub(super) fn done_reason(
    conn: &Connection,
    task: i64,
    note: Option<&str>,
) -> Result<Option<String>, Error> {
    let mut lines: Vec<String> = note.map(String::from).into_iter().collect();
    lines.extend(of(conn, task)?.iter().map(Note::line));
    Ok((!lines.is_empty()).then(|| lines.join("\n")))
}

/// What a review's brief says of what it reviews: its PR's head as last seen, the task it answers,
/// and the unasked question.
pub(super) fn brief_lines(conn: &Connection, review: &Task) -> Result<Vec<String>, Error> {
    let ask =
        "Say what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true.";
    let Some(pr) = review.pr else {
        return Ok(vec![ask.to_string()]);
    };
    let head: Option<String> = conn
        .query_row(
            "SELECT pr_head FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND pr_head IS NOT NULL
             ORDER BY number DESC LIMIT 1",
            params![review.chat, pr],
            |r| r.get(0),
        )
        .optional()?;
    let mut lines = vec![match head {
        Some(head) => format!("PR #{pr}, at {} as last seen.", short(&head)),
        None => format!("PR #{pr}. Its head isn't seen yet."),
    }];
    lines.push(match work_on(conn, &review.chat, pr)? {
        Some((_, number)) => {
            let work = super::records::read(conn, number)?;
            let giver = words::owner_name(conn, work.giver.as_deref())?;
            let why = work.why.as_deref().map_or(String::new(), |why| {
                format!(" Why: {}", words::sentence(why))
            });
            let what = words::sentence(&work.what);
            format!("It answers task #{number} from {giver}: {what}{why}")
        }
        None => "It answers no task in this chat: this review's own ask, above, is what was asked."
            .to_string(),
    });
    lines.push(ask.to_string());
    Ok(lines)
}

/// A commit as people read it: its first 7 characters.
fn short(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}
