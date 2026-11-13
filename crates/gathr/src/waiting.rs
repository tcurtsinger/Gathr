//! What a chat waits on (0050): one answer, from saved state plus what only the running service
//! knows. Each line says what it is, who or what can end it, since when, and where Gathr knows it
//! from. The window's line and list, the `waiting` tool and `gathr diag waiting` all read this, and
//! so does the rule that notices a chat where nothing moves.

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Error;
use crate::pauses::{self, Pause};
use crate::tasks::{self, Hold};
use crate::waits;

mod words;

pub use words::text;

/// Who or what can end a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Moves {
    /// It ends by itself: a turn runs, a look is made, a download ends.
    Itself,
    /// It waits on the person.
    You,
    /// Nobody and nothing is on it.
    Nobody,
    /// It follows other lines: a task that waits for another task.
    Others,
}

/// What the running service knows that no table holds. The default is a service that isn't
/// running: what it alone knew is then said as it was when it stopped.
#[derive(Debug, Clone, Default)]
pub struct Live {
    pub running: bool,
    /// Why each queued turn hasn't started, as its agent's queue says it, by turn.
    pub queued: HashMap<String, String>,
    /// The turn each of those is behind, when it is behind one: in this chat or another.
    pub behind: HashMap<String, String>,
    /// The turns with an approval waiting.
    pub approvals: Vec<String>,
    /// The version Gathr is updating to.
    pub updating: Option<String>,
}

/// One thing the chat waits on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    /// `turn`, `queued`, `sign_in`, `approval`, `question`, `wait`, `command`, `pr`, `given`,
    /// `merge`, `after`, `idle`, `trouble`, `update`, or `stopped` when the service isn't running.
    pub kind: &'static str,
    /// The line in words.
    pub what: String,
    /// What is waited for, in a few words: "CI on #157", an agent's name.
    pub on: Option<String>,
    /// The agent whose line it is.
    pub agent: Option<String>,
    pub task: Option<i64>,
    pub since_us: Option<i64>,
    pub moves: Moves,
    /// Where Gathr knows it from: `saved`, or `service` for what only the running service knows.
    pub from: &'static str,
    /// A wait's or a PR's last look: what it saw, when, and the error of looks that fail.
    pub seen: Option<String>,
    pub looked_us: Option<i64>,
    pub error: Option<String>,
}

impl Line {
    fn new(kind: &'static str, what: String, moves: Moves) -> Line {
        Line {
            kind,
            what,
            on: None,
            agent: None,
            task: None,
            since_us: None,
            moves,
            from: "saved",
            seen: None,
            looked_us: None,
            error: None,
        }
    }

    fn of(mut self, agent: Option<&str>, task: Option<i64>, since_us: i64) -> Line {
        self.agent = agent.map(String::from);
        self.task = task;
        self.since_us = Some(since_us);
        self
    }

    fn on(mut self, on: &str) -> Line {
        self.on = Some(on.to_string());
        self
    }
}

/// What an agent waits for, and since when.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct On {
    pub what: String,
    pub since_us: i64,
}

/// Where one agent of the chat stands.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Standing {
    pub agent: String,
    pub name: String,
    /// `needs_you`, `working`, `waiting` or `free`.
    pub state: &'static str,
    /// In a phrase: "Waiting for CI on #157", "Held", "Usage limit".
    pub words: String,
    /// The task its running turn works on.
    pub task: Option<i64>,
    /// Working: since when.
    pub since_us: Option<i64>,
    /// Waiting: for whom or what, each with since when.
    pub on: Vec<On>,
    /// Out of usage: when it resets.
    pub until_us: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub lines: Vec<Line>,
    pub agents: Vec<Standing>,
}

/// The answer as the API and the command line give it: its lines and agents, the same in words,
/// the clock its ages are counted from, and whether the running service or the database said it.
pub fn shown(answer: &Answer, now_us: i64, from: &str) -> serde_json::Value {
    serde_json::json!({
        "lines": answer.lines,
        "agents": answer.agents,
        "in_words": text(answer, now_us),
        "at_us": now_us,
        "from": from,
    })
}

/// A turn that hasn't ended.
struct Turn {
    id: String,
    agent: Option<String>,
    task: Option<i64>,
    status: String,
    held: Option<String>,
    why: Option<String>,
    created: i64,
}

