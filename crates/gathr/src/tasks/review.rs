//! Gathr gives the review task (0060): a work task's done report that first names its PR gives the
//! chat's Reviewer that PR's review, once per PR, as the builder would give it. A second review of
//! a PR with an open one is refused.

use rusqlite::{OptionalExtension, Transaction, params};

use super::{Brief, Finish, Give, Kind, Start, Task, give_in, refused, words};
use crate::Error;
use crate::event::Event;
use crate::team::{self, Role};

/// The review Gathr gave for a report: its number and its Reviewer's name.
pub type Given = Option<(i64, String)>;

/// Work `t`, as read before its report, now first names `pr`: the chat's Reviewer gets its review
/// when the report is done, no review covers the PR, and the Reviewer isn't the builder. Each
/// outcome is an event; a refused give fails nothing, as Gathr's fix (0053).
pub(super) fn give(
    tx: &Transaction,
    t: &Task,
    (how, pr): (Finish, i64),
    turn: Option<&str>,
    trace: &str,
) -> Result<(Given, Vec<Start>), Error> {
    let outcome = |said: &str, review: Option<i64>| {
        let event = Event::point("gathr.review.give").trace(trace).chat(&t.chat);
        let event = event.task(t.number).ok().attr("gathr.review.outcome", said);
        let event = event.attr("gathr.review.pr", pr);
        match review {
            Some(review) => event.attr("gathr.task.review", review),
            None => event,
        }
        .insert(tx)
    };
    // A review after a left or blocked report would wait on a done nothing asks for, or be
    // blocked at once, and hide the line that wakes the lead (0060).
    if how != Finish::Done {
        outcome("not_done", None)?;
        return Ok((None, Vec::new()));
    }
    let members = team::members(tx, &t.chat)?;
    let Some(reviewer) = members.into_iter().find(|m| m.role == Role::Reviewer) else {
        outcome("no_reviewer", None)?;
        return Ok((None, Vec::new()));
    };
    if t.owner.as_deref() == Some(reviewer.agent.as_str()) {
        outcome("own_review", None)?;
        return Ok((None, Vec::new()));
    }
    if let Some(review) = covered(tx, &t.chat, pr, t.number)? {
        outcome("covered", Some(review))?;
        return Ok((None, Vec::new()));
    }
    let given_in = given_in(tx, t.number, turn)?;
    let what = format!("Review #{pr}");
    // After the work, so it starts once the work is held only on its merge; in passing, as with
    // `after` a review that held the work would wait on it while it waited on the review.
    let g = Give {
        chat: &t.chat,
        giver: t.owner.as_deref(),
        owner: &reviewer.agent,
        given_in: given_in.as_deref(),
        parent: Some(t.number),
        kind: Kind::Review,
        what: &what,
        brief: Brief::default(),
        after: &[t.number],
        pr: Some(pr),
        in_passing: true,
    };
    match give_in(tx, &g, trace) {
        Ok((review, starts)) => {
            outcome("given", Some(review.number))?;
            Ok((Some((review.number, reviewer.name)), starts))
        }
        Err(Error::Refused(why)) => {
            outcome(&format!("refused: {why}"), None)?;
            Ok((None, Vec::new()))
        }
        Err(e) => Err(e),
    }
}

/// The turn the review is given in: the report's when it is on the work, else the work's newest,
/// else the report's. Given in a turn on another task, its end would wake the builder there.
fn given_in(tx: &Transaction, work: i64, turn: Option<&str>) -> Result<Option<String>, Error> {
    if let Some(turn) = turn {
        let on: Option<Option<i64>> = tx
            .query_row("SELECT task FROM turns WHERE id = ?1", [turn], |r| r.get(0))
            .optional()?;
        if on.flatten() == Some(work) {
            return Ok(Some(turn.to_string()));
        }
    }
    let newest: Option<String> = tx
        .query_row(
            "SELECT id FROM turns WHERE task = ?1 ORDER BY rowid DESC LIMIT 1",
            [work],
            |r| r.get(0),
        )
        .optional()?;
    Ok(newest.or(turn.map(String::from)))
}

/// The newest review in the chat, not cancelled, that covers the PR: one that names it, or one
/// with no PR tied to the work. A second look happens on that review (0053).
fn covered(tx: &Transaction, chat: &str, pr: i64, work: i64) -> Result<Option<i64>, Error> {
    Ok(tx
        .query_row(
            "SELECT r.number FROM tasks r
             WHERE r.chat_id = ?1 AND r.kind = 'review' AND r.state != 'cancelled'
               AND (r.pr = ?2
                    OR (r.pr IS NULL AND (r.parent = ?3
                        OR EXISTS (SELECT 1 FROM task_after a WHERE a.task = r.number AND a.after_task = ?3))))
             ORDER BY r.number DESC LIMIT 1",
            params![chat, pr, work],
            |r| r.get(0),
        )
        .optional()?)
}

/// A review given by hand for a PR with an open review is refused, naming that review and its
/// giver, who may cancel it.
pub(super) fn refuse_second(tx: &Transaction, g: &Give) -> Result<(), Error> {
    let (Kind::Review, Some(pr)) = (g.kind, g.pr) else {
        return Ok(());
    };
    let open: Option<(i64, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT number, owner, giver FROM tasks WHERE chat_id = ?1 AND kind = 'review' AND pr = ?2
             AND state IN ('waiting', 'ready', 'working', 'held') ORDER BY number LIMIT 1",
            params![g.chat, pr],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((n, owner, giver)) = open else {
        return Ok(());
    };
    let owner = words::owner_name(tx, owner.as_deref())?;
    let giver = words::owner_name(tx, giver.as_deref())?;
    Err(refused(format!(
        "PR #{pr} already has an open review: #{n}, {owner}'s, given by {giver}. \
         Give one once #{n} has ended or its giver cancels it."
    )))
}
