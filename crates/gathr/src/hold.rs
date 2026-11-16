//! The hold (0063): in a team chat, building on a new ask waits until its plan is approved; a
//! repair goes ahead. One rule says whether a turn is held, read whenever a caller acts, never
//! stored: a held turn's building gives are refused here, its writes and pushes by the hooks.

pub mod look;

use std::path::Path;

use rusqlite::{Connection, OptionalExtension};

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::plans::{self, State};
use crate::tasks::{Give, Kind};
use crate::team::{self, Role};

/// What the one rule reads of a turn.
#[derive(Debug, Clone)]
pub struct Held {
    pub ask: Option<String>,
    pub task: Option<i64>,
    /// Its task is a look-into task, in a chat with two or more agents.
    pub look_into: bool,
    /// Its task is building work: a work task that isn't a look-into task.
    pub building: bool,
    /// It serves an ask, in a chat with two or more agents, that isn't a repair and has no
    /// approved plan; read only when its task is neither building nor a look-into task.
    pub unplanned: bool,
}

impl Held {
    /// The rule: a look-into task's turn, or one with no building task on an unplanned ask.
    pub fn held(&self) -> bool {
        self.look_into || (!self.building && self.unplanned)
    }
}

/// The one rule for a turn (I0.1); `None` when there's no such turn.
pub fn turn(conn: &Connection, turn: &str) -> Result<Option<Held>, Error> {
    type Row = (Option<String>, Option<i64>, bool, bool, bool, bool);
    let found: Option<Row> = conn
        .prepare_cached(
            "SELECT u.ask_id, u.task, COALESCE(t.look_into, 0),
                    COALESCE(t.kind = 'work' AND t.look_into = 0, 0),
                    EXISTS (SELECT 1 FROM asks a WHERE a.id = u.ask_id AND a.size = 'repair'),
                    (SELECT count(*) FROM chat_members m WHERE m.chat_id = u.chat_id) >= 2
             FROM turns u LEFT JOIN tasks t ON t.number = u.task WHERE u.id = ?1",
        )?
        .query_row([turn], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })
        .optional()?;
    let Some((ask, task, look_into, building, repair, team)) = found else {
        return Ok(None);
    };
    let unplanned = match &ask {
        Some(ask) if !repair && team && !building && !look_into => !approved(conn, ask)?,
        _ => false,
    };
    Ok(Some(Held {
        ask,
        task,
        look_into: look_into && team,
        building,
        unplanned,
    }))
}

/// H8.1: the ask has an approved plan.
fn approved(conn: &Connection, ask: &str) -> Result<bool, Error> {
    let sql = format!("SELECT {}", plans::APPROVED);
    Ok(conn.prepare_cached(&sql)?.query_row([ask], |r| r.get(0))?)
}

/// The ask's half of the rule, for its tasks (I0.2): in a chat with two or more agents, not a
/// repair, and no approved plan.
pub fn ask_held(conn: &Connection, ask: &str) -> Result<bool, Error> {
    let found: Option<bool> = conn
        .prepare_cached(
            "SELECT a.size IS NOT 'repair'
                    AND (SELECT count(*) FROM chat_members m WHERE m.chat_id = a.chat_id) >= 2
             FROM asks a WHERE a.id = ?1",
        )?
        .query_row([ask], |r| r.get(0))
        .optional()?;
    match found {
        Some(true) => Ok(!approved(conn, ask)?),
        _ => Ok(false),
    }
}

