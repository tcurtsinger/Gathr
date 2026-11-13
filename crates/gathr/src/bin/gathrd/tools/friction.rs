//! `report_friction` (0048): an agent says what in Gathr got in its way, and Gathr keeps it with
//! who said it, in which chat and on which task. Nobody is woken.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::friction;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ReportFriction {
    /// What got in your way, in a sentence or two (at most 500 characters): what you tried, and
    /// what happened instead.
    what: String,
    /// Where: the tool, screen, command or file.
    #[serde(rename = "where")]
    place: Option<String>,
}

#[tool_router(router = friction_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Report something in Gathr that got in your way: a tool that refused wrongly, a message that went \
                       missing, a status that was wrong. Gathr keeps it with your name, the chat and your task; nobody is \
                       woken. One report for each thing, when it happens."
    )]
    async fn report_friction(
        &self,
        Parameters(ask): Parameters<ReportFriction>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let on = caller.on();
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let kept = self
            .db(move |c| {
                let report = friction::Report {
                    chat: &chat,
                    agent: agent.as_deref(),
                    turn: on.as_ref().map(|o| o.turn.as_str()),
                    task: on.as_ref().and_then(|o| o.task),
                    what: &ask.what,
                    place: ask.place.as_deref(),
                };
                let id = friction::report(c, &report, &trace)?;
                Ok(format!("Recorded as friction #{id}."))
            })
            .await;
        Ok(self
            .respond(&caller, "report_friction", started, kept)
            .await)
    }
}