/// A task that is neither done nor cancelled.
struct Open {
    number: i64,
    owner: Option<String>,
    giver: Option<String>,
    kind: String,
    state: String,
    reason: Option<String>,
    blocked_on: Option<i64>,
    heard: bool,
    created: i64,
    updated: i64,
    what: String,
    /// For the person's task: the member's request it was filed for.
    request: Option<i64>,
    after: Vec<i64>,
}

impl Open {
    /// Still under way: not blocked, failed or stopped either.
    fn under_way(&self) -> bool {
        matches!(
            self.state.as_str(),
            "waiting" | "ready" | "working" | "held"
        )
    }
}

/// What the chat waits on now.
pub fn answer(conn: &Connection, chat: &str, live: &Live, now_us: i64) -> Result<Answer, Error> {
    let pauses = pauses::list(conn)?;
    let turns = turns(conn, chat)?;
    let open = open_tasks(conn, chat)?;
    let waiting = waits::in_chat(conn, chat, None)?;
    let waiting: Vec<waits::Wait> = waiting.into_iter().filter(|w| w.state == "open").collect();
    let mut lines = Vec::new();
    turn_lines(conn, chat, live, &pauses, &turns, &mut lines)?;
    for q in open
        .iter()
        .filter(|o| o.kind == "person" && o.owner.is_none() && o.state == "ready")
    {
        let what = format!("A question for you: {}", q.what);
        let line = Line::new("question", what, Moves::You);
        lines.push(line.of(q.giver.as_deref(), Some(q.number), q.created));
    }
    wait_lines(conn, chat, &pauses, &waiting, &mut lines)?;
    command_lines(conn, chat, &mut lines)?;
    pr_lines(conn, chat, &mut lines)?;
    task_lines(conn, chat, &open, &turns, &mut lines)?;
    if let Some(version) = &live.updating {
        let mut line = Line::new(
            "update",
            format!("Gathr is updating to {version}"),
            Moves::Itself,
        );
        line.from = "service";
        lines.push(line);
    }
    if !live.running {
        // Nothing moves by itself while the service is stopped, whatever was under way.
        for line in lines.iter_mut().filter(|l| l.moves == Moves::Itself) {
            line.moves = Moves::Nobody;
        }
        let stopped = "Gathr's service isn't running: nothing here moves until it starts";
        lines.push(Line::new("stopped", stopped.to_string(), Moves::Nobody));
    }
    // What waits on the person first, then what nobody is on; the longest wait first in each.
    let rank = |m: Moves| {
        [Moves::You, Moves::Nobody, Moves::Itself, Moves::Others]
            .iter()
            .position(|x| *x == m)
    };
    lines.sort_by_key(|l| {
        (
            l.kind != "stopped",
            rank(l.moves),
            l.since_us.unwrap_or(i64::MAX),
        )
    });
    let agents = standings(conn, chat, live, (&pauses, &turns, &open, &waiting), now_us)?;
    Ok(Answer { lines, agents })
}

