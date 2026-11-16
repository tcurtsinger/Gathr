//! The catch (0063 I2a, I2b): what Gathr can't refuse of a held turn (a shell command's writes,
//! anything Codex writes) is read from what git tracks in its worktree, at its start and its end,
//! kept in `held_turns` so a stop loses nothing, shown in its chat and told to its agent.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;
use crate::hold::{self, Held, Place};
use crate::ids::now_us;
use crate::platform::env::Vars;
use crate::worktree;

/// More entries than this at a turn's start, and the turn isn't looked at (I2a.6).
pub const MOST: usize = 2_000;
/// The files Gathr's line names; the rest are counted.
const NAMED: usize = 20;

/// A worktree as git tracks it: its head, and `git status`'s entries with their content ids.
#[derive(Debug, Clone)]
pub struct Seen {
    pub head: String,
    pub entries: Vec<(String, Option<String>)>,
}

/// Why a turn's start wasn't read.
#[derive(Debug)]
pub enum Failed {
    TooMany(usize),
    Git(String),
}

/// What a held turn changed: the files, and how many of them came from commits it made.
#[derive(Debug, Clone, PartialEq)]
pub struct Changed {
    pub files: Vec<String>,
    pub committed: usize,
}

/// A held turn to look at.
#[derive(Debug, Clone)]
pub struct ToLook {
    pub turn: String,
    pub chat: String,
    pub agent: String,
    pub ask: Option<String>,
    /// The look-into task it is on, if one.
    pub task: Option<i64>,
    pub worktree: PathBuf,
    pub trace: String,
}

/// What a held turn's end finds: still held, its start to compare with; or released mid-turn.
#[derive(Debug)]
pub enum Ending {
    Look(ToLook, Seen),
    Released(ToLook),
}

/// The environment the looks run git with: the person's, with what the repository names switched
/// off as a read-only check has it.
pub fn env(mut vars: Vars) -> Vars {
    let guard = crate::approvals::read_only_env().into_iter();
    vars.extend(guard.map(|(k, v)| (k.into(), v.into())));
    vars
}

/// The worktree at a turn's start (I2a.3), unless it holds more than `MOST` entries.
pub async fn seen(folder: &Path, env: &Vars) -> Result<Seen, Failed> {
    let (head, entries) = worktree::snapshot(folder, env, MOST)
        .await
        .map_err(Failed::Git)?;
    if entries.len() > MOST {
        return Err(Failed::TooMany(entries.len()));
    }
    Ok(Seen { head, entries })
}

/// What the turn changed since `start`, and how many entries its end held.
pub async fn changes(folder: &Path, env: &Vars, start: &Seen) -> Result<(Changed, usize), String> {
    let (head, entries) = worktree::snapshot(folder, env, usize::MAX).await?;
    let committed = match head == start.head {
        true => Vec::new(),
        false => worktree::committed(folder, env, &start.head, &head).await?,
    };
    let count = entries.len();
    Ok((compare(start, &Seen { head, entries }, committed), count))
}

/// A change (I2a.5): an entry added to or gone from the status list, a listed file whose content
/// changed, or a file of a commit the turn made. A moved head alone is none.
pub fn compare(start: &Seen, end: &Seen, committed: Vec<String>) -> Changed {
    let ids = |s: &Seen| -> HashMap<String, Option<String>> { s.entries.iter().cloned().collect() };
    let (was, now) = (ids(start), ids(end));
    let path = |key: &String| key[3..].to_string();
    let mut files: Vec<String> = end
        .entries
        .iter()
        .filter(|(key, id)| was.get(key) != Some(id))
        .map(|(key, _)| path(key))
        .collect();
    let gone = start
        .entries
        .iter()
        .filter(|(key, _)| !now.contains_key(key));
    files.extend(gone.map(|(key, _)| path(key)));
    let count = committed.len();
    files.extend(committed);
    files.sort();
    files.dedup();
    Changed {
        files,
        committed: count,
    }
}

