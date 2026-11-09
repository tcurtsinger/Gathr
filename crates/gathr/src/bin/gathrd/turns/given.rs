//! Turns Gathr starts for an agent (0010): a task's, a wake-up or a hear-back. Its ask is Gathr's
//! message, never the person's, and it queues behind what the agent is doing, never steered into a
//! turn under way. Every start passes the gate.

use gathr::event::Event;
use gathr::ids::{now_us, trace_id, ulid};
use gathr::store;
use gathr::tasks::Start;
use serde_json::json;

use super::{Ids, Queued, Task};

impl Task {
    pub(super) async fn given(&mut self, start: Start) {
        let ids = Ids {
            chat: start.chat,
            turn: ulid(now_us()),
            trace: trace_id(),
            agent: Some(start.agent),
            task: start.task,
        };
        let row = ids.clone();
        let added = self
            .chats
            .writer
            .call(move |conn| {
                let at = (row.agent.as_deref(), row.task);
                store::add_turn(conn, &row.chat, &row.turn, &row.trace, at)
            })
            .await;
        let event = Event::point("gathr.chat.give")
            .trace(&ids.trace)
            .chat(&ids.chat);
        let event = match ids.task {
            Some(task) => event.task(task),
            None => event,
        };
        if let Some(Err(e)) = added {
            self.record(event.error("sqlite").attr("why", e.to_string()));
            return;
        }
        let message = self.db_message(&ids, "gathr", "text", &start.text).await;
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
