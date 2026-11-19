//! Which conversation a turn runs in (0055): a member's task has its own engine session, kept in
//! `task_sessions` with the fill it last showed; every other turn runs in the agent's latest
//! (`engine_sessions`).

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// What a turn runs on (N1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// The chat's Supervisor: one conversation per chat.
    Lead,
    /// The agent's latest conversation.
    Latest,
    /// The turn's own task.
    Task(i64),
    /// No task, but it answers a turn of the same agent's on this task: a hear-back, a person's
    /// answer, a carry-on. It never opens a new conversation.
    Answers(i64),
}

impl Key {
    /// `gathr.session.for`.
    pub fn as_for(&self) -> &'static str {
        match self {
            Key::Lead => "lead",
            Key::Latest => "latest",
            Key::Task(_) | Key::Answers(_) => "task",
        }
    }
}

/// The conversation a key gives (N2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Conversation {
    Latest,
    /// A new conversation for the task.
    New(i64),
    /// The engine opens for `task` by resuming `session`: the task's own, or for Gathr's fix, its
    /// PR's work task's (`work_task`).
    Resume {
        task: i64,
        session: String,
        work_task: bool,
    },
}

impl Conversation {
    /// `gathr.session.from`.
    pub fn as_from(&self) -> &'static str {
        match self {
            Conversation::Latest => "latest",
            Conversation::New(_) => "new",
            Conversation::Resume {
                work_task: false, ..
            } => "task",
            Conversation::Resume { .. } => "work_task",
        }
    }
}

/// The key of a turn, read from its row (N1).
pub fn want(conn: &Connection, turn: &str) -> Result<Key, Error> {
    type Row = (
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<String>,
        Option<i64>,
        String,
    );
    let (agent, task, role, answered, answered_task, chat): Row = conn.query_row(
        "SELECT u.agent_id, u.task, m.role, a.agent_id, a.task, u.chat_id FROM turns u
         LEFT JOIN chat_members m ON m.chat_id = u.chat_id AND m.agent_id = u.agent_id
         LEFT JOIN turns a ON a.id = u.answers
         WHERE u.id = ?1",
        [turn],
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
    )?;
    // A chat of one with no project keeps the keys it had before it kept a list (0028).
    let lead = role.as_deref() == Some("supervisor") && !crate::team::alone(conn, &chat)?;
    Ok(match (agent, task, answered_task) {
        (None, _, _) => Key::Latest,
        _ if lead => Key::Lead,
        (_, Some(task), _) => Key::Task(task),
        (agent, None, Some(task)) if agent == answered => Key::Answers(task),
        _ => Key::Latest,
    })
}

/// The conversation `key` gives `agent` on `account` (N2).
pub fn choose(
    conn: &Connection,
    agent: &str,
    account: &str,
    key: Key,
) -> Result<Conversation, Error> {
    let (task, answers) = match key {
        Key::Lead | Key::Latest => return Ok(Conversation::Latest),
        Key::Task(task) => (task, false),
        Key::Answers(task) => (task, true),
    };
    if let Some(session) = session_on(conn, task, agent, account)? {
        return Ok(Conversation::Resume {
            task,
            session,
            work_task: false,
        });
    }
    if answers {
        return Ok(Conversation::Latest);
    }
    if let Some(session) = work_tasks(conn, task, agent, account)? {
        return Ok(Conversation::Resume {
            task,
            session,
            work_task: true,
        });
    }
    Ok(Conversation::New(task))
}

/// The session of `task`'s row for `agent`, when it lives in `account`'s login folder.
fn session_on(
    conn: &Connection,
    task: i64,
    agent: &str,
    account: &str,
) -> Result<Option<String>, Error> {
    Ok(conn
        .query_row(
            "SELECT session_id FROM task_sessions WHERE task = ?1 AND agent_id = ?2 AND account_id = ?3",
            params![task, agent, account],
            |r| r.get(0),
        )
        .optional()?)
}

