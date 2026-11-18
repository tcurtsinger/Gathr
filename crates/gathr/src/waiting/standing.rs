//! Where each agent of a chat stands (0071): the one state its face shows, the most pressing that
//! holds, read from its turns, tasks, pauses, plans and the running service, with since when.

use rusqlite::{Connection, OptionalExtension, params};

use super::{Live, On, Open, Standing, Turn, words};
use crate::pauses::{self, Pause};
use crate::{Error, tasks, waits};

pub(super) type Saved<'a> = (&'a [Pause], &'a [Turn], &'a [Open], &'a [waits::Wait]);

/// The agent's latest turn in the chat, whatever its status: how it ended says whether the agent
/// can run, and whether it was stopped.
struct Last {
    status: String,
    why: Option<String>,
    /// Gathr stopped it itself (its task cancelled, held for its plan): not the person's stop.
    gathrs: bool,
    task: Option<i64>,
    ended: Option<i64>,
}

fn last_turn(conn: &Connection, chat: &str, agent: &str) -> Result<Option<Last>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT status, why, stop_why IS NOT NULL, task, ended_ts FROM turns
         WHERE chat_id = ?1 AND agent_id = ?2 ORDER BY created_ts DESC, rowid DESC LIMIT 1",
    )?;
    let row = |r: &rusqlite::Row<'_>| {
        Ok(Last {
            status: r.get(0)?,
            why: r.get(1)?,
            gathrs: r.get(2)?,
            task: r.get(3)?,
            ended: r.get(4)?,
        })
    };
    Ok(stmt.query_row(params![chat, agent], row).optional()?)
}

fn joined(conn: &Connection, chat: &str, agent: &str) -> Result<Option<i64>, Error> {
    let sql = "SELECT added_ts FROM chat_members WHERE chat_id = ?1 AND agent_id = ?2";
    Ok(conn
        .query_row(sql, params![chat, agent], |r| r.get(0))
        .optional()?)
}

/// One state, with its words, its word or two for the card, its task and since when.
struct Now {
    state: &'static str,
    words: String,
    what: Option<String>,
    task: Option<i64>,
    since: Option<i64>,
}

fn now(state: &'static str, words: &str, since: Option<i64>) -> Now {
    Now {
        state,
        words: words.to_string(),
        what: None,
        task: None,
        since,
    }
}

impl Now {
    fn what(mut self, what: &str) -> Now {
        self.what = Some(what.to_string());
        self
    }

    fn task(mut self, task: Option<i64>) -> Now {
        self.task = task;
        self
    }
}

/// Where each agent of the chat stands, most pressing first: it needs the person, can't run,
/// works, is stopped, is blocked, waits, or is free.
pub(super) fn standings(
    conn: &Connection,
    chat: &str,
    live: &Live,
    (pauses, turns, open, waiting): Saved,
    now_us: i64,
) -> Result<Vec<Standing>, Error> {
    let plans = crate::plans::waiting(conn, chat)?;
    let mut all = Vec::new();
    for m in crate::team::members(conn, chat)? {
        let id = m.agent.as_str();
        let own: Vec<&Turn> = turns
            .iter()
            .filter(|t| t.agent.as_deref() == Some(id))
            .collect();
        let running: Vec<&&Turn> = own.iter().filter(|t| t.status == "running").collect();
        let last = last_turn(conn, chat, id)?;
        let usage = usage_until(conn, id, now_us)?;
        let on = waits_for(conn, id, open, waiting)?;
        let mut stand = Standing {
            agent: m.agent.clone(),
            name: m.name.clone(),
            state: "free",
            words: String::new(),
            what: None,
            task: running.iter().find_map(|t| t.task),
            since_us: None,
            on: Vec::new(),
            until_us: None,
        };
        let state = needs_you(live, open, &plans, id, &running)
            .or_else(|| cant_run(&own, last.as_ref()))
            .or_else(|| working(&running))
            .or_else(|| stopped(pauses, chat, id, last.as_ref()))
            .or_else(|| blocked(open, id));
        let state = match state {
            Some(state) => state,
            None if usage.is_some() => {
                stand.until_us = usage;
                let since = own.iter().map(|t| t.created).min();
                now("waiting", "Usage limit", since).what("Usage limit")
            }
            None if !on.is_empty() => {
                let names: Vec<&str> = on.iter().map(|o| o.what.as_str()).collect();
                stand.on = on.clone();
                let since = on.iter().map(|o| o.since_us).min();
                let words = format!("Waiting for {}", words::list(&names));
                now("waiting", &words, since).what(&words::list(&names))
            }
            None => match owed(open, id, &own) {
                Some(owed) => owed,
                None => {
                    let ended = last.as_ref().and_then(|l| l.ended);
                    now("free", "Free", ended.or(joined(conn, chat, id)?))
                }
            },
        };
        stand.state = state.state;
        stand.words = state.words;
        stand.what = state.what;
        stand.task = state.task.or(stand.task);
        stand.since_us = state.since;
        all.push(stand);
    }
    Ok(all)
}

