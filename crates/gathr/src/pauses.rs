//! Pauses and usage holds (0010): what keeps turns from starting, kept in the database so both
//! outlast a restart. Each agent's queue asks [`holding`] before it starts a turn Gathr queued.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;
use crate::ids::now_us;

/// Turns held on a usage limit start this long after the reset, as 0010 says.
pub const AFTER_RESET_US: i64 = 30_000_000;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pause {
    /// `app`, `chat` or `agent`.
    pub scope: String,
    /// The chat's or agent's id; empty for the app.
    pub target: String,
    pub since_us: i64,
    /// When the service said nothing in it runs any more.
    pub quiet_us: Option<i64>,
    /// The person's Stop now: carry-ons wait for Restart, which is its resume.
    pub stopped: bool,
}

impl Pause {
    /// Whether it holds a turn in `chat` for `agent`.
    fn covers(&self, chat: &str, agent: Option<&str>) -> bool {
        match self.scope.as_str() {
            "app" => true,
            "chat" => self.target == chat,
            _ => agent == Some(self.target.as_str()),
        }
    }
}

/// The pause that holds a turn Gathr queued in `chat` for `agent`, if any. The person's own message
/// is never held: it starts the agent it's for (0010).
pub fn holding<'a>(pauses: &'a [Pause], chat: &str, agent: Option<&str>) -> Option<&'a Pause> {
    pauses.iter().find(|p| p.covers(chat, agent))
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

pub fn list(conn: &Connection) -> Result<Vec<Pause>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT scope, target, since_ts, quiet_ts, stopped FROM pauses ORDER BY since_ts",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Pause {
            scope: r.get(0)?,
            target: r.get(1)?,
            since_us: r.get(2)?,
            quiet_us: r.get(3)?,
            stopped: r.get(4)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Checks the scope names something that exists: the app takes no target.
fn checked(conn: &Connection, scope: &str, target: &str) -> Result<(), Error> {
    let table = match scope {
        "app" if target.is_empty() => return Ok(()),
        "app" => return Err(refused("the app's pause takes no target")),
        "chat" => "chats",
        "agent" => "agents",
        _ => return Err(refused("scope is app, chat or agent")),
    };
    let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
    let found: Option<i64> = conn.query_row(&sql, [target], |r| r.get(0)).optional()?;
    match found {
        Some(_) => Ok(()),
        None => Err(refused(format!("there's no {scope} {target}"))),
    }
}

/// Pauses a scope, by the person: running turns finish, nothing new starts on its own. `stopped`
/// is Stop now's pause. Pausing what's already paused changes nothing but a stop.
pub fn pause(
    conn: &Connection,
    scope: &str,
    target: &str,
    stopped: bool,
    trace: &str,
) -> Result<(), Error> {
    checked(conn, scope, target)?;
    conn.execute(
        "INSERT INTO pauses (scope, target, since_ts, stopped) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (scope, target) DO UPDATE SET stopped = max(stopped, ?4)",
        params![scope, target, now_us(), stopped],
    )?;
    let event = Event::point("gathr.pause.start").trace(trace).ok();
    let event = event
        .attr("gathr.pause.scope", scope)
        .attr("gathr.pause.stopped", stopped);
    event.attr("gathr.pause.target", target).insert(conn)?;
    Ok(())
}

/// A pause as the API shows it: `stopped` after Stop now, else `pausing` while something in it
/// still runs, else `paused`.
fn shown(p: &Pause) -> Value {
    let state = match (p.stopped, p.quiet_us) {
        (true, _) => "stopped",
        (false, None) => "pausing",
        (false, Some(_)) => "paused",
    };
    json!({ "scope": p.scope, "target": p.target, "since_us": p.since_us, "quiet_us": p.quiet_us,
        "state": state, "by": null })
}

/// How a chat runs: the pause that holds it (its own, or the app's), or `running`.
pub fn chat_state(pauses: &[Pause], chat: &str) -> Value {
    let own = pauses
        .iter()
        .find(|p| p.scope == "chat" && p.target == chat);
    match own.or_else(|| pauses.iter().find(|p| p.scope == "app")) {
        Some(p) => shown(p),
        None => json!({ "state": "running" }),
    }
}

/// A chat as the API shows it, with how it runs (`run`) and, on each member, its own pause and
/// when its account's usage limit resets while it's out (`usage_until_us`).
pub fn with_run(
    conn: &Connection,
    pauses: &[Pause],
    chat: &str,
    history: &mut Value,
) -> Result<(), Error> {
    history["run"] = chat_state(pauses, chat);
    let Some(members) = history["members"].as_array_mut() else {
        return Ok(());
    };
    let mut until = conn.prepare_cached(
        "SELECT u.until_ts FROM usage_holds u JOIN agents a ON a.account_id = u.account_id
         WHERE a.id = ?1 AND u.until_ts + ?2 > ?3",
    )?;
    for member in members {
        let agent = member["agent"].as_str().unwrap_or_default().to_string();
        let now = (AFTER_RESET_US, now_us());
        let out: Option<i64> = until
            .query_row(params![agent, now.0, now.1], |r| r.get(0))
            .optional()?;
        let paused = pauses
            .iter()
            .any(|p| p.scope == "agent" && p.target == agent);
        member["paused"] = paused.into();
        member["usage_until_us"] = out.into();
    }
    Ok(())
}

