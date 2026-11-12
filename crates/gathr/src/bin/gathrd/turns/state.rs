//! What a turn means for its chat's state in the chat list (0040).

use super::{Ids, Running, Task};

impl Running {
    /// Whether an approval of it waits for the person. A service action whose caller gave up has
    /// no one to answer.
    fn asks(&self) -> bool {
        self.approvals.keys().any(|request_id| {
            let service = self.service_approvals.get(request_id);
            service.is_none_or(|(_, reply)| !reply.is_closed())
        })
    }
}

impl Task {
    /// Tells the chat list where the turn's chat stands now: called once the turn has moved, and
    /// when an approval of it is asked or answered.
    pub(super) async fn chat_state(&self, ids: &Ids) {
        let own = self.turn.as_ref().filter(|t| t.ids.turn == ids.turn);
        let asks = own.is_some_and(Running::asks);
        self.chats.states.asking(&ids.turn, &ids.chat, asks);
        crate::chat_state::changed(&self.chats, &ids.chat, &ids.trace).await;
    }

    /// The same for the turn under way, after an approval of it was answered.
    pub(super) async fn approvals_changed(&self) {
        if let Some(ids) = self.turn.as_ref().map(|t| t.ids.clone()) {
            self.chat_state(&ids).await;
        }
    }
}
