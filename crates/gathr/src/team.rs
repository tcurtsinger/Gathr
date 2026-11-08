//! A chat's agents and their roles (0013), and who a message goes to (0028).

use rusqlite::{Connection, params};
use serde::Serialize;

use crate::Error;

/// The most agents a chat has: past five, nobody can follow it (0013).
pub const MOST: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Supervisor,
    Reviewer,
    Member,
}

impl Role {
    fn as_str(self) -> &'static str {
        match self {
            Role::Supervisor => "supervisor",
            Role::Reviewer => "reviewer",
            Role::Member => "member",
        }
    }

    pub fn named(text: &str) -> Option<Role> {
        match text {
            "supervisor" => Some(Role::Supervisor),
            "reviewer" => Some(Role::Reviewer),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

/// One of a chat's agents.
#[derive(Debug, Clone, Serialize)]
pub struct Member {
    pub agent: String,
    pub name: String,
    pub role: Role,
}

/// Why a team is turned down, or `None` when it holds: 1 to 5 agents, each once; with two or more,
/// one Supervisor; at most one Reviewer, and never on its own.
pub fn refused(team: &[(String, Role)]) -> Option<&'static str> {
    let count = |role| team.iter().filter(|(_, r)| *r == role).count();
    let mut agents: Vec<&str> = team.iter().map(|(a, _)| a.as_str()).collect();
    agents.sort_unstable();
    agents.dedup();
    if team.is_empty() || team.len() > MOST {
        Some("a chat has 1 to 5 agents")
    } else if agents.len() < team.len() {
        Some("an agent is in the chat only once")
    } else if count(Role::Supervisor) > 1 {
        Some("a chat has one Supervisor")
    } else if team.len() > 1 && count(Role::Supervisor) == 0 {
        Some("a chat with two or more agents needs a Supervisor")
    } else if count(Role::Reviewer) > 1 {
        Some("a chat has at most one Reviewer")
    } else if team.len() == 1 && count(Role::Reviewer) == 1 {
        Some("a Reviewer needs another agent in the chat")
    } else {
        None
    }
}

/// The chat's agents: the Supervisor first, then in the order they were added.
pub fn members(conn: &Connection, chat: &str) -> Result<Vec<Member>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT m.agent_id, a.name, m.role FROM chat_members m JOIN agents a ON a.id = m.agent_id
         WHERE m.chat_id = ?1 ORDER BY m.role != 'supervisor', m.added_ts, m.rowid",
    )?;
    let rows = statement.query_map([chat], |r| {
        Ok(Member {
            agent: r.get(0)?,
            name: r.get(1)?,
            // The table's CHECK allows only the three.
            role: Role::named(&r.get::<_, String>(2)?).unwrap_or(Role::Member),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Replaces the chat's agents with `team`, already checked; agents staying keep when they joined.
/// An agent that doesn't exist fails the write (its foreign key).
pub fn set_members(
    conn: &mut Connection,
    chat: &str,
    team: &[(String, Role)],
) -> Result<(), Error> {
    let tx = conn.transaction()?;
    let kept: Vec<&str> = team.iter().map(|(a, _)| a.as_str()).collect();
    let kept = serde_json::to_string(&kept).unwrap_or_default();
    tx.execute(
        "DELETE FROM chat_members WHERE chat_id = ?1 AND agent_id NOT IN (SELECT value FROM json_each(?2))",
        params![chat, kept],
    )?;
    for (agent, role) in team {
        tx.execute(
            "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (chat_id, agent_id) DO UPDATE SET role = ?3",
            params![chat, agent, role.as_str(), crate::ids::now_us()],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// The title of a chat the agent is in, if any: an agent isn't deleted while a chat has it.
pub fn a_chat_of(conn: &Connection, agent: &str) -> Result<Option<String>, Error> {
    use rusqlite::OptionalExtension;
    Ok(conn
        .query_row(
            "SELECT c.title FROM chat_members m JOIN chats c ON c.id = m.chat_id
             WHERE m.agent_id = ?1 ORDER BY c.created_ts LIMIT 1",
            [agent],
            |r| r.get(0),
        )
        .optional()?)
}

/// Who a message goes to in a chat with `team` (0028): the agent it was sent to, which must be one
/// of them; else the first agent its text names with `@Name`; else the Supervisor, or the only
/// agent. A chat with no agents keeps what it was sent to.
pub fn addressee(
    team: &[Member],
    sent_to: Option<&str>,
    text: &str,
) -> Result<Option<String>, &'static str> {
    if team.is_empty() {
        return Ok(sent_to.map(String::from));
    }
    if let Some(agent) = sent_to {
        return match team.iter().any(|m| m.agent == agent) {
            true => Ok(Some(agent.to_string())),
            false => Err("the agent isn't in this chat"),
        };
    }
    let named = first_named(team, text);
    let lead = team.iter().find(|m| m.role == Role::Supervisor);
    let only = (team.len() == 1).then(|| &team[0]);
    Ok(named.or(lead).or(only).map(|m| m.agent.clone()))
}

/// The member whose `@Name` comes first in `text`, ignoring case; at one place, the longest name.
/// A name counts only where it ends: `@Al` isn't in `@Alex`.
fn first_named<'a>(team: &'a [Member], text: &str) -> Option<&'a Member> {
    let text = text.to_lowercase();
    let mut best: Option<(usize, usize, &Member)> = None;
    for member in team {
        let mention = format!("@{}", member.name.to_lowercase());
        let found = text.match_indices(&mention).map(|(at, _)| at).find(|&at| {
            let after = text[at + mention.len()..].chars().next();
            !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
        });
        let Some(at) = found else { continue };
        let better = match best {
            None => true,
            Some((was, len, _)) => at < was || (at == was && mention.len() > len),
        };
        if better {
            best = Some((at, mention.len(), member));
        }
    }
    best.map(|(_, _, member)| member)
}
