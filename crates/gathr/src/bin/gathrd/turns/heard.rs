//! What an agent hears as its turn opens, since its conversation last ran: its team chat's lines
//! (0081), and the questions to the person from its task with their answers. Read when the turn is
//! sent, counted on its trace. A failed read is recorded, and the turn goes without it.

use gathr::event::Event;

use super::{Ids, Task};

impl Task {
    /// The chat's lines the agent hasn't heard, and its task's questions, as its turn opens: all of
    /// them when `fresh`, else since `resumed`, the task's conversation the engine opened again on,
    /// or the one open, last ran.
    pub(super) async fn heard(
        &self,
        ids: &Ids,
        fresh: bool,
        resumed: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let Some(agent) = ids.agent.clone() else {
            return (None, None);
        };
        let (chat, turn) = (ids.chat.clone(), ids.turn.clone());
        // The conversation the turn runs in, when the engine knows it already; a new one's Init says it.
        let ran_in = self.session.id.clone();
        let read = self
            .chats
            .writer
            .call(move |c| {
                let ran = "UPDATE turns SET session_id = ?2 WHERE id = ?1 AND ?2 IS NOT NULL";
                c.execute(ran, rusqlite::params![turn, ran_in])?;
                let resumed = resumed.as_deref();
                let since = match fresh {
                    true => None,
                    false => gathr::room::last_ran(c, &chat, &turn, &agent, resumed)?,
                };
                let heard = gathr::room::catch_up(c, &chat, (&turn, &agent), since, resumed)?;
                Ok::<_, gathr::Error>((heard, gathr::tasks::questions(c, &turn, since)?))
            })
            .await;
        let event = Event::point("gathr.room.heard")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .attr("gathr.turn.id", ids.turn.clone())
            .attr("gen_ai.agent.id", ids.agent.clone());
        let (heard, asked) = match read {
            Some(Ok(read)) => read,
            failed => {
                let why = format!("the chat's lines weren't read: {failed:?}");
                self.record(event.error("sqlite").attr("why", why));
                return (None, None);
            }
        };
        if let Some((_, count)) = &asked {
            let asked = Event::point("gathr.task.questions")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .ok()
                .attr("gathr.turn.id", ids.turn.clone())
                .attr("gathr.task.count", *count);
            self.record(asked);
        }
        if let Some(heard) = &heard {
            let chars = heard.text.chars().count();
            self.record(
                event
                    .ok()
                    .attr("lines", heard.lines)
                    .attr("chars", chars)
                    .attr("cut", heard.cut)
                    .attr("fresh", heard.fresh),
            );
        }
        (heard.map(|h| h.text), asked.map(|a| a.0))
    }
}
