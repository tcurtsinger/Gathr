//! Settings tools for agents (0043 C5a, owner's room task 90): an agent reads Gathr's settings and
//! changes them when the person asks, each change recorded with who made it. Never an agent's:
//! answering its own approval, answering its own question to the person, or changing which pushes
//! ask (an agent's Full approvals is what skips a push's ask).

use std::time::Instant;

use axum::http::request::Parts;
use gathr::agents::{Agent, Approvals, Change};
use gathr::event::Event;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::{Value, json};

use super::Caller;
use super::checks::NoArguments;
use super::server::{Tools, caller_of};
use crate::api::Trace;
use crate::routes::Answer;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SetSettings {
    /// Days without a move before a task is stale (1 to 365).
    stale_days: Option<i64>,
    /// Days stale before it's cancelled (1 to 365).
    cancel_days: Option<i64>,
    /// The agent folder: a full path, for new work.
    agent_folder: Option<String>,
    /// How many hand-offs a chain of agent messages may make; null for no limit.
    #[serde(default, deserialize_with = "crate::coord::present")]
    handoff_limit: Option<Option<i64>>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SetAgent {
    /// The agent, by name.
    agent: String,
    /// A new name.
    name: Option<String>,
    /// The account it runs on, by id.
    account: Option<String>,
    /// A model its account lists; "" for the engine's default.
    model: Option<String>,
    /// An effort its model lists; "" for the engine's default.
    effort: Option<String>,
    /// `ask`, `edits` or `auto`. Full is the person's to set.
    approvals: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AnswerApproval {
    /// The open approval's `request_id`, as the chat shows it.
    request_id: String,
    /// Whether it may go on.
    allow: bool,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AnswerPerson {
    /// The question's task number.
    task: i64,
    /// The labels picked, for a `choice` or `choices` question.
    pick: Option<Vec<String>>,
    /// For a `yesno` question.
    yes: Option<bool>,
    /// For a `done` question.
    done: Option<bool>,
    /// Words: they answer any form.
    text: Option<String>,
}

const OWN_APPROVAL: &str =
    "it's its own approval: an agent never answers its own approval, the person does";

#[tool_router(router = settings_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Gathr's settings: `general` (Settings > General), `agents` (each one's account, model, \
                       effort and approvals) and `projects` (folder, team, checks and merge, each with who set it)."
    )]
    async fn settings(
        &self,
        Parameters(_): Parameters<NoArguments>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let read = self.read_settings(&caller).await;
        Ok(self.respond(&caller, "settings", started, read).await)
    }

    #[tool(
        description = "When the person asks: change Settings > General (stale and cancel days, the agent folder, \
                       the hand-off limit). Recorded as yours."
    )]
    async fn set_settings(
        &self,
        Parameters(ask): Parameters<SetSettings>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let change = crate::coord::SettingsChange {
            stale_days: ask.stale_days,
            cancel_days: ask.cancel_days,
            agent_folder: ask.agent_folder,
            handoff_limit: ask.handoff_limit,
        };
        let t = trace(&caller);
        let set = crate::coord::change_settings_as(&self.state, &t, change, caller.agent.clone());
        let set = said(set.await);
        Ok(self.respond(&caller, "set_settings", started, set).await)
    }

    #[tool(
        description = "When the person asks: change an agent's name, account, model, effort or approvals (not \
                       Full). It applies from the agent's next turn. Recorded as yours."
    )]
    async fn set_agent(
        &self,
        Parameters(ask): Parameters<SetAgent>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let set = self.set_agent_as(&caller, ask).await;
        Ok(self.respond(&caller, "set_agent", started, set).await)
    }

    #[tool(
        description = "When the person asks: answer another agent's open approval in this chat. Never your own."
    )]
    async fn answer_approval(
        &self,
        Parameters(ask): Parameters<AnswerApproval>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let answered = self.answer_approval_as(&caller, ask).await;
        Ok(self
            .respond(&caller, "answer_approval", started, answered)
            .await)
    }

    #[tool(
        description = "When the person asks: answer one of the person's questions in this chat for them. Never \
                       your own question, nor one filed for you."
    )]
    async fn answer_person(
        &self,
        Parameters(ask): Parameters<AnswerPerson>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let answered = self.answer_person_as(&caller, ask).await;
        Ok(self
            .respond(&caller, "answer_person", started, answered)
            .await)
    }
}

