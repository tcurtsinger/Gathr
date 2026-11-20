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
    /// What its face is drawn from (0074): its name when it was made, a new one at each Shuffle. A rename
    /// keeps it.
    pub face_seed: String,
    /// `man` or `woman` once the person sets it; until then the seed picks.
    pub gender: Option<String>,
    /// Skin tone 1 to 6 once the person sets it; until then the seed picks.
    pub skin: Option<i64>,
    /// The background the person picked, `#RRGGBB`; until then a grey.
    pub background: Option<String>,
    /// A picture the person gave it, as a data URL the window made; shown instead of the drawn face.
    pub picture: Option<String>,
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
    pub face_seed: Option<String>,
    pub gender: Option<Option<String>>,
    pub skin: Option<Option<i64>>,
    /// `#rrggbb` in either case: kept in upper case.
    pub background: Option<Option<String>>,
    pub picture: Option<Option<String>>,
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
        next.face_seed = change.face_seed.unwrap_or(next.face_seed);
        next.gender = change.gender.unwrap_or(next.gender);
        next.skin = change.skin.unwrap_or(next.skin);
        let background = change.background.map(|b| b.map(|b| b.to_ascii_uppercase()));
        next.background = background.unwrap_or(next.background);
        next.picture = change.picture.unwrap_or(next.picture);
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
    /// The account, model or effort it was checked with changed before it was saved.
    Moved,
    Seed,
    Gender,
    Skin,
    Background,
}

impl Refused {
    pub fn why(&self) -> &'static str {
        match self {
            Refused::Name(name) => name.why(),
            Refused::Taken => "Another agent has that name. Use a different name.",
            Refused::Unknown => "no such agent",
            Refused::NoAccount => "no such account",
            Refused::Removing => "The account is being removed",
            Refused::Moved => {
                "The agent's account, model or effort changed while it was checked. Try again."
            }
            Refused::Seed => "the face seed is blank",
            Refused::Gender => "Choose Man or Woman",
            Refused::Skin => "Choose a skin tone from 1 to 6",
            Refused::Background => "Choose a background as #RRGGBB",
            Refused::Pushes => {
                "Full approvals decide which pushes ask, which only the person changes"
            }
        }
    }
}

pub type Saved = Result<Result<Agent, Refused>, Error>;

/// The agents that aren't removed (0069): a removed one is in no list and refused everywhere. A row written
/// without a face seed (by hand, or by a test) is drawn from its name.
const COLUMNS: &str =
    "SELECT id, name, account_id, model, effort, approvals, coalesce(face_seed, name), gender, skin,
                         background, picture FROM agents WHERE removed_ts IS NULL";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Agent> {
    Ok(Agent {
        id: r.get(0)?,
        name: r.get(1)?,
        account: r.get(2)?,
        model: r.get(3)?,
        effort: r.get(4)?,
        approvals: Approvals::parse(&r.get::<_, String>(5)?),
        face_seed: r.get(6)?,
        gender: r.get(7)?,
        skin: r.get(8)?,
        background: r.get(9)?,
        picture: r.get(10)?,
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
        .query_row(&format!("{COLUMNS} AND id = ?1"), [id], row)
        .optional()?;
    Ok(found)
}

/// The names of the agents on an account, oldest first.
pub fn on_account(conn: &Connection, account: &str) -> Result<Vec<String>, Error> {
    let mut statement = conn.prepare(
        "SELECT name FROM agents WHERE account_id = ?1 AND removed_ts IS NULL ORDER BY created_ts, id",
    )?;
    let names = statement
        .query_map([account], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(names)
}

/// Adds `agent` under a new id. `removing` holds the accounts being removed, checked in the same
/// call as the write, so a removal that has looked for agents never misses this one.
pub fn add(conn: &Connection, agent: &Agent, removing: &HashSet<String>) -> Saved {
    let mut agent = Agent {
        id: ids::span_id(),
        ..agent.clone()
    };
    // A new agent's face comes from its name unless a seed came with it (0074).
    if agent.face_seed.is_empty() {
        agent.face_seed = agent.name.trim().to_string();
    }
    let name = match checked(conn, &agent, removing)? {
        Ok(name) => name,
        Err(refused) => return Ok(Err(refused)),
    };
    let added = conn.execute(
        "INSERT INTO agents (id, name, name_key, account_id, model, effort, approvals, created_ts,
                             face_seed, gender, skin, background, picture)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            agent.id,
            name,
            accounts::key(&name),
            agent.account,
            agent.model,
            agent.effort,
            agent.approvals.as_str(),
            ids::now_us(),
            agent.face_seed,
            agent.gender,
            agent.skin,
            agent.background,
            agent.picture
        ],
    );
    saved(added.map(|_| Agent { name, ..agent }))
}