/// Ends a pause: what it held starts, in the order it was held.
pub fn resume(conn: &Connection, scope: &str, target: &str, trace: &str) -> Result<(), Error> {
    let gone = conn.execute(
        "DELETE FROM pauses WHERE scope = ?1 AND target = ?2",
        params![scope, target],
    )?;
    if gone == 0 {
        return Err(refused("it isn't paused"));
    }
    let event = Event::point("gathr.pause.end").trace(trace).ok();
    event
        .attr("gathr.pause.scope", scope)
        .attr("gathr.pause.target", target)
        .insert(conn)?;
    Ok(())
}

/// Turns still running and background commands still open in the scope: whose, and where.
pub fn running(conn: &Connection, p: &Pause) -> Result<Value, Error> {
    let scoped =
        "(?1 = 'app' OR (?1 = 'chat' AND t.chat_id = ?2) OR (?1 = 'agent' AND t.agent_id = ?2))";
    let mut turns = conn.prepare_cached(&format!(
        "SELECT t.id, t.chat_id, t.agent_id FROM turns t WHERE t.status = 'running' AND {scoped}"
    ))?;
    let turns = turns.query_map([&p.scope, &p.target], |r| {
        Ok(
            json!({ "turn": r.get::<_, String>(0)?, "chat": r.get::<_, String>(1)?,
            "agent": r.get::<_, Option<String>>(2)? }),
        )
    })?;
    let mut commands = conn.prepare_cached(&format!(
        "SELECT b.description, t.chat_id, t.agent_id FROM background b JOIN turns t ON t.id = b.turn_id
         WHERE {scoped} ORDER BY b.started_ts"
    ))?;
    let commands = commands.query_map([&p.scope, &p.target], |r| {
        Ok(
            json!({ "command": r.get::<_, String>(0)?, "chat": r.get::<_, String>(1)?,
            "agent": r.get::<_, Option<String>>(2)? }),
        )
    })?;
    Ok(json!({
        "turns": turns.collect::<Result<Vec<_>, _>>()?,
        "background": commands.collect::<Result<Vec<_>, _>>()?,
    }))
}

/// Each pause with its state: `pausing` while something in it still runs (and what), else `paused`.
pub fn listed(conn: &Connection) -> Result<Value, Error> {
    let mut pauses = Vec::new();
    for p in list(conn)? {
        let mut shown = shown(&p);
        shown["running"] = running(conn, &p)?;
        pauses.push(shown);
    }
    let usage = usage(conn)?;
    let mut agents =
        conn.prepare_cached("SELECT id FROM agents WHERE account_id = ?1 ORDER BY id")?;
    let mut held = Vec::new();
    for (account, until, kind) in usage {
        let on = agents.query_map([&account], |r| r.get::<_, String>(0))?;
        let on = on.collect::<Result<Vec<_>, _>>()?;
        held.push(json!({ "account": account, "until_us": until, "kind": kind, "agents": on }));
    }
    Ok(json!({ "pauses": pauses, "usage": held }))
}

/// Pauses in which nothing runs any more become `paused`, once, with an event that says so: the
/// service's own state decides it, never an agent (0010). One the person's own message started a
/// turn in is `pausing` again until that ends. The pauses that changed.
pub fn settle(conn: &Connection, trace: &str) -> Result<Vec<Pause>, Error> {
    let mut changed = Vec::new();
    for p in list(conn)? {
        let running = running(conn, &p)?;
        let idle = |key: &str| running[key].as_array().is_none_or(Vec::is_empty);
        let quiet = idle("turns") && idle("background");
        if quiet == p.quiet_us.is_some() {
            continue;
        }
        if !quiet {
            conn.execute(
                "UPDATE pauses SET quiet_ts = NULL WHERE scope = ?1 AND target = ?2",
                params![p.scope, p.target],
            )?;
            changed.push(Pause {
                quiet_us: None,
                ..p
            });
            continue;
        }
        let now = now_us();
        conn.execute(
            "UPDATE pauses SET quiet_ts = ?3 WHERE scope = ?1 AND target = ?2",
            params![p.scope, p.target, now],
        )?;
        let event = Event::point("gathr.pause.quiet").trace(trace).ok();
        let event = event.attr("gathr.pause.scope", p.scope.as_str());
        event
            .attr("gathr.pause.target", p.target.as_str())
            .insert(conn)?;
        changed.push(Pause {
            quiet_us: Some(now),
            ..p
        });
    }
    Ok(changed)
}

/// The account's engine turned a turn away until `until_us`: its turns wait until 30 s after.
pub fn hold_usage(
    conn: &Connection,
    account: &str,
    until_us: i64,
    kind: Option<&str>,
    trace: &str,
) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO usage_holds (account_id, until_ts, since_ts, kind) VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT (account_id) DO UPDATE SET until_ts = ?2, kind = ?4",
        params![account, until_us, now_us(), kind],
    )?;
    let event = Event::point("gathr.usage.hold")
        .trace(trace)
        .error("usage_limit");
    let event = event
        .attr("gathr.account", account)
        .attr("until_us", until_us);
    event.attr("kind", kind).insert(conn)?;
    Ok(())
}

