//! What a chat waits on (0050): one answer, from saved state plus what only the running service
//! knows. Each line says what it is, who or what can end it, since when, and where Gathr knows it
//! from. The window's line and list, the `waiting` tool and `gathr diag waiting` all read this, and
//! so does the rule that notices a chat where nothing moves.

use std::collections::HashMap;

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::pauses::{self, Pause};
use crate::tasks::{self, Hold};
use crate::team::Role;
use crate::waits;
use crate::{Error, changes};

mod standing;
pub(crate) mod words;

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
    /// The turns with an approval waiting, each with when its approvals began waiting.
    pub approvals: HashMap<String, i64>,
    /// The version Gathr is updating to.
    pub updating: Option<String>,
    /// The running turns that have made no progress past the limit, each with when it last did.
    pub silent: HashMap<String, i64>,
    /// The chat's PRs Gathr lands whose looks failed, by PR: how many in a row, and the last error.
    pub landing: HashMap<i64, (i64, String)>,
}

/// One thing the chat waits on.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Line {
    /// `turn`, `queued`, `sign_in`, `approval`, `question`, `plan`, `wait`, `command`, `pr`, `given`,
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
    /// A running turn that has made no progress past the limit: when it last did.
    pub silent_us: Option<i64>,
    /// The turn a `turn`, `queued`, `sign_in` or `approval` line is about.
    #[serde(skip)]
    pub turn: Option<String>,
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
            silent_us: None,
            turn: None,
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

/// Where one agent of the chat stands: the one state its face shows (0071).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Standing {
    pub agent: String,
    pub name: String,
    /// The most pressing that holds: `needs_you`, `cant_run`, `working`, `stopped`, `blocked`,
    /// `waiting` or `free`.
    pub state: &'static str,
    /// In a phrase: "Waiting for CI on #157", "Held", "Usage limit".
    pub words: String,
    /// What, in a word or two, for the face's card: "Approval", "Signed out", "Gathr restarted".
    pub what: Option<String>,
    /// The task it is on: its running turn's, its blocked task, the turn that failed or stopped.
    pub task: Option<i64>,
    /// Since when it has been in this state.
    pub since_us: Option<i64>,
    /// Waiting: for whom or what, each with since when.
    pub on: Vec<On>,
    /// Out of usage: when it resets.
    pub until_us: Option<i64>,
    /// Waiting: the looks at one of its waits keep failing, so nothing moves it.
    pub failing: bool,
}

/// What Gathr itself has going in the chat, that no agent is on: its update, a pull request it
/// lands, a command an agent's ended turn left running. Each is said as an agent's state is.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Own {
    /// `update`, `pr` or `command`.
    pub kind: &'static str,
    /// `working` or `waiting`.
    pub state: &'static str,
    /// In a few words: "Update to 0.1.240", "#18 to land", what the command runs.
    pub what: String,
    /// A command's: the agent whose turn left it running; none for the chat's own.
    pub name: Option<String>,
    pub since_us: Option<i64>,
    /// The looks at a pull request it lands keep failing.
    pub failing: bool,
}

