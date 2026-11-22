//! Deleting a project or an agent (0069): what a delete takes with it, in the counts the confirm
//! shows and the delete then does; an agent's removal in one transaction (its seats given up with
//! the lead passed on, its row kept as removed); and a project's rows, with its own folders
//! recorded for the sweep. Chats go through a chat's own delete, in the service.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;
use crate::event::Event;
use crate::folders::{self, SHARED};
use crate::team::{Member, Role};

/// Why a deleted agent's turns ended.
pub const WHY: &str = "its agent was deleted";

/// What a delete takes with it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The chats deleted, archived ones too.
    pub chats: Vec<String>,
    pub projects: Vec<String>,
    /// Who leads where the agent led, by name, each once.
    pub leads: Vec<String>,
}

/// The confirm's counts (and the delete's answer): `{chats, projects, leads}`.
#[derive(Debug, Serialize)]
pub struct Counts {
    pub chats: usize,
    pub projects: usize,
    pub leads: Vec<String>,
}

impl Plan {
    pub fn counts(&self) -> Counts {
        Counts {
            chats: self.chats.len(),
            projects: self.projects.len(),
            leads: self.leads.clone(),
        }
    }
}

/// A project's delete: every chat of it, archived ones too.
pub fn project_plan(conn: &Connection, project: &str) -> Result<Plan, Error> {
    Ok(Plan {
        chats: strings(
            conn,
            "SELECT id FROM chats WHERE project_id = ?1 ORDER BY created_ts, id",
            project,
        )?,
        projects: vec![project.to_string()],
        ..Plan::default()
    })
}

/// An agent's delete: the projects and chats it is the only agent of (a chat with no members by its
/// turns), and who leads after it. A chat with no project and the agent alone on
/// its list where another agent spoke stays, as it did before such a chat kept a list (0028).
pub fn agent_plan(conn: &Connection, agent: &str) -> Result<Plan, Error> {
    let projects = strings(
        conn,
        "SELECT m.project_id FROM project_members m JOIN projects p ON p.id = m.project_id
         WHERE m.agent_id = ?1 AND NOT EXISTS
             (SELECT 1 FROM project_members o WHERE o.project_id = m.project_id AND o.agent_id != ?1)
         ORDER BY p.created_ts, p.id",
        agent,
    )?;
    let mut chats = Vec::new();
    for project in &projects {
        chats.extend(project_plan(conn, project)?.chats);
    }
    let alone = strings(
        conn,
        "SELECT c.id FROM chats c WHERE
             (EXISTS (SELECT 1 FROM chat_members m WHERE m.chat_id = c.id AND m.agent_id = ?1)
              AND NOT EXISTS (SELECT 1 FROM chat_members o WHERE o.chat_id = c.id AND o.agent_id != ?1)
              AND NOT (c.project IS NULL
                  AND EXISTS (SELECT 1 FROM turns t WHERE t.chat_id = c.id AND t.agent_id != ?1)))
          OR (NOT EXISTS (SELECT 1 FROM chat_members m WHERE m.chat_id = c.id)
              AND EXISTS (SELECT 1 FROM turns t WHERE t.chat_id = c.id AND t.agent_id = ?1)
              AND NOT EXISTS (SELECT 1 FROM turns t WHERE t.chat_id = c.id AND t.agent_id != ?1))
         ORDER BY c.created_ts, c.id",
        agent,
    )?;
    for chat in alone {
        if !chats.contains(&chat) {
            chats.push(chat);
        }
    }
    let mut leads = Vec::new();
    for (_, team) in teams(conn, agent, &projects, &chats)? {
        if let Some(next) = next_lead(&team, agent).filter(|m| !leads.contains(&m.name)) {
            leads.push(next.name.clone());
        }
    }
    Ok(Plan {
        chats,
        projects,
        leads,
    })
}

/// A team the agent leaves: a project's (`Seat::Project`) or a chat's.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Seat {
    Project(String),
    Chat(String),
}

/// The teams the agent sits in that stay, each with its members in order: projects first.
fn teams(
    conn: &Connection,
    agent: &str,
    projects: &[String],
    chats: &[String],
) -> Result<Vec<(Seat, Vec<Member>)>, Error> {
    let mut found = Vec::new();
    let sql = "SELECT project_id FROM project_members WHERE agent_id = ?1 ORDER BY project_id";
    for project in strings(conn, sql, agent)? {
        if projects.contains(&project) {
            continue;
        }
        let members = crate::projects::get(conn, &project)?.map(|p| p.members);
        found.push((Seat::Project(project), members.unwrap_or_default()));
    }
    let sql = "SELECT chat_id FROM chat_members WHERE agent_id = ?1 ORDER BY chat_id";
    for chat in strings(conn, sql, agent)? {
        if chats.contains(&chat) {
            continue;
        }
        let members = crate::team::members(conn, &chat)?;
        found.push((Seat::Chat(chat), members));
    }
    Ok(found)
}

/// Who leads once `agent` leaves `team`: the first agent left in the team's order, when it led
/// (the rule Settings > Projects has, ProjectTeam.tsx).
fn next_lead<'a>(team: &'a [Member], agent: &str) -> Option<&'a Member> {
    let led = team
        .iter()
        .any(|m| m.agent == agent && m.role == Role::Supervisor);
    led.then(|| team.iter().find(|m| m.agent != agent))
        .flatten()
}

/// What an agent's removal did: its plan, and the teams it left (`chats` with their members now,
/// `projects` by id).
#[derive(Debug, Default)]
pub struct Removed {
    pub plan: Plan,
    pub chats: Vec<(String, Vec<Member>)>,
    pub projects: Vec<String>,
}

