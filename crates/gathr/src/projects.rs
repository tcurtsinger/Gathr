//! Projects (F8b): a name, a git folder and a team of agents with their roles. A new chat made in a
//! project copies its folder and team; the chat's own are what count after that (0028). Its checks,
//! CI choice and merge method say what gates a merge and how Gathr does it (0043).

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
    pub checks: Vec<Check>,
    /// Whose result gates a merge: one of `CI`.
    pub ci: String,
    /// How Gathr merges: one of `MERGE`.
    pub merge: String,
    /// Who set `ci` and when, once someone has.
    pub ci_by: Option<SetBy>,
    /// Who set `merge` and when, once someone has.
    pub merge_by: Option<SetBy>,
    pub created: String,
}

/// Who set one of Checks & merge and when: the Supervisor's id through Gathr's tool, or `person`.
#[derive(Debug, Clone, Serialize)]
pub struct SetBy {
    pub source: String,
    pub set: String,
}

/// How the person is named as who set something.
pub const PERSON: &str = "person";

/// A check as saved: the person never types one (owner, task 1455).
#[derive(Debug, Clone, Serialize)]
pub struct Check {
    pub command: String,
    /// The agent that set it, by id, or `found` when Gathr found it in the project.
    pub source: String,
    pub set: String,
}

/// GitHub's CI, Gathr's own checks, both, or none: a clean review alone (0045).
pub const CI: [&str; 4] = ["ci", "gathr", "both", "none"];
/// A fast-forward to the PR's head, a merge commit onto the base, or GitHub's squash.
pub const MERGE: [&str; 3] = ["fast-forward", "merge", "squash"];

/// What a change sets; `None` leaves it as it is. Each is already checked.
#[derive(Default)]
pub struct Change {
    pub name: Option<String>,
    pub folder: Option<String>,
    pub team: Option<Vec<(String, Role)>>,
    pub ci: Option<String>,
    pub merge: Option<String>,
    /// Who sets `ci` and `merge`: the person or an agent's id.
    pub by: Option<String>,
}

/// The name as saved, trimmed, or why it can't be one.
pub fn name(name: &str) -> Result<&str, &'static str> {
    crate::accounts::checked(name).map_err(|refused| refused.why())
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
    if let Some(folder) = &change.folder {
        tx.execute(
            "UPDATE projects SET folder = ?2 WHERE id = ?1",
            params![id, folder],
        )?;
    }
    let by = change.by.as_deref().unwrap_or(PERSON);
    let now = crate::ids::now_us();
    for (column, value) in [("ci", change.ci), ("merge", change.merge)] {
        if let Some(value) = value {
            let stored = if column == "ci" { "ci" } else { "merge_method" };
            let sql = format!(
                "UPDATE projects SET {stored} = ?2, {column}_source = ?3, {column}_set_ts = ?4 WHERE id = ?1"
            );
            tx.execute(&sql, params![id, value, by, now])?;
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

type Row = (
    String,
    String,
    i64,
    String,
    String,
    Option<SetBy>,
    Option<SetBy>,
);

/// Who set a setting, from its source and time columns.
fn set_by(r: &rusqlite::Row, at: usize) -> rusqlite::Result<Option<SetBy>> {
    let source: Option<String> = r.get(at)?;
    let ts: Option<i64> = r.get(at + 1)?;
    Ok(source.zip(ts).map(|(source, ts)| SetBy {
        source,
        set: crate::ids::utc(ts),
    }))
}

pub fn get(conn: &Connection, id: &str) -> Result<Option<Project>, Error> {
    let found: Option<Row> = conn
        .query_row(
            "SELECT name, folder, created_ts, ci, merge_method, ci_source, ci_set_ts, merge_source,
                    merge_set_ts FROM projects WHERE id = ?1",
            [id],
            |r| {
                let by = (set_by(r, 5)?, set_by(r, 7)?);
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    by.0,
                    by.1,
                ))
            },
        )
        .optional()?;
    let Some((name, folder, created, ci, merge, ci_by, merge_by)) = found else {
        return Ok(None);
    };
    let mut stmt = conn.prepare_cached(
        "SELECT command, source, set_ts FROM project_checks WHERE project_id = ?1 ORDER BY position",
    )?;
    let checks = stmt
        .query_map([id], |r| {
            Ok(Check {
                command: r.get(0)?,
                source: r.get(1)?,
                set: crate::ids::utc(r.get(2)?),
            })
        })?
        .collect::<Result<_, _>>()?;
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
        ci_by,
        merge_by,
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
