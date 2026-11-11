//! Pending removal of a finished PR's local branch and recorded worktrees (0036 C10).
use super::Clean;
use crate::Error;
use rusqlite::{Connection, OptionalExtension, params};

#[derive(Clone)]
pub struct Job {
    pub project: String,
    pub branch: String,
    pub head: String,
    pub chat: String,
    pub trace: String,
    pub queued: i64,
    pub skipped: Option<String>,
}

/// Called before settling tasks, so a stop between the two cannot lose cleanup work.
pub fn queue(
    conn: &Connection,
    chat: &str,
    branch: &str,
    head: &str,
    trace: &str,
) -> Result<(), Error> {
    let project: Option<String> = conn
        .query_row("SELECT project FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let Some(project) = project else {
        return Ok(());
    };
    conn.execute("INSERT INTO pr_cleanup (project,branch,head,chat,trace,queued_ts) VALUES (?1,?2,?3,?4,?5,?6)
        ON CONFLICT(project,branch) DO UPDATE SET head=excluded.head,chat=excluded.chat,
        trace=excluded.trace,queued_ts=excluded.queued_ts,skipped=NULL WHERE pr_cleanup.head<>excluded.head",
        params![project,branch,head,chat,trace,crate::ids::now_us()])?;
    Ok(())
}

pub fn pending(conn: &Connection, chat: Option<&str>) -> Result<Vec<Job>, Error> {
    let mut stmt=conn.prepare("SELECT project,branch,head,chat,trace,queued_ts,skipped FROM pr_cleanup WHERE ?1 IS NULL OR chat=?1")?;
    Ok(stmt
        .query_map([chat], |r| {
            Ok(Job {
                project: r.get(0)?,
                branch: r.get(1)?,
                head: r.get(2)?,
                chat: r.get(3)?,
                trace: r.get(4)?,
                queued: r.get(5)?,
                skipped: r.get(6)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

/// Only paths Gathr recorded before the PR finished; later work is never claimed by this job.
pub fn worktrees(conn: &Connection, job: &Job) -> Result<Vec<Clean>, Error> {
    let mut stmt=conn.prepare("SELECT f.path,f.chat_id,f.agent_id,
        EXISTS(SELECT 1 FROM chats c WHERE c.id=f.chat_id) FROM agent_folders f
        WHERE kind='worktree' AND project=?1 AND removed_ts IS NULL AND made_ts<=?2 ORDER BY made_ts")?;
    Ok(stmt
        .query_map(params![job.project, job.queued], |r| {
            Ok(Clean {
                path: r.get::<_, String>(0)?.into(),
                kind: "worktree".into(),
                chat: r.get(1)?,
                agent: r.get(2)?,
                project: Some(job.project.clone()),
                trace: job.trace.clone(),
                skipped: None,
                chat_kept: r.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?)
}

pub fn skipped(conn: &Connection, job: &Job, why: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE pr_cleanup SET skipped=?4 WHERE project=?1 AND branch=?2 AND head=?3",
        params![job.project, job.branch, job.head, why],
    )?;
    Ok(())
}

pub fn finished(conn: &Connection, job: &Job) -> Result<(), Error> {
    conn.execute(
        "DELETE FROM pr_cleanup WHERE project=?1 AND branch=?2 AND head=?3",
        params![job.project, job.branch, job.head],
    )?;
    Ok(())
}
