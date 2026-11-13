//! Tasks: work an agent owes in a chat, with named states and fixed transitions
//! (docs/decisions/0010-coordination-as-state.md). Each change and its event commit in one
//! transaction; the turns it calls for come back as [`Start`]s for the service to queue.

use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

mod held;
pub mod person;
mod records;
mod report;
pub mod stale;
pub mod touched;
mod words;

use records::{OptionalTask, read, set, state_of};

pub use held::background_changed;
pub use report::{Finish, Report, Said, Took, finish};

pub use words::{name_of, owner_name, wait_name};

/// The longest `what` of a question for the person: one line, read at a glance.
pub const WHAT_MOST: usize = 200;
/// The longest title of a task an agent gives: its hand-off line reads whole (ux-issues 27).
pub const TITLE_MOST: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum State {
    Waiting,
    Ready,
    Working,
    Held,
    Done,
    Blocked,
    Failed,
    Stopped,
    Cancelled,
}

const STATES: [(State, &str); 9] = [
    (State::Waiting, "waiting"),
    (State::Ready, "ready"),
    (State::Working, "working"),
    (State::Held, "held"),
    (State::Done, "done"),
    (State::Blocked, "blocked"),
    (State::Failed, "failed"),
    (State::Stopped, "stopped"),
    (State::Cancelled, "cancelled"),
];

/// The states nobody is on: the giver hears back once all its tasks from one turn are in one.
pub const SETTLED: &str = "('done', 'blocked', 'failed', 'stopped', 'cancelled')";

