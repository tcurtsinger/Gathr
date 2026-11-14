//! Gathr lands a pull request (0049): the reviews whose word lands one, what Gathr does about a PR
//! as a look saw it, and a record for each PR at each head of what it did there, so nothing is done
//! or said twice for one head: across looks, two triggers at once, and a restart.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::Start;

/// After Gathr asked GitHub to bring a PR up to date, how long its head may stay as it was before
/// its reviewer is told.
pub const UPDATE_SHOWS_US: i64 = 5 * 60 * 1_000_000;

/// The newest review of a PR in a project's chat that isn't cancelled, while the PR is open. An
/// older approval never lands a PR a newer review holds back, and a review in another chat is
/// another chat's.
#[derive(Debug, Clone, PartialEq)]
pub struct Review {
    pub chat: String,
    pub project: String,
    pub pr: i64,
    pub task: i64,
    pub reviewer: String,
    /// The commit reviewed, once the review is recorded done.
    pub reviewed: Option<String>,
    /// The reviewer signed off the PR's tests and check settings with this review.
    pub signed_off: bool,
}

/// The newest reviews of open PRs, in a project or in all, the one that came clean first first.
/// Never an archived chat's: archiving ended its work.
pub fn reviews(conn: &Connection, project: Option<&str>) -> Result<Vec<Review>, Error> {
    newest(conn, project, None)
}

/// The newest review of one PR in a chat.
pub fn review_of(conn: &Connection, chat: &str, pr: i64) -> Result<Option<Review>, Error> {
    Ok(newest(conn, None, Some((chat, pr)))?.into_iter().next())
}

