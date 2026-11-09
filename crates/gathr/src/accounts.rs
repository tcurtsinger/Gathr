//! Named accounts (0002, 0026): each bound to one engine and signed in on its own login folder.

use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::data::DataDir;
use crate::{Error, ids};

/// The design's name field takes this many characters.
const NAME_MAX: usize = 80;

#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub engine: String,
}

impl Account {
    /// The engine's own folder for this account: `CLAUDE_CONFIG_DIR` for Claude Code. Named by
    /// id, so a rename never moves a login.
    pub fn folder(&self, data: &DataDir) -> PathBuf {
        folder(data, &self.id, &self.engine)
    }
}

pub fn folder(data: &DataDir, id: &str, engine: &str) -> PathBuf {
    data.path().join("accounts").join(id).join(engine)
}

/// Why an add or rename was turned down.
#[derive(Debug, PartialEq, Eq)]
pub enum Refused {
    Blank,
    TooLong,
    Taken,
    Unknown,
    /// Only Claude Code has a driver so far; Codex comes with its own.
    Engine,
}

impl Refused {
    pub fn why(&self) -> &'static str {
        match self {
            Refused::Blank => "the name is blank",
            Refused::TooLong => "The name must be 80 characters or fewer",
            Refused::Taken => "Another account has that name. Use a different name.",
            Refused::Unknown => "no such account",
            Refused::Engine => "Gathr currently supports Claude Code accounts only",
        }
    }
}

pub type Changed = Result<Result<Account, Refused>, Error>;

const COLUMNS: &str = "SELECT id, name, engine FROM accounts";

fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<Account> {
    Ok(Account {
        id: r.get(0)?,
        name: r.get(1)?,
        engine: r.get(2)?,
    })
}

/// Every account, oldest first.
pub fn list(conn: &Connection) -> Result<Vec<Account>, Error> {
    let mut statement = conn.prepare(&format!("{COLUMNS} ORDER BY created_ts, id"))?;
    let accounts = statement.query_map([], row)?.collect::<Result<_, _>>()?;
    Ok(accounts)
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Account>, Error> {
    let found = conn
        .query_row(&format!("{COLUMNS} WHERE id = ?1"), [id], row)
        .optional()?;
    Ok(found)
}

pub fn add(conn: &Connection, engine: &str, name: &str) -> Changed {
    if engine != "claude" {
        return Ok(Err(Refused::Engine));
    }
    let name = match checked(name) {
        Ok(name) => name,
        Err(refused) => return Ok(Err(refused)),
    };
    let account = Account {
        id: ids::span_id(),
        name: name.to_string(),
        engine: engine.to_string(),
    };
    let added = conn.execute(
        "INSERT INTO accounts (id, engine, name, name_key, created_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            account.id,
            account.engine,
            account.name,
            key(name),
            ids::now_us()
        ],
    );
    taken_or(added.map(|_| account))
}

pub fn rename(conn: &Connection, id: &str, name: &str) -> Changed {
    let name = match checked(name) {
        Ok(name) => name,
        Err(refused) => return Ok(Err(refused)),
    };
    let renamed = conn.execute(
        "UPDATE accounts SET name = ?2, name_key = ?3 WHERE id = ?1",
        params![id, name, key(name)],
    );
    match taken_or(renamed)? {
        Ok(0) => Ok(Err(Refused::Unknown)),
        Ok(_) => Ok(get(conn, id)?.ok_or(Refused::Unknown)),
        Err(refused) => Ok(Err(refused)),
    }
}

/// Whether there was such an account.
pub fn remove(conn: &Connection, id: &str) -> Result<bool, Error> {
    Ok(conn.execute("DELETE FROM accounts WHERE id = ?1", [id])? > 0)
}

pub(crate) fn checked(name: &str) -> Result<&str, Refused> {
    let name = name.trim();
    match name.chars().count() {
        0 => Err(Refused::Blank),
        n if n > NAME_MAX => Err(Refused::TooLong),
        _ => Ok(name),
    }
}

/// Names are unique, ignoring case in any script (Équipe is équipe), so a picker never shows two
/// the same.
pub(crate) fn key(name: &str) -> String {
    name.to_lowercase()
}

/// SQLite reports every broken rule as a constraint failure: only the unique name index is a
/// taken name (the id is the primary key, its own code).
pub(crate) fn taken_or<T>(done: rusqlite::Result<T>) -> Result<Result<T, Refused>, Error> {
    match done {
        Ok(value) => Ok(Ok(value)),
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            Ok(Err(Refused::Taken))
        }
        Err(e) => Err(e.into()),
    }
}
