//! `post` (0079): an agent chooses where its turn's words show in a team chat, the room or a thread.
//! The whole turn's bubble moves, live, and what needs the person stays in the room.

use std::time::Instant;

use axum::http::request::Parts;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::Caller;
use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Post {
    /// `room` for the main list, a message's id or `#12` for that thread, or `person` for the
    /// thread of the person's message your turn answers.
    to: String,
}

#[tool_router(router = post_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Choose where your words this turn show in a team chat: `room` for the main list, a message's \
                       id or #task for that thread, `person` for the thread of the person's message you answer. \
                       Without it Gathr puts them where they belong: what you say to the person in the room, work \
                       in its task's thread. Call it before you write."
    )]
    async fn post(
        &self,
        Parameters(ask): Parameters<Post>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let chosen = self.choose_place(&caller, ask.to).await;
        Ok(self.respond(&caller, "post", started, chosen).await)
    }
}

impl Tools {
    async fn choose_place(&self, caller: &Caller, to: String) -> Result<String, String> {
        let on = caller.on().ok_or("you have no turn running in this chat")?;
        let (chat, turn, trace) = (caller.chat.clone(), on.turn.clone(), on.trace);
        let chosen = self
            .db(move |c| {
                let tx = c.transaction()?;
                let chosen = gathr::topics::place::choose(&tx, &chat, &turn, &to, &trace)?;
                tx.commit()?;
                Ok(chosen)
            })
            .await?;
        self.moved(caller, &on.turn, chosen.topic);
        Ok(chosen.said)
    }

    /// A turn's new place, live: the window merges the update into the turn it holds and moves its
    /// bubble. The engine is calling a tool, so the turn is running.
    pub(super) fn moved(&self, caller: &Caller, turn: &str, topic: Option<String>) {
        let update = json!({ "type": "turn", "chat": caller.chat, "turn": turn,
            "status": "running", "agent": caller.agent, "topic": topic });
        self.chats.publish(update);
    }
}
