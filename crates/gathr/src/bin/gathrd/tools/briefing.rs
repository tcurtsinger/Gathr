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
    role: Option<Role>,
    others: Vec<String>,
    team: bool,
    /// The agent's folder and the project it's a worktree of.
    worktree: Option<(PathBuf, String)>,
    /// The agent's own folder and its project's Shared (0034).
    own: Option<(PathBuf, PathBuf)>,
    /// In a project chat, the project's folder in the agent folder and the project's own.
    read: Vec<PathBuf>,
    /// What the person asked to be called.
    person: Option<String>,
    /// Whether this agent is the person's assistant.
    assistant: bool,
    /// In a chat with no project, the person's other agents, each with its description.
    agents: Vec<String>,
    /// The Skills folder and its enabled skills that can be read, by name and description.
    skills: Option<(PathBuf, Vec<(String, String)>)>,
}

/// The agent's worktree, its own folder and Shared, what it may read, and the Skills folder.
type Folders = (
    Option<(PathBuf, String)>,
    Option<(PathBuf, PathBuf)>,
    Vec<PathBuf>,
    Option<PathBuf>,
);

pub fn facts(
    conn: &Connection,
    chat: &str,
    agent: Option<&str>,
    (worktree, own, read, skills): Folders,
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
    let plain: bool = conn
        .query_row(
            "SELECT project IS NULL FROM chats WHERE id = ?1",
            [chat],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(false);
    let agents = match plain {
        true => gathr::agents::list(conn)?
            .into_iter()
            .filter(|a| Some(a.id.as_str()) != agent)
            .map(|a| match a.description {
                Some(what) => format!("{} ({what})", a.name),
                None => a.name,
            })
            .collect(),
        false => Vec::new(),
    };
    let assistant = gathr::assistant::assistant(conn)?;
    let off = gathr::skills::off(conn)?;
    let skills = skills.map(|dir| {
        let listed = gathr::skills::list(&dir, &off).into_iter();
        let usable = listed
            .filter(|s| s.enabled)
            .filter_map(|s| Some((s.name, s.description?)));
        (dir, usable.collect::<Vec<_>>())
    });
    Ok(Facts {
        title,
        name,
        role: role.filter(|_| members.len() > 1),
        others,
        team: !members.is_empty(),
        worktree,
        own,
        read,
        person: gathr::assistant::person_name(conn)?,
        assistant: plain && agent.is_some() && assistant.as_deref() == agent,
        agents,
        skills: skills.filter(|(_, listed)| !listed.is_empty()),
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
        Role::Supervisor => Some("lead"),
        Role::Member => None,
    }
}

/// The tools turned off for every agent, by name: what the briefing says is off.
pub fn off() -> Vec<&'static str> {
    DENIED_TOOLS.iter().map(|(name, _)| *name).collect()
}

/// A description is cut to this many characters, and past LISTED the skills are named only, so
/// many skills never crowd the briefing.
const DESCRIBED: usize = 200;
const LISTED: usize = 6_000;

/// Where the skills are, how to use one, and each one with its description.
fn skills(dir: &std::path::Path, listed: &[(String, String)]) -> String {
    let described: Vec<String> = listed
        .iter()
        .map(|(name, what)| match what.char_indices().nth(DESCRIBED) {
            Some((cut, _)) => format!("- {name}: {}…", &what[..cut]),
            None => format!("- {name}: {what}"),
        })
        .collect();
    let list = match described.iter().map(|l| l.len() + 1).sum::<usize>() > LISTED {
        true => {
            let names: Vec<&str> = listed.iter().map(|(name, _)| name.as_str()).collect();
            format!(" {}.", names.join(", "))
        }
        false => format!("\n{}", described.join("\n")),
    };
    format!(
        "Skills are in \"{}\", each a folder with a SKILL.md. Before a task a skill fits, read its \
         SKILL.md and follow it. The skills:{list}",
        dir.display()
    )
}

/// How a room chat works, for an agent with teammates there.
const ROOM_TEAM: &str = "How this room works:
- You get a turn when the person writes to you, or when a teammate names you with @Name. Write @Name to wake \
a teammate: they get your words when your turn ends. Name only who needs to act.
- When your words comment on a teammate's message, start them with REPLY on a line of its own, and they are \
filed under that message. Never start your answer to the person with REPLY.
- One of you is the lead. A message from the person that names nobody goes to it, and its plan or answer is \
checked by the teammates who took no part, before the person relies on it.";

/// What every agent in a room chat is told: the owner's own asks, the board and the tools.
const ROOM: &str = "- Only your last words in a turn are posted as your message; what you write before them stays \
in your steps. So @Name, REPLY and anything a teammate must read go in your last words.
- If you have nothing to add, write only NO_REPLY: nothing is posted. To agree, react with a \
thumbs-up, then write only NO_REPLY. Never write a message to say that you reacted or that you agree.
- A thumbs-up wakes nobody. When a teammate is waiting on your answer or your result, write it to them with @Name.
- Keep your line on the board current with set_board_line: what you work on, and its state.
- Keep your messages brief. Be friendly and professional.
- Don't push fixes for edge cases nobody has hit, and don't change things over wording or semantics.
- A reply, a rebuttal or a reported defect needs a strong argument: say what is wrong and show why.
- When you need the person to decide, allow or answer something, ask with ask_person, never in a message: a \
question in a message doesn't reach them as a question. Ask only what is theirs to decide.
- A file the person attaches is named in their message by a line \"Attached: <full path>\": open it from there.
- To show the person a picture, save it in your working folder or Shared and write it in your last words as \
![what it shows](<full path>), with the angle brackets.
Gathr's tools, on the gathr server: react, set_board_line, ask_person, read_chat for \
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
    if let Some(person) = &facts.person {
        lines.push(format!(
            "The person's name is {person}: call them {person}."
        ));
    }
    if facts.assistant {
        lines.push(
            "You are the person's assistant: a message that names nobody comes to you. Do small \
             things yourself."
                .to_string(),
        );
    }
    if !facts.agents.is_empty() {
        lines.push(format!(
            "The person's other agents: {}. Naming one with @Name in your last words brings them \
             into this chat to answer the person directly, so hand an ask to the agent it belongs \
             to instead of relaying it.",
            facts.agents.join(", ")
        ));
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
    if let [team, checkout] = &facts.read[..] {
        lines.push(format!(
            "You can read the whole project: your teammates' folders and worktrees in \"{}\", and \
             the person's checkout \"{}\". Write only in your worktree, your own folder and Shared.",
            team.display(),
            checkout.display()
        ));
    }
    if let Some((dir, listed)) = &facts.skills {
        lines.push(skills(dir, listed));
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
    if facts.team && !facts.others.is_empty() {
        lines.push(ROOM_TEAM.to_string());
    }
    lines.extend([ROOM.to_string(), unavailable]);
    lines.join("\n")
}
