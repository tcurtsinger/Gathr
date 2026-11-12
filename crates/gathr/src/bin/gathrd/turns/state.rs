//! What a turn means for its chat's state in the chat list (0040).

use super::{Ids, Running, Task};

impl Task {
    /// Tells the chat list where the turn's chat stands now: called once the turn has moved, and
    /// when an approval of it is asked or answered.
    pub(super) async fn chat_state(&self, ids: &Ids) {
        let own = self.turn.as_ref().filter(|t| t.ids.turn == ids.turn);
        let asks = own.is_some_and(Running::asks);
        self.chats.states.asking(&ids.turn, &ids.chat, asks);
        crate::chat_state::changed(&self.chats, &ids.chat, &ids.trace).await;
    }
}