/// The turn, when it is held in a project's chat and its agent has a worktree there, and it has no
/// start record yet (I2a.1). Any other turn costs this one read.
pub fn to_look(conn: &Connection, turn: &str) -> Result<Option<ToLook>, Error> {
    let Some(held) = hold::turn(conn, turn)?.filter(Held::held) else {
        return Ok(None);
    };
    let found: Option<(String, String, String, String)> = conn
        .query_row(
            "SELECT u.chat_id, u.agent_id, u.trace_id, f.path FROM turns u
             JOIN chats c ON c.id = u.chat_id AND c.project_id IS NOT NULL
             JOIN agent_folders f ON f.chat_id = u.chat_id AND f.agent_id = u.agent_id AND f.kind = 'worktree'
             WHERE u.id = ?1 AND NOT EXISTS (SELECT 1 FROM held_turns h WHERE h.turn_id = u.id)
             ORDER BY f.made_ts DESC LIMIT 1",
            [turn],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    Ok(found.map(|(chat, agent, trace, worktree)| ToLook {
        turn: turn.to_string(),
        chat,
        agent,
        ask: held.ask.clone(),
        task: held.task.filter(|_| held.look_into),
        worktree: PathBuf::from(worktree),
        trace,
    }))
}

/// Keeps the turn's start (I2a.7).
pub fn before(conn: &Connection, l: &ToLook, seen: &Seen) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO held_turns (turn_id, chat_id, agent_id, ask_id, task, worktree, head, status, started_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            l.turn,
            l.chat,
            l.agent,
            l.ask,
            l.task,
            l.worktree.display().to_string(),
            seen.head,
            serde_json::to_string(&seen.entries)?,
            now_us()
        ],
    )?;
    Ok(())
}

