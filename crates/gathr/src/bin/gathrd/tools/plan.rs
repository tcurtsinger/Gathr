//! `plan`: the chat's Supervisor writes the plan for the person's ask its turn serves, a version per
//! call, checked in the open before the person sees it (0062, 0080). The rules are `gathr::plans::write`'s.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::plans::{self, Part, Writing};
use gathr::tasks::{DONE_MOST, PROOF_MOST, SIZE_MOST, TITLE_MOST, WHERE_MOST};
use gathr::topics::place;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Plan {
    /// A short title, at most 80 characters.
    #[schemars(length(max = TITLE_MOST))]
    title: String,
    /// What the plan does and why, for the person: what you found and the choices made.
    text: String,
    /// The parts in order, from 1. Each is a task as give_task gives one.
    parts: Vec<PlanPart>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PlanPart {
    /// The agent who does it, by name.
    to: String,
    /// A short title, at most 80 characters.
    #[schemars(length(max = TITLE_MOST))]
    what: String,
    /// Required.
    why: Option<String>,
    /// What is true when it's done.
    #[schemars(length(max = DONE_MOST))]
    done: Option<String>,
    /// How that is shown.
    #[schemars(length(max = PROOF_MOST))]
    proof: Option<String>,
    /// The folder and branch, or "no code".
    #[serde(rename = "where")]
    #[schemars(length(max = WHERE_MOST))]
    place: Option<String>,
    /// The files or area it touches. Required.
    #[schemars(length(max = SIZE_MOST))]
    size: Option<String>,
    /// The rough lines it adds. Required.
    #[schemars(range(min = 1))]
    lines: Option<i64>,
    /// Earlier parts it starts after, by number.
    after: Option<Vec<i64>>,
    /// Tasks of this chat it starts after, by number.
    after_tasks: Option<Vec<i64>>,
}

#[tool_router(router = plan_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Write the plan for the person's ask this turn serves: a title, text, and its parts, each a complete \
                       task. Calling it again writes the next version. Once \
                       your turn ends, the members who took no part check it before the person sees it, and on the \
                       person's Approve each part becomes a task, word for word. The chat's Supervisor only."
    )]
    async fn plan(
        &self,
        Parameters(ask): Parameters<Plan>,
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
        let wrote = self
            .db(move |c| {
                let parts: Vec<Part> = ask
                    .parts
                    .iter()
                    .map(|p| Part {
                        to: &p.to,
                        what: &p.what,
                        why: p.why.as_deref(),
                        done: p.done.as_deref(),
                        proof: p.proof.as_deref(),
                        place: p.place.as_deref(),
                        size: p.size.as_deref(),
                        lines: p.lines,
                        after: p.after.as_deref().unwrap_or_default(),
                        after_tasks: p.after_tasks.as_deref().unwrap_or_default(),
                    })
                    .collect();

                let w = Writing {
                    chat: &chat,
                    agent: &agent,
                    turn: turn.as_deref(),
                    title: &ask.title,
                    text: &ask.text,
                    parts: &parts,
                };
                let wrote = plans::write(c, &w, &trace)?;
                // A plan waits on the person: a thread its agent chose gives way to the room (0079).
                let moved = match turn.as_deref() {
                    Some(t) if place::overridden(c, &chat, t, &trace, "plan")? => {
                        Some((t.to_string(), gathr::topics::of_turn(c, &chat, t)?))
                    }
                    _ => None,
                };
                Ok((wrote, moved))
            })
            .await;
        let answer = wrote.map(|(w, moved)| {
            let message = json!({ "type": "message", "chat": caller.chat, "message": w.message });
            self.chats.publish(message);
            if let Some((turn, topic)) = moved {
                self.moved(&caller, &turn, topic);
            }
            self.chats.start(w.starts);
            w.answer
        });
        Ok(self.respond(&caller, "plan", started, answer).await)
    }
}
