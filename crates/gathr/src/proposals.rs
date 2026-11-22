//! A project the person's assistant proposes with `propose_project`, in its own chat: a block of kind `project`
//! (0078's `block` messages), its body `{kind, project: {name, folder, team, chat, brief, made?}}`. The person makes it
//! from the card, once: the project, its first chat and the brief as that chat's first message, drawn as the person's,
//! which starts the lead's turn as their message would. The block then records what was made.

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::Error;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// The proposal kept in `message`, if it is one.
pub fn find(conn: &Connection, message: &str) -> Result<Option<Proposal>, Error> {
    let sql = "SELECT body FROM messages WHERE id = ?1 AND kind = 'block'";
    let body: Option<String> = conn.query_row(sql, [message], |r| r.get(0)).optional()?;
    let block: Option<Value> = body.and_then(|b| serde_json::from_str(&b).ok());
    Ok(block
        .filter(|b| b["kind"] == "project")
        .and_then(|b| serde_json::from_value(b["project"].clone()).ok()))
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
    let sql = "SELECT chat_id FROM messages WHERE id = ?1";
    let proposed_in = tx.query_row(sql, [message], |r| r.get(0))?;
    tx.commit()?;
    Ok(Making {
        project,
        chat,
        block,
        proposed_in,
        start,
    })
}

/// Where a brief's message is: the proposal's block whose made message it is. Text, so no other message's body is
/// read as JSON.
pub const BRIEF_OF: &str =
    "b.kind = 'block' AND instr(b.body, '\"message\":\"' || m.id || '\"') > 0";

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