impl State {
    pub fn as_str(self) -> &'static str {
        STATES
            .iter()
            .find(|(s, _)| *s == self)
            .map_or("", |(_, n)| n)
    }

    fn named(text: &str) -> State {
        STATES
            .iter()
            .find(|(_, n)| *n == text)
            .map_or(State::Waiting, |(s, _)| *s)
    }

    pub fn settled(self) -> bool {
        !matches!(
            self,
            State::Waiting | State::Ready | State::Working | State::Held
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Work,
    Review,
    /// The person's: only the chat's Supervisor gives one.
    Person,
    /// A member's need for the person's answer, owned by the Supervisor, who checks it first.
    #[serde(rename = "person_request")]
    PersonRequest,
}

impl Kind {
    pub fn named(text: &str) -> Option<Kind> {
        match text {
            "work" => Some(Kind::Work),
            "review" => Some(Kind::Review),
            "person" => Some(Kind::Person),
            "person_request" => Some(Kind::PersonRequest),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Kind::Work => "work",
            Kind::Review => "review",
            Kind::Person => "person",
            Kind::PersonRequest => "person_request",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Task {
    pub number: i64,
    pub chat: String,
    /// The agent who gave it; none when the person did.
    pub giver: Option<String>,
    /// The agent who does it; none when it's the person's.
    pub owner: Option<String>,
    /// The turn that gave it: its owner's work sits under that turn's message.
    pub given_in: Option<String>,
    pub parent: Option<i64>,
    pub kind: Kind,
    pub what: String,
    pub why: Option<String>,
    pub state: State,
    pub reason: Option<String>,
    pub after: Vec<i64>,
    pub blocked_on: Option<i64>,
    pub pr: Option<i64>,
    pub reviewed: Option<String>,
    pub outdated: bool,
    /// How the person answers it (a person task or a request for one).
    pub form: Option<person::Form>,
    pub choices: Vec<person::Choice>,
    pub files: Vec<person::File>,
    /// The member's request a person task was filed for.
    pub request: Option<i64>,
    /// The person's answer, as sent.
    pub answer: Option<serde_json::Value>,
    pub created_us: i64,
    /// Since when nothing has moved it, once it's stale (F11).
    pub stale_us: Option<i64>,
    /// The turn its giver heard it was settled in: the chat shows the person's answer there.
    pub heard_in: Option<String>,
    /// Its PR's head as the PR looks last saw it: the commit whose checks run shows on it (0043 C6).
    pub pr_head: Option<String>,
    /// Its owner's last report, kept until the owner reports again or the task is restarted (0047).
    pub report: Option<Report>,
    /// Gathr asked its owner for a report, and none has come since.
    pub asked: bool,
    /// What a held task still has open, each in a few words.
    pub waiting_on: Vec<String>,
    /// Given in passing: it doesn't hold the task whose turn gave it (G164).
    pub in_passing: bool,
}

/// A turn Gathr starts for an agent: a task's, a wake-up or a hear-back. `task` is the task it
/// works on. It's saved, queued with Gathr's message as its ask, in the transaction of the change
/// that calls for it: a stop right after that commit loses nothing, as the next start takes it over.
#[derive(Debug, Clone, PartialEq)]
pub struct Start {
    pub chat: String,
    pub agent: String,
    pub task: Option<i64>,
    /// The turn it answers: the one that gave its task, or that left its wait.
    pub answers: Option<String>,
    pub text: String,
    pub turn: String,
    pub trace: String,
    /// Gathr's message, as the live socket sends it.
    pub message: serde_json::Value,
    /// It's a hear-back: what became of the tasks its agent gave (`held::hear_backs`), which a
    /// merge doesn't send the agent that merged (G159).
    pub hear_back: bool,
}

impl Start {
    pub fn queue(
        conn: &Connection,
        (chat, agent): (&str, &str),
        (task, answers): (Option<i64>, Option<&str>),
        text: String,
    ) -> Result<Start, Error> {
        let (turn, trace, message) = Self::save(conn, (chat, Some(agent)), (task, answers), &text)?;
        Ok(Start {
            chat: chat.to_string(),
            agent: agent.to_string(),
            task,
            answers: answers.map(String::from),
            text,
            turn,
            trace,
            message,
            hear_back: false,
        })
    }

    /// Saves the turn and its ask, for an agent or (in a chat sent to no agent) none: its id, trace
    /// and message.
    pub fn save(
        conn: &Connection,
        (chat, agent): (&str, Option<&str>),
        (task, answers): (Option<i64>, Option<&str>),
        text: &str,
    ) -> Result<(String, String, serde_json::Value), Error> {
        let (turn, trace) = (crate::ids::ulid(now_us()), crate::ids::trace_id());
        crate::store::add_turn(conn, chat, &turn, &trace, (agent, task, answers))?;
        let message = crate::store::add_message(conn, chat, &turn, "gathr", "text", text)?;
        Ok((turn, trace, message))
    }
}

/// A new task.
pub struct Give<'a> {
    pub chat: &'a str,
    pub giver: Option<&'a str>,
    pub owner: &'a str,
    pub given_in: Option<&'a str>,
    pub parent: Option<i64>,
    pub kind: Kind,
    pub what: &'a str,
    pub why: Option<&'a str>,
    pub after: &'a [i64],
    pub pr: Option<i64>,
    /// Given in passing: the giver's own task doesn't wait on it, and it's heard back on by itself.
    pub in_passing: bool,
}

/// How a turn on a task ended.
pub enum Outcome {
    /// It ended well.
    Ok,
    Failed(String),
    Stopped(String),
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// Gives a task: `waiting` while its `after` tasks aren't all done, else `ready` with its turn.
pub fn give(conn: &mut Connection, g: &Give, trace: &str) -> Result<(Task, Vec<Start>), Error> {
    let what = g.what.trim();
    if what.is_empty() || what.contains('\n') {
        return Err(refused(format!(
            "`what` is one line of 1 to {TITLE_MOST} characters"
        )));
    }
    let long = what.chars().count();
    if long > TITLE_MOST {
        return Err(refused(format!(
            "`what` is {long} characters: cut {} to fit {TITLE_MOST}, and put the details in `why`",
            long - TITLE_MOST
        )));
    }
    let tx = conn.transaction()?;
    crate::archive::refuse_archived(&tx, g.chat)?;
    in_team(&tx, g.chat, g.owner)?;
    for &n in g.after.iter().chain(&g.parent) {
        let chat: Option<String> = tx
            .query_row("SELECT chat_id FROM tasks WHERE number = ?1", [n], |r| {
                r.get(0)
            })
            .optional()?;
        if chat.as_deref() != Some(g.chat) {
            return Err(refused(format!("there's no task #{n} in this chat")));
        }
    }
    let now = now_us();
    tx.execute(
        "INSERT INTO tasks (chat_id, giver, owner, given_in, parent, kind, what, why, state, pr,
                            created_ts, updated_ts, in_passing)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'waiting', ?9, ?10, ?10, ?11)",
        params![
            g.chat,
            g.giver,
            g.owner,
            g.given_in,
            g.parent,
            g.kind.as_str(),
            what,
            g.why.map(str::trim).filter(|w| !w.is_empty()),
            g.pr,
            now,
            g.in_passing
        ],
    )?;
    let number = tx.last_insert_rowid();
    for after in g.after {
        tx.execute(
            "INSERT OR IGNORE INTO task_after (task, after_task) VALUES (?1, ?2)",
            [number, *after],
        )?;
    }
    let mut starts = Vec::new();
    if waiting_on(&tx, number)?.is_empty() {
        starts.push(ready(&tx, number, None)?);
    }
    let task = read(&tx, number)?;
    let event = Event::point("gathr.task.give").trace(trace).chat(g.chat);
    event
        .task(number)
        .ok()
        .attr("gathr.task.giver", g.giver)
        .attr("gathr.task.owner", g.owner)
        .attr("gathr.task.state", task.state.as_str())
        .attr("gathr.task.given_in", g.given_in)
        .insert(&tx)?;
    tx.commit()?;
    Ok((task, starts))
}

/// Whether a turn Gathr queued on `task` may start (0010's gate): `None`, or why it won't run.
pub fn may_start(conn: &Connection, task: i64) -> Result<Option<String>, Error> {
    let state = state_of(conn, task)?;
    Ok(match state {
        None => Some(format!("task #{task} is gone")),
        Some(State::Done | State::Cancelled) => Some(format!(
            "task #{task} is {}",
            state.map_or("", |s| s.as_str())
        )),
        Some(State::Waiting) => Some(format!("task #{task} still waits on its `after` tasks")),
        Some(_) => None,
    })
}

/// Why the turn is to stop: its task was cancelled while it was queued or ran, even if it was
/// restarted since.
pub fn stop_why(conn: &Connection, turn: &str) -> Result<Option<String>, Error> {
    let why: Option<Option<String>> = conn
        .query_row("SELECT stop_why FROM turns WHERE id = ?1", [turn], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(why.flatten())
}

/// A turn on the task has started: it's `working`.
pub fn started(conn: &mut Connection, task: i64, trace: &str) -> Result<(), Error> {
    in_tx(conn, |tx| started_in(tx, task, false, trace))
}

/// `carries_on`: the turn goes on from one that was stopped on the same task (an update, Stop now,
/// a usage limit). It's the same piece of work, so what its owner reported in it still stands.
fn started_in(tx: &Transaction, task: i64, carries_on: bool, trace: &str) -> Result<(), Error> {
    let Some(was) = state_of(tx, task)? else {
        return Ok(());
    };
    if !was.settled() || matches!(was, State::Blocked | State::Failed | State::Stopped) {
        set(tx, task, State::Working, None, trace)?;
        // Its owner is at work on what it left: that word no longer stands. A done does (0047).
        if !carries_on {
            tx.execute(
                "UPDATE tasks SET finish = NULL, finish_note = NULL, finish_see = NULL, finish_ts = NULL
                 WHERE number = ?1 AND finish = 'left'",
                [task],
            )?;
        }
    }
    Ok(())
}

/// A turn on the task has ended: a failure or a stop is what it is, and a turn that ended well
/// settles by its owner's report, never by having ended (0047).
pub fn ended(
    conn: &mut Connection,
    task: i64,
    outcome: Outcome,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    in_tx(conn, |tx| ended_in(tx, task, outcome, trace))
}

fn ended_in(
    tx: &Transaction,
    task: i64,
    outcome: Outcome,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let Some(t) = read(tx, task).optional_task()? else {
        return Ok(Vec::new());
    };
    let mut starts = Vec::new();
    // Cancelled or settled while the turn ran: it stays so, and nothing follows twice. A turn that
    // failed or stopped before it ran (no engine, no worktree) leaves the task ready: it settles too.
    let before_it_ran = t.state == State::Ready && !matches!(outcome, Outcome::Ok);
    if t.state == State::Working || before_it_ran {
        match outcome {
            Outcome::Failed(why) => set(tx, task, State::Failed, Some(&why), trace)?,
            Outcome::Stopped(why) => set(tx, task, State::Stopped, Some(&why), trace)?,
            Outcome::Ok => starts.extend(held::settle(tx, &t, trace)?),
        }
        starts.extend(cascade(tx, task, trace)?);
    }
    Ok(starts)
}

/// A turn has ended: the agents who gave tasks in it hear back, once each is settled.
pub fn turn_ended(conn: &mut Connection, turn: &str, trace: &str) -> Result<Vec<Start>, Error> {
    in_tx(conn, |tx| held::hear_backs(tx, turn, trace))
}

/// A turn whose status changed.
pub struct Moved<'a> {
    pub chat: &'a str,
    pub turn: &'a str,
    pub agent: Option<&'a str>,
    pub task: Option<i64>,
    pub trace: &'a str,
    /// A stopped turn whose carry-on waits (Stop now, a usage limit): its task stays as it was,
    /// with this as why nothing runs on it. Nobody is told it settled (0047).
    pub carries_on: Option<&'a str>,
}

/// Saves a turn's status with what it does, in one transaction: its task moves, its giver hears
/// back, and waits on its agent may fire. A stop can't fall between an end and the round it closes.
pub fn turn_moved(
    conn: &mut Connection,
    m: &Moved,
    status: &str,
    why: Option<&str>,
) -> Result<Vec<Start>, Error> {
    in_tx(conn, |tx| {
        crate::store::set_turn(tx, m.turn, status, why)?;
        let mut starts = Vec::new();
        let outcome = match status {
            "running" => {
                if let Some(task) = m.task {
                    let carries_on: bool = tx.query_row(
                        "SELECT EXISTS (SELECT 1 FROM turns u JOIN turns was ON was.id = u.answers
                         WHERE u.id = ?1 AND was.task = u.task AND was.status = 'stopped')",
                        [m.turn],
                        |r| r.get(0),
                    )?;
                    started_in(tx, task, carries_on, m.trace)?;
                }
                return Ok(starts);
            }
            "done" => Outcome::Ok,
            "failed" => Outcome::Failed(why.unwrap_or(status).to_string()),
            "stopped" => Outcome::Stopped(why.unwrap_or(status).to_string()),
            _ => return Ok(starts),
        };
        match (m.task, m.carries_on) {
            (Some(task), Some(waits)) if status == "stopped" => {
                if state_of(tx, task)? == Some(State::Working) {
                    set(tx, task, State::Working, Some(waits), m.trace)?;
                }
            }
            (Some(task), _) => starts.extend(ended_in(tx, task, outcome, m.trace)?),
            (None, _) => {}
        }
        starts.extend(held::hear_backs(tx, m.turn, m.trace)?);
        if let Some(agent) = m.agent {
            starts.extend(crate::waits::agent_free_in(tx, m.chat, agent, m.trace)?);
        }
        Ok(starts)
    })
}

/// Runs `f` in a transaction, committed when it succeeds.
fn in_tx<R>(
    conn: &mut Connection,
    f: impl FnOnce(&Transaction) -> Result<R, Error>,
) -> Result<R, Error> {
    let tx = conn.transaction()?;
    let r = f(&tx)?;
    tx.commit()?;
    Ok(r)
}

/// Cancels a task not done or cancelled yet, by its giver or the person (`by` none); its open waits
/// end. A member's request the person has answered settles as done instead.
pub fn cancel(
    conn: &mut Connection,
    by: Option<&str>,
    task: i64,
    why: &str,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let t = read(&tx, task).map_err(|_| refused(format!("there's no task #{task}")))?;
    may_change(&t, by, "cancel")?;
    if matches!(t.state, State::Done | State::Cancelled) {
        let state = t.state.as_str();
        return Err(refused(format!("task #{task} is already {state}")));
    }
    // A request the person already answered is done with that answer, not cancelled: what waited
    // on it carries on, and its giver hears the answer (G183).
    if t.kind == Kind::PersonRequest
        && let Some(answer) = person::answered(&tx, task)?
    {
        set(&tx, task, State::Done, Some(&answer), trace)?;
        end_waits(&tx, task, "its request was answered")?;
        let starts = cascade(&tx, task, trace)?;
        tx.commit()?;
        return Ok(starts);
    }
    let mut starts = cancel_in(&tx, task, why, trace)?;
    if t.kind == Kind::Person {
        starts.extend(person::heard(&tx, &[task], by, trace)?);
    }
    tx.commit()?;
    Ok(starts)
}

/// Starts a failed, stopped, blocked or cancelled task again, by its giver or the person.
pub fn restart(
    conn: &mut Connection,
    by: Option<&str>,
    task: i64,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let t = read(&tx, task).map_err(|_| refused(format!("there's no task #{task}")))?;
    may_change(&t, by, "restart")?;
    crate::archive::refuse_archived(&tx, &t.chat)?;
    if !t.state.settled() || t.state == State::Done {
        let state = t.state.as_str();
        return Err(refused(format!(
            "Task #{task} is {state}. Only failed, stopped, blocked or cancelled tasks can restart."
        )));
    }
    let carry = match t.state {
        State::Stopped | State::Failed => t.reason.clone(),
        _ => None,
    };
    let starts = match waiting_on(&tx, task)?.is_empty() {
        true => vec![ready(&tx, task, carry.as_deref())?],
        false => {
            set(&tx, task, State::Waiting, None, trace)?;
            Vec::new()
        }
    };
    tx.execute(
        "UPDATE tasks SET asks = 0, blocked_on = NULL, heard = 0, review_done = 0, finish = NULL,
                          finish_note = NULL, finish_see = NULL, finish_ts = NULL
         WHERE number = ?1",
        [task],
    )?;
    tx.commit()?;
    Ok(starts)
}

/// The service started: tasks whose turn was cut off by the stop are `stopped`, with why, for a
/// restart that carries on (0010). Their dependants stay `waiting`. A cut-off turn that gave tasks
/// ends here, so its giver's hear-back is queued for the start to take over.
pub fn recover(conn: &mut Connection) -> Result<usize, Error> {
    let tx = conn.transaction()?;
    let working: Vec<i64> = {
        // A task with a carry-on waiting (a queued turn that answers a turn on the same task)
        // carries on in it: it isn't stopped (0032).
        let mut stmt = tx.prepare(
            "SELECT number FROM tasks WHERE state = 'working' AND NOT EXISTS
             (SELECT 1 FROM turns u JOIN turns was ON was.id = u.answers
              WHERE u.task = tasks.number AND u.status = 'queued' AND was.task = tasks.number)",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let giving: Vec<String> = {
        let mut stmt = tx.prepare(
            "SELECT DISTINCT u.id FROM turns u JOIN tasks t ON t.given_in = u.id
             WHERE u.status = 'running'",
        )?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    let trace = crate::ids::trace_id();
    for &task in &working {
        let why = "Gathr stopped during its turn";
        set(&tx, task, State::Stopped, Some(why), &trace)?;
    }
    // Background commands ended with their engines: a task held only on one carries on.
    crate::background::clear(&tx)?;
    let holding: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT number FROM tasks WHERE state = 'held'")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for task in holding {
        if held::open_on(&tx, task)?.is_empty() {
            let carry = "Gathr stopped and ended your background command";
            ready(&tx, task, Some(carry))?;
        }
    }
    for turn in &giving {
        let why = Some("the service stopped during this turn");
        crate::store::set_turn(&tx, turn, "failed", why)?;
        held::hear_backs(&tx, turn, &trace)?;
    }
    tx.commit()?;
    Ok(working.len())
}

/// The engine in `chat` for `agent` exited: its background commands ended with it, so they're
/// forgotten, and a task held only on one carries on, as after a restart.
pub fn engine_gone(
    conn: &mut Connection,
    engine: (&str, Option<&str>),
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let mut held_on: Vec<i64> = crate::background::set(&tx, engine, &[], None)?
        .into_iter()
        .filter_map(|(_, task)| task)
        .collect();
    held_on.dedup();
    let mut starts = Vec::new();
    for task in held_on {
        if state_of(&tx, task)? == Some(State::Held) && held::open_on(&tx, task)?.is_empty() {
            let carry = "your engine closed and ended your background command";
            starts.push(ready(&tx, task, Some(carry))?);
        }
    }
    tx.commit()?;
    Ok(starts)
}

/// A task's PR has been looked at: merged settles work reported done and any review, closed
/// cancels it (G67, G44). `covered` says the commits after the one reviewed only bring in the base. A review
/// whose PR moved past its commit is out of date (G73).
pub fn pr_looked(
    conn: &mut Connection,
    task: i64,
    pr_state: &str,
    head: Option<&str>,
    covered: bool,
    trace: &str,
) -> Result<Vec<Start>, Error> {
    let tx = conn.transaction()?;
    let t = read(&tx, task)?;
    if let (Some(reviewed), Some(head)) = (&t.reviewed, head) {
        let past = !head.starts_with(reviewed.as_str()) && !reviewed.starts_with(head);
        // Past the commit reviewed, but by nothing except clean merges of the base: the review
        // still covers it (G165), as the merge tool finds.
        let past = past && !covered;
        tx.execute(
            "UPDATE tasks SET outdated = ?2 WHERE number = ?1",
            params![task, past],
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

/// The chat's tasks, newest first.
pub fn list(conn: &Connection, chat: &str) -> Result<Vec<Task>, Error> {
    let numbers: Vec<i64> = {
        let sql = "SELECT number FROM tasks WHERE chat_id = ?1 ORDER BY number DESC";
        let mut stmt = conn.prepare_cached(sql)?;
        let rows = stmt.query_map([chat], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    numbers.into_iter().map(|n| read(conn, n)).collect()
}

pub fn get(conn: &Connection, task: i64) -> Result<Option<Task>, Error> {
    read(conn, task).optional_task()
}

/// Whether a turn is running on the task now.
fn turn_running(conn: &Connection, task: i64) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM turns WHERE task = ?1 AND status = 'running')",
        [task],
        |r| r.get(0),
    )?)
}

/// Whether a turn on the task is running or waiting to.
fn turn_due(conn: &Connection, task: i64) -> Result<bool, Error> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM turns WHERE task = ?1
         AND status IN ('queued', 'waiting_sign_in', 'running'))",
        [task],
        |r| r.get(0),
    )?)
}

