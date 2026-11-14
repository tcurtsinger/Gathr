//! What a task is held on, and how a turn that ended well settles it. The giver's hear-back is
//! in `hear.rs`.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::records::OptionalTask;
use super::{Finish, Start, State, Task, cascade, read, ready, set, words};
use crate::Error;

/// Why a task is blocked once its owner was asked for a report and gave none.
const NO_REPORT: &str = "no report: two turns ended without one";

/// Tasks `t` its owner called done that are held only until their PR merges, read from the reason
/// `settle` writes. They count as done for the tasks `after` them: a review of that PR may be what
/// the merge waits on (0010). Never null, so `NOT` keeps every other task.
pub(super) const ONLY_ITS_MERGE: &str = "(t.state = 'held' AND t.pr IS NOT NULL
    AND t.reason IS 'done, waiting on #' || t.pr || ' to merge')";

/// One thing a task still has open. Its reason is these in words, and what a chat waits on names
/// them too (0050): one list, so the two never say different things.
#[derive(Debug, Clone, PartialEq)]
pub enum Hold {
    /// A wait of its own, by name.
    Wait(String),
    /// A task given in one of its turns that isn't settled, or that its giver hasn't heard of.
    Given {
        number: i64,
        owner: String,
        since_us: i64,
    },
    /// A command its engine runs in the background.
    Command(String),
    /// Its PR, until it merges.
    Merge(i64),
}

impl Hold {
    /// In a few words, as a task's reason says it. `ONLY_ITS_MERGE` reads the last of these.
    pub fn words(&self) -> String {
        match self {
            Hold::Wait(name) => name.clone(),
            Hold::Given { number, owner, .. } => format!("#{number} ({owner})"),
            Hold::Command(described) => format!("a background command ({described})"),
            Hold::Merge(pr) => format!("#{pr} to merge"),
        }
    }
}

/// What the task still has open, each in a few words.
pub(super) fn open_on(conn: &Connection, task: i64) -> Result<Vec<String>, Error> {
    Ok(holds(conn, task)?.iter().map(Hold::words).collect())
}

/// What the task still has open: its waits, the tasks given in its turns that aren't settled (but
/// for those given in passing, G164), its engine's background commands, and its PR.
pub fn holds(conn: &Connection, task: i64) -> Result<Vec<Hold>, Error> {
    let mut open = Vec::new();
    let mut waits = conn.prepare_cached(
        "SELECT kind, target, (SELECT count(*) FROM waits p WHERE p.part_of = w.id)
         FROM waits w WHERE w.task = ?1 AND w.state IN ('open', 'met') AND w.part_of IS NULL
         ORDER BY w.created_ts",
    )?;
    let rows = waits.query_map([task], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?,
            r.get::<_, i64>(2)?,
        ))
    })?;
    for row in rows {
        let (kind, target, parts) = row?;
        open.push(Hold::Wait(words::wait_name(
            conn,
            &kind,
            target.as_deref(),
            parts,
        )?));
    }
    // Settled isn't enough: the giver isn't done before it has heard back on them.
    let mut given = conn.prepare_cached(&format!(
        "SELECT c.number, c.owner, c.created_ts FROM tasks c JOIN turns t ON t.id = c.given_in
         WHERE t.task = ?1 AND c.in_passing = 0 AND (c.state NOT IN {} OR c.heard = 0)
         ORDER BY c.number",
        super::SETTLED
    ))?;
    let rows = given.query_map([task], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    for row in rows {
        let (number, owner, since_us): (i64, Option<String>, i64) = row?;
        open.push(Hold::Given {
            number,
            owner: words::owner_name(conn, owner.as_deref())?,
            since_us,
        });
    }
    let mut commands = conn.prepare_cached(
        "SELECT b.description FROM background b JOIN turns t ON t.id = b.turn_id
         WHERE t.task = ?1 ORDER BY b.started_ts",
    )?;
    for described in commands.query_map([task], |r| r.get::<_, String>(0))? {
        open.push(Hold::Command(described?));
    }
    // A review's PR is what it reviewed, not work its owner still owes.
    let pr: Option<Option<i64>> = conn
        .query_row(
            "SELECT pr FROM tasks WHERE number = ?1 AND kind = 'work' AND pr_closed = 0",
            [task],
            |r| r.get(0),
        )
        .optional()?;
    if let Some(pr) = pr.flatten() {
        open.push(Hold::Merge(pr));
    }
    Ok(open)
}

