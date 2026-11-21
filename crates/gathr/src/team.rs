//! A chat's agents and their roles (0013), and who a message goes to (0028).

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

use crate::Error;

/// The most agents a chat has: past five, nobody can follow it (0013).
pub const MOST: usize = 5;

/// The lead is stored and sent as `supervisor`, its name before it was called the lead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Supervisor,
    Member,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Supervisor => "supervisor",
            Role::Member => "member",
        }
    }

    pub fn named(text: &str) -> Option<Role> {
        match text {
            "supervisor" => Some(Role::Supervisor),
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
/// one lead.
pub fn refused(team: &[(String, Role)]) -> Option<&'static str> {
    let count = |role| team.iter().filter(|(_, r)| *r == role).count();
    let mut agents: Vec<&str> = team.iter().map(|(a, _)| a.as_str()).collect();
    agents.sort_unstable();
    agents.dedup();
    if team.is_empty() || team.len() > MOST {
        Some("A chat needs 1 to 5 agents")
    } else if agents.len() < team.len() {
        Some("An agent can join a chat only once")
    } else if count(Role::Supervisor) > 1 {
        Some("A chat can have only one lead")
    } else if team.len() > 1 && count(Role::Supervisor) == 0 {
        Some("A team needs a lead")
    } else {
        None
    }
}

/// The chat's agents: the lead first, then in the order they were added.
pub fn members(conn: &Connection, chat: &str) -> Result<Vec<Member>, Error> {
    let mut statement = conn.prepare_cached(
        "SELECT m.agent_id, a.name, m.role FROM chat_members m JOIN agents a ON a.id = m.agent_id
         WHERE m.chat_id = ?1 ORDER BY m.role != 'supervisor', m.added_ts, m.rowid",
    )?;
    let rows = statement.query_map([chat], |r| {
        Ok(Member {
            agent: r.get(0)?,
            name: r.get(1)?,
            // Migration 0075 made every reviewer a member.
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
    put(&tx, chat, team)?;
    tx.commit()?;
    Ok(())
}

fn put(conn: &Connection, chat: &str, team: &[(String, Role)]) -> Result<(), Error> {
    for (agent, role) in team {
        conn.execute(
            "INSERT INTO chat_members (chat_id, agent_id, role, added_ts) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (chat_id, agent_id) DO UPDATE SET role = ?3",
            params![chat, agent, role.as_str(), crate::ids::now_us()],
        )?;
    }
    Ok(())
}

/// Adds `agent` to the chat as a member, its first agent leading, when the team still holds: false
/// when it wouldn't (a full chat), and nothing is written.
pub fn seat(conn: &Connection, chat: &str, agent: &str) -> Result<bool, Error> {
    let mut seats: Vec<(String, Role)> = members(conn, chat)?
        .into_iter()
        .map(|m| (m.agent, m.role))
        .collect();
    seats.push((agent.to_string(), Role::Member));
    if !seats.iter().any(|(_, role)| *role == Role::Supervisor) {
        seats[0].1 = Role::Supervisor;
    }
    if refused(&seats).is_some() {
        return Ok(false);
    }
    put(conn, chat, &seats)?;
    Ok(true)
}

/// Where a message goes: its agent, every other agent picked in it, and the chat's agents when
/// sending it added some.
pub struct Routed {
    pub agent: Option<String>,
    pub others: Vec<String>,
    pub joined: Option<Vec<Member>>,
}

/// Where a message will go, worked out and nothing written: its agent, every other agent picked in
/// it, and the chat's whole team when agents join it.
pub struct Plan {
    pub agent: Option<String>,
    pub others: Vec<String>,
    pub seats: Option<Vec<(String, Role)>>,
}

/// Who a message goes to (0028), and the outsiders it brings in, in one write. The `Err` is why the
/// message can't go; nothing is written then.
pub fn route(
    conn: &mut Connection,
    chat: &str,
    sent_to: Option<&str>,
    picked: &[String],
) -> Result<Result<Routed, &'static str>, Error> {
    match plan(conn, chat, sent_to, picked)? {
        Ok(plan) => Ok(Ok(join(conn, chat, plan)?)),
        Err(why) => Ok(Err(why)),
    }
}

/// Whether the chat has no project and one agent on its list. Such a chat kept no list before 0058;
/// what looks for a lead finds none in it still, until the owner decides each (0028).
pub fn alone(conn: &Connection, chat: &str) -> Result<bool, Error> {
    let sql = "SELECT c.project IS NULL AND (SELECT count(*) FROM chat_members m WHERE m.chat_id = c.id) = 1
               FROM chats c WHERE c.id = ?1";
    let alone = conn.prepare_cached(sql)?.query_row([chat], |r| r.get(0));
    Ok(alone.optional()?.unwrap_or(false))
}

/// Who a message goes to (0028): the agent it is sent to, and every other agent `picked` in it. In a
/// chat with no project, its first agent leads it, and the picked agents from outside it join it
/// (F8b) as members; with no agents yet, a message that names nobody goes to the assistant. In a
/// project's chat a pick outside its team refuses the send. Reads only: `join` writes it.
pub fn plan(
    conn: &Connection,
    chat: &str,
    sent_to: Option<&str>,
    picked: &[String],
) -> Result<Result<Plan, &'static str>, Error> {
    let team = members(conn, chat)?;
    let plain: bool = conn.query_row(
        "SELECT project IS NULL FROM chats WHERE id = ?1",
        [chat],
        |r| r.get(0),
    )?;
    // A message that names nobody in a new chat with no project goes to the person's assistant.
    let assistant = match plain && team.is_empty() && sent_to.is_none() && picked.is_empty() {
        true => crate::assistant::assistant(conn)?,
        false => None,
    };
    let sent_to = sent_to.or(assistant.as_deref());
    let mut others: Vec<String> = Vec::new();
    for agent in picked {
        if Some(agent.as_str()) != sent_to && !others.contains(agent) {
            others.push(agent.clone());
        }
    }
    let outside = |agent: &str| !team.iter().any(|m| m.agent == agent);
    let joining: Vec<&str> = sent_to
        .into_iter()
        .chain(others.iter().map(String::as_str))
        .filter(|a| outside(a))
        .collect();
    let planned = |agent: Option<String>, others: Vec<String>, seats| {
        let others = others.into_iter().filter(|o| Some(o) != agent.as_ref());
        Plan {
            others: others.collect(),
            agent,
            seats,
        }
    };
    if joining.is_empty() || (!plain && team.is_empty()) {
        let agent = addressee(&team, sent_to);
        return Ok(agent.map(|agent| planned(agent, others, None)));
    }
    if !plain {
        return Ok(Err(match sent_to.is_some_and(outside) {
            true => "the agent isn't in this chat",
            false => "a picked agent isn't in this chat",
        }));
    }
    let mut seats: Vec<(String, Role)> = team.iter().map(|m| (m.agent.clone(), m.role)).collect();
    if seats.is_empty() {
        // The agent the chat has run on so far, if it still exists.
        let first: Option<String> = conn
            .query_row(
                "SELECT t.agent_id FROM turns t JOIN agents a ON a.id = t.agent_id
                 WHERE t.chat_id = ?1 AND a.removed_ts IS NULL ORDER BY t.created_ts DESC LIMIT 1",
                [chat],
                |r| r.get(0),
            )
            .optional()?;
        seats.extend(
            first
                .filter(|f| !joining.contains(&f.as_str()))
                .map(|f| (f, Role::Member)),
        );
    }
    seats.extend(joining.iter().map(|a| (a.to_string(), Role::Member)));
    if !seats.iter().any(|(_, role)| *role == Role::Supervisor) {
        seats[0].1 = Role::Supervisor;
    }
    if let Some(why) = refused(&seats) {
        return Ok(Err(why));
    }
    let lead = seats.iter().find(|(_, role)| *role == Role::Supervisor);
    let agent = sent_to
        .map(String::from)
        .or_else(|| lead.map(|(a, _)| a.clone()));
    Ok(Ok(planned(agent, others, Some(seats))))
}

/// Writes what `plan` worked out: the chat's team when agents join it.
pub fn join(conn: &mut Connection, chat: &str, plan: Plan) -> Result<Routed, Error> {
    let joined = match &plan.seats {
        Some(seats) => {
            set_members(conn, chat, seats)?;
            Some(members(conn, chat)?)
        }
        None => None,
    };
    Ok(Routed {
        agent: plan.agent,
        others: plan.others,
        joined,
    })
}
/// Who a message goes to in a chat with `team` (0028): the agent it was sent to, which must be one
/// of them; else the lead, or the only agent. A chat with no agents keeps what it was sent
/// to. The text is never read for names: a name counts only when picked from the window's list,
/// which sends its agent, so `bob@ann.com` names nobody.
pub fn addressee(team: &[Member], sent_to: Option<&str>) -> Result<Option<String>, &'static str> {
    if team.is_empty() {
        return Ok(sent_to.map(String::from));
    }
    if let Some(agent) = sent_to {
        return match team.iter().any(|m| m.agent == agent) {
            true => Ok(Some(agent.to_string())),
            false => Err("the agent isn't in this chat"),
        };
    }
    let lead = team.iter().find(|m| m.role == Role::Supervisor);
    let only = (team.len() == 1).then(|| &team[0]);
    Ok(lead.or(only).map(|m| m.agent.clone()))
}

/// The members `text` names with `@Name` (0081), ignoring case, in the order they are first named.
/// Longest name first, each place it is found blanked, so `@Bo` is never found inside `@Bobby`. A
/// name counts only as a whole word: `@Al` isn't in `@Alex`, and `bob@ann.com` names nobody.
pub fn mentioned<'a>(team: &'a [Member], text: &str) -> Vec<&'a Member> {
    let mut left: Vec<char> = text.to_lowercase().chars().collect();
    let mut longest: Vec<&Member> = team.iter().collect();
    longest.sort_by_key(|m| std::cmp::Reverse(m.name.chars().count()));
    let word = |c: &char| c.is_alphanumeric() || *c == '_' || *c == '-';
    let mut found = Vec::new();
    for member in longest {
        let name: Vec<char> = format!("@{}", member.name.to_lowercase()).chars().collect();
        let mut first = None;
        let mut at = 0;
        while name.len() > 1 && at + name.len() <= left.len() {
            let starts = at == 0 || !word(&left[at - 1]);
            let ends = left.get(at + name.len()).is_none_or(|c| !word(c));
            if starts && ends && left[at..at + name.len()] == name[..] {
                left[at..at + name.len()].fill(' ');
                first.get_or_insert(at);
                at += name.len();
            } else {
                at += 1;
            }
        }
        if let Some(at) = first {
            found.push((at, member));
        }
    }
    found.sort_by_key(|(at, _)| *at);
    found.into_iter().map(|(_, member)| member).collect()
}