fn turns(conn: &Connection, chat: &str) -> Result<Vec<Turn>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, agent_id, task, status, held, why, created_ts FROM turns
         WHERE chat_id = ?1 AND status IN ('queued', 'waiting_sign_in', 'running')
         ORDER BY created_ts, rowid",
    )?;
    let rows = stmt.query_map([chat], |r| {
        Ok(Turn {
            id: r.get(0)?,
            agent: r.get(1)?,
            task: r.get(2)?,
            status: r.get(3)?,
            held: r.get(4)?,
            why: r.get(5)?,
            created: r.get(6)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

fn open_tasks(conn: &Connection, chat: &str) -> Result<Vec<Open>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT number, owner, giver, kind, state, reason, blocked_on, heard, created_ts, updated_ts, what,
                request
         FROM tasks WHERE chat_id = ?1 AND state NOT IN ('done', 'cancelled') ORDER BY number",
    )?;
    let rows = stmt.query_map([chat], |r| {
        Ok(Open {
            number: r.get(0)?,
            owner: r.get(1)?,
            giver: r.get(2)?,
            kind: r.get(3)?,
            state: r.get(4)?,
            reason: r.get(5)?,
            blocked_on: r.get(6)?,
            heard: r.get(7)?,
            created: r.get(8)?,
            updated: r.get(9)?,
            what: r.get(10)?,
            request: r.get(11)?,
            after: Vec::new(),
        })
    })?;
    let mut open: Vec<Open> = rows.collect::<Result<_, _>>()?;
    let mut after =
        conn.prepare_cached("SELECT after_task FROM task_after WHERE task = ?1 ORDER BY 1")?;
    for o in &mut open {
        o.after = after
            .query_map([o.number], |r| r.get(0))?
            .collect::<Result<_, _>>()?;
    }
    Ok(open)
}

/// A turn's agent by name; a turn sent to no agent is the chat's own.
fn whose(conn: &Connection, agent: Option<&str>) -> Result<String, Error> {
    match agent {
        Some(agent) => tasks::name_of(conn, agent),
        None => Ok("The chat".to_string()),
    }
}

fn turn_lines(
    conn: &Connection,
    chat: &str,
    live: &Live,
    pauses: &[Pause],
    turns: &[Turn],
    lines: &mut Vec<Line>,
) -> Result<(), Error> {
    for t in turns {
        let who = whose(conn, t.agent.as_deref())?;
        let agent = t.agent.as_deref();
        let on_task = t.task.map_or(String::new(), |n| format!(" on #{n}"));
        let line = match t.status.as_str() {
            "running" => {
                if live.approvals.contains(&t.id) {
                    let what = format!("{who} needs your approval");
                    let mut asks =
                        Line::new("approval", what, Moves::You).of(agent, t.task, t.created);
                    asks.from = "service";
                    lines.push(asks);
                }
                match live.running {
                    true => Line::new("turn", format!("{who} is working{on_task}"), Moves::Itself),
                    false => Line::new(
                        "turn",
                        format!("{who}'s turn{on_task} was running when the service stopped"),
                        Moves::Nobody,
                    ),
                }
            }
            "waiting_sign_in" => Line::new(
                "sign_in",
                format!("{who}'s turn{on_task} waits for you to sign in"),
                Moves::You,
            ),
            _ => {
                let (why, moves, from) = queued_why(conn, live, pauses, chat, t)?;
                let what = format!("{who}'s turn{on_task} is queued: {why}");
                let mut line = Line::new("queued", what, moves);
                line.from = from;
                line
            }
        };
        lines.push(line.of(agent, t.task, t.created));
    }
    Ok(())
}

/// Why a queued turn hasn't started: what is saved on it, else what its agent's queue says.
fn queued_why(
    conn: &Connection,
    live: &Live,
    pauses: &[Pause],
    chat: &str,
    t: &Turn,
) -> Result<(String, Moves, &'static str), Error> {
    let saved = |fallback: &str| t.why.clone().unwrap_or_else(|| fallback.to_string());
    // Behind a turn that waits for a sign-in, in this chat or another, it waits on the person too.
    let behind_sign_in = match live.behind.get(&t.id) {
        Some(ahead) => conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM turns WHERE id = ?1 AND status = 'waiting_sign_in')",
            [ahead],
            |r| r.get(0),
        )?,
        None => false,
    };
    Ok(match t.held.as_deref() {
        Some("pause") => {
            let stopped =
                pauses::holding(pauses, chat, t.agent.as_deref()).is_some_and(|p| p.stopped);
            let why = match stopped {
                true => "stopped by you, it carries on at Restart",
                false => "paused",
            };
            (why.to_string(), Moves::You, "saved")
        }
        Some("update") => (saved("Gathr is updating"), Moves::Itself, "saved"),
        Some("usage") => (
            saved("out of usage, and when it comes back isn't known"),
            Moves::Itself,
            "saved",
        ),
        Some(_) => (saved("its engine is downloading"), Moves::Itself, "saved"),
        None => match (live.queued.get(&t.id), live.running) {
            (Some(why), _) if behind_sign_in => (
                format!("{why}, which waits for you to sign in"),
                Moves::You,
                "service",
            ),
            (Some(why), _) => (why.clone(), Moves::Itself, "service"),
            (None, true) => (
                "unknown: its agent's queue hasn't said why".to_string(),
                Moves::Itself,
                "service",
            ),
            (None, false) => (
                "it was queued when the service stopped".to_string(),
                Moves::Nobody,
                "saved",
            ),
        },
    })
}

