//! What the person's assistant proposes in its own chat, each a block (0078's `block` messages) the person acts on from
//! its card, once. A project (`propose_project`), its body `{kind: "project", project: {name, folder, team, chat, brief,
//! made?}}`: the person makes the project, its first chat and the brief as that chat's first message, drawn as the
//! person's, which starts the lead's turn as their message would. An agent (`propose_agent`), its body `{kind: "agent",
//! agent: {name, description, account, model, shape, eyes, colour, added?}}`: the person adds it. The block then records
//! what was made or added. A change to an agent or a project the person has (`propose_agent_change`,
//! `propose_project_change`) is kept the same way, with what it was and what it becomes, and saved from its card.

use std::collections::HashSet;

use rusqlite::{Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Error;
use crate::agents::{self, Agent, Approvals, Refused};
use crate::ids::{now_us, trace_id, ulid};
use crate::store::{self, Start};
use crate::team::{self, Role};
use crate::words::Opened;

/// The most a brief may be.
pub const BRIEF_MOST: usize = 4_000;

/// What the assistant proposed; once made, what was made.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Proposal {
    pub name: String,
    pub folder: String,
    /// The lead first.
    pub team: Vec<Seat>,
    /// The first chat's name.
    pub chat: String,
    pub brief: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub made: Option<Made>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seat {
    pub agent: String,
    #[serde(default)]
    pub lead: bool,
}

/// The project made from it, its first chat and the brief there.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Made {
    pub project: String,
    pub chat: String,
    pub message: String,
}

impl Proposal {
    /// Its team as roles: the lead leads.
    pub fn seats(&self) -> Vec<(String, Role)> {
        let role = |lead| if lead { Role::Supervisor } else { Role::Member };
        self.team
            .iter()
            .map(|s| (s.agent.clone(), role(s.lead)))
            .collect()
    }

    /// The block's body as kept.
    pub fn body(&self) -> String {
        json!({ "kind": "project", "project": self }).to_string()
    }
}

/// The project proposed in `message`, if it is one.
pub fn find(conn: &Connection, message: &str) -> Result<Option<Proposal>, Error> {
    proposed(conn, message, "project")
}

/// The proposal of `kind` kept in `message`, if it is one.
fn proposed<T: DeserializeOwned>(
    conn: &Connection,
    message: &str,
    kind: &str,
) -> Result<Option<T>, Error> {
    let sql = "SELECT body FROM messages WHERE id = ?1 AND kind = 'block'";
    let body: Option<String> = conn.query_row(sql, [message], |r| r.get(0)).optional()?;
    let block: Option<Value> = body.and_then(|b| serde_json::from_str(&b).ok());
    Ok(block
        .filter(|b| b["kind"] == kind)
        .and_then(|b| serde_json::from_value(b[kind].clone()).ok()))
}

/// The chat a proposal's message is in.
fn proposed_in(conn: &Connection, message: &str) -> Result<String, Error> {
    let sql = "SELECT chat_id FROM messages WHERE id = ?1";
    Ok(conn.query_row(sql, [message], |r| r.get(0))?)
}

/// What making it did: the project's id, its chat, the proposal's block as it is now and the chat it is in, and the
/// lead's turn.
pub struct Making {
    pub project: String,
    pub chat: Value,
    pub block: Value,
    pub proposed_in: String,
    pub start: Start,
}

/// Makes the project proposed in `message` with `name`, `folder` and `team` (checked already), in one transaction:
/// the project, its first chat with that team, and the brief as the person's message there, its lead's turn queued.
/// Refused for a proposal that isn't there or was made already.
pub fn make(
    conn: &mut Connection,
    message: &str,
    (name, folder): (&str, &str),
    team: &[(String, Role)],
) -> Result<Making, Error> {
    let tx = conn.transaction()?;
    let mut p =
        find(&tx, message)?.ok_or_else(|| Error::Refused("There's no such proposal.".into()))?;
    if p.made.is_some() {
        return Err(Error::Refused("This project was made already.".into()));
    }
    let project = crate::projects::add(&tx, name, folder, team)?;
    let chat = store::create_chat(&tx, &p.chat, (Some(&project), Some(folder)))?;
    let id = chat["id"].as_str().unwrap_or_default().to_string();
    team::replace(&tx, &id, team)?;
    let lead = team
        .iter()
        .find(|(_, r)| *r == Role::Supervisor)
        .or(team.first());
    let lead = lead.map(|(a, _)| a.clone()).unwrap_or_default();
    let (turn, trace) = (ulid(now_us()), trace_id());
    store::add_turn(&tx, &id, &turn, &trace, (Some(&lead), None), Opened::Brief)?;
    let saved = store::add_person_message(&tx, &id, &turn, &p.brief, None)?;
    let brief = saved["id"].as_str().unwrap_or_default().to_string();
    p.name = name.to_string();
    p.folder = folder.to_string();
    p.team = team
        .iter()
        .map(|(agent, role)| Seat {
            agent: agent.clone(),
            lead: *role == Role::Supervisor,
        })
        .collect();
    p.made = Some(Made {
        project: project.clone(),
        chat: id.clone(),
        message: brief.clone(),
    });
    store::set_message_body(&tx, message, &p.body())?;
    let start = Start {
        chat: id,
        agent: lead,
        answers: None,
        text: crate::replies::read_as(&tx, &brief, &p.brief)?,
        turn,
        trace,
        message: store::message(&tx, &brief)?,
    };
    let block = store::message(&tx, message)?;
    let proposed_in = proposed_in(&tx, message)?;
    tx.commit()?;
    Ok(Making {
        project,
        chat,
        block,
        proposed_in,
        start,
    })
}