/// For a fix Gathr gave (named in `review_notes.fix`): its PR's work task's session, when that task
/// is `agent`'s and its session lives in `account`'s folder (answer 4).
fn work_tasks(
    conn: &Connection,
    fix: i64,
    agent: &str,
    account: &str,
) -> Result<Option<String>, Error> {
    let of_fix: Option<(String, i64)> = conn
        .query_row(
            "SELECT chat_id, pr FROM tasks WHERE number = ?1 AND pr IS NOT NULL
             AND EXISTS (SELECT 1 FROM review_notes WHERE fix = ?1)",
            [fix],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let Some((chat, pr)) = of_fix else {
        return Ok(None);
    };
    match crate::tasks::notes::work_on(conn, &chat, pr)? {
        Some((owner, work)) if owner == agent && work != fix => {
            session_on(conn, work, agent, account)
        }
        _ => Ok(None),
    }
}

/// The engine opened for `task` reported `session` (N5), or a task that would have opened New
/// stays in the open one (N4). Another session than the row had starts with no fill and untold.
pub fn set(
    conn: &Connection,
    task: i64,
    agent: &str,
    chat: &str,
    account: &str,
    session: &str,
) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO task_sessions (task, agent_id, chat_id, account_id, session_id)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (task, agent_id) DO UPDATE SET account_id = ?4, session_id = ?5,
             used = CASE WHEN session_id = ?5 THEN used END,
             told = CASE WHEN session_id = ?5 THEN told ELSE 0 END",
        params![task, agent, chat, account, session],
    )?;
    Ok(())
}

/// The fill of `task`'s conversation, as `context::filled` saves the agent's (N7).
pub fn used(conn: &Connection, task: i64, agent: &str, used: i64) -> Result<(), Error> {
    conn.execute(
        "UPDATE task_sessions SET used = ?3 WHERE task = ?1 AND agent_id = ?2",
        params![task, agent, used],
    )?;
    Ok(())
}

/// Whether `task`'s conversation was told it nears compaction, as `context::tell_soon` and
/// `context::untold` mark the agent's (N7).
pub fn told(conn: &Connection, task: i64, agent: &str, told: bool) -> Result<(), Error> {
    conn.execute(
        "UPDATE task_sessions SET told = ?3 WHERE task = ?1 AND agent_id = ?2",
        params![task, agent, told],
    )?;
    Ok(())
}

/// The engine refused to resume `session` (N8): `agent`'s rows holding it go, in both tables, so
/// the next start opens new. `agent` none is a chat's turns sent to no agent.
pub fn forget(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    session: &str,
) -> Result<(), Error> {
    let agent = agent.unwrap_or_default();
    conn.execute(
        "DELETE FROM task_sessions WHERE chat_id = ?1 AND agent_id = ?2 AND session_id = ?3",
        params![chat, agent, session],
    )?;
    conn.execute(
        "DELETE FROM engine_sessions WHERE chat_id = ?1 AND agent_key = ?2 AND session_id = ?3",
        params![chat, agent, session],
    )?;
    Ok(())
}

/// The fill of `session`, resumed for `task`, and whether it was told, for `context::switched`:
/// the task's own row, else the newest task's row on the same session that holds one. A fix
/// resumes its work task's session before it has a row, and a task kept in the open conversation
/// (N4) never saved a fill of its own.
pub(crate) fn context_of(
    conn: &Connection,
    task: i64,
    agent: &str,
    session: &str,
) -> Result<Option<(Option<i64>, bool)>, Error> {
    Ok(conn
        .query_row(
            "SELECT used, told FROM task_sessions
             WHERE agent_id = ?2 AND (task = ?1 OR session_id = ?3)
             ORDER BY used IS NOT NULL DESC, task = ?1 DESC, task DESC LIMIT 1",
            params![task, agent, session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?)
}
