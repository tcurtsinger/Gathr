//! Service-side approvals (0029): a git push from an agent's worktree asks the person on the
//! engine's active turn.

use gathr::agents::Approvals;
use gathr::event::Event;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::Task;

impl Task {
    /// Full never asks. A push that isn't risky never asks, and one that is inherits an allowed
    /// push command.
    pub(super) async fn ask_service(
        &mut self,
        kind: &'static str,
        chat: &str,
        input: Value,
        reply: oneshot::Sender<bool>,
    ) {
        let full = self
            .assigned
            .as_ref()
            .is_some_and(|a| a.approvals == Approvals::Full);
        let turn = self.turn.as_mut().filter(|t| t.ids.chat == chat);
        let trace = turn.as_ref().map(|t| t.ids.trace.clone());
        let event = Event::point(&format!("gathr.approvals.{kind}"))
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .chat(chat)
            .attr("gen_ai.agent.id", self.agent.clone())
            .attr("remote", input["remote"].as_str().map(String::from));
        // An ordinary push goes through (the owner's "Ask only for risky pushes"); `risk` is why
        // the hook found one risky.
        let risk = input["risk"].as_str().map(String::from);
        if risk.is_none() {
            self.record(event.ok().attr("allowed_by", "an ordinary push"));
            let _ = reply.send(true);
            return;
        }
        let event = event.attr("why", risk.clone());
        let Some(turn) = turn else {
            let why = "no turn of the agent's is running in the chat";
            self.record(event.error(&format!("{kind}_refused")).attr("why", why));
            let _ = reply.send(false);
            return;
        };
        if full || turn.push_allowed {
            let by = if full {
                "full"
            } else {
                "the command was allowed"
            };
            self.record(event.ok().attr("allowed_by", by));
            let _ = reply.send(true);
            return;
        }
        let request_id = format!("gathr-{kind}-{}", gathr::ids::uuid());
        let why = format!(
            "{} {}",
            gathr::approvals::GATHR_ASKS,
            risk.unwrap_or_default()
        );
        let tool = "git push".to_string();
        let asked = (tool.clone(), input.clone(), Some(why.clone()));
        turn.approvals.insert(request_id.clone(), asked);
        turn.service_approvals
            .insert(request_id.clone(), (kind, reply));
        let ids = turn.ids.clone();
        self.record(event.ok().attr("asked", request_id.clone()));
        self.publish(
            &ids.chat,
            json!({ "type": "approval", "turn": ids.turn, "request_id": request_id,
            "tool": tool, "input": input, "why": why }),
        );
        self.chat_state(&ids).await;
    }

    /// Answers the service-side action: false when its caller has already given up.
    pub(super) fn answer_service(
        &mut self,
        kind: &str,
        request_id: &str,
        allow: bool,
        reply: oneshot::Sender<bool>,
    ) -> bool {
        let trace = self.turn.as_ref().map(|t| t.ids.trace.clone());
        let sent = reply.send(allow).is_ok();
        let event = Event::point(&format!("gathr.approvals.{kind}_answer"))
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .attr("request_id", request_id.to_string())
            .attr("allow", allow);
        self.record(match sent {
            true => event.ok(),
            false => event
                .error(&format!("{kind}_gone"))
                .attr("why", "the caller ended the action before the answer"),
        });
        sent
    }
}
