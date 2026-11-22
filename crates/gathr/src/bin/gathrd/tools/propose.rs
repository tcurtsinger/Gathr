//! `propose_project` and `propose_agent`: the person's assistant, in its own chat, proposes a project with its team and
//! the brief its lead starts from, or an agent. Nothing is made: the proposal is a block of its turn
//! (`gathr::proposals`), drawn as a card the person makes the project or adds the agent from. What would stop that is
//! refused here.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::accounts::Account;
use gathr::agents::Refused;
use gathr::proposals::{BRIEF_MOST, NewAgent, Proposal, Seat};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::{Value, json};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ProposeProject {
    name: String,
    /// The project's folder, a full path in a git repository.
    folder: String,
    /// 1 to 5 of the person's agents, by name.
    team: Vec<String>,
    /// The one of them who leads.
    lead: String,
    /// The first chat's name: 2 to 5 words.
    chat: String,
    /// What the lead starts from. At most 4,000 characters.
    brief: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ProposeAgent {
    name: String,
    /// What it is for: one line, at most 80 characters.
    description: String,
    /// One of the person's accounts, by name. Yours if not given.
    account: Option<String>,
    /// A model that account lists. Yours if not given.
    model: Option<String>,
}

#[tool_router(router = propose_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "In your own chat as the person's assistant: propose a project, its team and the brief its lead \
                       starts from. The person makes it from the card shown; nothing is made before."
    )]
    async fn propose_project(
        &self,
        Parameters(ask): Parameters<ProposeProject>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let folder = crate::team::top_level(std::path::Path::new(ask.folder.trim())).await;
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        let proposed = match (folder, caller.on()) {
            (Err(why), _) => Err(format!("{why}: give the full path of a git folder.")),
            (_, None) => Err("you have no turn running in this chat".into()),
            (Ok(folder), Some(on)) => self
                .chats
                .writer
                .call(move |c| {
                    let proposal = proposal(c, (&chat, agent.as_deref()), ask, folder)?;
                    gathr::store::add_message(
                        c,
                        &chat,
                        &on.turn,
                        "assistant",
                        "block",
                        &proposal.body(),
                    )
                })
                .await
                .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
                .map_err(|e| e.to_string()),
        };
        let said = proposed.map(|message| {
            let live = json!({ "type": "message", "chat": caller.chat, "message": message });
            self.chats.publish(live);
            "The card is shown: the person makes the project with a click.".to_string()
        });
        Ok(self
            .respond(&caller, "propose_project", started, said)
            .await)
    }

    #[tool(
        description = "In your own chat as the person's assistant: propose a new agent, what it is for, and the \
                       account and model it runs on. The person adds it from the card shown; nothing is added before."
    )]
    async fn propose_agent(
        &self,
        Parameters(ask): Parameters<ProposeAgent>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let said = match caller.on() {
            None => Err("you have no turn running in this chat".into()),
            Some(on) => {
                self.agent_proposed((&caller.chat, caller.agent.clone()), &on, ask)
                    .await
            }
        };
        let said = said.map(|message| {
            let live = json!({ "type": "message", "chat": caller.chat, "message": message });
            self.chats.publish(live);
            "The card is shown: the person adds the agent with a click.".to_string()
        });
        Ok(self.respond(&caller, "propose_agent", started, said).await)
    }
}

impl Tools {
    /// The agent proposed and kept as a block of the caller's turn: the message, or what would stop it being added.
    async fn agent_proposed(
        &self,
        (chat, caller): (&str, Option<String>),
        on: &super::OnTurn,
        ask: ProposeAgent,
    ) -> Result<Value, String> {
        let writer = &self.chats.writer;
        let id = chat.to_string();
        let asked = ask.model.clone();
        let drafted = writer
            .call(move |c| new_agent(c, (&id, caller.as_deref()), ask))
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())));
        let (mut proposal, account) = drafted.map_err(|e| e.to_string())?;
        if let Some(model) = asked {
            let listed = crate::models::list(&self.state, &on.trace, &account)
                .await
                .map_err(|why| format!("The models of {} weren't listed: {why}", account.name))?;
            proposal.model = Some(listed_model(&listed, &model, &account)?);
        }
        let (chat, turn) = (chat.to_string(), on.turn.clone());
        writer
            .call(move |c| {
                gathr::store::add_message(c, &chat, &turn, "assistant", "block", &proposal.body())
            })
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string())
    }
}

/// Refused unless the caller is the person's assistant in its own chat.
fn assistants_own(
    c: &rusqlite::Connection,
    (chat, caller): (&str, Option<&str>),
    what: &str,
) -> Result<(), Error> {
    if !gathr::assistant::is_chat(c, chat)?
        || caller.is_none()
        || gathr::assistant::assistant(c)?.as_deref() != caller
    {
        return Err(Error::Refused(format!(
            "only the person's assistant proposes {what}, in its own chat"
        )));
    }
    Ok(())
}

