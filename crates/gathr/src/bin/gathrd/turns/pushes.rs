//! Service-side approvals (0029): git pushes and Codex wait checks use the same active-turn
//! question and response path. Claude wait checks have already passed its native PreToolUse ask.

use gathr::agents::Approvals;
use gathr::event::Event;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::Task;

impl Task {
    /// Full bypasses both asks. A push also inherits an allowed push command; a check does not.
    /// Claude checks use its native hook; Codex checks always ask here before being stored.
    pub(super) fn ask_service(
        &mut self,
        kind: &'static str,
        chat: &str,
        input: Value,
        reply: oneshot::Sender<bool>,
    ) {
        let native_hook =
            kind == "check" && self.assigned.as_ref().is_some_and(|a| a.engine == "claude");
        let full = self.assigned.as_ref().map(|a| a.approvals) == Some(Approvals::Full);
        let turn = self.turn.as_mut().filter(|t| t.ids.chat == chat);
        let trace = turn.as_ref().map(|t| t.ids.trace.clone());
        let event = Event::point(&format!("gathr.approvals.{kind}"))
            .trace(&trace.unwrap_or_else(gathr::ids::trace_id))
            .chat(chat)
            .attr("gen_ai.agent.id", self.agent.clone())
            .attr("remote", input["remote"].as_str().map(String::from));
        // Claude already asked through its native hook. Keep that path valid even if its
        // completed turn ended just before the MCP request reached us.
        if native_hook {
            self.record(event.ok().attr("allowed_by", "Claude Code PreToolUse"));
            let _ = reply.send(true);
            return;
        }
        let Some(turn) = turn else {
            let why = "no turn of the agent's is running in the chat";
            self.record(event.error(&format!("{kind}_refused")).attr("why", why));
            let _ = reply.send(false);
            return;
        };
        if full || (kind == "push" && turn.push_allowed) {
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
        let (tool, why) = if kind == "check" {
            (
                "wait check".to_string(),
                "Gathr runs this check outside the engine sandbox",
            )
        } else {
            ("git push".to_string(), "Gathr asks because it pushes")
        };
        let asked = (tool.clone(), input.clone(), Some(why.to_string()));
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