fn wait_lines(
    conn: &Connection,
    chat: &str,
    pauses: &[Pause],
    waiting: &[waits::Wait],
    lines: &mut Vec<Line>,
) -> Result<(), Error> {
    for w in waiting {
        let name = tasks::wait_name(conn, &w.kind, w.target.as_deref(), w.parts.len() as i64)?;
        let who = tasks::name_of(conn, &w.agent)?;
        let failing = w.failing();
        let paused = pauses::holding(pauses, chat, Some(&w.agent)).is_some();
        // A look that keeps failing moves nothing, and a pause holds every look in its scope.
        let (moves, note) = match (&failing, paused, w.kind.as_str()) {
            (Some((failed, _)), ..) if *failed >= waits::SHOWN_AFTER => (Moves::Nobody, ""),
            (_, true, _) => (Moves::You, ", paused"),
            (_, _, "agent") => (Moves::Others, ""),
            _ => (Moves::Itself, ""),
        };
        let what = format!("{who} waits for {name}{note}");
        let mut line = Line::new("wait", what, moves).of(Some(&w.agent), w.task, w.created_ts);
        line.seen = w.saw();
        let parts = w.parts.iter().filter_map(|p| p.looked_ts);
        line.looked_us = w.looked_ts.into_iter().chain(parts).max();
        line.error = failing.map(|(_, error)| error);
        lines.push(line.on(&name));
    }
    Ok(())
}

fn command_lines(conn: &Connection, chat: &str, lines: &mut Vec<Line>) -> Result<(), Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT b.description, b.started_ts, t.agent_id, t.task FROM background b
         JOIN turns t ON t.id = b.turn_id WHERE t.chat_id = ?1 ORDER BY b.started_ts",
    )?;
    let rows = stmt.query_map([chat], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
    for row in rows {
        let (described, started, agent, task): (String, i64, Option<String>, Option<i64>) = row?;
        let who = whose(conn, agent.as_deref())?;
        let what = format!("{who} runs a command in the background: {described}");
        lines.push(Line::new("command", what, Moves::Itself).of(agent.as_deref(), task, started));
    }
    Ok(())
}

/// The PRs Gathr may land (0049): each as its last look left it.
fn pr_lines(conn: &Connection, chat: &str, lines: &mut Vec<Line>) -> Result<(), Error> {
    let reviews = crate::landings::reviews(conn, None)?;
    for r in reviews
        .iter()
        .filter(|r| r.chat == chat && r.reviewed.is_some())
    {
        let clean: Option<i64> = conn.query_row(
            "SELECT finish_ts FROM tasks WHERE number = ?1",
            [r.task],
            |row| row.get(0),
        )?;
        let (saw, moving, looked) = match crate::landings::last_look(conn, chat, r.pr)? {
            Some((saw, moving, looked)) => (saw, moving, Some(looked)),
            None => ("Gathr looks at it within 2 minutes".to_string(), true, None),
        };
        let moves = if moving { Moves::Itself } else { Moves::Nobody };
        let mut line = Line::new("pr", format!("#{} to land: {saw}", r.pr), moves);
        line.looked_us = looked;
        line.agent = Some(r.reviewer.clone());
        line.task = Some(r.task);
        line.since_us = clean;
        lines.push(line.on(&format!("#{} to land", r.pr)));
    }
    Ok(())
}

