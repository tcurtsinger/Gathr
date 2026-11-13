//! Waits: an agent waits on another agent, CI, a PR or a check instead of looping
//! (docs/decisions/0010-coordination-as-state.md). Kept in the database, so a wait outlasts a
//! restart. Several parts make an `all` wait, which fires once when every part is met.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::ids::{now_us, ulid};
use crate::tasks::Start;

pub mod ci;

/// How long a wait lasts before its agent is woken anyway.
pub const WAIT_FOR_US: i64 = 24 * 60 * 60 * 1_000_000;
/// Looks that failed in a row before it shows on the wait and in diagnostics (0010).
pub const SHOWN_AFTER: i64 = 3;
const MOST_PARTS: usize = 5;

/// One thing waited on: its kind (`agent`, `ci`, `pr`, `check`), and the agent's id, the PR or
/// commit, or the command.
#[derive(Debug, Clone)]
pub struct Part {
    pub kind: String,
    pub target: String,
    pub repo: Option<String>,
}

pub struct Leave<'a> {
    pub chat: &'a str,
    pub agent: &'a str,
    pub task: Option<i64>,
    pub left_in: Option<&'a str>,
    /// Where its checks run: the agent's folder.
    pub cwd: Option<&'a str>,
    pub then_do: &'a str,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Wait {
    pub id: String,
    pub chat: String,
    pub agent: String,
    pub task: Option<i64>,
    /// The turn that left it: a wake-up answers that turn (the inline thread).
    pub left_in: Option<String>,
    pub kind: String,
    pub target: Option<String>,
    pub repo: Option<String>,
    pub cwd: Option<String>,
    pub then_do: String,
    pub state: String,
    pub line: Option<String>,
    pub head: Option<String>,
    pub head_ts: Option<i64>,
    pub failures: i64,
    pub last_error: Option<String>,
    /// What its last look saw while it wasn't met, and when it last looked.
    pub seen: Option<String>,
    pub looked_ts: Option<i64>,
    pub created_ts: i64,
    pub parts: Vec<Wait>,
}

impl Wait {
    /// What the last look saw, each part's for a wait of several.
    pub fn saw(&self) -> Option<String> {
        if self.parts.is_empty() {
            return self.seen.clone();
        }
        let parts: Vec<&str> = self
            .parts
            .iter()
            .filter_map(|p| p.seen.as_deref())
            .collect();
        (!parts.is_empty()).then(|| parts.join("; "))
    }