/// Where a brief's message is: the proposal's block whose made message it is. Only a block that holds the id is read
/// as JSON, and only a project's counts: a table an agent showed can hold any words, a message's id too.
pub const BRIEF_OF: &str = "b.kind = 'block' AND instr(b.body, m.id) > 0
     AND CASE WHEN json_valid(b.body) THEN json_extract(b.body, '$.kind') = 'project'
              AND json_extract(b.body, '$.project.made.message') = m.id END";

/// Gathr's line before a brief's words, for the lead: who wrote it, and that the person agreed to it.
pub fn line(conn: &Connection, message: &str) -> Result<Option<String>, Error> {
    let sql = format!(
        "SELECT a.name FROM messages m JOIN turns t ON t.id = m.turn_id AND t.opened_by = 'brief'
         JOIN messages b ON {BRIEF_OF} LEFT JOIN turns bt ON bt.id = b.turn_id LEFT JOIN agents a ON a.id = bt.agent_id
         WHERE m.id = ?1 AND m.role = 'user'"
    );
    let found: Option<Option<String>> = conn
        .prepare_cached(&sql)?
        .query_row([message], |r| r.get(0))
        .optional()?;
    Ok(found.map(|name| {
        let who = match name {
            Some(name) => format!("{name}, the person's assistant,"),
            None => "The person's assistant".into(),
        };
        format!(
            "{who} wrote this brief for the person, and the person agreed to it when they made this project."
        )
    }))
}

/// An agent the assistant proposed; once added, the agent as it was added.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAgent {
    pub name: String,
    pub description: String,
    /// The account's id.
    pub account: String,
    /// `None` is the engine's own default.
    #[serde(default)]
    pub model: Option<String>,
    /// Its face, picked as it was proposed, so the card shows the face it is added with.
    pub shape: String,
    pub eyes: String,
    pub colour: String,
    /// The agent's id, once added.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub added: Option<String>,
}

impl NewAgent {
    /// The block's body as kept.
    pub fn body(&self) -> String {
        json!({ "kind": "agent", "agent": self }).to_string()
    }

    /// The agent to add: what was proposed, and for the rest what a new agent gets.
    pub fn agent(&self) -> Agent {
        Agent {
            id: String::new(),
            name: self.name.clone(),
            account: self.account.clone(),
            model: self.model.clone(),
            effort: None,
            approvals: Approvals::default(),
            shape: Some(self.shape.clone()),
            eyes: Some(self.eyes.clone()),
            colour: Some(self.colour.clone()),
            description: Some(self.description.clone()),
        }
    }
}

/// The agent proposed in `message`, if it is one.
pub fn find_agent(conn: &Connection, message: &str) -> Result<Option<NewAgent>, Error> {
    proposed(conn, message, "agent")
}

/// What adding it did: the agent, the proposal's block as it is now, and the chat it is in.
pub struct Adding {
    pub agent: Agent,
    pub block: Value,
    pub proposed_in: String,
}

/// Adds `agent` (its model and effort checked already) for the proposal in `message`, as `agents::add` adds one, and
/// marks the proposal added with it, in one transaction. Refused for a proposal that isn't there or was added already.
pub fn add_agent(
    conn: &mut Connection,
    message: &str,
    agent: &Agent,
    removing: &HashSet<String>,
) -> Result<Result<Adding, Refused>, Error> {
    let tx = conn.transaction()?;
    let mut p = find_agent(&tx, message)?
        .ok_or_else(|| Error::Refused("There's no such proposal.".into()))?;
    if p.added.is_some() {
        return Err(Error::Refused("This agent was added already.".into()));
    }
    let agent = match agents::add(&tx, agent, removing)? {
        Ok(agent) => agent,
        Err(refused) => return Ok(Err(refused)),
    };
    p = NewAgent {
        name: agent.name.clone(),
        description: agent.description.clone().unwrap_or_default(),
        account: agent.account.clone(),
        model: agent.model.clone(),
        shape: agent.shape.clone().unwrap_or(p.shape),
        eyes: agent.eyes.clone().unwrap_or(p.eyes),
        colour: agent.colour.clone().unwrap_or(p.colour),
        added: Some(agent.id.clone()),
    };
    store::set_message_body(&tx, message, &p.body())?;
    let block = store::message(&tx, message)?;
    let proposed_in = proposed_in(&tx, message)?;
    tx.commit()?;
    Ok(Ok(Adding {
        agent,
        block,
        proposed_in,
    }))
}