fn refused_with(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// What the words of a refusal read of the ask: its title, its lead, whether the person sized it,
/// and its open plan's state.
struct About {
    title: String,
    lead: Option<String>,
    lead_agent: Option<String>,
    by_person: bool,
    plan: Option<State>,
}

fn about(conn: &Connection, chat: &str, ask: Option<&str>) -> Result<About, Error> {
    let lead = team::members(conn, chat)?
        .into_iter()
        .find(|m| m.role == Role::Supervisor);
    let Some(ask) = ask else {
        let (lead, lead_agent) = (lead.as_ref().map(|m| m.name.clone()), lead.map(|m| m.agent));
        let (title, by_person, plan) = (String::new(), false, None);
        return Ok(About {
            title,
            lead,
            lead_agent,
            by_person,
            plan,
        });
    };
    let by: Option<String> = conn
        .query_row("SELECT sized_by FROM asks WHERE id = ?1", [ask], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    Ok(About {
        title: crate::asks::title(conn, ask)?,
        lead: lead.as_ref().map(|m| m.name.clone()),
        lead_agent: lead.map(|m| m.agent),
        by_person: by.as_deref() == Some("person"),
        plan: plans::open_state(conn, ask)?,
    })
}

fn plan_word(plan: State) -> &'static str {
    match plan {
        State::Checking => "checking",
        State::Waiting => "waiting",
        State::Revising => "revising",
        State::Approved => "approved",
        State::Dropped => "dropped",
    }
}

/// Refuses a held turn's building give (I1.2), recording it (I1.3). Gathr's own gives never pass
/// here: they share `give_in`, and wait for the plan at `ready` instead.
pub fn may_give(conn: &mut Connection, g: &Give, trace: &str) -> Result<(), Error> {
    if g.look_into && g.kind == Kind::Review {
        return Err(refused_with(
            "A review isn't a look-into task: leave out look_into, or give it as work.",
        ));
    }
    if g.look_into && g.pr.is_some() {
        return Err(refused_with(
            "A look-into task takes no pr: it changes nothing.",
        ));
    }
    let Some(given_in) = g.given_in.filter(|_| g.kind == Kind::Work && !g.look_into) else {
        return Ok(());
    };
    let Some(held) = turn(conn, given_in)?.filter(Held::held) else {
        return Ok(());
    };
    let a = about(conn, g.chat, held.ask.as_deref())?;
    let words = match held.task.filter(|_| held.look_into) {
        Some(n) => format!(
            "Task #{n} is a look-into task, so it gives no building work: give a look-into task \
             (look_into: true), or report what you found."
        ),
        None => give_words(&a, g.giver.is_some() && g.giver == a.lead_agent.as_deref()),
    };
    let r = Refusal {
        chat: g.chat.to_string(),
        trace: trace.to_string(),
        turn: given_in.to_string(),
        agent: g.giver.map(String::from),
        ask: held.ask.clone(),
        what: "give",
        why: if held.look_into {
            "look_into"
        } else {
            "unplanned"
        },
        plan: a.plan,
        tool: Some("give_task".to_string()),
    };
    refused(conn, &r)?;
    Err(refused_with(words))
}

/// A refused building give's words, by who gives and the ask's open plan.
fn give_words(a: &About, lead: bool) -> String {
    let title = &a.title;
    let name = a.lead.as_deref().unwrap_or("the lead");
    match (lead, a.plan) {
        (true, None) if a.by_person => format!(
            "\"{title}\" has no approved plan, so building waits: write its plan with plan, or give a look-into \
             task (look_into: true) for what the plan needs first."
        ),
        (true, None) => format!(
            "\"{title}\" has no approved plan, so building waits. If it is a repair, size it repair with size; \
             else write its plan with plan, or give a look-into task (look_into: true) for what the plan needs first."
        ),
        (true, Some(State::Checking)) => format!(
            "\"{title}\" has no approved plan yet: its plan is being checked, and building starts once the person \
             approves it. Meanwhile give a look-into task (look_into: true) if the plan needs more."
        ),
        (true, Some(State::Waiting)) => format!(
            "\"{title}\" has no approved plan yet: its plan waits for the person's Approve. Tell them so, and \
             don't write it again."
        ),
        (true, Some(_)) => format!(
            "\"{title}\" has no approved plan yet: its plan was sent back or its check found problems. Write the \
             next version with plan."
        ),
        (false, None) => format!(
            "\"{title}\" has no approved plan, so building waits: give a look-into task (look_into: true), or \
             tell {name} what you found."
        ),
        (false, Some(plan)) => {
            let is = match plan {
                State::Checking => "being checked".to_string(),
                State::Waiting => "waiting for the person's Approve".to_string(),
                _ => format!("being revised by {name}"),
            };
            format!(
                "\"{title}\" has no approved plan yet: its plan is {is}, so building waits. Give a look-into task \
                 (look_into: true), or tell {name} what you found."
            )
        }
    }
}

/// Where a held turn writes: its worktree, refused, and its own folder and Shared, each when it has
/// it (I2.3).
pub struct Place<'a> {
    pub worktree: &'a Path,
    pub own: Option<&'a Path>,
    pub shared: Option<&'a Path>,
}

/// What Gathr refused of a held turn, to record (I1.3).
#[derive(Debug, Clone)]
pub struct Refusal {
    pub chat: String,
    pub trace: String,
    pub turn: String,
    pub agent: Option<String>,
    pub ask: Option<String>,
    /// `give`, `write` or `push`.
    pub what: &'static str,
    /// `look_into` or `unplanned`.
    pub why: &'static str,
    pub plan: Option<State>,
    pub tool: Option<String>,
}

