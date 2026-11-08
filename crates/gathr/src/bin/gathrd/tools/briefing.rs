//! Each engine's briefing (docs/decisions/0021-no-prompting-needed.md, 0020-context-per-turn.md):
//! who the agent is, where, who else has worked here, the rules tools can't carry, Gathr's tools
//! and what's turned off (G21). Written once per engine, so it stays the same every turn of its
//! session and the provider's cache keeps it.

use gathr::claude::command::DENIED_TOOLS;
use rusqlite::{Connection, OptionalExtension};

/// What the briefing says about the chat: its title, the agent's name, and the other agents who
/// have run turns in it.
pub struct Facts {
    title: String,
    name: Option<String>,
    others: Vec<String>,
}

pub fn facts(conn: &Connection, chat: &str, agent: Option<&str>) -> rusqlite::Result<Facts> {
    let title: String = conn
        .query_row("SELECT title FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .unwrap_or_default();
    let name = match agent {
        Some(id) => conn
            .query_row("SELECT name FROM agents WHERE id = ?1", [id], |r| r.get(0))
            .optional()?,
        None => None,
    };
    let mut others = conn.prepare(
        "SELECT a.name FROM agents a WHERE a.id IN \
         (SELECT agent_id FROM turns WHERE chat_id = ?1 AND agent_id IS NOT NULL) \
         AND a.id IS NOT ?2 ORDER BY a.name",
    )?;
    let others = others
        .query_map(rusqlite::params![chat, agent], |r| r.get(0))?
        .collect::<rusqlite::Result<Vec<String>>>()?;
    Ok(Facts {
        title,
        name,
        others,
    })
}

/// The tools turned off for every agent, by name: what the briefing says is off.
pub fn off() -> Vec<&'static str> {
    DENIED_TOOLS.iter().map(|(name, _)| *name).collect()
}

pub fn text(facts: &Facts) -> String {
    let who = match &facts.name {
        Some(name) => format!("You are {name}, an agent"),
        None => "You are an agent".to_string(),
    };
    let mut lines = vec![format!("{who} in Gathr, in the chat \"{}\".", facts.title)];
    if !facts.others.is_empty() {
        lines.push(format!(
            "Other agents who have worked here: {}. Their replies may be missing from your context.",
            facts.others.join(", ")
        ));
    }
    lines.extend([
        "Follow the project's AGENTS.md and CLAUDE.md when present.".to_string(),
        "Check facts in code, current docs or a live run before stating or relying on them. Say \
         what you couldn't verify."
            .to_string(),
        // Claude Code defers MCP tools behind ToolSearch, so an unnamed tool is never found (0005).
        "Use read_chat on the gathr server for earlier messages and other agents' replies missing \
         from your context. Find it with ToolSearch if needed."
            .to_string(),
        format!(
            "Unavailable tools: {}. Engine timers cannot wake you after Gathr closes the engine.",
            off().join(", ")
        ),
    ]);
    lines.join("\n")
}
