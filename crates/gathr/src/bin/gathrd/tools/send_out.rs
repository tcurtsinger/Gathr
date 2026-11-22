//! `send_out`: the person's assistant, in its own chat, sends an agent it brought in back out once
//! that agent's subject is done. The window hears the team change and a quiet line.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SendOut {
    /// The agent's name.
    agent: String,
}

#[tool_router(router = send_out_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "In your own chat as the person's assistant: send an agent you brought in out of it, \
                       once its subject is done. It can be brought in again with @Name."
    )]
    async fn send_out(
        &self,
        Parameters(ask): Parameters<SendOut>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent, name) = (caller.chat.clone(), caller.agent.clone(), ask.agent);
        let said = format!("Sent {} out.", name.trim().trim_start_matches('@'));
        let done = self
            .chats
            .writer
            .call(move |c| gathr::assistant::send_out(c, &chat, agent.as_deref(), &name))
            .await
            .unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string());
        let trace = crate::api::Trace(caller.trace().unwrap_or_else(gathr::ids::trace_id));
        let said = done.map(|changed| {
            crate::team::changed(&self.state, &trace, &changed.chat, changed.members);
            crate::team::stayed(&self.chats, &changed.chat, changed.stays);
            said
        });
        Ok(self.respond(&caller, "send_out", started, said).await)
    }
}
