//! Each engine's briefing (docs/decisions/0021-no-prompting-needed.md, 0020-context-per-turn.md):
//! who the agent is, where, its role and the chat's other agents, its own folder, the rules tools
//! can't carry, Gathr's tools and what's turned off (G21). Written once per engine, so it stays the
//! same every turn of its session and the provider's cache keeps it.

use std::path::PathBuf;

use gathr::claude::command::DENIED_TOOLS;
use gathr::team::{self, Role};
use rusqlite::{Connection, OptionalExtension};

/// What the briefing says about the chat: its title, the agent's name and role, the other agents
/// (the chat's team, or with none, those who have run turns in it), and the agent's worktree.
pub struct Facts {
    title: String,
    name: Option<String>,
    /// Named in a chat of two or more only.
    pub role: Option<Role>,
    others: Vec<String>,
    team: bool,
    /// The agent's folder and the project it's a worktree of.
    worktree: Option<(PathBuf, String)>,
    /// The agent's own folder and its project's Shared (0034).
    own: Option<(PathBuf, PathBuf)>,
    /// A room chat, briefed with `ROOM` instead of the old team's tools and rules.
    room: bool,
}

pub fn facts(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    worktree: Option<(PathBuf, String)>,
    own: Option<(PathBuf, PathBuf)>,
) -> Result<Facts, gathr::Error> {
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
    // A chat of one with no project is briefed as it was before it kept a list (0028).
    let members = match team::alone(conn, chat)? {
        true => Vec::new(),
        false => team::members(conn, chat)?,
    };
    let role = members
        .iter()
        .find(|m| Some(m.agent.as_str()) == agent)
        .map(|m| m.role);
    let others = match members.is_empty() {
        true => worked_here(conn, chat, agent)?,
        false => members
            .iter()
            .filter(|m| Some(m.agent.as_str()) != agent)
            .map(|m| match title_of(m.role) {
                Some(role) => format!("{} ({role})", m.name),
                None => m.name.clone(),
            })
            .collect(),
    };
    Ok(Facts {
        title,
        name,
        role: role.filter(|_| members.len() > 1),
        others,
        team: !members.is_empty(),
        worktree,
        own,
        room: gathr::room::is_room(conn, chat)?,
    })
}

fn worked_here(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
) -> rusqlite::Result<Vec<String>> {
    let mut others = conn.prepare(
        "SELECT a.name FROM agents a WHERE a.id IN \
         (SELECT agent_id FROM turns WHERE chat_id = ?1 AND agent_id IS NOT NULL) \
         AND a.id IS NOT ?2 AND a.removed_ts IS NULL ORDER BY a.name",
    )?;
    let names = others.query_map(rusqlite::params![chat, agent], |r| r.get(0))?;
    names.collect()
}

fn title_of(role: Role) -> Option<&'static str> {
    match role {
        Role::Supervisor => Some("Supervisor"),
        Role::Reviewer => Some("Reviewer"),
        Role::Member => None,
    }
}

/// The tools turned off for every agent, by name: what the briefing says is off.
pub fn off() -> Vec<&'static str> {
    DENIED_TOOLS.iter().map(|(name, _)| *name).collect()
}

/// How a room chat works, for an agent with teammates there.
const ROOM_TEAM: &str = "How this room works:
- You get a turn when the person writes to you, or when a teammate names you with @Name. Write @Name to wake \
a teammate: they get your words when your turn ends. Name only who needs to act.
- When your words comment on a teammate's message, start them with REPLY on a line of its own, and they are \
filed under that message. Never start your answer to the person with REPLY.
- The Supervisor leads. A message from the person that names nobody goes to it, and its plan or answer is \
checked by the teammates who took no part, before the person relies on it.";

/// What every agent in a room chat is told: the owner's own asks, the board and the tools.
const ROOM: &str = "- If you have nothing to add, end your turn without writing anything. To agree, react with a \
thumbs-up instead of writing.
- Keep your line on the board current with set_board_line: what you work on, and its state.
- Keep your messages brief. Be friendly and professional.
- Don't push fixes for edge cases nobody has hit, and don't change things over wording or semantics.
- A reply, a rebuttal or a reported defect needs a strong argument: say what is wrong and show why.
- Ask the person with ask_person only when the decision is theirs to make.
Gathr's tools, on the gathr server (find them with ToolSearch if needed): react, set_board_line, ask_person, read_chat for \
earlier messages, and pr, ci_failures and update_pr if you need GitHub, through Gathr's sign-in.";

