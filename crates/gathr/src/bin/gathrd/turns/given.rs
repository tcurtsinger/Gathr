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
            answers: start.answers,
            person: false,
        };
        self.queue_given(ids, start.text, start.message).await;
    }

    /// Queues a turn Gathr saved, with its message, as `given` does: for an agent, or none.
    pub(super) async fn queue_given(&mut self, ids: Ids, text: String, message: serde_json::Value) {
        let event = Event::point("gathr.chat.give")
            .trace(&ids.trace)
            .chat(&ids.chat);
        let event = match ids.task {
            Some(task) => event.task(task),
            None => event,
        };
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        self.record(
            event
                .ok()
                .attr("gathr.turn.id", ids.turn.clone())
                .attr("gen_ai.agent.id", ids.agent.clone())
                .attr("gathr.message.chars", text.chars().count()),
        );
        let queued = Queued {
            ids,
            text,
            held: None,
        };
        let _ = self.enqueue(&queued).await;
    }

    /// Why a turn may not start now (0010's gate), or `None`: its chat is archived or deleted
    /// (F11), its task was cancelled while it was queued, or its task may not start. Pauses and
    /// usage holds keep it queued instead (`holds`).
    pub(super) async fn gate(&self, ids: &Ids) -> Option<String> {
        let (chat, task, turn) = (ids.chat.clone(), ids.task, ids.turn.clone());
        let asked = self
            .chats
            .writer
            .call(move |conn| {
                match gathr::archive::archived(conn, &chat) {
                    Ok(Some(_)) => return Ok(Some("its chat is archived".to_string())),
                    Err(gathr::Error::Refused(_)) => {
                        return Ok(Some("its chat was deleted".into()));
                    }
                    Err(e) => return Err(e),
                    Ok(None) => {}
                }
                if let Some(why) = gathr::tasks::stop_why(conn, &turn)? {
                    return Ok(Some(why));
                }
                task.map_or(Ok(None), |task| gathr::tasks::may_start(conn, task))
            })
            .await;
        match asked {
            Some(Ok(why)) => why,
            Some(Err(e)) => Some(format!("its chat or task wasn't read: {e}")),
            None => Some("its chat or task wasn't read: the database didn't answer".into()),
        }
    }
}
