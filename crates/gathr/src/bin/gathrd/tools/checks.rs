//! `checks` and `set_checks` (0036, owner's call on task 1455): the chat's Supervisor sets its
//! project's checks, starting from what Gathr finds in the project; the person never types one.
//! A command the project's own files don't name is the person's to allow, once per project.

use std::path::PathBuf;
use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::event::Event;
use gathr::team::Role;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use rusqlite::OptionalExtension;
use serde_json::json;

use super::Caller;
use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct NoArguments {}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SetChecks {
    /// The project's checks, in the order they run: each a command run by `sh` in a clean
    /// checkout of the PR's head. Replaces the list.
    checks: Vec<String>,
}

/// The project a Supervisor's call is about.
struct Mine {
    project: String,
    folder: PathBuf,
}

const ONLY: &str = "Only the chat's Supervisor sets the project's checks. To suggest a change, give the Supervisor a task";

#[tool_router(router = checks_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Supervisor only: the project's checks, each with who set it and when, and `found`: what the \
                       project's files name (package.json scripts, Cargo, pytest, CI workflow steps) as a start."
    )]
    async fn checks(
        &self,
        Parameters(_): Parameters<NoArguments>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let listed = match self.mine(&caller).await {
            Ok(mine) => {
                let folder = mine.folder.clone();
                let found = tokio::task::spawn_blocking(move || gathr::checks::found(&folder));
                let found = found.await.unwrap_or_default();
                let project = mine.project.clone();
                self.db(move |c| Ok(gathr::projects::get(c, &project)?.map(|p| p.checks)))
                    .await
                    .and_then(|checks| checks.ok_or_else(|| "the project is gone".to_string()))
                    .map(|checks| json!({ "checks": checks, "found": found }).to_string())
            }
            Err(why) => Err(why),
        };
        Ok(self.respond(&caller, "checks", started, listed).await)
    }

    #[tool(
        description = "Supervisor only: replace the project's checks, in order. A command the project's files name \
                       runs freely; any other asks the person once for this project."
    )]
    async fn set_checks(
        &self,
        Parameters(ask): Parameters<SetChecks>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let set = self.set(&caller, ask.checks).await;
        Ok(self.respond(&caller, "set_checks", started, set).await)
    }
}

impl Tools {
    /// The caller's project, when it's the chat's Supervisor (or its only agent) in a project chat.
    async fn mine(&self, caller: &Caller) -> Result<Mine, String> {
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        self.db(move |c| {
            let members = gathr::team::members(c, &chat)?;
            let lead = members.len() == 1
                || members
                    .iter()
                    .any(|m| m.role == Role::Supervisor && Some(&m.agent) == agent.as_ref());
            if !lead {
                return Err(Error::Refused(ONLY.into()));
            }
            let project: Option<String> = c
                .query_row("SELECT project_id FROM chats WHERE id = ?1", [&chat], |r| {
                    r.get(0)
                })
                .optional()?
                .flatten();
            let project =
                project.ok_or_else(|| Error::Refused("This chat isn't in a project".into()))?;
            let found = gathr::projects::get(c, &project)?;
            let folder = found
                .ok_or_else(|| Error::Refused("the project is gone".into()))?
                .folder;
            Ok(Mine {
                project,
                folder: PathBuf::from(folder),
            })
        })
        .await
    }

    async fn set(&self, caller: &Caller, checks: Vec<String>) -> Result<String, String> {
        let mine = self.mine(caller).await?;
        let commands = gathr::checks::commands(checks)?;
        let folder = mine.folder.clone();
        let own = tokio::task::spawn_blocking(move || gathr::checks::found(&folder))
            .await
            .unwrap_or_default();
        let (project, others) = (mine.project.clone(), commands.clone());
        let others: Vec<String> = others.into_iter().filter(|c| !own.contains(c)).collect();
        let ask = self
            .db(move |c| gathr::checks::not_allowed(c, &project, &others))
            .await?;
        if !ask.is_empty() {
            let input = json!({ "command": ask.join("\n"), "commands": ask });
            let mut stopping = self.stopping.clone();
            let allowed = tokio::select! {
                allowed = self.chats.ask_service(caller, input, "checks") => allowed,
                _ = stopping.wait_for(|stop| *stop) => false,
            };
            if !allowed {
                return Err(format!(
                    "the person didn't allow {}, so nothing was saved",
                    ask.join(", ")
                ));
            }
        }
        let source = caller.agent.clone().unwrap_or_default();
        let (project, saved, allowed) = (mine.project.clone(), commands.clone(), ask.clone());
        self.db(move |c| gathr::checks::set(c, &project, &saved, &source, &allowed))
            .await?;
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let event = Event::point("gathr.checks.set")
            .trace(&trace)
            .chat(&caller.chat)
            .ok()
            .attr("gathr.project.id", mine.project.clone())
            .attr("gen_ai.agent.id", caller.agent.clone())
            .attr("checks", commands.len())
            .attr("asked", ask.len());
        self.chats.writer.record(event);
        self.chats
            .publish(json!({ "type": "project", "project": mine.project, "change": "changed" }));
        Ok(format!(
            "Saved {} checks. Gathr runs them in this order on each PR's head.",
            commands.len()
        ))
    }
}
