//! What an agent hears of its team chat when its turn opens (0081): read when the turn is sent,
//! counted on its trace. A failed read is recorded, and the turn goes without it.

use gathr::event::Event;

use super::{Ids, Task};

impl Task {
    /// The chat's lines the agent hasn't heard, as its turn opens: the last ones when `fresh`.
    pub(super) async fn heard(&self, ids: &Ids, fresh: bool) -> Option<String> {
        let agent = ids.agent.clone()?;
        let (chat, turn) = (ids.chat.clone(), ids.turn.clone());
        let read = self
            .chats
            .writer
            .call(move |c| gathr::room::catch_up(c, &chat, &turn, &agent, fresh))
            .await;
        let event = Event::point("gathr.room.heard")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .attr("gathr.turn.id", ids.turn.clone())
            .attr("gen_ai.agent.id", ids.agent.clone());
        let heard = match read {
            Some(Ok(heard)) => heard?,
            failed => {
                let why = format!("the chat's lines weren't read: {failed:?}");
                self.record(event.error("sqlite").attr("why", why));
                return None;
            }
        };
        let chars = heard.text.chars().count();
        self.record(
            event
                .ok()
                .attr("lines", heard.lines)
                .attr("chars", chars)
                .attr("cut", heard.cut)
                .attr("fresh", heard.fresh),
        );
        Some(heard.text)
    }
}