/// An approval waits on the person, a question of its is open, or its plan waits for their answer.
/// A discussed question is held, off the person's list: it doesn't need them.
fn needs_you(
    live: &Live,
    open: &[Open],
    plans: &[(String, String, i64)],
    id: &str,
    running: &[&&Turn],
) -> Option<Now> {
    let approval = running.iter().find_map(|t| live.approvals.get(&t.id));
    if let Some(since) = approval {
        return Some(now("needs_you", "Needs your approval", Some(*since)).what("Approval"));
    }
    let asked = open
        .iter()
        .filter(|o| o.kind == "person" && o.giver.as_deref() == Some(id) && o.state == "ready");
    if let Some(since) = asked.map(|o| o.created).min() {
        return Some(now("needs_you", "Asked you a question", Some(since)).what("Question"));
    }
    let plan = plans.iter().filter(|(lead, ..)| lead == id);
    let sent = plan.map(|(_, _, sent)| *sent).min()?;
    Some(now("needs_you", "A plan for you", Some(sent)).what("Plan"))
}

/// Its account is signed out, or its last turn failed and nobody retried it. A turn a restart cut
/// reads failed too, but the agent can run: it was stopped.
fn cant_run(own: &[&Turn], last: Option<&Last>) -> Option<Now> {
    if let Some(t) = own.iter().find(|t| t.status == "waiting_sign_in") {
        let signed_out = now("cant_run", "Signed out", Some(t.created));
        return Some(signed_out.what("Signed out").task(t.task));
    }
    let last = last.filter(|l| l.status == "failed")?;
    if last.why.as_deref() == Some(crate::store::CUT_BY_RESTART) {
        return None;
    }
    let failed = now("cant_run", "Its last turn failed", last.ended);
    Some(failed.what("Failed").task(last.task))
}

fn working(running: &[&&Turn]) -> Option<Now> {
    let since = running.iter().map(|t| t.created).min()?;
    Some(now("working", "Working", Some(since)))
}

/// The person stopped the chat or paused it or the agent, a restart cut its last turn, or the
/// person stopped that turn.
fn stopped(pauses: &[Pause], chat: &str, id: &str, last: Option<&Last>) -> Option<Now> {
    if let Some(p) = pauses::holding(pauses, chat, None).filter(|p| p.stopped) {
        return Some(now("stopped", "Stopped", Some(p.since_us)));
    }
    if let Some(p) = pauses::holding(pauses, chat, Some(id)) {
        return Some(now("stopped", "Paused", Some(p.since_us)).what("Paused"));
    }
    let last = last?;
    let cut = last.status == "failed" && last.why.as_deref() == Some(crate::store::CUT_BY_RESTART);
    if cut {
        let restarted = now("stopped", "Gathr restarted", last.ended);
        return Some(restarted.what("Gathr restarted").task(last.task));
    }
    (last.status == "stopped" && !last.gathrs)
        .then(|| now("stopped", "Stopped", last.ended).task(last.task))
}

fn blocked(open: &[Open], id: &str) -> Option<Now> {
    let task = open
        .iter()
        .filter(|o| o.owner.as_deref() == Some(id) && o.state == "blocked")
        .min_by_key(|o| o.updated)?;
    let words = format!("#{} is blocked", task.number);
    Some(now("blocked", &words, Some(task.updated)).task(Some(task.number)))
}

/// Work of its own to come, a turn queued or a task under way, that nothing above explains.
fn owed(open: &[Open], id: &str, own: &[&Turn]) -> Option<Now> {
    let tasks = open
        .iter()
        .filter(|o| o.owner.as_deref() == Some(id) && o.under_way());
    let since = own
        .iter()
        .filter(|t| t.status != "running")
        .map(|t| t.created)
        .chain(tasks.map(|o| o.updated))
        .min()?;
    Some(now("waiting", "Waiting", Some(since)))
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
