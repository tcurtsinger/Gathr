//! Agents (0026): each runs on one account, with a model and an effort from that account's
//! engine. A saved change applies from the agent's next turn, in every chat.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::accounts::{self, Refused as Name};
use crate::{Error, ids};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Agent {
    pub id: String,
    pub name: String,
    pub account: String,
    /// `None` is the engine's own default.
    pub model: Option<String>,
    pub effort: Option<String>,
    pub approvals: Approvals,
}

/// What the agent may do without asking (0029). Gathr asks before a push or a write outside the
/// agent's folders (0034) in every mode but `Full`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Approvals {
    /// Everything but reading asks.
    Ask,
    /// File edits inside the worktree run; every command asks.
    Edits,
    /// The engine's own check decides each action.
    #[default]
    Auto,
    /// Nothing asks.
    Full,
}

impl Approvals {
    pub fn as_str(self) -> &'static str {
        match self {
            Approvals::Ask => "ask",
            Approvals::Edits => "edits",
            Approvals::Auto => "auto",
            Approvals::Full => "full",
        }
    }

    fn parse(text: &str) -> Approvals {
        match text {
            "ask" => Approvals::Ask,
            "edits" => Approvals::Edits,
            "full" => Approvals::Full,
            _ => Approvals::Auto,
        }
    }
}

/// A change to an agent: each field left out stays; `Some(None)` clears a model or effort.
#[derive(Debug, Default, Clone)]
pub struct Change {
    pub name: Option<String>,
    pub account: Option<String>,
    pub model: Option<Option<String>>,
    pub effort: Option<Option<String>>,
    pub approvals: Option<Approvals>,
}

impl Agent {
    /// The agent after `change`. A new account clears the model and effort it doesn't set,
    /// since they came from the old account's engine (0026).
    pub fn changed(&self, change: Change) -> Agent {
        let mut next = self.clone();
        if let Some(account) = change.account.filter(|a| *a != self.account) {
            next.account = account;
            next.model = None;
            next.effort = None;
        }
        next.name = change.name.unwrap_or(next.name);
        next.model = change.model.unwrap_or(next.model);
        next.effort = change.effort.unwrap_or(next.effort);
        next.approvals = change.approvals.unwrap_or(next.approvals);
        next
    }
}

/// Why a save was turned down.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Name(Name),
    Taken,
    Unknown,
    NoAccount,
    /// The account is being removed.
    Removing,
    /// An agent moved one into or out of Full approvals, which decide which pushes ask.
    Pushes,
}

impl Refused {
    pub fn why(&self) -> &'static str {
        match self {
            Refused::Name(name) => name.why(),
            Refused::Taken => "Another agent has that name. Use a different name.",
            Refused::Unknown => "no such agent",
            Refused::NoAccount => "no such account",
            Refused::Removing => "The account is being removed",
            Refused::Pushes => {
                "Full approvals decide which pushes ask, which only the person changes"
            }
        }
    }
}

pub type Saved = Result<Result<Agent, Refused>, Error>;

const COLUMNS: &str = "SELECT id, name, account_id, model, effort, approvals FROM agents";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Agent> {
    Ok(Agent {
        id: r.get(0)?,
        name: r.get(1)?,
        account: r.get(2)?,
        model: r.get(3)?,
        effort: r.get(4)?,
        approvals: Approvals::parse(&r.get::<_, String>(5)?),
    })
}

