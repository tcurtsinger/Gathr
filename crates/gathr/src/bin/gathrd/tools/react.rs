//! `react`: an agent puts its thumbs-up on the person's message or another agent's in its own chat.
//! One row and one event: it posts no message, counts no hand-off and wakes nobody, but a plan's
//! checker's thumbs-up on the version it checks finishes that check (0062).

use std::time::Instant;

use axum::http::request::Parts;
use gathr::tasks::Took;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::Caller;
use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct React {
    /// The message's id, as read_chat or a message to you gives it, or `person` for the person's
    /// message your turn answers.
    message: String,
}

#[tool_router(router = react_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Agree with the person's message or another agent's in this chat: puts your thumbs-up on it, by \
                       its id, or `person` for the person's message your turn answers. It posts nothing and wakes \
                       nobody. To say more than that you agree, send a message instead."
    )]
    async fn react(
        &self,
        Parameters(ask): Parameters<React>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let given = self.give_thumbs_up(&caller, ask.message).await;
        Ok(self.respond(&caller, "react", started, given).await)
    }
}

impl Tools {
    async fn give_thumbs_up(&self, caller: &Caller, message: String) -> Result<String, String> {
        let on = caller.on().ok_or("you have no turn running in this chat")?;
        let (chat, from, asked) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
            message,
        );
        let giver = from.clone();
        let (added, agreed) = self
            .db(move |c| {
                let tx = c.transaction()?;
                gathr::archive::refuse_archived(&tx, &chat)?;
                let asked = match asked.as_str() {
                    "person" => gathr::reactions::persons(&tx, &on.turn)?,
                    _ => asked,
                };
                let added = gathr::reactions::add(&tx, &chat, &asked, &giver, &on.trace)?;
                let agreed = match added.new {
                    true => gathr::plans::agreed(&tx, &chat, &asked, &giver, &on.trace)?,
                    false => None,
                };
                tx.commit()?;
                Ok((added, agreed))
            })
            .await?;
        // Named by the message it is on: `person`, or Gathr's copy, stands for that one.
        let message = &added.message;
        if !added.new {
            return Ok(format!("{message} already has your thumbs-up."));
        }
        // Each names its chat: a window keeps only the live events of its own (G161).
        let reaction = json!({ "message": added.message, "agent": from, "time": added.time });
        self.chats
            .publish(json!({ "type": "reaction", "chat": caller.chat, "reaction": reaction }));
        let mut said = format!("Gave {message} a thumbs-up.");
        if let Some(a) = agreed {
            self.chats.start(a.starts);
            let (n, v) = (a.check, a.version);
            said.push_str(&match a.took {
                Took::Kept => format!(
                    "\nRecorded: your check #{n} of version {v} is done when the turn running on it ends."
                ),
                _ => format!("\nYour check #{n} of version {v} is done."),
            });
        }
        Ok(said)
    }
}