impl Own {
    fn new(kind: &'static str, state: &'static str, what: String) -> Own {
        Own {
            kind,
            state,
            what,
            name: None,
            since_us: None,
            failing: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Answer {
    pub lines: Vec<Line>,
    pub agents: Vec<Standing>,
    /// Gathr's own, while the service runs: its update, then the PRs it lands, then commands.
    pub gathr: Vec<Own>,
}

/// The answer as the API and the command line give it: its lines and agents, the same in words,
/// the clock its ages are counted from, and whether the running service or the database said it.
pub fn shown(answer: &Answer, now_us: i64, from: &str) -> serde_json::Value {
    serde_json::json!({
        "lines": answer.lines,
        "agents": answer.agents,
        "gathr": answer.gathr,
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
    for (lead, title, sent) in crate::plans::waiting(conn, chat)? {
        let line = Line::new("plan", format!("A plan for you: {title}"), Moves::You);
        lines.push(line.of(Some(&lead), None, sent));
    }
    wait_lines(conn, chat, &pauses, &waiting, &mut lines)?;
    let mut gathr = Vec::new();
    if let Some(version) = &live.updating {
        let mut line = Line::new(
            "update",
            format!("Gathr is updating to {version}"),
            Moves::Itself,
        );
        line.from = "service";
        lines.push(line);
        gathr.push(Own::new(
            "update",
            "working",
            format!("Update to {version}"),
        ));
    }
    pr_lines(conn, chat, live, &mut lines, &mut gathr)?;
    command_lines(conn, chat, &mut lines, &mut gathr)?;
    task_lines(conn, chat, &pauses, &open, &turns, &mut lines)?;
    if !live.running {
        gathr.clear();
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
    let saved = (&pauses[..], &turns[..], &open[..], &waiting[..]);
    let agents = standing::standings(conn, chat, live, saved, now_us)?;
    Ok(Answer {
        lines,
        agents,
        gathr,
    })
}

/// The chat's turns under way or to come. One to come that a cancel or its task's wait marked is
/// dropped when its agent's queue reaches it, never run: what waits is its task's line.
fn turns(conn: &Connection, chat: &str) -> Result<Vec<Turn>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, agent_id, task, status, held, why, created_ts FROM turns
         WHERE chat_id = ?1 AND (status = 'running'
             OR (status IN ('queued', 'waiting_sign_in') AND stop_why IS NULL))
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
                if let Some(since) = live.approvals.get(&t.id) {
                    let what = format!("{who} needs your approval");
                    let mut asks =
                        Line::new("approval", what, Moves::You).of(agent, t.task, *since);
                    asks.from = "service";
                    asks.turn = Some(t.id.clone());
                    lines.push(asks);
                }
                match live.running {
                    true => {
                        let working = format!("{who} is working{on_task}");
                        let mut line = Line::new("turn", working, Moves::Itself);
                        // It runs, with no progress past the limit: it moves nothing.
                        if let Some(since) = live.silent.get(&t.id) {
                            line.moves = Moves::Nobody;
                            line.silent_us = Some(*since);
                        }
                        line
                    }
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
        lines.push(Line {
            turn: Some(t.id.clone()),
            ..line.of(agent, t.task, t.created)
        });
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
    // A pause the person lifted holds nothing, though its mark stays on the turn until the agent's
    // queue reads the pauses again and lets it go.
    let pause = pauses::holding(pauses, chat, t.agent.as_deref());
    let held = t
        .held
        .as_deref()
        .filter(|h| *h != "pause" || pause.is_some());
    Ok(match held {
        Some("pause") => {
            let stopped = pause.is_some_and(|p| p.stopped);
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

fn command_lines(
    conn: &Connection,
    chat: &str,
    lines: &mut Vec<Line>,
    gathr: &mut Vec<Own>,
) -> Result<(), Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT b.description, b.started_ts, t.agent_id, t.task, t.status = 'running' FROM background b
         JOIN turns t ON t.id = b.turn_id WHERE t.chat_id = ?1 ORDER BY b.started_ts",
    )?;
    let rows = stmt.query_map([chat], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
    })?;
    for row in rows {
        let (described, started, agent, task, running): (String, i64, Option<String>, _, bool) =
            row?;
        let who = whose(conn, agent.as_deref())?;
        let what = format!("{who} runs a command in the background: {described}");
        lines.push(Line::new("command", what, Moves::Itself).of(agent.as_deref(), task, started));
        // While its turn runs, the command is its agent's work; once the turn ends, Gathr's.
        if !running {
            let mut own = Own::new("command", "working", described);
            own.name = agent.is_some().then_some(who);
            own.since_us = Some(started);
            gathr.push(own);
        }
    }
    Ok(())
}

/// The PRs Gathr may land (0049): each as its last look left it.
fn pr_lines(
    conn: &Connection,
    chat: &str,
    live: &Live,
    lines: &mut Vec<Line>,
    gathr: &mut Vec<Own>,
) -> Result<(), Error> {
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
        // Past its project's line, it waits on the person's answer, or on a new head (0054).
        let (saw, moves) = match changes::standing(conn, chat, r.pr)? {
            Some(changes::Standing::Asked(q)) => {
                (format!("waits on your answer to #{q}"), Moves::You)
            }
            Some(changes::Standing::SentBack) => {
                let why = "you sent it back; waits on a new head";
                (why.to_string(), Moves::Others)
            }
            Some(changes::Standing::Cancelled(q)) => {
                let why = format!("you cancelled #{q}; waits on a new head");
                (why, Moves::Others)
            }
            None if moving => (saw, Moves::Itself),
            None => (saw, Moves::Nobody),
        };
        let on = format!("#{} to land", r.pr);
        // Gathr's while it lands it by itself, as its last look that worked left it: not once it is
        // left to its reviewer or the person.
        if moves == Moves::Itself {
            let mut own = Own::new("pr", "waiting", on.clone());
            own.since_us = clean;
            own.failing = live
                .landing
                .get(&r.pr)
                .is_some_and(|(failed, _)| *failed >= waits::SHOWN_AFTER);
            gathr.push(own);
        }
        let mut line = Line::new("pr", format!("#{} to land: {saw}", r.pr), moves);
        line.looked_us = looked;
        line.agent = Some(r.reviewer.clone());
        line.task = Some(r.task);
        line.since_us = clean;
        lines.push(line.on(&on));
    }
    Ok(())
}

/// The agents' tasks no turn is on: each says what it waits for, or that nothing is on it. One
/// whose owner a pause holds waits on the person, whatever else it waits for.
fn task_lines(
    conn: &Connection,
    chat: &str,
    pauses: &[Pause],
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
        let paused = pauses::holding(pauses, chat, Some(agent)).is_some();
        let mut push = |mut line: Line, since: i64| {
            if paused {
                line.moves = Moves::You;
                line.what.push_str(", paused");
            }
            lines.push(line.of(Some(agent), Some(o.number), since))
        };
        let blocker = o.blocked_on.and_then(find);
        match o.state.as_str() {
            "waiting" => {
                let before: Vec<&Open> = o.after.iter().filter_map(|n| find(*n)).collect();
                for b in &before {
                    let by = named(b)?;
                    let what = format!("{this} waits for #{} ({by})", b.number);
                    push(Line::new("after", what, Moves::Others).on(&by), o.created);
                }
                // It follows its plan, which its lead writes and the person decides (0063).
                if before.is_empty() && o.reason.as_deref() == Some(tasks::plan_wait::PLAN_WAIT) {
                    let line = Line::new(
                        "plan_wait",
                        format!("{this} waits for its plan"),
                        Moves::Others,
                    );
                    let lead = crate::team::members(conn, chat)?
                        .into_iter()
                        .find(|m| m.role == Role::Supervisor);
                    let line = match lead {
                        Some(lead) => line.on(&lead.name),
                        None => line,
                    };
                    push(line, o.updated);
                } else if before.is_empty() {
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
                                // Its review waits on Gathr's question, whose line says so (0064).
                                false
                                    if matches!(
                                        changes::standing(conn, chat, *pr)?,
                                        Some(changes::Standing::Asked(_))
                                    ) =>
                                {
                                    continue;
                                }
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
                    .filter(|q| {
                        q.kind == "person" && q.request == Some(o.number) && q.state == "ready"
                    })
                    .filter(|q| {
                        !holds
                            .iter()
                            .any(|h| matches!(h, Hold::Given { number, .. } if *number == q.number))
                    })
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
                    (true, true) => (Moves::Nobody, format!("{giver} was told")),
                    (true, false) => (Moves::Nobody, String::new()),
                    (false, _) => (Moves::Others, format!("{giver} hears of it next")),
                };
                let what = match told.is_empty() {
                    true => format!("{this} is {state}: {reason}"),
                    false => format!("{this} is {state}: {} {told}", tasks::sentence(reason)),
                };
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