/// Applies `change` to the agent as saved now, and saves it: in one call on the writer, so nothing
/// saved since a read is lost. `checked` is the agent the model and effort were checked as: one
/// whose account, model or effort came out otherwise is refused, never saved unchecked. An agent
/// (`by_agent`) never moves one into or out of Full, which decides which pushes ask (0043 C5a).
pub fn change(
    conn: &Connection,
    checked: &Agent,
    change: Change,
    by_agent: bool,
    removing: &HashSet<String>,
) -> Saved {
    let Some(current) = get(conn, &checked.id)? else {
        return Ok(Err(Refused::Unknown));
    };
    let next = current.changed(change);
    let engine = |a: &Agent| (a.account.clone(), a.model.clone(), a.effort.clone());
    if engine(&next) != engine(checked) {
        return Ok(Err(Refused::Moved));
    }
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
         approvals = ?7, face_seed = ?8, gender = ?9, skin = ?10,
         background = ?11, picture = ?12 WHERE id = ?1 AND removed_ts IS NULL",
        params![
            agent.id,
            name,
            accounts::key(&name),
            agent.account,
            agent.model,
            agent.effort,
            agent.approvals.as_str(),
            agent.face_seed,
            agent.gender,
            agent.skin,
            agent.background,
            agent.picture
        ],
    );
    match saved(updated)? {
        Ok(0) => Ok(Err(Refused::Unknown)),
        Ok(_) => Ok(get(conn, &agent.id)?.ok_or(Refused::Unknown)),
        Err(refused) => Ok(Err(refused)),
    }
}

/// An agent's name, removed or not: what its old messages and records still say. None when
/// there was never such an agent.
pub fn name(conn: &Connection, id: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT name FROM agents WHERE id = ?1";
    Ok(conn.query_row(sql, [id], |r| r.get(0)).optional()?)
}

/// Whether the agent was removed (0069).
pub fn is_removed(conn: &Connection, id: &str) -> Result<bool, Error> {
    let sql = "SELECT removed_ts IS NOT NULL FROM agents WHERE id = ?1";
    let found = conn.query_row(sql, [id], |r| r.get(0)).optional()?;
    Ok(found.unwrap_or(false))
}

/// A removed agent, as the window names its old messages and draws its face on them.
#[derive(Debug, Clone, Serialize)]
pub struct Removed {
    pub id: String,
    pub name: String,
    pub face_seed: String,
    pub gender: Option<String>,
    pub skin: Option<i64>,
    pub background: Option<String>,
}

/// The removed agents, oldest first.
pub fn removed(conn: &Connection) -> Result<Vec<Removed>, Error> {
    let mut statement = conn.prepare(
        "SELECT id, name, coalesce(face_seed, name), gender, skin, background FROM agents
         WHERE removed_ts IS NOT NULL
         ORDER BY created_ts, id",
    )?;
    let rows = statement.query_map([], |r| {
        Ok(Removed {
            id: r.get(0)?,
            name: r.get(1)?,
            face_seed: r.get(2)?,
            gender: r.get(3)?,
            skin: r.get(4)?,
            background: r.get(5)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Marks the agent removed, lets go of its account and drops its picture, once: a second call
/// keeps the first time.
pub fn mark_removed(conn: &Connection, id: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE agents SET removed_ts = ?2, account_id = NULL, picture = NULL
         WHERE id = ?1 AND removed_ts IS NULL",
        params![id, ids::now_us()],
    )?;
    Ok(())
}

/// The trimmed name, once the name, the face and the account pass.
fn checked(
    conn: &Connection,
    agent: &Agent,
    removing: &HashSet<String>,
) -> Result<Result<String, Refused>, Error> {
    let name = match accounts::checked(&agent.name) {
        Ok(name) => name.to_string(),
        Err(refused) => return Ok(Err(Refused::Name(refused))),
    };
    if agent.face_seed.trim().is_empty() {
        return Ok(Err(Refused::Seed));
    }
    if agent
        .gender
        .as_deref()
        .is_some_and(|g| g != "man" && g != "woman")
    {
        return Ok(Err(Refused::Gender));
    }
    if agent.skin.is_some_and(|s| !(1..=6).contains(&s)) {
        return Ok(Err(Refused::Skin));
    }
    if agent
        .background
        .as_deref()
        .is_some_and(|b| !is_background(b))
    {
        return Ok(Err(Refused::Background));
    }
    if removing.contains(&agent.account) {
        return Ok(Err(Refused::Removing));
    }
    if accounts::get(conn, &agent.account)?.is_none() {
        return Ok(Err(Refused::NoAccount));
    }
    Ok(Ok(name))
}

/// `#RRGGBB`, upper case: what the database's check holds too.
fn is_background(text: &str) -> bool {
    let bytes = text.as_bytes();
    let hex = |c: &u8| c.is_ascii_digit() || (b'A'..=b'F').contains(c);
    bytes.len() == 7 && bytes[0] == b'#' && bytes[1..].iter().all(hex)
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
            face_seed: name.into(),
            gender: None,
            skin: None,
            background: None,
            picture: None,
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