/// The agent as proposed, on the account asked for or the caller's, with the caller's model there and a face picked
/// as a new agent's is; or what would stop it being added, said so the assistant can fix it.
fn new_agent(
    c: &rusqlite::Connection,
    (chat, caller): (&str, Option<&str>),
    ask: ProposeAgent,
) -> Result<(NewAgent, Account), Error> {
    let refuse = |why: String| Err(Error::Refused(why));
    assistants_own(c, (chat, caller), "an agent")?;
    let me = gathr::agents::get(c, caller.unwrap_or_default())?
        .ok_or_else(|| Error::Refused("you are no longer an agent of the person's".into()))?;
    let accounts = gathr::accounts::list(c)?;
    let account = match &ask.account {
        None => accounts.iter().find(|a| a.id == me.account),
        Some(name) => {
            let name = name.trim().to_lowercase();
            accounts.iter().find(|a| a.name.to_lowercase() == name)
        }
    };
    let Some(account) = account.cloned() else {
        let all: Vec<&str> = accounts.iter().map(|a| a.name.as_str()).collect();
        return refuse(format!(
            "The person has no account called {}: their accounts are {}.",
            ask.account.unwrap_or_default().trim(),
            all.join(", ")
        ));
    };
    let description = ask.description.trim().to_string();
    if description.is_empty() {
        return refuse("Say what the agent is for in its description.".into());
    }
    let [shape, eyes, colour] = gathr::agents::shuffled();
    let proposal = NewAgent {
        name: ask.name.trim().to_string(),
        description,
        model: (account.id == me.account)
            .then(|| me.model.clone())
            .flatten(),
        account: account.id.clone(),
        shape,
        eyes,
        colour,
        added: None,
    };
    let why = match gathr::agents::would_refuse(c, &proposal.agent())? {
        None => return Ok((proposal, account)),
        Some(Refused::Taken) => format!(
            "An agent is called {} already: give another name.",
            proposal.name
        ),
        Some(Refused::Name(_)) => "The name must be 1 to 80 characters.".into(),
        Some(Refused::Description) => {
            "The description must be one line of at most 80 characters.".into()
        }
        Some(other) => format!("{}.", other.why()),
    };
    refuse(why)
}

/// The id of the model the account's engine lists by `model`'s id or name, ignoring case, an exact model before a
/// family's Latest of the same name; or which exact models it lists.
fn listed_model(listed: &Value, model: &str, account: &Account) -> Result<String, String> {
    let models = listed["models"].as_array().into_iter().flatten();
    // The exact models first, as the Agents screen offers them.
    let (exact, rest): (Vec<&Value>, Vec<&Value>) =
        models.partition(|m| m["id"] != "default" && m["latest"] != true);
    let wanted = model.trim().to_lowercase();
    let named = |m: &&Value| {
        [&m["id"], &m["name"]]
            .iter()
            .any(|v| v.as_str().is_some_and(|v| v.to_lowercase() == wanted))
    };
    let found = exact.iter().chain(&rest).copied().find(named);
    if let Some(id) = found.and_then(|m| m["id"].as_str()) {
        return Ok(id.to_string());
    }
    let names: Vec<&str> = exact.iter().filter_map(|m| m["name"].as_str()).collect();
    Err(format!(
        "The account {} has no model {}: it lists {}.",
        account.name,
        model.trim(),
        names.join(", ")
    ))
}

/// The proposal as kept, or what would stop the project being made, said so the assistant can fix it.
fn proposal(
    c: &rusqlite::Connection,
    (chat, caller): (&str, Option<&str>),
    ask: ProposeProject,
    folder: String,
) -> Result<Proposal, Error> {
    let refuse = |why: String| Err(Error::Refused(why));
    assistants_own(c, (chat, caller), "a project")?;
    let name = gathr::projects::name(&ask.name).map_err(|why| Error::Refused(why.into()))?;
    if gathr::projects::taken(c, name)? {
        return refuse(format!(
            "A project is called {name} already: give another name."
        ));
    }
    let chat_name = gathr::titles::checked(&ask.chat)
        .map_err(|e| Error::Refused(format!("chat: {e}.")))?
        .to_string();
    let brief = ask.brief.trim();
    let n = brief.chars().count();
    if n == 0 || n > BRIEF_MOST {
        return refuse(format!("The brief is {n} characters: give 1 to 4,000."));
    }
    if ask.team.is_empty() || ask.team.len() > gathr::team::MOST {
        return refuse(format!(
            "The team is {} agents: give 1 to 5.",
            ask.team.len()
        ));
    }
    let agents = gathr::agents::list(c)?;
    let by_name = |name: &str| {
        let name = name.trim().trim_start_matches('@').to_lowercase();
        agents.iter().find(|a| a.name.to_lowercase() == name)
    };
    let mut team: Vec<Seat> = Vec::new();
    for name in &ask.team {
        let Some(agent) = by_name(name) else {
            let all: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
            return refuse(format!(
                "The person has no agent called {name}: their agents are {}.",
                all.join(", ")
            ));
        };
        if team.iter().any(|s| s.agent == agent.id) {
            return refuse(format!("{} is in the team twice.", agent.name));
        }
        let lead = by_name(&ask.lead).is_some_and(|l| l.id == agent.id);
        team.push(Seat {
            agent: agent.id.clone(),
            lead,
        });
    }
    if !team.iter().any(|s| s.lead) {
        return refuse(format!("The lead, {}, isn't in the team.", ask.lead));
    }
    // The lead first, as a project lists its team.
    team.sort_by_key(|s| !s.lead);
    Ok(Proposal {
        name: name.to_string(),
        folder,
        team,
        chat: chat_name,
        brief: brief.to_string(),
        made: None,
    })
}
