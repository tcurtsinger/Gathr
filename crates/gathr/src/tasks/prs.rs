//! A task's PR, as Gathr's look last saw it: merged settles the task, closed cancels it, and a
//! review is out of date once the PR has moved past its commit by more than merges of the base.

use rusqlite::{Connection, params};

use super::records::{read, set};
use super::{Finish, Kind, Start, State, cascade, end_waits, ready, turn_due};
use crate::Error;

/// What a look found for a review: the commit reviewed that it judged, and whether a review of it
/// still covers the PR's head (nothing after it but clean merges of the base, G165).
pub type Seen<'a> = Option<(&'a str, bool)>;

/// A task's PR has been looked at: merged settles work reported done and any review, closed
/// cancels it (G67, G44). `seen` is the look's verdict on a review, when it asked git. A review
/// whose PR moved past its commit is out of date (G73).
pub fn pr_looked(
    conn: &mut Connection,
    task: i64,
    pr_state: &str,
    head: Option<&str>,
    seen: Seen,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let t = read(&tx, task)?;
    if let (Some(reviewed), Some(head)) = (&t.reviewed, head) {
        // The look's verdict counts only for the commit reviewed now: a review recorded again
        // while git was asked is judged at the next look. With no verdict, any head that isn't
        // the commit reviewed is past it (G73).
        let judged = seen.filter(|(of, _)| of == reviewed);
        let outdated = match judged {
            Some((_, covers)) => !covers,
            None => !head.starts_with(reviewed.as_str()) && !reviewed.starts_with(head),
        };
        let pair = judged.map(|_| format!("{reviewed}:{head}"));
        tx.execute(
            "UPDATE tasks SET outdated = ?2, judged = ?3 WHERE number = ?1",
            params![task, outdated, pair],
        )?;
    }
    if let Some(head) = head {
        tx.execute(
            "UPDATE tasks SET pr_head = ?2 WHERE number = ?1",
            params![task, head],
        )?;
    }
    let pr = t.pr.unwrap_or_default();
    let pr_state = pr_state.to_ascii_uppercase();
    if matches!(pr_state.as_str(), "MERGED" | "CLOSED") {
        tx.execute("UPDATE tasks SET pr_closed = 1 WHERE number = ?1", [task])?;
    }
    let mut starts = Vec::new();
    if !t.state.settled() || t.state == State::Blocked {
        match pr_state.as_str() {
            "MERGED" => {
                end_waits(&tx, task, "its PR merged")?;
                let said = t.report.as_ref().map(|r| r.how);
                if t.kind == Kind::Work && said != Some(Finish::Done) {
                    // Work is done by its merge only if its owner reported it done (0047, live
                    // #148). Else the owner is woken with what it last said, and this is the one
                    // ask; with a turn of its own running or queued, that turn's end settles it.
                    if !turn_due(&tx, task)? {
                        tx.execute("UPDATE tasks SET asks = 1 WHERE number = ?1", [task])?;
                        let note = t.report.as_ref().and_then(|r| r.note.as_deref());
                        let stands = match (said, note) {
                            (Some(Finish::Left), Some(note)) => {
                                format!("you left this work: {note}")
                            }
                            (Some(Finish::Blocked), Some(note)) => {
                                format!("you reported this task blocked: {note}")
                            }
                            _ => "this task has no report that it's done".to_string(),
                        };
                        let carry = format!("#{pr} merged, and {stands}");
                        starts.push(ready(&tx, task, Some(&carry))?);
                    }
                } else {
                    let why = format!("#{pr} merged");
                    set(&tx, task, State::Done, Some(&why), trace)?;
                    starts = cascade(&tx, task, trace)?;
                }
            }
            "CLOSED" => {
                let why = format!("#{pr} was closed without merging");
                set(&tx, task, State::Cancelled, Some(&why), trace)?;
                end_waits(&tx, task, "its PR was closed")?;
                starts = cascade(&tx, task, trace)?;
            }
            _ => {}
        }
    }
    tx.commit()?;
    Ok(starts)
}

/// Tasks whose open PR the service looks at: open tasks, and done reviews while their PR may move
/// past the commit reviewed (G73). Number, chat, owner and PR.
pub fn with_prs(conn: &Connection) -> Result<Vec<(i64, String, String, i64)>, Error> {
    let mut stmt = conn.prepare(
        "SELECT number, chat_id, owner, pr FROM tasks
         WHERE pr IS NOT NULL AND pr_closed = 0 AND owner IS NOT NULL
         AND (state NOT IN ('done', 'cancelled') OR (state = 'done' AND kind = 'review'))",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}