    /// The looks that failed in a row and the last one's error: its own, or for a wait of several
    /// the part's whose looks have failed most. Looks are made part by part.
    pub fn failing(&self) -> Option<(i64, String)> {
        let own = std::iter::once(self).filter(|_| self.parts.is_empty());
        let worst = own.chain(&self.parts).max_by_key(|w| w.failures)?;
        let error = worst.last_error.clone()?;
        (worst.failures > 0).then_some((worst.failures, error))
    }
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// Leaves a wait, or says again what to do then on the same one already open.
pub fn leave(conn: &mut Connection, l: &Leave, trace: &str) -> Result<Wait, Error> {
    let then_do = l.then_do.trim();
    if then_do.is_empty() {
        return Err(refused("say what you'll do then (`then`)"));
    }
    if l.parts.is_empty() || l.parts.len() > MOST_PARTS {
        return Err(refused(format!("a wait has 1 to {MOST_PARTS} parts")));
    }
    let tx = conn.transaction()?;
    crate::archive::refuse_archived(&tx, l.chat)?;
    for part in &l.parts {
        checked(&tx, l, part)?;
    }
    if let [part] = l.parts.as_slice() {
        let same: Option<String> = tx
            .query_row(
                "SELECT id FROM waits WHERE chat_id = ?1 AND agent_id = ?2 AND kind = ?3
                 AND target = ?4 AND state = 'open' AND part_of IS NULL",
                params![l.chat, l.agent, part.kind, part.target],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(id) = same {
            tx.execute(
                "UPDATE waits SET then_do = ?2 WHERE id = ?1",
                params![id, then_do],
            )?;
            let wait = read(&tx, &id)?;
            tx.commit()?;
            return Ok(wait);
        }
    }
    let now = now_us();
    let id = ulid(now);
    let (kind, target, repo) = match l.parts.as_slice() {
        [one] => (
            one.kind.as_str(),
            Some(one.target.as_str()),
            one.repo.as_deref(),
        ),
        _ => ("all", None, None),
    };
    insert(&tx, &id, None, l, (kind, target, repo), now)?;
    if l.parts.len() > 1 {
        for (n, part) in l.parts.iter().enumerate() {
            let at = (
                part.kind.as_str(),
                Some(part.target.as_str()),
                part.repo.as_deref(),
            );
            insert(&tx, &ulid(now + 1 + n as i64), Some(&id), l, at, now)?;
        }
    }
    let event = Event::point("gathr.wait.leave").trace(trace).chat(l.chat);
    let event = match l.task {
        Some(task) => event.task(task),
        None => event,
    };
    event
        .ok()
        .attr("gathr.wait.id", id.clone())
        .attr("gathr.wait.kind", kind)
        .attr("gen_ai.agent.id", l.agent)
        .insert(&tx)?;
    let wait = read(&tx, &id)?;
    tx.commit()?;
    Ok(wait)
}

fn checked(conn: &Connection, l: &Leave, part: &Part) -> Result<(), Error> {
    let target = part.target.trim();
    if let Some(repo) = &part.repo {
        let ok = repo.split('/').count() == 2
            && repo
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_./".contains(c));
        if !ok {
            return Err(refused(format!("`repo` is owner/name, not \"{repo}\"")));
        }
    }
    match part.kind.as_str() {
        "agent" => {
            if target == l.agent {
                return Err(refused("an agent can't wait on itself"));
            }
            let name = crate::tasks::name_of(conn, target)?;
            // The hear-back already wakes the giver: a wait too woke it twice (F5's round trip).
            let yours: Option<i64> = conn
                .query_row(
                    &format!(
                        "SELECT number FROM tasks WHERE chat_id = ?1 AND giver = ?2 AND owner = ?3
                         AND state NOT IN {} ORDER BY number LIMIT 1",
                        crate::tasks::SETTLED
                    ),
                    [l.chat, l.agent, target],
                    |r| r.get(0),
                )
                .optional()?;
            if let Some(task) = yours {
                return Err(refused(format!(
                    "{name} is on your task #{task}: you hear back when it's settled, so don't wait \
                     on {name}"
                )));
            }
            if !busy(conn, l.chat, target)? {
                return Err(refused(format!(
                    "{name} has nothing running or queued in this chat"
                )));
            }
        }
        "ci" => {
            ci::Target::parse(target).map_err(refused)?;
        }
        "pr" | "up_to_date" => {
            if target.parse::<u32>().is_err() {
                return Err(refused(format!(
                    "a PR wait takes its number, not \"{target}\""
                )));
            }
        }
        "check" if target.is_empty() => return Err(refused("a check wait needs its command")),
        // Never the service's own folder: a check runs where its agent works.
        "check" if l.cwd.is_none() => {
            return Err(refused(
                "a check runs in its agent's folder, and this wait has none",
            ));
        }
        "check" => {}
        other => {
            return Err(refused(format!(
                "{other} isn't a wait: agent, ci, pr, up_to_date or check"
            )));
        }
    }
    Ok(())
}

/// Whether `agent` has a turn running or queued in the chat.
fn busy(conn: &Connection, chat: &str, agent: &str) -> Result<bool, Error> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM turns WHERE chat_id = ?1 AND agent_id = ?2
             AND status IN ('queued', 'waiting_sign_in', 'running') LIMIT 1",
            [chat, agent],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

fn insert(
    tx: &Transaction,
    id: &str,
    part_of: Option<&str>,
    l: &Leave,
    (kind, target, repo): (&str, Option<&str>, Option<&str>),
    now: i64,
) -> Result<(), Error> {
    tx.execute(
        "INSERT INTO waits (id, chat_id, agent_id, task, left_in, part_of, kind, target, repo, cwd,
                            then_do, state, until_ts, created_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 'open', ?12, ?13)",
        params![
            id,
            l.chat,
            l.agent,
            l.task,
            l.left_in,
            part_of,
            kind,
            target.map(str::trim),
            repo,
            l.cwd,
            l.then_do.trim(),
            now + WAIT_FOR_US,
            now
        ],
    )?;
    Ok(())
}

/// The waits the service looks at itself: open CI, PR, up-to-date and check waits, alone or parts.
pub fn to_look_at(conn: &Connection) -> Result<Vec<Wait>, Error> {
    ids(
        conn,
        "SELECT id FROM waits WHERE state = 'open' AND kind IN ('ci', 'pr', 'up_to_date', 'check')
         ORDER BY created_ts",
        [],
    )
    .and_then(|ids| ids.iter().map(|id| read(conn, id)).collect())
}

/// How a look went that didn't meet the wait: the head and what it saw, or why it failed (what an
/// earlier look saw then stands). The failures in a row since the last look that worked.
pub fn looked(
    conn: &Connection,
    id: &str,
    head: Option<(&str, i64)>,
    seen: Option<&str>,
    error: Option<&str>,
) -> Result<i64, Error> {
    Ok(conn.query_row(
        "UPDATE waits SET looked_ts = ?2,
             head = COALESCE(?3, head), head_ts = COALESCE(?4, head_ts),
             failures = CASE WHEN ?5 IS NULL THEN 0 ELSE failures + 1 END, last_error = ?5,
             seen = CASE WHEN ?5 IS NULL THEN ?6 ELSE seen END
         WHERE id = ?1 RETURNING failures",
        params![
            id,
            now_us(),
            head.map(|h| h.0),
            head.map(|h| h.1),
            error,
            seen
        ],
        |r| r.get(0),
    )?)
}

/// What one wait waited on has happened: a part is `met`, and its wait fires once every part is.
pub fn met(conn: &mut Connection, id: &str, line: &str, trace: &str) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let starts = met_in(&tx, id, line, trace)?;
    tx.commit()?;
    Ok(starts)
}