/// Every agent, oldest first.
pub fn list(conn: &Connection) -> Result<Vec<Agent>, Error> {
    let mut statement = conn.prepare(&format!("{COLUMNS} ORDER BY created_ts, id"))?;
    let agents = statement.query_map([], row)?.collect::<Result<_, _>>()?;
    Ok(agents)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Agent>, Error> {
    let found = conn
        .query_row(&format!("{COLUMNS} WHERE id = ?1"), [id], row)
        .optional()?;
    Ok(found)
}

/// The names of the agents on an account, oldest first.
pub fn on_account(conn: &Connection, account: &str) -> Result<Vec<String>, Error> {
    let mut statement =
        conn.prepare("SELECT name FROM agents WHERE account_id = ?1 ORDER BY created_ts, id")?;
    let names = statement
        .query_map([account], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

/// Adds `agent` under a new id. `removing` holds the accounts being removed, checked in the same
/// call as the write, so a removal that has looked for agents never misses this one.
pub fn add(conn: &Connection, agent: &Agent, removing: &HashSet<String>) -> Saved {
    let agent = Agent {
        id: ids::span_id(),
        ..agent.clone()
    };
    let name = match checked(conn, &agent, removing)? {
        Ok(name) => name,
        Err(refused) => return Ok(Err(refused)),
    };
    let added = conn.execute(
        "INSERT INTO agents (id, name, name_key, account_id, model, effort, approvals, created_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            agent.id,
            name,
            accounts::key(&name),
            agent.account,
            agent.model,
            agent.effort,
            agent.approvals.as_str(),
            ids::now_us()
        ],
    );
    saved(added.map(|_| Agent { name, ..agent }))
}

/// Applies `change` to the agent as saved now, and saves it: in one call on the writer, so nothing
/// saved since a read is lost. An agent (`by_agent`) never moves one into or out of Full, which
/// decides which pushes ask (0036 C5a).
pub fn change(
    conn: &Connection,
    id: &str,
    change: Change,
    by_agent: bool,
    removing: &HashSet<String>,
) -> Saved {
    let Some(current) = get(conn, id)? else {
        return Ok(Err(Refused::Unknown));
    };
    let next = current.changed(change);
    let full = |a: &Agent| a.approvals == Approvals::Full;
    if by_agent && full(&next) != full(&current) {
        return Ok(Err(Refused::Pushes));
    }
    update(conn, &next, removing)
}

/// Saves `agent` over the one with its id.
pub fn update(conn: &Connection, agent: &Agent, removing: &HashSet<String>) -> Saved {
    let name = match checked(conn, agent, removing)? {
        Ok(name) => name,
        Err(refused) => return Ok(Err(refused)),
    };
    let updated = conn.execute(
        "UPDATE agents SET name = ?2, name_key = ?3, account_id = ?4, model = ?5, effort = ?6,
         approvals = ?7 WHERE id = ?1",
        params![
            agent.id,
            name,
            accounts::key(&name),
            agent.account,
            agent.model,
            agent.effort,
            agent.approvals.as_str()
        ],
    );
    match saved(updated)? {
        Ok(0) => Ok(Err(Refused::Unknown)),
        Ok(_) => Ok(get(conn, &agent.id)?.ok_or(Refused::Unknown)),
        Err(refused) => Ok(Err(refused)),
    }
}

/// Whether there was such an agent.
pub fn remove(conn: &Connection, id: &str) -> Result<bool, Error> {
    Ok(conn.execute("DELETE FROM agents WHERE id = ?1", [id])? > 0)
}

/// The trimmed name, once the name and account pass.
fn checked(
    conn: &Connection,
    agent: &Agent,
    removing: &HashSet<String>,
) -> Result<Result<String, Refused>, Error> {
    let name = match accounts::checked(&agent.name) {
        Ok(name) => name.to_string(),
        Err(refused) => return Ok(Err(Refused::Name(refused))),
    };
    if removing.contains(&agent.account) {
        return Ok(Err(Refused::Removing));
    }
    if accounts::get(conn, &agent.account)?.is_none() {
        return Ok(Err(Refused::NoAccount));
    }
    Ok(Ok(name))
}

fn saved<T>(done: rusqlite::Result<T>) -> Result<Result<T, Refused>, Error> {
    Ok(accounts::taken_or(done)?.map_err(|_| Refused::Taken))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn db() -> (Connection, String) {
        let dir = std::env::temp_dir().join(format!("gathr-agents-{}", ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let conn = crate::db::open(&dir.join("gathr.db")).unwrap();
        let account = accounts::add(&conn, "claude", "Work").unwrap().unwrap();
        (conn, account.id)
    }

    fn draft(name: &str, account: &str) -> Agent {
        Agent {
            id: String::new(),
            name: name.into(),
            account: account.into(),
            model: Some("opus".into()),
            effort: Some("high".into()),
            approvals: Approvals::Auto,
        }
    }

    #[test]
    fn a_new_account_clears_the_model_and_effort_it_does_not_set() {
        let agent = Agent {
            id: "a".into(),
            ..draft("Orin", "one")
        };
        let moved = agent.changed(Change {
            account: Some("two".into()),
            ..Change::default()
        });
        assert_eq!(
            (moved.model, moved.effort),
            (None, None),
            "moved: account two"
        );
        let moved = agent.changed(Change {
            account: Some("two".into()),
            model: Some(Some("sonnet".into())),
            ..Change::default()
        });
        assert_eq!(moved.model.as_deref(), Some("sonnet"));
        let same = agent.changed(Change {
            account: Some("one".into()),
            ..Change::default()
        });
        assert_eq!(same, agent, "the same account changes nothing");
    }

    #[test]
    fn names_are_unique_ignoring_case() {
        let (conn, account) = db();
        add(&conn, &draft("Équipe", &account), &HashSet::new())
            .unwrap()
            .unwrap();
        let again = add(&conn, &draft(" équipe ", &account), &HashSet::new()).unwrap();
        assert_eq!(again, Err(Refused::Taken));
    }

    #[test]
    fn an_agent_needs_an_account_that_stays() {
        let (conn, account) = db();
        let none = add(&conn, &draft("Orin", "nope"), &HashSet::new()).unwrap();
        assert_eq!(none, Err(Refused::NoAccount));
        let removing = HashSet::from([account.clone()]);
        let refused = add(&conn, &draft("Orin", &account), &removing).unwrap();
        assert_eq!(refused, Err(Refused::Removing));
        add(&conn, &draft("Orin", &account), &HashSet::new())
            .unwrap()
            .unwrap();
        assert_eq!(on_account(&conn, &account).unwrap(), ["Orin"]);
        // The database refuses too, whatever the service checked first.
        assert!(accounts::remove(&conn, &account).is_err());
    }
}