/// Until when the account's turns wait (the reset plus 30 s), or `None`: a hold that has passed
/// is ended here.
pub fn usage_until(conn: &Connection, account: &str) -> Result<Option<i64>, Error> {
    let until: Option<i64> = conn
        .query_row(
            "SELECT until_ts FROM usage_holds WHERE account_id = ?1",
            [account],
            |r| r.get(0),
        )
        .optional()?;
    let Some(until) = until.map(|u| u + AFTER_RESET_US) else {
        return Ok(None);
    };
    if until > now_us() {
        return Ok(Some(until));
    }
    conn.execute("DELETE FROM usage_holds WHERE account_id = ?1", [account])?;
    Ok(None)
}

/// Every usage hold: account, reset and the limit's name.
pub fn usage(conn: &Connection) -> Result<Vec<(String, i64, Option<String>)>, Error> {
    let mut stmt = conn
        .prepare_cached("SELECT account_id, until_ts, kind FROM usage_holds ORDER BY until_ts")?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pause_of(scope: &str, target: &str) -> Pause {
        Pause {
            scope: scope.into(),
            target: target.into(),
            since_us: 1,
            quiet_us: None,
            stopped: false,
        }
    }

    #[test]
    fn a_pause_holds_only_turns_in_its_scope() {
        let chat = [pause_of("chat", "c1")];
        assert!(holding(&chat, "c1", Some("ann")).is_some());
        assert!(holding(&chat, "c2", Some("ann")).is_none());
        let agent = [pause_of("agent", "ann")];
        assert!(holding(&agent, "c2", Some("ann")).is_some());
        assert!(holding(&agent, "c2", Some("bob")).is_none());
        assert!(
            holding(&agent, "c2", None).is_none(),
            "a turn sent to no agent"
        );
        assert!(holding(&[pause_of("app", "")], "c9", None).is_some());
    }

    fn db() -> Connection {
        let dir = std::env::temp_dir().join(format!("gathr-pauses-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO chats (id, title, created_ts) VALUES ('c1', 'x', 1);
             INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES ('a1', 'claude', 'A', 'a', 1);",
        )
        .unwrap();
        conn
    }

    #[test]
    fn a_pause_is_pausing_until_nothing_in_it_runs_then_says_so_once() {
        let conn = db();
        conn.execute_batch(
            "INSERT INTO turns (id, chat_id, trace_id, status, created_ts) VALUES ('t1', 'c1', 'x', 'running', 1);
             INSERT INTO background (id, turn_id, description, started_ts) VALUES ('b1', 't1', 'ping', 1);",
        )
        .unwrap();
        pause(&conn, "chat", "c1", false, "tr").unwrap();
        assert!(settle(&conn, "tr").unwrap().is_empty(), "a turn runs");
        let shown = listed(&conn).unwrap();
        assert_eq!(shown["pauses"][0]["state"], "pausing");
        assert_eq!(
            shown["pauses"][0]["running"]["background"][0]["command"],
            "ping"
        );
        conn.execute("UPDATE turns SET status = 'done'", [])
            .unwrap();
        assert!(
            settle(&conn, "tr").unwrap().is_empty(),
            "its command still runs"
        );
        conn.execute("DELETE FROM background", []).unwrap();
        assert_eq!(settle(&conn, "tr").unwrap().len(), 1);
        assert!(settle(&conn, "tr").unwrap().is_empty(), "said once");
        assert_eq!(listed(&conn).unwrap()["pauses"][0]["state"], "paused");
    }

    #[test]
    fn a_pause_names_something_that_exists_and_resume_ends_only_a_pause() {
        let conn = db();
        let no_chat = pause(&conn, "chat", "nope", false, "tr").unwrap_err();
        assert_eq!(no_chat.to_string(), "there's no chat nope");
        assert!(pause(&conn, "app", "c1", false, "tr").is_err());
        pause(&conn, "app", "", false, "tr").unwrap();
        pause(&conn, "app", "", false, "tr").unwrap();
        assert_eq!(list(&conn).unwrap().len(), 1);
        resume(&conn, "app", "", "tr").unwrap();
        assert!(resume(&conn, "app", "", "tr").is_err());
    }

    #[test]
    fn a_usage_hold_lasts_until_30_s_after_the_reset_then_ends() {
        let conn = db();
        let reset = now_us() + 60_000_000;
        hold_usage(&conn, "a1", reset, Some("five_hour"), "tr").unwrap();
        assert_eq!(
            usage_until(&conn, "a1").unwrap(),
            Some(reset + AFTER_RESET_US)
        );
        hold_usage(&conn, "a1", now_us() - AFTER_RESET_US - 1, None, "tr").unwrap();
        assert_eq!(usage_until(&conn, "a1").unwrap(), None);
        assert!(
            usage(&conn).unwrap().is_empty(),
            "a hold that passed is gone"
        );
    }
}