/// The member called `name`, ignoring case, matched as the person's `@Name` is.
pub fn named<'a>(team: &'a [Member], name: &str) -> Option<&'a Member> {
    first_named(team, &format!("@{}", name.trim().trim_start_matches('@')))
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

#[cfg(test)]
mod tests {
    use super::*;

    fn team(names: &[&str]) -> Vec<Member> {
        let member = |name: &&str| Member {
            agent: name.to_lowercase(),
            name: name.to_string(),
            role: Role::Member,
        };
        names.iter().map(member).collect()
    }

    fn named(team: &[Member], text: &str) -> Vec<String> {
        mentioned(team, text)
            .iter()
            .map(|m| m.name.clone())
            .collect()
    }

    #[test]
    fn mentions_are_found_longest_first_in_the_order_named() {
        let crew = team(&["Bo", "Bobby Tables", "Made-up", "Made"]);
        assert_eq!(
            named(&crew, "@bobby tables then @BO"),
            ["Bobby Tables", "Bo"]
        );
        assert_eq!(named(&crew, "@Made-up, not @Made."), ["Made-up", "Made"]);
        assert_eq!(named(&crew, "@Bo and @Bo again"), ["Bo"]);
    }

    #[test]
    fn a_name_counts_only_as_a_whole_word() {
        let crew = team(&["Ann", "Al"]);
        assert!(named(&crew, "@Annette, @Alex and bob@ann.com").is_empty());
        assert_eq!(named(&crew, "(@ann) and @Al: done"), ["Ann", "Al"]);
    }
}