/// A change to one of the person's agents the assistant proposed (`propose_agent_change`), its body
/// `{kind: "agent_change", agent_change: {agent, was, to, saved?}}`: the person saves it from its card, once. `was` is
/// the agent when it was proposed, so the card shows what changes; saving sets only what differs from it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentChange {
    /// The agent's id.
    pub agent: String,
    pub was: AgentSet,
    pub to: AgentSet,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub saved: bool,
}

/// What `propose_agent` sets on an agent: what `propose_agent_change` may change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSet {
    pub name: String,
    pub description: String,
    /// The account's id.
    pub account: String,
    /// `None` is the engine's own default.
    #[serde(default)]
    pub model: Option<String>,
}

impl AgentSet {
    pub fn of(agent: &Agent) -> AgentSet {
        AgentSet {
            name: agent.name.clone(),
            description: agent.description.clone().unwrap_or_default(),
            account: agent.account.clone(),
            model: agent.model.clone(),
        }
    }
}

impl AgentChange {
    /// The block's body as kept.
    pub fn body(&self) -> String {
        json!({ "kind": "agent_change", "agent_change": self }).to_string()
    }

    /// The change to save: what differs from `was`. A new account always sets the model, since the old one came
    /// from the other engine.
    pub fn change(&self) -> agents::Change {
        let (was, to) = (&self.was, &self.to);
        let differs = |a: &String, b: &String| (a != b).then(|| b.clone());
        let account = differs(&was.account, &to.account);
        agents::Change {
            name: differs(&was.name, &to.name),
            description: differs(&was.description, &to.description).map(Some),
            model: (account.is_some() || was.model != to.model).then(|| to.model.clone()),
            account,
            ..Default::default()
        }
    }
}

/// A team as roles, each agent by id.
pub type Team = Vec<(String, Role)>;

/// A change to one of the person's projects the assistant proposed (`propose_project_change`), its body
/// `{kind: "project_change", project_change: {project, was, to, saved?}}`: the person saves it from its card, once.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectChange {
    /// The project's id.
    pub project: String,
    pub was: ProjectSet,
    pub to: ProjectSet,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub saved: bool,
}

/// What a project is: its name, folder and team, the lead first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectSet {
    pub name: String,
    pub folder: String,
    pub team: Vec<Seat>,
}

impl ProjectSet {
    pub fn of(project: &crate::projects::Project) -> ProjectSet {
        let mut team: Vec<Seat> = (project.members.iter())
            .map(|m| Seat {
                agent: m.agent.clone(),
                lead: m.role == Role::Supervisor,
            })
            .collect();
        team.sort_by_key(|s| !s.lead);
        ProjectSet {
            name: project.name.clone(),
            folder: project.folder.clone(),
            team,
        }
    }
}

impl ProjectChange {
    /// The block's body as kept.
    pub fn body(&self) -> String {
        json!({ "kind": "project_change", "project_change": self }).to_string()
    }

    /// What differs from `was`: a name, a folder, a team as roles.
    pub fn change(&self) -> (Option<String>, Option<String>, Option<Team>) {
        let (was, to) = (&self.was, &self.to);
        let role = |lead| if lead { Role::Supervisor } else { Role::Member };
        let team = (was.team != to.team).then(|| {
            let seats = to.team.iter();
            seats.map(|s| (s.agent.clone(), role(s.lead))).collect()
        });
        (
            (was.name != to.name).then(|| to.name.clone()),
            (was.folder != to.folder).then(|| to.folder.clone()),
            team,
        )
    }
}

/// The agent change proposed in `message`, if it is one.
pub fn find_agent_change(conn: &Connection, message: &str) -> Result<Option<AgentChange>, Error> {
    proposed(conn, message, "agent_change")
}

/// The project change proposed in `message`, if it is one.
pub fn find_project_change(
    conn: &Connection,
    message: &str,
) -> Result<Option<ProjectChange>, Error> {
    proposed(conn, message, "project_change")
}

/// Keeps `body` as the proposal's block once its change is saved: the block as it is now, and the chat it is in.
pub fn mark_saved(conn: &Connection, message: &str, body: &str) -> Result<(Value, String), Error> {
    store::set_message_body(conn, message, body)?;
    Ok((store::message(conn, message)?, proposed_in(conn, message)?))
}