/// The agents' tasks no turn is on: each says what it waits for, or that nothing is on it.
fn task_lines(
    conn: &Connection,
    chat: &str,
    open: &[Open],
    turns: &[Turn],
    lines: &mut Vec<Line>,
) -> Result<(), Error> {
    let find = |n: i64| open.iter().find(|o| o.number == n);
    for o in open {
        let Some(agent) = o.owner.as_deref() else {
            continue;
        };
        // A turn on it, running or to come, is its line.
        if turns.iter().any(|t| t.task == Some(o.number)) {
            continue;
        }
        let owner = tasks::name_of(conn, agent)?;
        let this = format!("#{} ({owner})", o.number);
        let named =
            |b: &Open| -> Result<String, Error> { tasks::owner_name(conn, b.owner.as_deref()) };
        let mut push =
            |line: Line, since: i64| lines.push(line.of(Some(agent), Some(o.number), since));
        let blocker = o.blocked_on.and_then(find);
        match o.state.as_str() {
            "waiting" => {
                let before: Vec<&Open> = o.after.iter().filter_map(|n| find(*n)).collect();
                for b in &before {
                    let by = named(b)?;
                    let what = format!("{this} waits for #{} ({by})", b.number);
                    push(Line::new("after", what, Moves::Others).on(&by), o.created);
                }
                if before.is_empty() {
                    let what = format!("{this} is waiting, and nothing it comes after is open");
                    push(Line::new("idle", what, Moves::Nobody), o.updated);
                }
            }
            "held" => {
                let holds = tasks::holds(conn, o.number)?;
                for hold in &holds {
                    match hold {
                        Hold::Given {
                            number,
                            owner: by,
                            since_us,
                        } => {
                            let what = format!("{this} waits for #{number} ({by})");
                            push(Line::new("given", what, Moves::Others).on(by), *since_us);
                        }
                        Hold::Merge(pr) => {
                            let said =
                                match o.reason.as_deref().is_some_and(|r| r.starts_with("done")) {
                                    true => "is done and waits",
                                    false => "waits",
                                };
                            let (moves, note) = match reviewed(conn, chat, *pr)? {
                                true => (Moves::Others, ""),
                                false => (Moves::Nobody, ", which has no review"),
                            };
                            let what = format!("{this} {said} for #{pr} to merge{note}");
                            push(
                                Line::new("merge", what, moves).on(&format!("#{pr} to merge")),
                                o.updated,
                            );
                        }
                        // Its waits and its commands have lines of their own.
                        Hold::Wait(_) | Hold::Command(_) => {}
                    }
                }
                // A member's request filed as a question to the person waits for that answer.
                let asked: Vec<&Open> = open
                    .iter()
                    .filter(|q| q.kind == "person" && q.request == Some(o.number))
                    .collect();
                for q in &asked {
                    let what = format!("{this} waits for your answer to #{}", q.number);
                    push(Line::new("given", what, Moves::Others).on("you"), q.created);
                }
                if holds.is_empty() && asked.is_empty() {
                    let reason = o.reason.as_deref().unwrap_or("no reason given");
                    let what =
                        format!("{this} is held ({reason}), and nothing that holds it is open");
                    push(Line::new("idle", what, Moves::Nobody), o.updated);
                }
            }
            "ready" | "working" => {
                let what = format!("{this} is {}, and no turn is on it", o.state);
                push(Line::new("idle", what, Moves::Nobody), o.updated);
            }
            "blocked" if blocker.is_some_and(Open::under_way) => {
                let b = blocker.expect("checked");
                let by = named(b)?;
                let what = format!("{this} is blocked until #{} ({by}) is done", b.number);
                push(Line::new("after", what, Moves::Others).on(&by), o.updated);
            }
            state => {
                let reason = o.reason.as_deref().unwrap_or("no reason given");
                let giver = match &o.giver {
                    Some(giver) => tasks::name_of(conn, giver)?,
                    None => "You".to_string(),
                };
                // Told, its giver has done nothing since: nobody is on it. Not yet told, it hears next.
                let (moves, told) = match (o.heard || o.giver.is_none(), o.giver.is_some()) {
                    (true, true) => (Moves::Nobody, format!(". {giver} was told")),
                    (true, false) => (Moves::Nobody, String::new()),
                    (false, _) => (Moves::Others, format!(". {giver} hears of it next")),
                };
                let what = format!("{this} is {state}: {reason}{told}");
                push(Line::new("trouble", what, moves), o.updated);
            }
        }
    }
    Ok(())
}

/// Whether a review of the PR is open or stands in the chat: someone, or Gathr, is on its merge.
fn reviewed(conn: &Connection, chat: &str, pr: i64) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM tasks WHERE chat_id = ?1 AND kind = 'review' AND pr = ?2
         AND state != 'cancelled' AND pr_closed = 0)",
        params![chat, pr],
        |r| r.get(0),
    )?)
}

type Saved<'a> = (&'a [Pause], &'a [Turn], &'a [Open], &'a [waits::Wait]);

