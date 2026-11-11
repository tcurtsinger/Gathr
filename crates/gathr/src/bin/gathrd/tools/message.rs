//! `message` (plans/agent-messages.md): a short message from one agent to one member of its team
//! chat. Shown there as the sender's, it wakes the member at most once, and counts as a hand-off:
//! past the limit the chat pauses and nobody is woken.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::event::Event;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::Caller;
use super::server::{Tools, caller_of};
use crate::chats::Cmd;

/// The most a message holds: a short ask or a heads-up, not a task.
const MOST: usize = 500;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Message {
    /// The member it's for, by name.
    to: String,
    /// What you tell them, in a few lines (at most 500 characters).
    text: String,
}

/// What was saved: the shown message, and where it goes.
struct Sent {
    shown: Value,
    to: String,
    to_name: String,
    from_name: String,
    depth: i64,
    over: Option<i64>,
    /// The pause line a chain past the limit adds, as saved.
    paused: Option<Value>,
    answers: String,
}

#[tool_router(router = message_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Send one member of this team chat a short message (a quick ask or a heads-up): shown in the team \
                       chat, it wakes them at most once. For work to be done and heard back on, give a task instead."
    )]
    async fn message(
        &self,
        Parameters(ask): Parameters<Message>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let sent = self.send_message(&caller, ask).await;
        Ok(self.respond(&caller, "message", started, sent).await)
    }
}

impl Tools {
    async fn send_message(&self, caller: &Caller, ask: Message) -> Result<String, String> {
        let text = ask.text.trim().to_string();
        let on = caller.on().ok_or("you have no turn running in this chat")?;
        let trace = on.trace.clone();
        let (chat, from, to) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
            ask.to,
        );
        let saved = self
            .db(move |c| {
                if text.is_empty() || text.chars().count() > MOST {
                    return Err(Error::Refused(format!(
                        "A message holds 1 to {MOST} characters"
                    )));
                }
                gathr::archive::refuse_archived(c, &chat)?;
                let team = gathr::team::members(c, &chat)?;
                if team.len() < 2 {
                    return Err(Error::Refused(
                        "message is for a team chat: this one has one agent".into(),
                    ));
                }
                let found = gathr::team::named(&team, &to);
                let member =
                    found.ok_or_else(|| Error::Refused(format!("{to} isn't in this chat")))?;
                if member.agent == from {
                    return Err(Error::Refused("An agent can't message itself".into()));
                }
                let from_name = team
                    .iter()
                    .find(|m| m.agent == from)
                    .map_or(from.clone(), |m| m.name.clone());
                let shown =
                    gathr::store::add_message(c, &chat, &on.turn, "assistant", "text", &text)?;
                let id = shown["id"].as_str().unwrap_or_default().to_string();
                c.execute(
                    "UPDATE messages SET to_agent = ?2 WHERE id = ?1",
                    [&id, &member.agent],
                )?;
                let mut shown = shown;
                shown["to"] = json!(member.agent);
                let counted = gathr::handoffs::record(c, &chat, &from, &member.agent)?;
                let mut paused = None;
                if let Some(limit) = counted.over {
                    gathr::pauses::pause(c, "chat", &chat, false, &on.trace)?;
                    let line = gathr::handoffs::paused_line(limit);
                    paused = Some(gathr::store::add_message(
                        c, &chat, &on.turn, "gathr", "text", &line,
                    )?);
                }
                Ok(Sent {
                    shown,
                    to: member.agent.clone(),
                    to_name: member.name.clone(),
                    from_name,
                    depth: counted.depth,
                    paused,
                    over: counted.over,
                    answers: on.turn.clone(),
                })
            })
            .await?;
        self.chats
            .publish(json!({ "type": "message", "message": saved.shown }));
        if let Some(line) = &saved.paused {
            self.chats
                .publish(json!({ "type": "message", "message": line }));
        }
        let delivered = match saved.over {
            Some(_) => "paused",
            None => self.deliver(caller, &saved, ask.text.trim()).await,
        };
        let event = Event::point("gathr.message.send")
            .trace(&trace)
            .chat(&caller.chat)
            .ok()
            .attr("from", caller.agent.clone())
            .attr("to", saved.to.clone())
            .attr("chars", ask.text.trim().chars().count())
            .attr("depth", saved.depth)
            .attr("delivered", delivered);
        self.chats.writer.record(event);
        if let Some(limit) = saved.over {
            let event = Event::point("gathr.handoff.limit")
                .trace(&trace)
                .chat(&caller.chat);
            self.chats
                .writer
                .record(event.ok().attr("limit", limit).attr("depth", saved.depth));
            self.chats.pauses_changed().await;
            return Ok(gathr::handoffs::paused_line(limit));
        }
        Ok(match delivered {
            "in_turn" => format!("Sent to {}, in the turn they're running.", saved.to_name),
            "failed" => format!(
                "Shown to {}, but it didn't reach them: give a task instead.",
                saved.to_name
            ),
            "held" => format!(
                "Sent to {}: their turn starts when the chat is resumed.",
                saved.to_name
            ),
            _ => format!("Sent to {}: it wakes them once.", saved.to_name),
        })
    }

    /// Hands the message to the member's own worker, which says where it went.
    async fn deliver(&self, caller: &Caller, sent: &Sent, text: &str) -> &'static str {
        let (reply, answer) = oneshot::channel();
        let ask = format!("{} wrote to you: {text}", sent.from_name);
        let task = self.chats.task(&caller.chat, Some(&sent.to));
        let command = Cmd::Message {
            chat: caller.chat.clone(),
            ask,
            answers: sent.answers.clone(),
            copy_of: sent.shown["id"].as_str().unwrap_or_default().to_string(),
            reply,
        };
        let _ = task.send(command);
        answer.await.unwrap_or("failed")
    }
}
