//! Turns Gathr starts for an agent (0010): a task's, a wake-up or a hear-back. Its ask is Gathr's
//! message, never the person's, and it queues behind what the agent is doing, never steered into a
//! turn under way. Every start passes the gate. Both are saved already, with the change that
//! called for them.

use gathr::event::Event;
use gathr::tasks::Start;
use serde_json::json;

use super::{Ids, Queued, Task};

impl Task {
    pub(super) async fn given(&mut self, start: Start) {
        let ids = Ids {
            chat: start.chat,
            turn: start.turn,
            trace: start.trace,
            agent: Some(start.agent),
            task: start.task,
        };
        let event = Event::point("gathr.chat.give")
            .trace(&ids.trace)
            .chat(&ids.chat);
        let event = match ids.task {
            Some(task) => event.task(task),
            None => event,
        };
        let message = start.message;
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        self.record(
            event
                .ok()
                .attr("gathr.turn.id", ids.turn.clone())
                .attr("gen_ai.agent.id", ids.agent.clone())
                .attr("gathr.message.chars", start.text.chars().count()),
        );
        let queued = Queued {
            ids,
            text: start.text,
        };
        let _ = self.enqueue(&queued).await;
    }

    /// Why a turn on a task may not start now (0010's gate), or `None`. Pause and usage holds join
    /// it with F10.
    pub(super) async fn gate(&self, ids: &Ids) -> Option<String> {
        let task = ids.task?;
        let asked = self
            .chats
            .writer
            .call(move |conn| gathr::tasks::may_start(conn, task))
            .await;
        match asked {
            Some(Ok(why)) => why,
            Some(Err(e)) => Some(format!("its task wasn't read: {e}")),
            None => Some("its task wasn't read: the database didn't answer".into()),
        }
    }
}
