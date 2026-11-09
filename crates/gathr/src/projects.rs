//! Projects (F8b): a name, a git folder and a team of agents with their roles. A new chat made in a
//! project copies its folder and team; the chat's own are what count after that (0028).

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
    pub created: String,
}

/// The name as saved, trimmed, or why it can't be one.
pub fn name(name: &str) -> Result<&str, &'static str> {
    crate::accounts::checked(name).map_err(|refused| refused.why())
}

/// Saves a project whose name, folder and team are already checked. A name another project has,
/// ignoring case, is refused.
pub fn create(
    conn: &mut Connection,
    name: &str,
    folder: &str,
    team: &[(String, Role)],
) -> Result<Project, Error> {
    let ts = crate::ids::now_us();
    let id = crate::ids::ulid(ts);
    let tx = conn.transaction()?;
    let added = tx.execute(
        "INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![id, name, crate::accounts::key(name), folder, ts],
    );
    match added {
        // Only the name's own constraint: a trigger's abort is a constraint violation too.
        Err(rusqlite::Error::SqliteFailure(e, _))
            if e.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            return Err(Error::Refused("another project has that name".into()));
        }
        other => other?,
    };
    for (position, (agent, role)) in team.iter().enumerate() {
        tx.execute(
            "INSERT INTO project_members (project_id, agent_id, role, position) VALUES (?1, ?2, ?3, ?4)",
            params![id, agent, role.as_str(), position as i64],
        )?;
    }
    tx.commit()?;
    get(conn, &id)?.ok_or_else(|| Error::Other("the project wasn't saved".into()))
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

pub fn get(conn: &Connection, id: &str) -> Result<Option<Project>, Error> {
    let found: Option<(String, String, i64)> = conn
        .query_row(
            "SELECT name, folder, created_ts FROM projects WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((name, folder, created)) = found else {
        return Ok(None);
    };
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