fn met_in(tx: &Transaction, id: &str, line: &str, trace: &str) -> Result<Vec<Start>, Error> {
    let wait = read(tx, id)?;
    if !matches!(wait.state.as_str(), "open") {
        return Ok(Vec::new());
    }
    let part_of: Option<String> =
        tx.query_row("SELECT part_of FROM waits WHERE id = ?1", [id], |r| {
            r.get(0)
        })?;
    let starts = match part_of {
        None => fire(tx, &wait, "fired", line, trace)?,
        Some(whole) => {
            tx.execute(
                "UPDATE waits SET state = 'met', line = ?2, ended_ts = ?3 WHERE id = ?1",
                params![id, line, now_us()],
            )?;
            let whole = read(tx, &whole)?;
            match whole.parts.iter().all(|p| p.state == "met") {
                true => {
                    let lines: Vec<&str> = whole
                        .parts
                        .iter()
                        .filter_map(|p| p.line.as_deref())
                        .collect();
                    fire(tx, &whole, "fired", &lines.join("; "), trace)?
                }
                false => Vec::new(),
            }
        }
    };
    Ok(starts)
}

/// `agent`'s turn in the chat has ended: waits on it there are met once it has nothing running or
/// queued in the chat, saying how its last turn ended.
pub fn agent_free(
    conn: &mut Connection,
    chat: &str,
    agent: &str,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let starts = agent_free_in(&tx, chat, agent, trace)?;
    tx.commit()?;
    Ok(starts)
}

pub(crate) fn agent_free_in(
    conn: &Transaction,
    chat: &str,
    agent: &str,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    if busy(conn, chat, agent)? {
        return Ok(Vec::new());
    }
    let last: Option<String> = conn
        .query_row(
            "SELECT status FROM turns WHERE chat_id = ?1 AND agent_id = ?2
             ORDER BY ended_ts DESC LIMIT 1",
            [chat, agent],
            |r| r.get(0),
        )
        .optional()?;
    let how = match last.as_deref() {
        Some("failed") => "failed",
        Some("stopped") => "was stopped",
        _ => "ended well",
    };
    let open = ids(
        conn,
        "SELECT id FROM waits WHERE chat_id = ?1 AND kind = 'agent' AND target = ?2
         AND state = 'open' ORDER BY created_ts",
        [chat, agent],
    )?;
    let name = crate::tasks::name_of(conn, agent)?;
    let mut starts = Vec::new();
    for id in open {
        let line = format!("{name} is done here: its last turn {how}");
        starts.extend(met_in(conn, &id, &line, trace)?);
    }
    Ok(starts)
}

/// The chats and agents that open agent waits wait on.
pub fn waited_on(conn: &Connection) -> Result<Vec<(String, String)>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT chat_id, target FROM waits WHERE kind = 'agent' AND state = 'open'",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Waits a day old wake their agent anyway (`expired`).
pub fn expire(conn: &mut Connection, now: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let old = ids(
        conn,
        "SELECT id FROM waits WHERE state = 'open' AND part_of IS NULL AND until_ts <= ?1",
        [now],
    )?;
    let mut starts = Vec::new();
    for id in old {
        let tx = conn.transaction()?;
        let wait = read(&tx, &id)?;
        let parts = wait.parts.len() as i64;
        let what = crate::tasks::wait_name(&tx, &wait.kind, wait.target.as_deref(), parts)?;
        let failing = wait
            .last_error
            .as_ref()
            .map_or(String::new(), |e| format!(" (the last check failed: {e})"));
        let line = format!("You have waited 24 hours for {what}{failing}");
        starts.extend(fire(&tx, &wait, "expired", &line, trace)?);
        tx.commit()?;
    }
    Ok(starts)
}