/// Records a refusal in a transaction of its own: the ask was held, so an unsized ask's line shows
/// (I6), and `gathr.hold.refused` on the turn's trace.
pub fn refused(conn: &mut Connection, r: &Refusal) -> Result<(), Error> {
    let tx = conn.transaction()?;
    if let Some(ask) = &r.ask {
        tx.execute(
            "UPDATE asks SET held_ts = COALESCE(held_ts, ?2) WHERE id = ?1",
            rusqlite::params![ask, now_us()],
        )?;
    }
    let mut event = Event::point("gathr.hold.refused")
        .trace(&r.trace)
        .chat(&r.chat)
        .ok()
        .attr("gathr.ask.id", r.ask.clone())
        .attr("gathr.turn.id", r.turn.as_str())
        .attr("gen_ai.agent.id", r.agent.clone())
        .attr("gathr.hold.what", r.what)
        .attr("gathr.hold.why", r.why);
    if let Some(plan) = r.plan {
        event = event.attr("gathr.hold.plan", plan_word(plan));
    }
    if let Some(tool) = &r.tool {
        event = event.attr("gen_ai.tool.name", tool.as_str());
    }
    event.insert(&tx)?;
    tx.commit()?;
    Ok(())
}

/// A held turn as the hooks read it: the rule, its chat, agent and trace, and the words' facts.
struct Seen {
    held: Held,
    chat: String,
    agent: Option<String>,
    trace: String,
    project: bool,
    about: About,
}

fn seen(conn: &Connection, turn_id: &str) -> Result<Option<Seen>, Error> {
    let Some(held) = turn(conn, turn_id)?.filter(Held::held) else {
        return Ok(None);
    };
    let (chat, agent, trace, project): (String, Option<String>, String, bool) = conn.query_row(
        "SELECT u.chat_id, u.agent_id, u.trace_id, c.project_id IS NOT NULL
         FROM turns u JOIN chats c ON c.id = u.chat_id WHERE u.id = ?1",
        [turn_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
    )?;
    let about = about(conn, &chat, held.ask.as_deref())?;
    Ok(Some(Seen {
        held,
        chat,
        agent,
        trace,
        project,
        about,
    }))
}

fn refusal(s: &Seen, turn: &str, what: &'static str) -> Refusal {
    Refusal {
        chat: s.chat.clone(),
        trace: s.trace.clone(),
        turn: turn.to_string(),
        agent: s.agent.clone(),
        ask: s.held.ask.clone(),
        what,
        why: if s.held.look_into {
            "look_into"
        } else {
            "unplanned"
        },
        plan: s.about.plan,
        tool: None,
    }
}

/// Whether a write of the turn's into its worktree is held (I2.2): the deny reason, and the record
/// to write, for its caller to name the tool. It only reads. Not held, or a chat with no project:
/// `None`.
pub fn write_held(
    conn: &Connection,
    turn: &str,
    place: &Place,
) -> Result<Option<(String, Refusal)>, Error> {
    let Some(s) = seen(conn, turn)?.filter(|s| s.project) else {
        return Ok(None);
    };
    let folders =
        |p: Option<&Path>, none: &str| p.map_or(none.to_string(), |p| p.display().to_string());
    let (worktree, own) = (
        place.worktree.display(),
        folders(place.own, "your own folder"),
    );
    let shared = folders(place.shared, "Shared");
    let instead = format!("Write notes, experiments and mock-ups in {own} or {shared} instead.");
    let why = match s.held.task.filter(|_| s.held.look_into) {
        Some(n) => {
            format!("Task #{n} is a look-into task: don't change files in {worktree}. {instead}")
        }
        None => {
            let title = &s.about.title;
            let mut why = format!(
                "Held until the plan for \"{title}\" is approved: don't change files in {worktree}. {instead}"
            );
            if s.agent.is_some() && s.agent == s.about.lead_agent {
                why.push_str(match s.about.plan {
                    None if s.about.by_person => "",
                    None => " If it is a repair, size it repair with size.",
                    Some(State::Checking) => " Its plan is being checked: building starts once the person approves it.",
                    Some(State::Waiting) => " Its plan waits for the person's Approve: tell them so, and don't write it again.",
                    Some(_) => " Write the next version of its plan with plan.",
                });
            }
            why
        }
    };
    let r = refusal(&s, turn, "write");
    Ok(Some((why, r)))
}

/// Whether a push of the turn's is held (I2.5): the answer for Gathr's pre-push hook, and the
/// record to write. It only reads.
pub fn push_held(conn: &Connection, turn: &str) -> Result<Option<(String, Refusal)>, Error> {
    let Some(s) = seen(conn, turn)? else {
        return Ok(None);
    };
    let why = match s.held.look_into {
        true => "held: a look-into task pushes nothing".to_string(),
        false => format!(
            "held: \"{}\" has no approved plan, so nothing is pushed for it",
            s.about.title
        ),
    };
    let r = refusal(&s, turn, "push");
    Ok(Some((why, r)))
}
