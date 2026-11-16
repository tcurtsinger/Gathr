//! `size`: the chat's Supervisor records the size of the person's ask, repair or new, or makes a
//! message that joined another ask an ask of its own (0061). The rules are `gathr::asks::size`'s.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Size {
    /// repair or new.
    size: String,
    /// The id of a message of the person's, as read_chat gives it. Left out: the ask this turn serves.
    message: Option<String>,
}

#[tool_router(router = size_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Record the size of the person's ask this turn serves: repair or new. An ask with no size counts \
                       as new. With message, size the ask that message started, or make a message that joined another \
                       ask an ask of its own. The chat's Supervisor only."
    )]
    async fn size(
        &self,
        Parameters(ask): Parameters<Size>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let turn = caller.on().map(|on| on.turn);
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let sized = self
            .db(move |c| {
                let sizing = gathr::asks::Sizing {
                    chat: &chat,
                    agent: &agent,
                    turn: turn.as_deref(),
                    size: &ask.size,
                    message: ask.message.as_deref(),
                    trace: &trace,
                };
                gathr::asks::size(c, &sizing)
            })
            .await;
        // The tasks a repair releases from waiting for their plan start (0063).
        let sized = sized.map(|(said, starts)| {
            self.chats.start(starts);
            said
        });
        Ok(self.respond(&caller, "size", started, sized).await)
    }
}