/// Ends one of the agent's own open waits (G54).
pub fn end(
    conn: &mut Connection,
    chat: &str,
    agent: &str,
    id: &str,
    trace: &str,
) -> Result<(), Error> {
    let tx = conn.transaction()?;
    let wait = read(&tx, id).map_err(|_| refused(format!("there's no wait {id}")))?;
    if wait.chat != chat || wait.agent != agent {
        return Err(refused(format!("wait {id} isn't yours in this chat")));
    }
    if wait.state != "open" {
        return Err(refused(format!("wait {id} is already {}", wait.state)));
    }
    tx.execute(
        "UPDATE waits SET state = 'cancelled', ended_ts = ?2, line = 'ended by its agent'
         WHERE id = ?1 OR part_of = ?1",
        params![id, now_us()],
    )?;
    let event = Event::point("gathr.wait.end").trace(trace).chat(chat).ok();
    event.attr("gathr.wait.id", id).insert(&tx)?;
    tx.commit()?;
    Ok(())
}

/// The chat's waits, open first, then those ended in the last day, newest first; with `agent`,
/// only its own.
pub fn in_chat(conn: &Connection, chat: &str, agent: Option<&str>) -> Result<Vec<Wait>, Error> {
    let since = now_us() - WAIT_FOR_US;
    let found = ids(
        conn,
        "SELECT id FROM waits WHERE chat_id = ?1 AND part_of IS NULL AND agent_id = COALESCE(?2, agent_id)
         AND (state = 'open' OR ended_ts >= ?3)
         ORDER BY state != 'open', created_ts DESC",
        params![chat, agent, since],
    )?;
    found.iter().map(|id| read(conn, id)).collect()
}

/// The wait fires: ended as `fired` or `expired`, with the turn that wakes its agent where it left
/// it, on its task.
fn fire(
    tx: &Transaction,
    wait: &Wait,
    state: &str,
    line: &str,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    tx.execute(
        "UPDATE waits SET state = ?2, line = ?3, ended_ts = ?4 WHERE id = ?1",
        params![wait.id, state, line, now_us()],
    )?;
    let event = Event::point("gathr.wait.fire")
        .trace(trace)
        .chat(&wait.chat);
    let event = match wait.task {
        Some(task) => event.task(task),
        None => event,
    };
    event
        .ok()
        .attr("gathr.wait.id", wait.id.clone())
        .attr("gathr.wait.state", state)
        .attr("gen_ai.agent.id", wait.agent.clone())
        .insert(tx)?;
    let mut text = format!(
        "Your wait ended: {line}. You planned to do this: {}. Check the current status, then continue or explain what is blocking you.",
        wait.then_do.trim_end_matches('.')
    );
    if let Some(task) = wait.task {
        text.push_str(&format!(" End the turn with finish_task for task #{task}."));
    }
    let at = (wait.chat.as_str(), wait.agent.as_str());
    let on = (wait.task, wait.left_in.as_deref());
    Ok(vec![Start::queue(tx, at, on, text)?])
}

fn ids(conn: &Connection, sql: &str, p: impl rusqlite::Params) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached(sql)?;
    let rows = stmt.query_map(p, |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn read(conn: &Connection, id: &str) -> Result<Wait, Error> {
    let mut wait = conn.query_row(
        "SELECT id, chat_id, agent_id, task, kind, target, repo, cwd, then_do, state, line, head,
                head_ts, failures, last_error, created_ts, left_in, seen, looked_ts
         FROM waits WHERE id = ?1",
        [id],
        |r| {
            Ok(Wait {
                id: r.get(0)?,
                chat: r.get(1)?,
                agent: r.get(2)?,
                task: r.get(3)?,
                kind: r.get(4)?,
                target: r.get(5)?,
                repo: r.get(6)?,
                cwd: r.get(7)?,
                then_do: r.get(8)?,
                state: r.get(9)?,
                line: r.get(10)?,
                head: r.get(11)?,
                head_ts: r.get(12)?,
                failures: r.get(13)?,
                last_error: r.get(14)?,
                created_ts: r.get(15)?,
                left_in: r.get(16)?,
                seen: r.get(17)?,
                looked_ts: r.get(18)?,
                parts: Vec::new(),
            })
        },
    )?;
    if wait.kind == "all" {
        let parts = ids(
            conn,
            "SELECT id FROM waits WHERE part_of = ?1 ORDER BY id",
            [id],
        )?;
        wait.parts = parts
            .iter()
            .map(|p| read(conn, p))
            .collect::<Result<_, _>>()?;
    }
    Ok(wait)
}