impl Tools {
    async fn read_settings(&self, caller: &Caller) -> Result<String, String> {
        let general = said(crate::coord::settings_shown(&self.state, &trace(caller)).await)?;
        let (agents, projects) = self
            .db(|c| Ok((gathr::agents::list(c)?, gathr::projects::list(c)?)))
            .await?;
        let mut agents: Vec<Value> = agents
            .iter()
            .map(|a| {
                json!({ "name": a.name, "account": a.account, "model": a.model,
                "effort": a.effort, "approvals": a.approvals.as_str() })
            })
            .collect();
        agents.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
        let general: Value = serde_json::from_str(&general).unwrap_or_default();
        Ok(json!({ "general": general, "agents": agents, "projects": projects }).to_string())
    }

    async fn set_agent_as(&self, caller: &Caller, ask: SetAgent) -> Result<String, String> {
        let named = ask.agent.trim().to_lowercase();
        let agents = self.db(|c| gathr::agents::list(c)).await?;
        let agent: Agent = agents
            .into_iter()
            .find(|a| a.name.to_lowercase() == named)
            .ok_or_else(|| format!("there's no agent named {}", ask.agent))?;
        let approvals = match &ask.approvals {
            Some(mode) => Some(
                serde_json::from_value::<Approvals>(json!(mode))
                    .map_err(|_| format!("{mode} isn't an approvals mode: ask, edits or auto"))?,
            ),
            None => None,
        };
        let default = |v: Option<String>| v.map(|v| Some(v).filter(|v| !v.trim().is_empty()));
        let change = Change {
            name: ask.name,
            account: ask.account,
            model: default(ask.model),
            effort: default(ask.effort),
            approvals,
        };
        let t = trace(caller);
        let set =
            crate::agents::change_as(&self.state, &t, &agent.id, change, caller.agent.clone());
        said(set.await)
    }

    async fn answer_approval_as(
        &self,
        caller: &Caller,
        ask: AnswerApproval,
    ) -> Result<String, String> {
        let (open, _) = self.chats.open(&caller.chat).await;
        let found = open
            .iter()
            .find(|a| a["request_id"] == ask.request_id.as_str());
        let Some(found) = found else {
            return Err(format!(
                "no approval {} is open in this chat",
                ask.request_id
            ));
        };
        if found["agent"].as_str() == caller.agent.as_deref() {
            return Err(OWN_APPROVAL.into());
        }
        if !self
            .chats
            .approve(&caller.chat, &ask.request_id, ask.allow)
            .await
        {
            return Err(format!("approval {} is no longer open", ask.request_id));
        }
        let event = Event::point("gathr.approvals.answer")
            .trace(&caller.trace().unwrap_or_else(gathr::ids::trace_id))
            .chat(&caller.chat)
            .ok()
            .attr("request_id", ask.request_id.clone())
            .attr("allow", ask.allow)
            .attr("for", found["agent"].clone())
            .attr("by", caller.agent.clone());
        self.chats.writer.record(event);
        let how = if ask.allow { "Allowed" } else { "Denied" };
        Ok(format!("{how} {}.", ask.request_id))
    }

    async fn answer_person_as(&self, caller: &Caller, ask: AnswerPerson) -> Result<String, String> {
        let chat = caller.chat.clone();
        let task = ask.task;
        let here = self
            .db(move |c| gathr::tasks::person::open(c, Some(&chat)))
            .await?
            .iter()
            .any(|o| o.task.number == task);
        if !here {
            return Err(format!(
                "#{task} isn't one of the person's open questions in this chat"
            ));
        }
        let answer = gathr::tasks::person::Answer {
            task,
            pick: ask.pick.unwrap_or_default(),
            yes: ask.yes,
            done: ask.done.unwrap_or_default(),
            text: ask.text,
        };
        let (by, trace) = (caller.agent.clone(), trace(caller).0);
        let sent = self
            .db(move |c| gathr::tasks::person::answer_as(c, &[answer], by.as_deref(), &trace))
            .await?;
        self.chats.start(sent.starts);
        match sent.refused.into_iter().next() {
            Some((_, why)) => Err(why),
            None => Ok(format!("Answered #{task} for the person.")),
        }
    }
}

/// The turn's trace, so the API's own events land on it.
fn trace(caller: &Caller) -> Trace {
    Trace(caller.trace().unwrap_or_else(gathr::ids::trace_id))
}

/// The API's answer as a tool's: its JSON, or why it failed.
fn said(answer: Answer) -> Result<String, String> {
    answer
        .map(|json| json.0.to_string())
        .map_err(|fail| fail.why().to_string())
}
