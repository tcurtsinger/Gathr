//! The database: one SQLite file, its pragmas and numbered migrations (docs/decisions/0003-storage.md).

use std::path::Path;
use std::time::Duration;

use rusqlite::{Connection, OpenFlags, TransactionBehavior};

use crate::Error;

const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_init.sql"),
    include_str!("../migrations/0002_turns.sql"),
];

/// The schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

/// Opens the database for writing, creating it if needed, and brings its schema up to date.
pub fn open(path: &Path) -> Result<Connection, Error> {
    let mut conn = Connection::open(path)?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Error::Other(format!(
            "the database stayed in journal mode {mode}, not WAL"
        )));
    }
    conn.execute_batch("PRAGMA synchronous = NORMAL; PRAGMA foreign_keys = ON;")?;
    migrate(&mut conn)?;
    Ok(conn)
}

/// Opens an existing database read-only, for diagnostics. Fails if the file isn't a Gathr
/// database this build can read.
pub fn open_read_only(path: &Path) -> Result<(Connection, i64), Error> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    conn.busy_timeout(Duration::from_secs(5))?;
    let version = schema_version(&conn)?;
    if version > SCHEMA_VERSION {
        return Err(Error::NewerDatabase {
            found: version,
            known: SCHEMA_VERSION,
        });
    }
    Ok((conn, version))
}

pub fn schema_version(conn: &Connection) -> Result<i64, Error> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

fn migrate(conn: &mut Connection) -> Result<(), Error> {
    let current = schema_version(conn)?;
    if current > SCHEMA_VERSION {
        return Err(Error::NewerDatabase {
            found: current,
            known: SCHEMA_VERSION,
        });
    }
    for (number, sql) in (1..).zip(MIGRATIONS).skip(current as usize) {
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", number)?;
        tx.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("gathr-db-{name}-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("gathr.db")
    }

    #[test]
    fn an_empty_database_gets_every_migration_once() {
        let path = temp_db("empty");
        drop(open(&path).unwrap());
        let conn = open(&path).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);
        let tables: i64 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_schema WHERE name IN ('events', 'chats', 'messages')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 3);
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let path = temp_db("newer");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        match open(&path) {
            Err(Error::NewerDatabase {
                found: 99,
                known: SCHEMA_VERSION,
            }) => {}
            other => panic!("expected a refusal of schema 99, got {other:?}"),
        }
    }
}