/// Where each agent of the chat stands: who needs the person, who works, who waits and for what,
/// and who is free.
fn standings(
    conn: &Connection,
    chat: &str,
    live: &Live,
    (pauses, turns, open, waiting): Saved,
    now_us: i64,
) -> Result<Vec<Standing>, Error> {
    let mut all = Vec::new();
    for m in crate::team::members(conn, chat)? {
        let id = m.agent.as_str();
        let own: Vec<&Turn> = turns
            .iter()
            .filter(|t| t.agent.as_deref() == Some(id))
            .collect();
        let running: Vec<&&Turn> = own.iter().filter(|t| t.status == "running").collect();
        let task = running.iter().find_map(|t| t.task);
        let mut stand = Standing {
            agent: m.agent.clone(),
            name: m.name.clone(),
            state: "waiting",
            words: String::new(),
            task,
            since_us: None,
            on: Vec::new(),
            until_us: None,
        };
        let asked = open
            .iter()
            .any(|o| o.kind == "person" && o.giver.as_deref() == Some(id) && o.under_way());
        let mine = open
            .iter()
            .any(|o| o.owner.as_deref() == Some(id) && o.under_way());
        // A turn to come, queued or waiting for a sign-in.
        let queued = own.iter().any(|t| t.status != "running");
        let sign_in = own.iter().any(|t| t.status == "waiting_sign_in");
        let usage = usage_until(conn, id, now_us)?;
        let on = waits_for(conn, id, open, waiting)?;
        let held = |words: &str| ("waiting", words.to_string());
        let (state, words) = if running.iter().any(|t| live.approvals.contains(&t.id)) {
            ("needs_you", "Needs your approval".to_string())
        } else if asked {
            ("needs_you", "Asked you a question".to_string())
        } else if !running.is_empty() {
            stand.since_us = running.iter().map(|t| t.created).min();
            ("working", "Working".to_string())
        } else if sign_in {
            held("Waiting for you to sign in")
        } else if usage.is_some() {
            stand.until_us = usage;
            held("Usage limit")
        } else if pauses::holding(pauses, chat, None).is_some_and(|p| p.stopped) {
            held("Stopped")
        } else if pauses::holding(pauses, chat, Some(id)).is_some() {
            held(if mine || queued { "Held" } else { "Paused" })
        } else if !on.is_empty() {
            let names: Vec<&str> = on.iter().map(|o| o.what.as_str()).collect();
            stand.on = on.clone();
            held(&format!("Waiting for {}", words::list(&names)))
        } else if mine || queued {
            held("Waiting")
        } else {
            ("free", "Ready".to_string())
        };
        stand.state = state;
        stand.words = words;
        all.push(stand);
    }
    Ok(all)
}

/// When the agent's account is back, while it is out of usage.
fn usage_until(conn: &Connection, agent: &str, now_us: i64) -> Result<Option<i64>, Error> {
    let found = conn.query_row(
        "SELECT h.until_ts FROM agents a JOIN usage_holds h ON h.account_id = a.account_id WHERE a.id = ?1",
        [agent],
        |r| r.get::<_, i64>(0),
    );
    match found {
        // Held turns start a little after the reset: a hold that has just passed still holds.
        Ok(until) if until + pauses::AFTER_RESET_US > now_us => Ok(Some(until)),
        Ok(_) | Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Whom and what an agent waits for, each from when its earliest wait on it began: the agents its
/// waits name, the owners of what its own tasks come after and of the tasks it gave, then its
/// other waits by name.
fn waits_for(
    conn: &Connection,
    agent: &str,
    open: &[Open],
    waiting: &[waits::Wait],
) -> Result<Vec<On>, Error> {
    let (mut agents, mut other): (Vec<On>, Vec<On>) = (Vec::new(), Vec::new());
    let add = |list: &mut Vec<On>, what: String, since_us: i64| match list
        .iter_mut()
        .find(|o| o.what == what)
    {
        Some(found) => found.since_us = found.since_us.min(since_us),
        None => list.push(On { what, since_us }),
    };
    for w in waiting.iter().filter(|w| w.agent == agent) {
        let name = tasks::wait_name(conn, &w.kind, w.target.as_deref(), w.parts.len() as i64)?;
        match w.kind.as_str() {
            "agent" => add(&mut agents, name, w.created_ts),
            _ => add(&mut other, name, w.created_ts),
        }
    }
    let name = |owner: Option<&str>| match owner {
        Some(owner) => tasks::name_of(conn, owner),
        None => Ok("you".to_string()),
    };
    for t in open
        .iter()
        .filter(|o| o.owner.as_deref() == Some(agent) && o.under_way())
    {
        for b in open
            .iter()
            .filter(|b| t.after.contains(&b.number) && b.under_way())
        {
            add(&mut agents, name(b.owner.as_deref())?, t.created);
        }
    }
    for t in open
        .iter()
        .filter(|o| o.giver.as_deref() == Some(agent) && o.under_way())
    {
        add(&mut agents, name(t.owner.as_deref())?, t.created);
    }
    agents.extend(other);
    Ok(agents)
}
