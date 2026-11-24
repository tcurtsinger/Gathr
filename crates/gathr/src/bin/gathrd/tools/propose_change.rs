//! `propose_agent_change` and `propose_project_change`: the person's assistant, in its own chat, proposes a change to
//! one of the person's agents or projects, covering what `propose_agent` and `propose_project` set. Nothing changes:
//! the proposal is a block of its turn (`gathr::proposals`), drawn as a card with what changes, which the person saves.
//! What would stop that is refused here.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::agents::{Agent, Refused};
use gathr::proposals::{AgentChange, AgentSet, ProjectChange, ProjectSet};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use rusqlite::Connection;
use serde_json::{Value, json};

use super::propose::{assistants_own, listed_model, team};
use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ProposeAgentChange {
    /// The agent to change, by name.
    agent: String,
    /// A new name.
    name: Option<String>,
    /// What it is for: one line, at most 80 characters.
    description: Option<String>,
    /// Another of the person's accounts, by name. Its model is the engine's default unless model is given.
    account: Option<String>,
    /// A model its account lists.
    model: Option<String>,
    /// Its face's colour: red, orange, amber, lime, green, teal, sky, blue, violet, plum, pink or grey.
    colour: Option<String>,
    /// Its face's shape: circle, tile, cloud, flower, hexagon, pebble, shield, drop, star, diamond, bubble or leaf.
    shape: Option<String>,
    /// Its face's eyes: lines, glance, side, squint, dots, rings, ovals, happy, rest, wink, calm or quirk.
    eyes: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ProposeProjectChange {
    /// The project to change, by name.
    project: String,
    /// A new name.
    name: Option<String>,
    /// A new folder, a full path in a git repository.
    folder: Option<String>,
    /// The whole new team: 1 to 5 of the person's agents, by name.
    team: Option<Vec<String>>,
    /// Who leads: one of the team.
    lead: Option<String>,
}

#[tool_router(router = propose_change_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "In your own chat as the person's assistant: propose a change to one of the person's agents: \
                       its name, what it is for, its account or model, its face's colour, shape or eyes. Give only what changes; for \
                       several agents, one call each. The person saves it \
                       from the card shown; nothing changes before."
    )]
    async fn propose_agent_change(
        &self,
        Parameters(ask): Parameters<ProposeAgentChange>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let said = match caller.on() {
            None => Err("you have no turn running in this chat".into()),
            Some(on) => {
                let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
                let asked = ask.model.clone();
                let drafted = self
                    .chats
                    .writer
                    .call(move |c| agent_change(c, (&chat, agent.as_deref()), ask))
                    .await
                    .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
                    .map_err(|e| e.to_string());
                match drafted {
                    Err(why) => Err(why),
                    Ok(mut change) => match self.model_of(&on.trace, &mut change, asked).await {
                        Err(why) => Err(why),
                        Ok(()) => self.keep(&caller.chat, &on.turn, change.body()).await,
                    },
                }
            }
        };
        let said = said.map(|message| {
            let live = json!({ "type": "message", "chat": caller.chat, "message": message });
            self.chats.publish(live);
            "The card is shown: the person saves the change with a click.".to_string()
        });
        Ok(self
            .respond(&caller, "propose_agent_change", started, said)
            .await)
    }

    #[tool(
        description = "In your own chat as the person's assistant: propose a change to one of the person's projects: \
                       its name, its folder, its team or who leads. Give only what changes. The person saves it from \
                       the card shown; nothing changes before. Chats already made keep their team."
    )]
    async fn propose_project_change(
        &self,
        Parameters(ask): Parameters<ProposeProjectChange>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let folder = match ask.folder.as_deref().map(str::trim) {
            None => Ok(None),
            Some(folder) => crate::team::top_level(std::path::Path::new(folder))
                .await
                .map(Some)
                .map_err(|why| format!("{why}: give the full path of a git folder.")),
        };
        let said = match (folder, caller.on()) {
            (Err(why), _) => Err(why),
            (_, None) => Err("you have no turn running in this chat".into()),
            (Ok(folder), Some(on)) => {
                let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
                let drafted = self
                    .chats
                    .writer
                    .call(move |c| project_change(c, (&chat, agent.as_deref()), ask, folder))
                    .await
                    .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
                    .map_err(|e| e.to_string());
                match drafted {
                    Err(why) => Err(why),
                    Ok(change) => self.keep(&caller.chat, &on.turn, change.body()).await,
                }
            }
        };
        let said = said.map(|message| {
            let live = json!({ "type": "message", "chat": caller.chat, "message": message });
            self.chats.publish(live);
            "The card is shown: the person saves the change with a click.".to_string()
        });
        Ok(self
            .respond(&caller, "propose_project_change", started, said)
            .await)
    }
}

impl Tools {
    /// Sets the model asked for, as the change's account lists it; or says which it lists.
    async fn model_of(
        &self,
        trace: &str,
        change: &mut AgentChange,
        asked: Option<String>,
    ) -> Result<(), String> {
        let Some(model) = asked else {
            return Ok(());
        };
        let id = change.to.account.clone();
        let account = self
            .chats
            .writer
            .call(move |c| gathr::accounts::get(c, &id))
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string())?
            .ok_or("The account is gone.")?;
        let listed = crate::models::list(&self.state, trace, &account)
            .await
            .map_err(|why| format!("The models of {} weren't listed: {why}", account.name))?;
        change.to.model = Some(listed_model(&listed, &model, &account)?);
        if change.to == change.was {
            return Err(nothing());
        }
        Ok(())
    }

    /// Keeps `body` as a block of the caller's turn: the message.
    async fn keep(&self, chat: &str, turn: &str, body: String) -> Result<Value, String> {
        let (chat, turn) = (chat.to_string(), turn.to_string());
        self.chats
            .writer
            .call(move |c| gathr::store::add_message(c, &chat, &turn, "assistant", "block", &body))
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string())
    }
}