/// Who may cancel or restart: the giver, or the person (`by` none).
fn may_change(t: &Task, by: Option<&str>, what: &str) -> Result<(), Error> {
    match by.is_none() || by == t.giver.as_deref() {
        true => Ok(()),
        false => Err(refused(format!(
            "only task #{}'s giver or the person may {what} it",
            t.number
        ))),
    }
}

fn in_team(conn: &Connection, chat: &str, agent: &str) -> Result<(), Error> {
    let members = crate::team::members(conn, chat)?;
    if members.iter().any(|m| m.agent == agent) {
        return Ok(());
    }
    let names: Vec<String> = members.into_iter().map(|m| m.name).collect();
    Err(refused(match names.is_empty() {
        true => "tasks are given in a chat with a team".to_string(),
        false => format!(
            "that agent isn't in this chat: its agents are {}",
            names.join(", ")
        ),
    }))
}

/// The task's `after` tasks not done yet, or not held only on their merge.
fn waiting_on(conn: &Connection, task: i64) -> Result<Vec<i64>, Error> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT a.after_task FROM task_after a JOIN tasks t ON t.number = a.after_task
         WHERE a.task = ?1 AND t.state != 'done' AND NOT {}",
        held::ONLY_ITS_MERGE
    ))?;
    let rows = stmt.query_map([task], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Makes the task `ready`, with the turn that starts it.
fn ready(tx: &Transaction, task: i64, carry: Option<&str>) -> Result<Start, Error> {
    set(tx, task, State::Ready, None, &crate::ids::trace_id())?;
    let t = read(tx, task)?;
    let owner = t.owner.as_deref().ok_or_else(|| {
        refused(format!(
            "task #{task} is the person's: it has no turn to start"
        ))
    })?;
    let giver = match &t.giver {
        Some(id) => words::name_of(tx, id)?,
        None => "the person".to_string(),
    };
    let text = words::brief(&t, &giver, carry);
    let on = (Some(task), t.given_in.as_deref());
    Start::queue(tx, (&t.chat, owner), on, text)
}

/// What follows a task's change: dependants ready or blocked, tasks blocked on it carry on, and
/// the giver's hear-back once all it gave in that turn are settled.
fn cascade(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    let Some(state) = state_of(tx, task)? else {
        return Ok(Vec::new());
    };
    let mut starts = Vec::new();
    // A held task may count as done for its dependants (`waiting_on`); it settles nothing else.
    if !state.settled() && state != State::Held {
        return Ok(starts);
    }
    // A blocked dependant waits on another `after` task that failed or stopped: only a cancel
    // moves it.
    let dependants: Vec<(i64, bool)> = {
        let mut stmt = tx.prepare_cached(
            "SELECT t.number, t.state = 'blocked' FROM task_after a JOIN tasks t ON t.number = a.task
             WHERE a.after_task = ?1 AND t.state IN ('waiting', 'blocked')",
        )?;
        let rows = stmt.query_map([task], |r| Ok((r.get(0)?, r.get(1)?)))?;
        rows.collect::<Result<_, _>>()?
    };
    for (dependant, blocked) in dependants {
        if blocked && state != State::Cancelled {
            continue;
        }
        match state {
            State::Done | State::Held if waiting_on(tx, dependant)?.is_empty() => {
                starts.push(ready(tx, dependant, None)?)
            }
            State::Done | State::Held => {}
            // It can never go on (F11): cancelled at once, not left blocked.
            State::Cancelled => {
                let why = format!("#{task} was cancelled");
                starts.extend(cancel_in(tx, dependant, &why, trace)?);
            }
            _ => {
                block_after(tx, dependant, task, state, trace)?;
                starts.extend(cascade(tx, dependant, trace)?);
            }
        }
    }
    let on_it: Vec<i64> = {
        let mut stmt = tx.prepare_cached(
            "SELECT number FROM tasks WHERE blocked_on = ?1 AND state = 'blocked'",
        )?;
        let rows = stmt.query_map([task], |r| r.get(0))?;
        rows.collect::<Result<_, _>>()?
    };
    for blocked in on_it {
        let after: bool = tx.query_row(
            "SELECT EXISTS (SELECT 1 FROM task_after WHERE task = ?1 AND after_task = ?2)",
            [blocked, task],
            |r| r.get(0),
        )?;
        match state {
            // Blocked by Gathr for the task it comes after, which now counts as done for it.
            State::Done | State::Held if after => {
                if !waiting_on(tx, blocked)?.contains(&task) {
                    starts.extend(back_in_line(tx, blocked, trace)?);
                }
            }
            State::Done => {
                tx.execute(
                    "UPDATE tasks SET blocked_on = NULL WHERE number = ?1",
                    [blocked],
                )?;
                let carry = format!("the task blocking this one, #{task}, is done");
                starts.push(ready(tx, blocked, Some(&carry))?);
            }
            State::Cancelled => {
                let why = format!("The task blocking this one, #{task}, was cancelled");
                starts.extend(cancel_in(tx, blocked, &why, trace)?);
            }
            _ => {}
        }
    }
    let given_in: Option<String> = tx.query_row(
        "SELECT given_in FROM tasks WHERE number = ?1",
        [task],
        |r| r.get(0),
    )?;
    if let Some(turn) = given_in {
        starts.extend(held::hear_backs(tx, &turn, trace)?);
    }
    if state.settled() {
        starts.extend(held::after_review(tx, task, trace)?);
    }
    Ok(starts)
}

/// Blocks a task for the one it comes after, which failed, stopped or blocked, and records which:
/// it carries on when that one is restarted and finishes (`cascade`).
fn block_after(
    tx: &Transaction,
    task: i64,
    after: i64,
    state: State,
    trace: &str,
) -> Result<(), Error> {
    let why = format!("#{after} is {}", state.as_str());
    set(tx, task, State::Blocked, Some(&why), trace)?;
    tx.execute(
        "UPDATE tasks SET blocked_on = ?2 WHERE number = ?1",
        [task, after],
    )?;
    Ok(())
}

/// A task Gathr blocked for one it comes after goes back in line: ready, or waiting on what it
/// still comes after, or blocked for one of those that has stopped too.
fn back_in_line(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    tx.execute(
        "UPDATE tasks SET blocked_on = NULL WHERE number = ?1",
        [task],
    )?;
    let waiting = waiting_on(tx, task)?;
    if waiting.is_empty() {
        return Ok(vec![ready(tx, task, None)?]);
    }
    for after in waiting {
        if let Some(state) = state_of(tx, after)?.filter(|s| s.settled()) {
            block_after(tx, task, after, state, trace)?;
            return Ok(Vec::new());
        }
    }
    set(tx, task, State::Waiting, None, trace)?;
    Ok(Vec::new())
}

/// What follows the task's change, as `cascade`: for archiving, once its chat's tasks are all
/// cancelled.
pub(crate) fn followed(tx: &Transaction, task: i64, trace: &str) -> Result<Vec<Start>, Error> {
    cascade(tx, task, trace)
}

/// Cancels the task with why and nothing else: for archiving, which ends everything around it.
pub(crate) fn cancelled_with(
    conn: &Connection,
    task: i64,
    why: &str,
    trace: &str,
) -> Result<(), Error> {
    set(conn, task, State::Cancelled, Some(why), trace)
}

/// Cancels the task with why: its open waits end, and what follows it moves.
fn cancel_in(tx: &Transaction, task: i64, why: &str, trace: &str) -> Result<Vec<Start>, Error> {
    set(tx, task, State::Cancelled, Some(why), trace)?;
    end_waits(tx, task, "its task was cancelled")?;
    cascade(tx, task, trace)
}

fn end_waits(tx: &Transaction, task: i64, why: &str) -> Result<(), Error> {
    tx.execute(
        "UPDATE waits SET state = 'cancelled', ended_ts = ?2, line = ?3
         WHERE task = ?1 AND state IN ('open', 'met')",
        params![task, now_us(), why],
    )?;
    Ok(())
}
