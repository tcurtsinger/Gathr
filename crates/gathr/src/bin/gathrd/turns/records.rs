//! What a turn writes: its messages and status in the database, live to every window, and its
//! events in the record (0011, 0019).

use gathr::event::Event;
use gathr::store;
use serde_json::{Value, json};

use super::{Ids, Task};
use crate::chats::Failed;

impl Task {
    pub(super) async fn db_message(
        &self,
        ids: &Ids,
        role: &'static str,
        kind: &'static str,
        body: &str,
    ) -> Value {
        let (chat, turn, body) = (ids.chat.clone(), ids.turn.clone(), body.to_string());
        let added = self
            .chats
            .writer
            .call(move |conn| store::add_message(conn, &chat, &turn, role, kind, &body))
            .await;
        match added {
            Some(Ok(message)) => message,
            Some(Err(e)) => {
                self.record(
                    Event::point("gathr.chat.message")
                        .chat(&ids.chat)
                        .error("sqlite")
                        .attr("why", e.to_string()),
                );
                Value::Null
            }
            None => Value::Null,
        }
    }

    /// Saves the turn's status, with what it moves, and sends it live. The update is built from
    /// what this task holds, not from the write: a failed write still names the turn's agent, so
    /// Retry goes to it.
    pub(super) async fn set_turn(&self, ids: &Ids, status: &str, why: Option<&str>) {
        let moved = gathr::tasks::Moved {
            chat: &ids.chat,
            turn: &ids.turn,
            agent: ids.agent.as_deref(),
            task: ids.task,
            trace: &ids.trace,
        };
        crate::coord::turn_moved(&self.chats, moved, status, why).await;
        let reference = gathr::ids::reference(&ids.trace);
        self.publish(
            &ids.chat,
            json!({ "type": "turn", "turn": ids.turn, "status": status,
            "agent": ids.agent, "why": why, "ref": reference, "task": ids.task,
            "answers": ids.answers }),
        );
    }

    pub(super) fn fail_send(&self, ids: &Ids, why: String) -> Failed {
        self.record(
            Event::point("gathr.chat.send")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("send")
                .attr("why", why.clone()),
        );
        Failed {
            why,
            trace: ids.trace.clone(),
        }
    }

    /// Sends a live update about `chat`.
    pub(super) fn publish(&self, chat: &str, mut event: Value) {
        if let Some(object) = event.as_object_mut() {
            object.insert("chat".into(), chat.into());
        }
        self.chats.publish(event);
    }

    /// Records an event, under the chat the engine runs in when it names none.
    pub(super) fn record(&self, event: Event) {
        let here = self.assigned.as_ref().map(|a| a.chat.as_str());
        let event = match (&event.conversation_id, here) {
            (None, Some(chat)) => event.chat(chat),
            _ => event,
        };
        self.chats.writer.record(event);
    }

    pub(super) async fn record_now(&self, event: Event) {
        self.chats.writer.record_now(event).await;
    }
}
