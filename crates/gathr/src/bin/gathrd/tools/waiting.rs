//! `waiting` (0050): what the caller's chat waits on, in words, for any agent.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, tool, tool_router};

use super::coord::Nothing;
use super::server::{Tools, caller_of};

#[tool_router(router = waiting_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "What this chat waits on now: each running and queued turn, wait, question, background command and \
                       PR to land, with who or what can end it and since when, then where each agent stands. Read it before \
                       you ask a teammate what they are waiting for."
    )]
    async fn waiting(
        &self,
        Parameters(_): Parameters<Nothing>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let live = crate::coord::live_now(&self.chats, &caller.chat);
        let chat = caller.chat.clone();
        let said = self
            .db(move |c| {
                let now = gathr::ids::now_us();
                let answer = gathr::waiting::answer(c, &chat, &live, now)?;
                Ok(gathr::waiting::text(&answer, now))
            })
            .await;
        Ok(self.respond(&caller, "waiting", started, said).await)
    }
}
