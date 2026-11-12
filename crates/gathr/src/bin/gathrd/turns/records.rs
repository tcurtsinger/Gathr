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
        self.db_message_from(ids, (role, kind), body, None).await
    }

    /// `db_message`, for one whose time is `began` rather than now.
    pub(super) async fn db_message_from(
        &self,
        ids: &Ids,
        (role, kind): (&'static str, &'static str),
        body: &str,
        began: Option<i64>,
    ) -> Result<Value, String> {
        let (chat, turn, body) = (ids.chat.clone(), ids.turn.clone(), body.to_string());
        let add = move |conn: &mut rusqlite::Connection| match began {
            Some(began) => store::add_message_at(conn, &chat, &turn, role, kind, &body, began),
            None => store::add_message(conn, &chat, &turn, role, kind, &body),
        };
        let added = self.chats.writer.call(add).await;
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

    /// Saves a block of the turn's reply. Once one is refused, it and every later block are kept
    /// in order, unsaved, until a save works: a window reads them as `streaming` meanwhile.
    pub(super) async fn save_reply(&mut self, ids: &Ids, text: String, ending: bool) {
        if text.is_empty() {
            return;
        }
        if self.turn.as_ref().is_none_or(|t| t.save_failed.is_none()) {
            match self.db_message(ids, "assistant", "text", &text).await {
                Ok(message) => {
                    return self
                        .publish(&ids.chat, json!({ "type": "message", "message": message }));
                }
                Err(why) => self.save_failed(why, ending).await,
            }
        }
        self.unsaved.push((ids.clone(), text));
    }

    /// The turn's reply couldn't be saved: the turn fails with why, and, unless it's ending anyway,
    /// the engine is stopped, so nothing more is done or said that couldn't be kept either.
    pub(super) async fn save_failed(&mut self, why: String, ending: bool) {
        let Some(turn) = self.turn.as_mut().filter(|t| t.save_failed.is_none()) else {
            return;
        };
        turn.save_failed = Some(why);
        let chat = turn.ids.chat.clone();
        if !ending {
            self.interrupt(&chat).await;
        }
    }

    /// Replies kept after their turn ended because they weren't saved: tried again before the
    /// next turn, each kept until its save works.
    pub(super) async fn save_unsaved(&mut self) {
        let mut kept = std::mem::take(&mut self.unsaved).into_iter();
        for (ids, text) in kept.by_ref() {
            match self.db_message(&ids, "assistant", "text", &text).await {
                Ok(message) => {
                    self.publish(&ids.chat, json!({ "type": "message", "message": message }))
                }
                // The rest wait behind it, so the reply's blocks are saved in their order.
                Err(_) => {
                    self.unsaved.push((ids, text));
                    break;
                }
            }
        }
        self.unsaved.extend(kept);
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
        let update = json!({ "type": "turn", "turn": ids.turn, "status": status,
            "agent": ids.agent, "why": why, "ref": reference, "task": ids.task,
            "answers": ids.answers });
        self.publish_turn(ids, update).await;
        self.chat_state(ids).await;
    }

    /// Sends a turn's change live with when it was made and its place once started, read from
    /// its row, so a window following live shows and orders it as a reload would (0028).
    pub(super) async fn publish_turn(&self, ids: &Ids, mut update: Value) {
        let turn = ids.turn.clone();
        let place = self.chats.writer.call(move |c| store::turn_place(c, &turn));
        match place.await {
            Some(Ok((position, created))) => {
                update["position"] = json!(position);
                update["created"] = json!(created);
            }
            Some(Err(e)) => {
                let event = Event::point("gathr.turn.status").trace(&ids.trace);
                self.record(event.error("sqlite").attr("why", e.to_string()));
            }
            None => {}
        }
        self.publish(&ids.chat, update);
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
