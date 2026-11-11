//! Projects (F8b): a name, a git folder and a team of agents with their roles. A new chat made in a
//! project copies its folder and team; the chat's own are what count after that (0028). Its checks,
//! CI choice and merge method say what gates a merge and how Gathr does it (0036).

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;
use crate::team::{Member, Role};

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub members: Vec<Member>,
    /// The commands Gathr runs on a PR's head, in order.
    pub checks: Vec<String>,
    /// Whose result gates a merge: one of `CI`.
    pub ci: String,
    /// How Gathr merges: one of `MERGE`.
    pub merge: String,
    pub created: String,
}

/// GitHub's CI, Gathr's own checks, or both.
pub const CI: [&str; 3] = ["ci", "gathr", "both"];
/// A fast-forward to the PR's head, a merge commit onto the base, or GitHub's squash.
pub const MERGE: [&str; 3] = ["fast-forward", "merge", "squash"];

/// What a change sets; `None` leaves it as it is. Each is already checked.
#[derive(Default)]
pub struct Change {
    pub name: Option<String>,
    pub folder: Option<String>,
    pub team: Option<Vec<(String, Role)>>,
    pub checks: Option<Vec<String>>,
    pub ci: Option<String>,
    pub merge: Option<String>,
}

/// The name as saved, trimmed, or why it can't be one.
pub fn name(name: &str) -> Result<&str, &'static str> {
    crate::accounts::checked(name).map_err(|refused| refused.why())
}

/// The check commands as saved, each trimmed, or why they can't be.
pub fn checks(commands: Vec<String>) -> Result<Vec<String>, &'static str> {
    let trimmed: Vec<String> = commands.iter().map(|c| c.trim().to_string()).collect();
    match trimmed.iter().any(String::is_empty) {
        true => Err("A check needs a command"),
        false => Ok(trimmed),
    }
}

/// Saves a project whose name, folder and team are already checked. A name another project has,
/// ignoring case, is refused. `ci` is one of `CI`, picked by the caller from what the folder holds.
pub fn create(
    conn: &mut Connection,
    name: &str,
    folder: &str,
    team: &[(String, Role)],
    ci: &str,
) -> Result<Project, Error> {
    let ts = crate::ids::now_us();
    let id = crate::ids::ulid(ts);
    let tx = conn.transaction()?;
    let added = tx.execute(
        "INSERT INTO projects (id, name, name_key, folder, created_ts, ci) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, name, crate::accounts::key(name), folder, ts, ci],
    );
    name_taken(added)?;
    seat(&tx, &id, team)?;
    tx.commit()?;
    get(conn, &id)?.ok_or_else(|| Error::Other("the project wasn't saved".into()))
}

/// Saves what `change` sets, in one step: `None` when there's no such project.
pub fn update(conn: &mut Connection, id: &str, change: Change) -> Result<Option<Project>, Error> {
    let tx = conn.transaction()?;
    let found: Option<String> = tx
        .query_row("SELECT id FROM projects WHERE id = ?1", [id], |r| r.get(0))
        .optional()?;
    if found.is_none() {
        return Ok(None);
    }
    if let Some(name) = &change.name {
        let renamed = tx.execute(
            "UPDATE projects SET name = ?2, name_key = ?3 WHERE id = ?1",
            params![id, name, crate::accounts::key(name)],
        );
        name_taken(renamed)?;
    }
    let checks = change.checks.map(|c| serde_json::to_string(&c));
    let checks = checks
        .transpose()
        .map_err(|e| Error::Other(e.to_string()))?;
    let fields = [
        ("folder", change.folder),
        ("checks", checks),
        ("ci", change.ci),
        ("merge_method", change.merge),
    ];
    for (column, value) in fields {
        if let Some(value) = value {
            let sql = format!("UPDATE projects SET {column} = ?2 WHERE id = ?1");
            tx.execute(&sql, params![id, value])?;
        }
    }
    if let Some(team) = &change.team {
        tx.execute("DELETE FROM project_members WHERE project_id = ?1", [id])?;
        seat(&tx, id, team)?;
    }
    tx.commit()?;
    get(conn, id)
}

/// The write, or refused when it hit the name's own constraint (a trigger's abort is a constraint
/// violation too, so only that one).
fn name_taken(written: rusqlite::Result<usize>) -> Result<(), Error> {
    match written {
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            Err(Error::Refused(
                "Another project has that name. Use a different name.".into(),
            ))
        }
        other => {
            other?;
            Ok(())
        }
    }
}

fn seat(conn: &Connection, id: &str, team: &[(String, Role)]) -> Result<(), Error> {
    for (position, (agent, role)) in team.iter().enumerate() {
        conn.execute(
            "INSERT INTO project_members (project_id, agent_id, role, position) VALUES (?1, ?2, ?3, ?4)",
            params![id, agent, role.as_str(), position as i64],
        )?;
    }
    Ok(())
}

/// Every project, by name.
pub fn list(conn: &Connection) -> Result<Vec<Project>, Error> {
    let mut stmt = conn.prepare_cached("SELECT id FROM projects ORDER BY name_key")?;
    let ids: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    ids.iter()
        .filter_map(|id| get(conn, id).transpose())
        .collect()
}

type Row = (String, String, i64, String, String, String);

pub fn get(conn: &Connection, id: &str) -> Result<Option<Project>, Error> {
    let found: Option<Row> = conn
        .query_row(
            "SELECT name, folder, created_ts, checks, ci, merge_method FROM projects WHERE id = ?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            },
        )
        .optional()?;
    let Some((name, folder, created, checks, ci, merge)) = found else {
        return Ok(None);
    };
    // The column's CHECK keeps it an array; one holding anything but text reads as none.
    let checks = serde_json::from_str(&checks).unwrap_or_default();
    let mut stmt = conn.prepare_cached(
        "SELECT m.agent_id, a.name, m.role FROM project_members m JOIN agents a ON a.id = m.agent_id
         WHERE m.project_id = ?1 ORDER BY m.role != 'supervisor', m.position",
    )?;
    let members = stmt
        .query_map([id], |r| {
            Ok(Member {
                agent: r.get(0)?,
                name: r.get(1)?,
                // The table's CHECK allows only the three.
                role: Role::named(&r.get::<_, String>(2)?).unwrap_or(Role::Member),
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(Some(Project {
        id: id.to_string(),
        name,
        folder,
        members,
        checks,
        ci,
        merge,
        created: crate::ids::utc(created),
    }))
}

/// The title of a project the agent is in, if any: an agent isn't deleted while a project has it.
pub fn a_project_of(conn: &Connection, agent: &str) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row(
            "SELECT p.name FROM project_members m JOIN projects p ON p.id = m.project_id
             WHERE m.agent_id = ?1 ORDER BY p.created_ts LIMIT 1",
            [agent],
            |r| r.get(0),
        )
        .optional()?)
}
