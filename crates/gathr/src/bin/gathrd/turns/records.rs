//! What a turn writes: its messages and status in the database, live to every window, and its
//! events in the record (0011, 0019).

use gathr::event::Event;
use gathr::store;
use serde_json::{Value, json};

use super::{Ids, Task};
use crate::chats::Failed;

impl Task {
    /// Saves a message of the turn. A refused write is recorded on the turn's trace, so its
    /// reference finds why (audit 2.2.6); the caller decides what the turn does about it.
    pub(super) async fn db_message(
        &self,
        ids: &Ids,
        role: &'static str,
        kind: &'static str,
        body: &str,
    ) -> Result<Value, String> {
        let (chat, turn, body) = (ids.chat.clone(), ids.turn.clone(), body.to_string());
        let added = self
            .chats
            .writer
            .call(move |conn| store::add_message(conn, &chat, &turn, role, kind, &body))
            .await;
        let why = match added {
            Some(Ok(message)) => return Ok(message),
            Some(Err(e)) => e.to_string(),
            None => "the database didn't answer".to_string(),
        };
        let why = format!("A {role} message wasn't saved: {why}");
        self.record(
            Event::point("gathr.chat.message")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("sqlite")
                .attr("why", why.clone()),
        );
        Err(why)
    }

    /// The turn's reply couldn't be saved: the turn fails with why, and the engine is stopped, so
    /// nothing more is done or said that couldn't be kept either.
    pub(super) async fn save_failed(&mut self, why: String) {
        let Some(turn) = self.turn.as_mut().filter(|t| t.save_failed.is_none()) else {
            return;
        };
        turn.save_failed = Some(why);
        let chat = turn.ids.chat.clone();
        self.interrupt(&chat).await;
    }

    /// Replies kept after their turn ended because they weren't saved: tried again before the
    /// next turn, each kept until its save works.
    pub(super) async fn save_unsaved(&mut self) {
        for (ids, text) in std::mem::take(&mut self.unsaved) {
            match self.db_message(&ids, "assistant", "text", &text).await {
                Ok(message) => {
                    self.publish(&ids.chat, json!({ "type": "message", "message": message }))
                }
                Err(_) => self.unsaved.push((ids, text)),
            }
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
        self.chats.settle_pauses(false).await;
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