pub fn text(facts: &Facts) -> String {
    let who = match &facts.name {
        Some(name) => format!("You are {name}, an agent"),
        None => "You are an agent".to_string(),
    };
    let role = match facts.role.and_then(title_of) {
        Some(role) => format!(", as its {role}"),
        None => String::new(),
    };
    let mut lines = vec![format!(
        "{who} in Gathr, in the chat \"{}\"{role}.",
        facts.title
    )];
    if let Some(role) = facts.role.filter(|_| !facts.room) {
        lines.extend(gathr::roles::lines(role).into_iter().map(String::from));
    }
    if !facts.others.is_empty() {
        // A team chat's turn opens with what the chat said since the agent's last turn (0081).
        let (lead, heard) = match facts.team {
            true => (
                "The chat's other agents",
                "Each turn opens with what the chat said since your last one; read_chat has the rest.",
            ),
            false => (
                "Other agents who have worked here",
                "Their replies may be missing from your context.",
            ),
        };
        lines.push(format!("{lead}: {}. {heard}", facts.others.join(", ")));
    }
    if let Some((folder, project)) = &facts.worktree {
        lines.push(format!(
            "Your worktree is \"{}\", from \"{project}\". Make changes and commits only in your \
             worktree.",
            folder.display()
        ));
    }
    if facts.worktree.is_some() && !facts.room {
        // Gathr reaches GitHub with its own sign-in, which an engine's sandbox may not have (0046).
        lines.push(
            "For this project's PRs use Gathr's pr, ci_failures and update_pr, not your own gh: Gathr asks \
             GitHub for you. Gathr merges a PR once its review is done and its checks passed; by hand, \
             merge only with merge."
                .to_string(),
        );
    }
    if let Some((own, shared)) = &facts.own {
        lines.push(format!(
            "Your own folder is \"{}\": keep test data, builds and notes there. The team's shared \
             folder is \"{}\": put plans, reviews and evidence for others there. You can write in \
             both without asking. Gathr deletes your TEMP folder when it closes your session, and \
             a worktree once the chat is archived, keeping its branch and anything not committed.",
            own.display(),
            shared.display()
        ));
    }
    lines.extend([
        "Follow the project's AGENTS.md and CLAUDE.md when present.".to_string(),
        "Check facts in code, current documentation or a live run. Say what you could not verify."
            .to_string(),
    ]);
    let unavailable = format!(
        "Unavailable tools: {}. Engine timers cannot wake you after Gathr closes the engine.",
        off().join(", ")
    );
    if facts.room {
        if facts.team && !facts.others.is_empty() {
            lines.push(ROOM_TEAM.to_string());
        }
        lines.extend([ROOM.to_string(), unavailable]);
        return lines.join("\n");
    }
    lines.extend([
        // Claude Code defers MCP tools behind ToolSearch, so an unnamed tool is never found (0005).
        "Gathr's tools, on the gathr server (find them with ToolSearch if needed): read_chat for \
         earlier messages and other agents' replies; give_task, edit_task, finish_task, list_tasks, \
         cancel_task and restart_task for tasks; message to reach one member of a team chat now; react to give \
         the person's message or another agent's your thumbs-up, by its id; post to choose where your words this turn \
         show in a team chat; size, for the Supervisor, to record the person's ask as \
         repair or new; plan, for the Supervisor, to write the plan for a new ask; wait and \
         end_wait instead of polling in a loop; waiting for what this chat waits on and where each agent \
         stands; diag for Gathr's own record of what went wrong (health, failures, trace with a reference); \
         ask_person for anything you need from the person."
            .to_string(),
        "End a task turn with finish_task. Assign work with give_task and ask the person with ask_person. Writing it in chat does not create a task."
            .to_string(),
        "To reach a member of a team chat, write @Name in your words: they get them when your turn ends. message \
         reaches one now, while you work. Work to be done and heard back on is a task."
            .to_string(),
        "When something in Gathr gets in your way (a tool that refuses wrongly, a lost message, a wrong status), say so \
         with report_friction, once for each thing."
            .to_string(),
        unavailable,
    ]);
    lines.join("\n")
}