/// Where a task stands by its owner's report and what it still has open (0047), worked out when
/// a turn on it ends well and when a report arrives outside one: `blocked` as reported; `held`
/// while something is open, saying on what and what was reported (G75); with nothing open, `done`
/// if reported so, and stopped there if reported left, for its giver to hear. With nothing open and
/// no report its owner is asked once; silent again, the task is blocked and its giver hears. A turn
/// that ended is never read as work that ended.
pub fn settle(tx: &Transaction, t: &Task, trace: &str) -> Result<Vec<Start>, Error> {
    let task = t.number;
    let (finish, note, on, asks): (Option<String>, Option<String>, Option<i64>, i64) = tx
        .query_row(
            "SELECT finish, finish_note, blocked_on, asks FROM tasks WHERE number = ?1",
            [task],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )?;
    let finish = finish.as_deref().and_then(Finish::named);
    let asked = |n: i64| {
        tx.execute(
            "UPDATE tasks SET asks = ?2 WHERE number = ?1",
            params![task, n],
        )
    };
    if finish == Some(Finish::Blocked) {
        asked(0)?;
        let why = match (note, on) {
            (Some(note), _) => note,
            (None, Some(on)) => format!("blocked on #{on}"),
            (None, None) => "its owner can't go on".to_string(),
        };
        set(tx, task, State::Blocked, Some(&why), trace)?;
        return Ok(Vec::new());
    }
    let open = open_on(tx, task)?;
    if !open.is_empty() {
        asked(0)?;
        // `ONLY_ITS_MERGE` reads these words for a done task.
        let said = match finish {
            Some(Finish::Left) => "left, ",
            Some(_) => "done, ",
            None => "",
        };
        let why = format!("{said}waiting on {}", open.join(", "));
        set(tx, task, State::Held, Some(&why), trace)?;
        return Ok(Vec::new());
    }
    match finish {
        Some(Finish::Left) => {
            asked(0)?;
            let left = note.as_deref().unwrap_or("it isn't said what");
            let why = format!("left: {left}");
            set(tx, task, State::Blocked, Some(&why), trace)?;
        }
        Some(_) => {
            asked(0)?;
            set(tx, task, State::Done, note.as_deref(), trace)?;
        }
        None if asks == 0 => {
            asked(1)?;
            return Ok(vec![ask(tx, t, trace)?]);
        }
        None => set(tx, task, State::Blocked, Some(NO_REPORT), trace)?,
    }
    Ok(Vec::new())
}

/// The one ask for a report: a short turn for the task's owner, on the task.
fn ask(tx: &Transaction, t: &Task, trace: &str) -> Result<Start, Error> {
    let owner = t.owner.as_deref().ok_or_else(|| {
        Error::Refused(format!(
            "task #{} is the person's: it has no turn to start",
            t.number
        ))
    })?;
    set(tx, t.number, State::Ready, None, trace)?;
    let on = (Some(t.number), t.given_in.as_deref());
    Start::queue(tx, (&t.chat, owner), on, words::ask(t))
}

/// Commands that ended: the turn that started each, and that turn's task.
pub type Ended = Vec<(String, Option<i64>)>;

/// What the engine lists as running in the background now, saved; with `release`, each task held on
/// a command that ended and left nothing open settles (G174). Released when the engine is at work on
/// another turn: Claude Code says a command ended inside that turn and begins none for the task.
/// With the engine idle, the turn it begins on the task settles it, as any turn on it does.
pub fn background_changed(
    conn: &mut Connection,
    engine: (&str, Option<&str>),
    listed: &[(String, String)],
    turn: Option<&str>,
    release: bool,
    trace: &str,
) -> Result<(Ended, Vec<Start>), Error> {
    let tx = conn.transaction()?;
    let ended = crate::background::set(&tx, engine, listed, turn)?;
    let mut starts = Vec::new();
    if release {
        let mut tasks: Vec<i64> = ended.iter().filter_map(|(_, task)| *task).collect();
        tasks.dedup();
        for task in tasks {
            starts.extend(released(&tx, task, trace)?);
        }
    }
    tx.commit()?;
    Ok((ended, starts))
}

/// A held task's hold may be over, or narrower, with no turn of its own to say so (a command that
/// ended during another turn). Still held on something else, its reason names only what's left,
/// and what follows it moves on that (#156's review: work held on its merge alone counts as done
/// for a review of it). With nothing left open it's done if its owner reported so. Else its owner
/// is woken on it and told what ended: a missing report is never read as done (0047), and the wake
/// is the one ask for it.
pub(super) fn released(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let Some(t) = read(tx, task).optional_task()? else {
        return Ok(Vec::new());
    };
    if t.state != State::Held {
        return Ok(Vec::new());
    }
    let said = t.report.as_ref().map(|r| r.how);
    if !open_on(tx, task)?.is_empty() || matches!(said, Some(Finish::Done | Finish::Blocked)) {
        let mut starts = settle(tx, &t, trace)?;
        starts.extend(cascade(tx, task, trace)?);
        return Ok(starts);
    }
    tx.execute("UPDATE tasks SET asks = 1 WHERE number = ?1", [task])?;
    let carry = match t.report.and_then(|r| r.note) {
        Some(left) => {
            format!("what this task waited on has ended, and you left this work: {left}")
        }
        None => "what this task waited on has ended".to_string(),
    };
    Ok(vec![ready(tx, task, Some(&carry))?])
}