/// Removes `agent` in one transaction (0069): its queued turns stopped and its schedules deleted; it
/// leaves every team it isn't the only agent of, the next agent leading where it led; its row stays,
/// marked removed. What it was the only agent of stays for the service to delete, so a second call
/// finds it again.
pub fn remove_agent(conn: &mut Connection, agent: &str) -> Result<Removed, Error> {
    let tx = conn.transaction()?;
    let plan = agent_plan(&tx, agent)?;
    let now = crate::ids::now_us();
    let mut removed = Removed::default();
    for (seat, team) in teams(&tx, agent, &plan.projects, &plan.chats)? {
        let next = next_lead(&team, agent).map(|m| m.agent.clone());
        let (table, key, id) = match &seat {
            Seat::Project(id) => ("project_members", "project_id", id),
            Seat::Chat(id) => ("chat_members", "chat_id", id),
        };
        if let Some(next) = next {
            let sql = format!(
                "UPDATE {table} SET role = 'supervisor' WHERE {key} = ?1 AND agent_id = ?2"
            );
            tx.execute(&sql, params![id, next])?;
        }
        let sql = format!("DELETE FROM {table} WHERE {key} = ?1 AND agent_id = ?2");
        tx.execute(&sql, params![id, agent])?;
        match seat {
            Seat::Project(id) => removed.projects.push(id),
            Seat::Chat(id) => {
                let members = crate::team::members(&tx, &id)?;
                removed.chats.push((id, members));
            }
        }
    }
    tx.execute(
        "UPDATE turns SET status = 'stopped', why = ?2, ended_ts = ?3, held = NULL
         WHERE agent_id = ?1 AND status IN ('queued', 'waiting_sign_in')",
        params![agent, WHY, now],
    )?;
    tx.execute("DELETE FROM schedules WHERE agent_id = ?1", [agent])?;
    crate::agents::mark_removed(&tx, agent)?;
    tx.commit()?;
    removed.plan = plan;
    Ok(removed)
}

/// Deletes the project's rows once its chats are gone (0069), in one transaction, and records its
/// own folders as due for the sweep: each agent's folder, its checks checkout and caches, never
/// Shared, the project's folder itself or a path that is or holds the person's checkout. Answers
/// those folders.
pub fn delete_project(
    conn: &mut Connection,
    project: &str,
    data: &Path,
    trace: &str,
) -> Result<Vec<PathBuf>, Error> {
    let tx = conn.transaction()?;
    let found: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT folder, folder_name FROM projects WHERE id = ?1",
            [project],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((checkout, folder_name)) = found else {
        return Err(Error::Refused("there's no such project".into()));
    };
    let chats: i64 = tx.query_row(
        "SELECT count(*) FROM chats WHERE project_id = ?1",
        [project],
        |r| r.get(0),
    )?;
    if chats > 0 {
        return Err(Error::Refused(
            "A chat was added meanwhile. Delete again.".into(),
        ));
    }
    let parts = match folder_name {
        Some(name) => parts(
            &tx,
            &folders::real(&folders::root(&tx, data)?).join(name),
            &checkout,
        )?,
        None => Vec::new(),
    };
    let now = crate::ids::now_us();
    for (path, kind) in &parts {
        let repo = (*kind == "worktree").then_some(checkout.as_str());
        tx.execute(
            "INSERT INTO agent_folders (path, kind, chat_id, agent_id, project, made_ts, due_ts)
             VALUES (?1, ?2, NULL, NULL, ?3, ?4, ?4)
             ON CONFLICT (path) DO UPDATE SET kind = excluded.kind, chat_id = NULL, agent_id = NULL,
               project = excluded.project, due_ts = excluded.due_ts, trace = NULL, skipped = NULL,
               removed_ts = NULL",
            params![path.to_string_lossy(), kind, repo, now],
        )?;
    }
    for sql in [
        "DELETE FROM project_members WHERE project_id = ?1",
        "DELETE FROM projects WHERE id = ?1",
    ] {
        tx.execute(sql, [project])?;
    }
    Event::point("gathr.project.delete")
        .trace(trace)
        .ok()
        .attr("gathr.project.id", project)
        .attr("gathr.folder.due", parts.len())
        .insert(&tx)?;
    tx.commit()?;
    Ok(parts.into_iter().map(|(path, _)| path).collect())
}

/// The project's own folders under `place` that are there: its checks checkout (a worktree of the
/// person's checkout), its caches, and the folder of each agent, removed ones too.
fn parts(
    conn: &Connection,
    place: &Path,
    checkout: &str,
) -> Result<Vec<(PathBuf, &'static str)>, Error> {
    let mut named = vec![
        ("checks".to_string(), "worktree"),
        ("caches".to_string(), "folder"),
    ];
    let mut stmt = conn.prepare(
        "SELECT folder_name FROM agents WHERE folder_name IS NOT NULL ORDER BY folder_name",
    )?;
    for agent in stmt.query_map([], |r| r.get(0))? {
        named.push((agent?, "folder"));
    }
    let checkout = folders::real(Path::new(checkout));
    let parts = named
        .into_iter()
        .filter(|(name, _)| !name.eq_ignore_ascii_case(SHARED));
    let parts = parts.map(|(name, kind)| (place.join(name), kind));
    Ok(parts
        .filter(|(path, _)| path.symlink_metadata().is_ok())
        .filter(|(path, _)| !crate::approvals::overlaps(path, &checkout))
        .collect())
}

/// The first column of `sql`'s rows, with `arg` as `?1`.
fn strings(conn: &Connection, sql: &str, arg: &str) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([arg], |r| r.get(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}