/// The agent's held turns in the chat that ended with no look, a stop having cut them off,
/// oldest first: looked at before `turn` starts (I2a.7).
pub fn pending(
    conn: &Connection,
    chat: &str,
    agent: &str,
    turn: &str,
) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT h.turn_id FROM held_turns h LEFT JOIN turns u ON u.id = h.turn_id
         WHERE h.chat_id = ?1 AND h.agent_id = ?2 AND h.looked_ts IS NULL AND h.turn_id != ?3
           AND COALESCE(u.status NOT IN ('queued', 'running', 'waiting_sign_in'), 1)
         ORDER BY h.started_ts, h.rowid",
    )?;
    let rows = stmt.query_map(params![chat, agent, turn], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// A turn's end look, asking the rule again (I2a.2): a turn no longer held is marked looked and
/// reports nothing. `None` for a turn with no start record, or one looked at already.
pub fn ending(conn: &Connection, turn: &str) -> Result<Option<Ending>, Error> {
    type Row = (
        String,
        String,
        Option<String>,
        Option<i64>,
        String,
        Option<String>,
        String,
        String,
    );
    let found: Option<Row> = conn
        .query_row(
            "SELECT h.chat_id, h.agent_id, h.ask_id, h.task, h.worktree, h.head, h.status,
                    COALESCE(u.trace_id, '')
             FROM held_turns h LEFT JOIN turns u ON u.id = h.turn_id
             WHERE h.turn_id = ?1 AND h.looked_ts IS NULL",
            [turn],
            |r| {
                let (chat, agent, ask, task) = (r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?);
                Ok((
                    chat,
                    agent,
                    ask,
                    task,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                    r.get(7)?,
                ))
            },
        )
        .optional()?;
    let Some((chat, agent, ask, task, worktree, head, status, trace)) = found else {
        return Ok(None);
    };
    let l = ToLook {
        turn: turn.to_string(),
        chat,
        agent,
        ask,
        task,
        worktree: PathBuf::from(worktree),
        trace,
    };
    if hold::turn(conn, turn)?.is_some_and(|h| h.held()) {
        let entries = serde_json::from_str(&status)?;
        let head = head.unwrap_or_default();
        return Ok(Some(Ending::Look(l, Seen { head, entries })));
    }
    let looked = "UPDATE held_turns SET looked_ts = ?2 WHERE turn_id = ?1";
    conn.execute(looked, params![turn, now_us()])?;
    Ok(Some(Ending::Released(l)))
}

/// Keeps what the turn's end found, `None` when git couldn't say: with a change, Gathr's line in
/// the turn, the ask held (I6) and `gathr.hold.changed`, in one transaction. The line, if one.
pub fn after(
    conn: &mut Connection,
    l: &ToLook,
    changed: Option<&Changed>,
) -> Result<Option<Value>, Error> {
    let tx = conn.transaction()?;
    let found = changed.filter(|c| !c.files.is_empty());
    let files = found.map(|c| serde_json::to_string(&c.files)).transpose()?;
    tx.execute(
        "UPDATE held_turns SET looked_ts = ?2, changed = ?3 WHERE turn_id = ?1",
        params![l.turn, now_us(), files],
    )?;
    let Some(c) = found else {
        tx.commit()?;
        return Ok(None);
    };
    let name: Option<String> = tx
        .query_row("SELECT name FROM agents WHERE id = ?1", [&l.agent], |r| {
            r.get(0)
        })
        .optional()?;
    let body = line(name.as_deref().unwrap_or(&l.agent), l.task, c);
    let message = crate::store::add_message(&tx, &l.chat, &l.turn, "gathr", "text", &body)?;
    let id = message["id"].as_str().unwrap_or_default().to_string();
    let said = "UPDATE held_turns SET line = ?2 WHERE turn_id = ?1";
    tx.execute(said, params![l.turn, id])?;
    if let Some(ask) = &l.ask {
        let held = "UPDATE asks SET held_ts = COALESCE(held_ts, ?2) WHERE id = ?1";
        tx.execute(held, params![ask, now_us()])?;
    }
    Event::point("gathr.hold.changed")
        .trace(&l.trace)
        .chat(&l.chat)
        .ok()
        .attr("gathr.ask.id", l.ask.clone())
        .attr("gathr.turn.id", l.turn.as_str())
        .attr("gen_ai.agent.id", l.agent.as_str())
        .attr("gathr.task.id", l.task)
        .attr("gathr.hold.files", json!(c.files))
        .attr("gathr.hold.count", c.files.len())
        .attr("gathr.hold.committed", c.committed)
        .attr("gathr.message.id", id)
        .insert(&tx)?;
    tx.commit()?;
    Ok(Some(message))
}

fn files(n: usize) -> String {
    match n {
        1 => "1 file".to_string(),
        n => format!("{n} files"),
    }
}

/// The first `NAMED` files, and "and {k} more" when there are more.
fn named(files: &[String]) -> (&[String], Option<String>) {
    let more = files.len().saturating_sub(NAMED);
    let rest = (more > 0).then(|| format!("and {more} more"));
    (&files[..files.len().min(NAMED)], rest)
}

/// Gathr's line for a caught change (I2b.1).
fn line(agent: &str, task: Option<i64>, c: &Changed) -> String {
    let n = files(c.files.len());
    let mut lines = vec![match task {
        Some(t) => format!("{agent} changed {n} on look-into task #{t}"),
        None => format!("{agent} changed {n} before the plan was approved"),
    }];
    let (shown, rest) = named(&c.files);
    lines.extend(shown.iter().cloned());
    lines.extend(rest);
    if c.committed > 0 {
        lines.push(format!("Committed: {}", c.committed));
    }
    lines.join("\n")
}

/// What the agent's newest caught turn in the chat changed, told before Gathr's text of its next
/// turn while that turn is still held (I2b.5), with `gathr.hold.told`. Every catch not told yet is
/// marked told either way: the next turn's words would name the wrong turn.
pub fn untold(
    conn: &mut Connection,
    turn: &str,
    chat: &str,
    agent: &str,
    place: &Place,
) -> Result<Option<String>, Error> {
    let found: Option<(String, String, String)> = conn
        .query_row(
            "SELECT turn_id, worktree, changed FROM held_turns
             WHERE chat_id = ?1 AND agent_id = ?2 AND changed IS NOT NULL AND told_ts IS NULL
             ORDER BY started_ts DESC, rowid DESC LIMIT 1",
            [chat, agent],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((of, worktree, changed)) = found else {
        return Ok(None);
    };
    let tx = conn.transaction()?;
    tx.execute(
        "UPDATE held_turns SET told_ts = ?3
         WHERE chat_id = ?1 AND agent_id = ?2 AND changed IS NOT NULL AND told_ts IS NULL",
        params![chat, agent, now_us()],
    )?;
    let Some((reason, r)) = hold::write_held(&tx, turn, place)? else {
        tx.commit()?;
        return Ok(None);
    };
    let changed: Vec<String> = serde_json::from_str(&changed)?;
    let n = files(changed.len());
    let (shown, rest) = named(&changed);
    let rest = rest.map_or(String::new(), |r| format!(" {r}"));
    let words = format!(
        "In your last turn you changed {n} in {worktree}: {}{rest}. The person sees them.\n{reason}",
        shown.join(", ")
    );
    Event::point("gathr.hold.told")
        .trace(&r.trace)
        .chat(chat)
        .ok()
        .attr("gathr.turn.id", turn)
        .attr("gathr.hold.of", of)
        .insert(&tx)?;
    tx.commit()?;
    Ok(Some(words))
}

/// The chat's caught lines, oldest first (I2b.6).
pub fn caught(conn: &Connection, chat: &str) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT line FROM held_turns WHERE chat_id = ?1 AND line IS NOT NULL ORDER BY rowid",
    )?;
    let rows = stmt.query_map([chat], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The ask's caught turns, for its plan's tab (I2b.7): who, which files, and Gathr's line.
pub fn of_ask(conn: &Connection, ask: &str) -> Result<Vec<Value>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT agent_id, changed, line FROM held_turns
         WHERE ask_id = ?1 AND line IS NOT NULL ORDER BY rowid",
    )?;
    let rows = stmt.query_map([ask], |r| {
        let (agent, changed, line): (String, String, String) = (r.get(0)?, r.get(1)?, r.get(2)?);
        Ok((agent, changed, line))
    })?;
    let mut found = Vec::new();
    for row in rows {
        let (agent, changed, line) = row?;
        let files: Value = serde_json::from_str(&changed)?;
        found.push(json!({ "agent": agent, "files": files, "message": line }));
    }
    Ok(found)
}
