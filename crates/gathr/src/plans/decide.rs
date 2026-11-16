//! The person's word on a plan (0062, H6, H6a): Approve makes each part of the newest version a
//! task, word for word, with no turn of the lead's; Send back wakes the lead with the person's
//! words; Drop ends the plan. Each sets the version's word before it cancels the version's check,
//! so the check's end says no line.

use rusqlite::{Connection, OptionalExtension, Transaction, params};

use super::{Summary, check, refused};
use crate::Error;
use crate::event::Event;
use crate::ids::now_us;
use crate::tasks::{self, Brief, Give, Kind, Start, State};
use crate::team::Role;
use crate::words::Opened;

/// What the person's word did: the plan as it is now, and the turns it starts.
#[derive(Debug)]
pub struct Decided {
    pub summary: Summary,
    pub starts: Vec<Start>,
}

/// The version the person decides on, as the guard found it.
struct Deciding {
    chat: String,
    lead: String,
    version: i64,
    turn: String,
    title: String,
}

/// H6.1, in order: the plan, its chat, its newest version, undecided; for Approve and Send back,
/// also sent to the person and still its lead's.
fn guard(tx: &Connection, plan: i64, n: i64, sent: bool) -> Result<Deciding, Error> {
    let found: Option<(String, String)> = tx
        .query_row(
            "SELECT chat_id, lead FROM plans WHERE id = ?1",
            [plan],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let (chat, lead) = found.ok_or_else(|| refused(format!("there's no plan #{plan}")))?;
    crate::archive::refuse_archived(tx, &chat)?;
    let newest: i64 = tx.query_row(
        "SELECT max(n) FROM plan_versions WHERE plan_id = ?1",
        [plan],
        |r| r.get(0),
    )?;
    if n != newest {
        return Err(refused(format!(
            "Version {n} isn't the newest: version {newest} is."
        )));
    }
    type Row = (i64, String, String, bool, Option<String>);
    let (version, turn, title, reached, decided): Row = tx.query_row(
        "SELECT id, turn_id, title, sent_ts IS NOT NULL, decided FROM plan_versions
         WHERE plan_id = ?1 AND n = ?2",
        [plan, n],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )?;
    if let Some(decided) = decided {
        let word = decided.replace('_', " ");
        return Err(refused(format!("Version {n} is already {word}.")));
    }
    if sent && !reached {
        return Err(refused(format!("Version {n} is still being checked.")));
    }
    let members = crate::team::members(tx, &chat)?;
    let leads = members
        .iter()
        .any(|m| m.agent == lead && m.role == Role::Supervisor);
    if sent && !leads {
        let name = tasks::name_of(tx, &lead)?;
        return Err(refused(format!(
            "{name} no longer leads this chat: drop this plan."
        )));
    }
    Ok(Deciding {
        chat,
        lead,
        version,
        turn,
        title,
    })
}

/// Records the person's word on the version, before anything else changes.
fn decide(tx: &Connection, version: i64, word: &str, words: Option<&str>) -> Result<(), Error> {
    tx.execute(
        "UPDATE plan_versions SET decided = ?2, decided_ts = ?3, words = ?4 WHERE id = ?1",
        params![version, word, now_us(), words],
    )?;
    Ok(())
}

fn decided_line(tx: &Connection, version: i64, line: &str) -> Result<(), Error> {
    tx.execute(
        "UPDATE plan_versions SET decided_line = ?2 WHERE id = ?1",
        params![version, line],
    )?;
    Ok(())
}

fn event(d: &Deciding, (plan, n): (i64, i64), how: &str, trace: &str) -> Event {
    Event::point("gathr.plan.decided")
        .trace(trace)
        .chat(&d.chat)
        .ok()
        .attr("gathr.plan.id", plan)
        .attr("gathr.plan.version", n)
        .attr("gathr.plan.how", how)
}

fn done(tx: Transaction, plan: i64, starts: Vec<Start>) -> Result<Decided, Error> {
    let summary =
        super::summary(&tx, plan)?.ok_or_else(|| refused(format!("there's no plan #{plan}")))?;
    tx.commit()?;
    Ok(Decided { summary, starts })
}

/// "1 task", "3 tasks".
fn counted(k: usize) -> String {
    match k {
        1 => "1 task".to_string(),
        k => format!("{k} tasks"),
    }
}

/// Approve (H6.2): each part of version `n` becomes a task, in order, in one transaction.
pub fn approve(conn: &mut Connection, plan: i64, n: i64, trace: &str) -> Result<Decided, Error> {
    let tx = conn.transaction()?;
    let d = guard(&tx, plan, n, true)?;
    let parts = super::read_parts(&tx, d.version)?;
    let members = crate::team::members(&tx, &d.chat)?;
    for (i, p) in parts.iter().enumerate() {
        let k = i + 1;
        if !members.iter().any(|m| m.agent == p.owner) {
            let name = tasks::name_of(&tx, &p.owner)?;
            return Err(refused(format!(
                "Part {k}'s owner, {name}, has left this chat: send the plan back."
            )));
        }
        for &t in &p.after_tasks {
            if tasks::get(&tx, t)?.is_some_and(|t| t.state == State::Cancelled) {
                return Err(refused(format!(
                    "Part {k} comes after #{t}, which was cancelled: send the plan back."
                )));
            }
        }
    }
    decide(&tx, d.version, "approved", None)?;
    let why = format!("version {n} was approved");
    let mut starts = check::cancel_open(&tx, d.version, &why, trace)?;
    let ask: String = tx.query_row("SELECT ask_id FROM plans WHERE id = ?1", [plan], |r| {
        r.get(0)
    })?;
    starts.extend(tasks::plan_wait::resized(&tx, &ask, trace)?.1);
    let (mut made, mut started) = (Vec::new(), 0);
    for (i, p) in parts.iter().enumerate() {
        let mut after: Vec<i64> = p
            .after
            .iter()
            .filter_map(|a| made.get(*a as usize - 1).copied())
            .collect();
        after.extend(&p.after_tasks);
        let g = Give {
            chat: &d.chat,
            giver: Some(&d.lead),
            owner: &p.owner,
            given_in: Some(&d.turn),
            parent: None,
            kind: Kind::Work,
            what: &p.what,
            brief: Brief {
                why: Some(&p.why),
                done: Some(&p.done),
                proof: Some(&p.proof),
                place: Some(&p.place),
                size: Some((&p.size, p.lines)),
            },
            after: &after,
            pr: None,
            in_passing: false,
            look_into: false,
        };
        let (task, given) = tasks::give_in(&tx, &g, trace)?;
        tx.execute(
            "UPDATE plan_parts SET task = ?3 WHERE version_id = ?1 AND n = ?2",
            params![d.version, i as i64 + 1, task.number],
        )?;
        started += usize::from(task.state == State::Ready);
        made.push(task.number);
        starts.extend(given);
    }
    let line = format!(
        "Approved: version {n} of the plan \"{}\", {}.",
        d.title,
        counted(made.len())
    );
    let said = crate::store::say(&tx, &d.chat, &line)?;
    decided_line(&tx, d.version, said["id"].as_str().unwrap_or_default())?;
    event(&d, (plan, n), "approved", trace)
        .attr("gathr.task.count", made.len())
        .attr("gathr.task.started", started)
        .insert(&tx)?;
    done(tx, plan, starts)
}

/// Send back (H6a.1): the lead's turn, with the person's words, answering the version's turn.
pub fn send_back(
    conn: &mut Connection,
    plan: i64,
    n: i64,
    words: &str,
    trace: &str,
) -> Result<Decided, Error> {
    let tx = conn.transaction()?;
    let d = guard(&tx, plan, n, true)?;
    let words = words.trim();
    if words.is_empty() {
        return Err(refused("Say what to change."));
    }
    decide(&tx, d.version, "sent_back", Some(words))?;
    let why = format!("version {n} was sent back");
    let mut starts = check::cancel_open(&tx, d.version, &why, trace)?;
    let text = format!(
        "The person sent back version {n} of your plan \"{}\" (plan #{plan}):\n{words}\nWrite the next version with plan.",
        d.title
    );
    let on = (Some(d.turn.as_str()), Opened::SentBack);
    let start = Start::queue(&tx, (&d.chat, &d.lead), (None, on.0), on.1, text)?;
    decided_line(
        &tx,
        d.version,
        start.message["id"].as_str().unwrap_or_default(),
    )?;
    event(&d, (plan, n), "sent_back", trace).insert(&tx)?;
    starts.push(start);
    done(tx, plan, starts)
}

/// Drop (H6a.2): the plan ends, with no turn; its ask stays as it is sized.
pub fn drop(conn: &mut Connection, plan: i64, n: i64, trace: &str) -> Result<Decided, Error> {
    let tx = conn.transaction()?;
    let d = guard(&tx, plan, n, false)?;
    decide(&tx, d.version, "dropped", None)?;
    let starts = check::cancel_open(&tx, d.version, "the plan was dropped", trace)?;
    let line = format!("Dropped: the plan \"{}\".", d.title);
    let said = crate::store::say(&tx, &d.chat, &line)?;
    decided_line(&tx, d.version, said["id"].as_str().unwrap_or_default())?;
    event(&d, (plan, n), "dropped", trace).insert(&tx)?;
    done(tx, plan, starts)
}