fn newest(
    conn: &Connection,
    project: Option<&str>,
    one: Option<(&str, i64)>,
) -> Result<Vec<Review>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT t.chat_id, c.project_id, t.pr, t.number, t.owner,
                CASE WHEN t.review_done THEN t.reviewed END, t.signed_off
         FROM tasks t JOIN chats c ON c.id = t.chat_id
         WHERE t.kind = 'review' AND t.pr IS NOT NULL AND t.pr_closed = 0 AND t.owner IS NOT NULL
           AND t.state != 'cancelled' AND c.project_id IS NOT NULL AND c.archived_ts IS NULL
           AND t.number = (SELECT max(n.number) FROM tasks n WHERE n.chat_id = t.chat_id
                           AND n.kind = 'review' AND n.pr = t.pr AND n.state != 'cancelled')
           AND (?1 IS NULL OR c.project_id = ?1)
           AND (?2 IS NULL OR (t.chat_id = ?2 AND t.pr = ?3))
         ORDER BY t.finish_ts, t.number",
    )?;
    let (chat, pr) = one.unzip();
    let rows = stmt.query_map(params![project, chat, pr], |r| {
        Ok(Review {
            chat: r.get(0)?,
            project: r.get(1)?,
            pr: r.get(2)?,
            task: r.get(3)?,
            reviewer: r.get(4)?,
            reviewed: r.get(5)?,
            signed_off: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// How a PR whose newest review is done stands for landing, as one look saw it.
#[derive(Debug, Clone, PartialEq)]
pub enum Stand {
    /// Everything holds: it lands.
    Ready,
    /// Its base has commits it doesn't have.
    Behind,
    /// Something still running will move it, and what.
    Waits(String),
    /// Nothing lands it and nobody is told, and why: its review doesn't reach its head, which needs
    /// a review again as it did before Gathr landed PRs, or its head is in its base already.
    Rests(String),
    /// Nothing Gathr does will land it at this head, and why: its reviewer judges.
    Stops(String),
}

/// What Gathr has done for a PR at one head.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct Did {
    /// When it asked GitHub to bring the PR up to date.
    pub updated_us: Option<i64>,
    /// It told the PR's reviewer the PR can't land.
    pub told: bool,
    /// It pushed a merge GitHub didn't confirm.
    pub pushed: bool,
}

/// What Gathr does about a PR now.
#[derive(Debug, Clone, PartialEq)]
pub enum Act {
    Land,
    /// Bring it up to date with its base.
    Update,
    /// Tell its reviewer, once for this head, what stops it.
    Tell(String),
    /// Nothing: it moves by itself, and holds its place in line.
    Wait(String),
    /// Nothing more from Gathr at this head.
    Rest(String),
}

/// What Gathr does about a PR that stands so, given what it did at this head already. Only the
/// first PR in line is brought up to date: every merge puts the others behind again, and each
/// update runs their checks again.
pub fn act(stand: Stand, did: Did, first_in_line: bool, now_us: i64) -> Act {
    if did.pushed {
        return Act::Rest(
            "Gathr pushed its merge, and GitHub doesn't show it merged yet".to_string(),
        );
    }
    match (stand, did.updated_us) {
        (Stand::Ready, _) => Act::Land,
        (Stand::Behind, None) if first_in_line => Act::Update,
        (Stand::Behind, None) => Act::Wait(
            "it's behind its base, and Gathr brings it up to date in its turn".to_string(),
        ),
        (Stand::Behind, Some(asked)) if now_us - asked < UPDATE_SHOWS_US => Act::Wait(
            "Gathr asked GitHub to bring it up to date, and its new head isn't shown yet"
                .to_string(),
        ),
        (Stand::Behind, Some(asked)) => {
            let minutes = (now_us - asked) / 60_000_000;
            let why = format!(
                "Gathr asked GitHub to bring it up to date {minutes} minutes ago, and it's still behind its base"
            );
            if did.told {
                Act::Rest(why)
            } else {
                Act::Tell(why)
            }
        }
        (Stand::Waits(why), _) => Act::Wait(why),
        (Stand::Rests(why), _) => Act::Rest(why),
        (Stand::Stops(why), _) if did.told => Act::Rest(why),
        (Stand::Stops(why), _) => Act::Tell(why),
    }
}

pub fn did(conn: &Connection, chat: &str, pr: i64, head: &str) -> Result<Did, Error> {
    let found = conn
        .query_row(
            "SELECT updated_ts, told_ts IS NOT NULL, pushed_ts IS NOT NULL FROM landings
             WHERE chat_id = ?1 AND pr = ?2 AND head = ?3",
            params![chat, pr, head],
            |r| {
                Ok(Did {
                    updated_us: r.get(0)?,
                    told: r.get(1)?,
                    pushed: r.get(2)?,
                })
            },
        )
        .optional()?;
    Ok(found.unwrap_or_default())
}

/// What Gathr's look at the PR at this head saw, kept for whoever asks what the chat waits on
/// (0050): the words, and whether the PR moves by itself from here or is left to its reviewer.
pub fn looked(
    conn: &Connection,
    (chat, pr, head): (&str, i64, &str),
    saw: &str,
    moving: bool,
) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO landings (chat_id, pr, head, saw, moving, looked_ts) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT DO UPDATE SET saw = excluded.saw, moving = excluded.moving,
             looked_ts = excluded.looked_ts",
        params![chat, pr, head, saw, moving, now_us()],
    )?;
    Ok(())
}

/// What the newest look at the PR saw, whether it moves by itself, and when it looked.
pub fn last_look(
    conn: &Connection,
    chat: &str,
    pr: i64,
) -> Result<Option<(String, bool, i64)>, Error> {
    Ok(conn
        .query_row(
            "SELECT saw, moving, looked_ts FROM landings
             WHERE chat_id = ?1 AND pr = ?2 AND looked_ts IS NOT NULL ORDER BY looked_ts DESC LIMIT 1",
            params![chat, pr],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

/// When Gathr last asked GitHub to bring this PR up to date, at any head.
pub fn last_update(conn: &Connection, chat: &str, pr: i64) -> Result<Option<i64>, Error> {
    Ok(conn.query_row(
        "SELECT max(updated_ts) FROM landings WHERE chat_id = ?1 AND pr = ?2",
        params![chat, pr],
        |r| r.get(0),
    )?)
}

/// Gathr is asking GitHub to bring the PR up to date at this head: false when it has already.
pub fn updating(conn: &Connection, chat: &str, pr: i64, head: &str) -> Result<bool, Error> {
    mark(conn, (chat, pr, head), "updated_ts")
}

/// Gathr pushed the PR's merge at this head, and GitHub didn't confirm it.
pub fn pushed(conn: &Connection, chat: &str, pr: i64, head: &str) -> Result<bool, Error> {
    mark(conn, (chat, pr, head), "pushed_ts")
}

/// Marks one thing done for the PR at this head: whether it's this call that marked it.
fn mark(
    conn: &Connection,
    (chat, pr, head): (&str, i64, &str),
    column: &str,
) -> Result<bool, Error> {
    conn.execute(
        "INSERT INTO landings (chat_id, pr, head) VALUES (?1, ?2, ?3) ON CONFLICT DO NOTHING",
        params![chat, pr, head],
    )?;
    let set = format!(
        "UPDATE landings SET {column} = ?4 WHERE chat_id = ?1 AND pr = ?2 AND head = ?3 AND {column} IS NULL"
    );
    Ok(conn.execute(&set, params![chat, pr, head, now_us()])? == 1)
}

/// Tells the PR's reviewer what stops it at this head, once: a turn on no task with `wake` as its
/// ask, or nothing more when the reviewer is told where it stands (the answer to its own report).
/// Its done review stands either way (0047: a turn on a task would change it). None when the
/// reviewer was told for this head already.
///
/// That the PR is left to its reviewer is kept with the wake, in one write: kept first, the PR
/// would be nobody's until the wake was queued, and the chat's lead woken for it (0052).
pub fn tell(
    conn: &mut Connection,
    review: &Review,
    head: &str,
    why: &str,
    wake: Option<&str>,
    trace: &str,
) -> Result<Option<Start>, Error> {
    let tx = conn.transaction()?;
    let at = (review.chat.as_str(), review.pr, head);
    looked(&tx, at, &format!("left to its reviewer: {why}"), false)?;
    if !mark(&tx, at, "told_ts")? {
        tx.commit()?;
        return Ok(None);
    }
    let start = match wake {
        Some(text) => Some(Start::queue(
            &tx,
            (&review.chat, &review.reviewer),
            (None, None),
            text.to_string(),
        )?),
        None => None,
    };
    let event = Event::point("gathr.land.told").trace(trace).ok();
    let event = event.chat(&review.chat).attr("pr", review.pr);
    let event = event.attr("head", head).attr("why", why);
    event
        .attr("gen_ai.agent.id", review.reviewer.clone())
        .attr("woken", start.is_some())
        .insert(&tx)?;
    tx.commit()?;
    Ok(start)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_000 * 60_000_000;

    fn stops() -> Stand {
        Stand::Stops("CI failed".to_string())
    }

    #[test]
    fn each_thing_is_done_once_for_a_head_and_only_the_first_in_line_is_updated() {
        let fresh = Did::default();
        assert_eq!(act(Stand::Ready, fresh, false, NOW), Act::Land);
        assert_eq!(act(Stand::Behind, fresh, true, NOW), Act::Update);
        assert!(matches!(
            act(Stand::Behind, fresh, false, NOW),
            Act::Wait(_)
        ));
        assert_eq!(
            act(stops(), fresh, true, NOW),
            Act::Tell("CI failed".into())
        );

        // Told at this head: never again, though the approval stands and it lands once it can.
        let told = Did {
            told: true,
            ..fresh
        };
        assert_eq!(act(stops(), told, true, NOW), Act::Rest("CI failed".into()));
        assert_eq!(act(Stand::Ready, told, true, NOW), Act::Land);
        assert_eq!(act(Stand::Behind, told, true, NOW), Act::Update);

        // Asked for the update: not asked again, and its reviewer is told once it never shows.
        let just = Did {
            updated_us: Some(NOW - 60_000_000),
            ..fresh
        };
        assert!(matches!(act(Stand::Behind, just, true, NOW), Act::Wait(_)));
        let long = Did {
            updated_us: Some(NOW - UPDATE_SHOWS_US),
            ..fresh
        };
        assert!(matches!(act(Stand::Behind, long, true, NOW), Act::Tell(_)));
        let long_told = Did { told: true, ..long };
        assert!(matches!(
            act(Stand::Behind, long_told, true, NOW),
            Act::Rest(_)
        ));

        // A push GitHub didn't confirm is never made again.
        let pushed = Did {
            pushed: true,
            ..fresh
        };
        assert!(matches!(act(Stand::Ready, pushed, true, NOW), Act::Rest(_)));
        // A review that doesn't reach the head wakes nobody: it needs a review, as before.
        let moved = Stand::Rests("its head moved".into());
        assert!(matches!(act(moved, fresh, true, NOW), Act::Rest(_)));
    }
}