fn nothing() -> String {
    "Nothing would change: give only what changes, different from what it is.".into()
}

/// The agent change as proposed, the model still the one it has unless the account changes; or what would stop it
/// being saved, said so the assistant can fix it.
fn agent_change(
    c: &Connection,
    caller: (&str, Option<&str>),
    ask: ProposeAgentChange,
) -> Result<AgentChange, Error> {
    let refuse = |why: String| Err(Error::Refused(why));
    assistants_own(c, caller, "a change to an agent")?;
    let agents = gathr::agents::list(c)?;
    let wanted = ask.agent.trim().trim_start_matches('@').to_lowercase();
    let Some(agent) = agents.iter().find(|a| a.name.to_lowercase() == wanted) else {
        let all: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        return refuse(format!(
            "The person has no agent called {}: their agents are {}.",
            ask.agent.trim(),
            all.join(", ")
        ));
    };
    let was = AgentSet::of(agent);
    let mut to = was.clone();
    if let Some(name) = &ask.name {
        to.name = name.trim().to_string();
        let key = to.name.to_lowercase();
        if agents
            .iter()
            .any(|a| a.id != agent.id && a.name.to_lowercase() == key)
        {
            return refuse(format!(
                "An agent is called {} already: give another name.",
                to.name
            ));
        }
    }
    if let Some(description) = &ask.description {
        to.description = description.trim().to_string();
        if to.description.is_empty() {
            return refuse("Say what the agent is for in its description.".into());
        }
    }
    if let Some(name) = &ask.account {
        let accounts = gathr::accounts::list(c)?;
        let key = name.trim().to_lowercase();
        let Some(account) = accounts.iter().find(|a| a.name.to_lowercase() == key) else {
            let all: Vec<&str> = accounts.iter().map(|a| a.name.as_str()).collect();
            return refuse(format!(
                "The person has no account called {}: their accounts are {}.",
                name.trim(),
                all.join(", ")
            ));
        };
        if account.id != was.account {
            to.account = account.id.clone();
            to.model = None;
        }
    }
    let pick = |asked: &Option<String>, now: &mut Option<String>| {
        if let Some(picked) = asked {
            *now = Some(picked.trim().to_lowercase());
        }
    };
    pick(&ask.colour, &mut to.colour);
    pick(&ask.shape, &mut to.shape);
    pick(&ask.eyes, &mut to.eyes);
    if ask.model.is_none() && to == was {
        return refuse(nothing());
    }
    let next: Agent = agent.changed(
        AgentChange {
            agent: agent.id.clone(),
            was: was.clone(),
            to: to.clone(),
            saved: false,
        }
        .change(),
    );
    let why = match gathr::agents::would_refuse(c, &next)? {
        None | Some(Refused::Taken) => {
            return Ok(AgentChange {
                agent: agent.id.clone(),
                was,
                to,
                saved: false,
            });
        }
        Some(Refused::Name(_)) => "The name must be 1 to 80 characters.".into(),
        Some(Refused::Description) => {
            "The description must be one line of at most 80 characters.".into()
        }
        Some(Refused::Picture) => {
            "Give a colour, shape and eyes from their lists in this tool's description.".into()
        }
        Some(other) => format!("{}.", other.why()),
    };
    refuse(why)
}

/// The project change as proposed, or what would stop it being saved, said so the assistant can fix it.
fn project_change(
    c: &Connection,
    caller: (&str, Option<&str>),
    ask: ProposeProjectChange,
    folder: Option<String>,
) -> Result<ProjectChange, Error> {
    let refuse = |why: String| Err(Error::Refused(why));
    assistants_own(c, caller, "a change to a project")?;
    let projects = gathr::projects::list(c)?;
    let wanted = ask.project.trim().to_lowercase();
    let Some(project) = projects.iter().find(|p| p.name.to_lowercase() == wanted) else {
        let all: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
        return refuse(format!(
            "The person has no project called {}: their projects are {}.",
            ask.project.trim(),
            all.join(", ")
        ));
    };
    let was = ProjectSet::of(project);
    let mut to = was.clone();
    if let Some(name) = &ask.name {
        let name = gathr::projects::name(name).map_err(|why| Error::Refused(why.into()))?;
        let key = name.to_lowercase();
        if (projects.iter()).any(|p| p.id != project.id && p.name.to_lowercase() == key) {
            return refuse(format!(
                "A project is called {name} already: give another name."
            ));
        }
        to.name = name.to_string();
    }
    if let Some(folder) = folder {
        to.folder = folder;
    }
    if ask.team.is_some() || ask.lead.is_some() {
        let name_of = |agent: &str| {
            let found = project.members.iter().find(|m| m.agent == agent);
            found.map(|m| m.name.clone()).unwrap_or_default()
        };
        let names = (ask.team.clone())
            .unwrap_or_else(|| was.team.iter().map(|s| name_of(&s.agent)).collect());
        let lead = match (&ask.lead, was.team.iter().find(|s| s.lead)) {
            (Some(lead), _) => lead.clone(),
            (None, Some(seat)) => name_of(&seat.agent),
            (None, None) if names.len() == 1 => names[0].clone(),
            (None, None) => return refuse("Say who leads, one of the team.".into()),
        };
        to.team = team(c, &names, &lead)?;
    }
    if to == was {
        return refuse(nothing());
    }
    Ok(ProjectChange {
        project: project.id.clone(),
        was,
        to,
        saved: false,
    })
}
