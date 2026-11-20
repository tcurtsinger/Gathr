//! What an agent hears as its turn opens, since its conversation last ran: its chat's lines (0081).
//! Read when the turn is sent, counted on its trace. A failed read is recorded, and the turn goes
//! without it.

use gathr::event::Event;

use super::{Ids, Task};

impl Task {
    /// The chat's lines the agent hasn't heard as its turn opens: the last of them when `fresh`, else
    /// since its conversation last ran.
    pub(super) async fn heard(&self, ids: &Ids, fresh: bool) -> Option<String> {
        let agent = ids.agent.clone()?;
        let (chat, turn) = (ids.chat.clone(), ids.turn.clone());
        let read = self
            .chats
            .writer
            .call(move |c| {
                let since = match fresh {
                    true => None,
                    false => gathr::room::last_ran(c, &chat, &turn, &agent)?,
                };
                gathr::room::catch_up(c, &chat, (&turn, &agent), since)
            })
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
