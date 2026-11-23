//! Who added a skill, a tool, an agent or a project, when an AI app outside Gathr did (`added_by`): the name the app
//! gave as it connected. A skill is kept by its name, the rest by their id; the thing's own delete forgets it.

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// Notes that `app` added the `kind` (`skill`, `tool`, `agent` or `project`) called `id`.
pub fn note(conn: &Connection, kind: &str, id: &str, app: &str) -> Result<(), Error> {
    conn.execute(
        "INSERT OR REPLACE INTO added_by (kind, id, app, added_ts) VALUES (?1, ?2, ?3, ?4)",
        params![kind, id, app, crate::ids::now_us()],
    )?;
    Ok(())
}

/// The app that added it, if one did.
pub fn of(conn: &Connection, kind: &str, id: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT app FROM added_by WHERE kind = ?1 AND id = ?2";
    Ok(conn
        .prepare_cached(sql)?
        .query_row([kind, id], |r| r.get(0))
        .optional()?)
}

/// Forgets who added it: its delete.
pub fn forget(conn: &Connection, kind: &str, id: &str) -> Result<(), Error> {
    let sql = "DELETE FROM added_by WHERE kind = ?1 AND id = ?2";
    conn.execute(sql, [kind, id])?;
    Ok(())
}
