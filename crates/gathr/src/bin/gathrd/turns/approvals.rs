//! The questions a turn asks the person and their answers (0029). A push or check Gathr itself asks
//! about is in `pushes.rs`.

use serde_json::{Value, json};

use super::{Running, Task};

impl Running {
    /// Whether an approval of it waits for the person. A service action whose caller gave up has
    /// no one to answer.
    pub(super) fn asks(&self) -> bool {
        self.approvals.keys().any(|request_id| {
            let service = self.service_approvals.get(request_id);
            service.is_none_or(|(_, reply)| !reply.is_closed())
        })
    }
}

impl Task {
    pub(super) async fn approve(&mut self, chat: &str, request_id: &str, allow: bool) -> bool {
        let Some(turn) = self.turn.as_mut().filter(|t| t.ids.chat == chat) else {
            return false;
        };
        let Some((tool, input, _)) = turn.approvals.remove(request_id) else {
            return false;
        };
        if let Some((kind, reply)) = turn.service_approvals.remove(request_id) {
            return self.answer_service(kind, request_id, allow, reply);
        }
        if allow && gathr::approvals::runs_push(&tool, &input) {
            turn.push_allowed = true;
        }
        let turn = turn.ids.turn.clone();
        let Some(answer) = self.driver.approval(request_id, allow, &input) else {
            return false;
        };
        self.write(&answer, Some(&turn)).await.is_ok()
    }

    /// The chat's questions still open, as the live socket sent them, for a window that opens
    /// the chat later.
    pub(super) fn open_approvals(&self, chat: &str) -> Vec<Value> {
        let Some(turn) = self.turn.as_ref().filter(|t| t.ids.chat == chat) else {
            return Vec::new();
        };
        // A service action whose caller gave up has no one to answer.
        let open = turn.approvals.iter().filter(|(request_id, _)| {
            let push = turn.service_approvals.get(*request_id);
            push.is_none_or(|(_, reply)| !reply.is_closed())
        });
        let open = open.map(|(request_id, (tool, input, why))| {
            json!({ "turn": turn.ids.turn, "request_id": request_id, "tool": tool, "input": input,
                "why": why, "agent": turn.ids.agent })
        });
        open.collect()
    }

    /// Tells the chat list after an approval of the turn under way was answered.
    pub(super) async fn approvals_changed(&self) {
        if let Some(ids) = self.turn.as_ref().map(|t| t.ids.clone()) {
            self.chat_state(&ids).await;
        }
    }
}
