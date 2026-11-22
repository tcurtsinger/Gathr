//! `propose_project`: the person's assistant, in its own chat, proposes a project with its team and the brief its lead
//! starts from. Nothing is made: the proposal is a block of its turn (`gathr::proposals`), drawn as a card the person
//! makes the project from. What would stop the project being made is refused here.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::proposals::{BRIEF_MOST, Proposal, Seat};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

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
}

/// The proposal as kept, or what would stop the project being made, said so the assistant can fix it.
fn proposal(
    c: &rusqlite::Connection,
    (chat, caller): (&str, Option<&str>),
    ask: ProposeProject,
    folder: String,
) -> Result<Proposal, Error> {
    let refuse = |why: String| Err(Error::Refused(why));
    if !gathr::assistant::is_chat(c, chat)?
        || caller.is_none()
        || gathr::assistant::assistant(c)?.as_deref() != caller
    {
        return refuse("only the person's assistant proposes a project, in its own chat".into());
    }
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
