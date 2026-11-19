//! Gathr's copy of a message reaching an agent for the person: their like of its message (0057), or
//! their message to several agents, for each agent but the one it was sent to (0028). As a message of
//! theirs would: into the agent's running turn in the chat, else a turn of its own that a pause never
//! holds and an update does. The chat draws no bubble for a copy, and nothing that looks for the
//! person's message of a turn (Retry, asks, the quiet check) finds one there: the window's Retry
//! finds the message it carries.

use gathr::event::Event;
use gathr::ids::{now_us, ulid};
use gathr::store;
use gathr::words::Opened;
use serde_json::{Value, json};

use super::{Ids, Queued, Task};
use crate::chats::Failed;

impl Task {
    /// The running turn a message to `agent` in `chat` goes into: that chat's, not stopping or
    /// compacting, and with no update about to stop it (0032).
    pub(super) fn steerable(&self, chat: &str, agent: Option<&str>) -> Option<Ids> {
        self.turn
            .as_ref()
            .filter(|t| !t.stopping && t.ids.chat == chat && t.ids.agent.as_deref() == agent)
            .filter(|t| !self.compactions.contains(&t.ids.turn))
            .filter(|_| self.chats.updating().is_none())
            .map(|t| t.ids.clone())
    }

    /// Whether a like opened the turn: one that then says nothing did what its line asked.
    pub(super) async fn opened_by_like(&self, turn: &str) -> bool {
        let id = turn.to_string();
        let sql = "SELECT opened_by IS 'liked' FROM turns WHERE id = ?1";
        let read = move |c: &mut rusqlite::Connection| c.query_row(sql, [&id], |r| r.get(0));
        matches!(self.chats.writer.call(read).await, Some(Ok(true)))
    }

    pub(super) async fn copy(
        &mut self,
        chat: String,
        (of, line, opened): (String, String, Opened),
        trace: String,
    ) -> Result<Value, Failed> {
        self.save_unsaved().await;
        let agent = self.agent.clone();
        if let Some(running) = self.steerable(&chat, agent.as_deref()) {
            // Failures are the copy's own: its reference finds them.
            let ids = Ids {
                trace: trace.clone(),
                ..running
            };
            let (c, t, text, of) = (chat.clone(), ids.turn.clone(), line.clone(), of.clone());
            let add = move |tx: &rusqlite::Transaction| store::add_copy(tx, &c, &t, &text, &of);
            let message = self.db_save(&ids, "gathr", add).await;
            let message = message.map_err(|why| self.fail_send(&ids, why))?;
            if let Err(e) = self.write_user(&line, Some(&ids.turn)).await {
                let why = format!("{} didn't take it: {e}", self.engine_label());
                return Err(self.fail_send(&ids, why));
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id, line, opened));
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            self.record(
                Event::point("gathr.turn.steer")
                    .trace(&trace)
                    .chat(&chat)
                    .ok(),
            );
            return Ok(json!({ "turn": ids.turn, "status": "running", "steered": true }));
        }
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace,
            agent,
            task: None,
            answers: None,
            person: true,
        };
        let (row, text) = (ids.clone(), line.clone());
        // The turn and the copy that opens it are one write, as the person's message and its turn are.
        let saved = self.chats.writer.call(move |conn| {
            let tx = conn.transaction()?;
            let on = (row.agent.as_deref(), None, None);
            store::add_turn(&tx, &row.chat, &row.turn, &row.trace, on, opened)?;
            let message = store::add_copy(&tx, &row.chat, &row.turn, &text, &of)?;
            tx.commit()?;
            Ok::<_, gathr::Error>(message)
        });
        let saved = match saved.await {
            Some(saved) => saved.map_err(|e| e.to_string()),
            None => Err("the database didn't answer".to_string()),
        };
        let message = saved
            .map_err(|why| self.fail_send(&ids, format!("Gathr's copy wasn't saved: {why}")))?;
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        let queued = Queued {
            ids,
            text: line,
            held: None,
        };
        let status = self.enqueue(&queued).await;
        let trace = queued.ids.trace.clone();
        let status = status.map_err(|why| Failed { why, trace })?;
        Ok(json!({ "turn": queued.ids.turn, "status": status }))
    }
}
