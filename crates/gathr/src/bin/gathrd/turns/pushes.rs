//! A `git push` from the agent's worktree (0029): its `pre-push` hook asks Gathr, and the push
//! waits for the person's answer like any approval, whichever command or script ran it.

use gathr::agents::Approvals;
use gathr::event::Event;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::Task;

impl Task {
    /// Answers at once in Full or right after the person allowed the command that pushes, and
    /// refuses with no turn running in the chat; otherwise the push waits as an approval.
    pub(super) fn ask_push(&mut self, chat: &str, input: Value, reply: oneshot::Sender<bool>) {
        let full = self.assigned.as_ref().map(|a| a.approvals) == Some(Approvals::Full);
        let turn = self.turn.as_mut().filter(|t| t.ids.chat == chat);
        let trace = turn.as_ref().map(|t| t.ids.trace.clone());
        let event = Event::point("gathr.approvals.push")
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .chat(chat)
            .attr("gen_ai.agent.id", self.agent.clone())
            .attr("remote", input["remote"].as_str().map(String::from));
        let Some(turn) = turn else {
            let why = "no turn of the agent's is running in the chat";
            self.record(event.error("push_refused").attr("why", why));
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
        let request_id = format!("gathr-push-{}", gathr::ids::uuid());
        let (tool, why) = ("git push".to_string(), "Gathr asks because it pushes");
        let asked = (tool.clone(), input.clone(), Some(why.to_string()));
        turn.approvals.insert(request_id.clone(), asked);
        turn.pushes.insert(request_id.clone(), reply);
        let ids = turn.ids.clone();
        self.record(event.ok().attr("asked", request_id.clone()));
        self.publish(
            &ids.chat,
            json!({ "type": "approval", "turn": ids.turn, "request_id": request_id,
            "tool": tool, "input": input, "why": why }),
        );
    }

    /// The person's answer to a push, sent to its hook: false when git already gave up on it.
    pub(super) fn answer_push(
        &mut self,
        request_id: &str,
        allow: bool,
        reply: oneshot::Sender<bool>,
    ) -> bool {
        let trace = self.turn.as_ref().map(|t| t.ids.trace.clone());
        let sent = reply.send(allow).is_ok();
        let event = Event::point("gathr.approvals.push_answer")
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .attr("request_id", request_id.to_string())
            .attr("allow", allow);
        self.record(match sent {
            true => event.ok(),
            false => event
                .error("push_gone")
                .attr("why", "git ended the push before the answer"),
        });
        sent
    }
}
