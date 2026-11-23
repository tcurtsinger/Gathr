//! The person's message reaching the agent it is sent to: into the agent's running turn in the chat,
//! else a turn of its own. A reply under a message (0076) keeps what it answers; sent to several agents
//! (0028), each other one gets Gathr's copy (`liked.rs`). Each agent reads Gathr's line naming what a
//! reply answers and who else got it before the person's words.

use gathr::event::Event;
use gathr::ids::{now_us, ulid};
use gathr::replies::{self, Target};
use gathr::store;
use gathr::words::Opened;
use serde_json::{Value, json};

use super::{Ids, Queued, Task};
use crate::chats::Failed;

impl Task {
    pub(super) async fn send(
        &mut self,
        chat: String,
        (text, picks, target): (String, Option<String>, Target),
        (agent, agents): (Option<String>, usize),
        (trace, app): (String, Option<String>),
    ) -> Result<Value, Failed> {
        // A reply kept unsaved goes in first, before the message that follows it.
        self.save_unsaved().await;
        // Gathr's line before the words says what a reply answers, and who else got it (0028).
        let opened = match (target.is_reply(), agents > 1) {
            (true, _) => Opened::Reply,
            (false, true) => Opened::Picked,
            (false, false) if app.is_some() => Opened::App,
            (false, false) => Opened::Person,
        };
        if let Some(ids) = self.steerable(&chat, agent.as_deref()) {
            // G13: into the turn under way. Claude Code reads it after the step it's on and
            // says so (`Frame::Read`), as Esc-free typing does in its terminal.
            // Never sent on to the engine unsaved: its answer would answer a message nobody has.
            let saved = self
                .db_person_message(&ids, &text, (picks, target), app)
                .await;
            let (message, read) = saved.map_err(|why| self.fail_send(&ids, why))?;
            if let Err(e) = self.write_user(&read, Some(&ids.turn)).await {
                return Err(self.fail_send(&ids, format!("Claude Code didn't take it: {e}")));
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id.clone(), read, opened));
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            let steer = Event::point("gathr.turn.steer").trace(&ids.trace);
            self.record(steer.chat(&ids.chat).ok());
            return Ok(
                json!({ "turn": ids.turn, "status": "running", "steered": true, "message": id }),
            );
        }
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace,
            agent,
            answers: None,
            person: true,
        };
        let (row, said) = (ids.clone(), text.clone());
        let replying = target.clone();
        // The turn and the message that opens it are one write, as Gathr's own are: a check between
        // two would see the person's turn move the chat, and not that they wrote (0052).
        let saved = self.chats.writer.call(move |conn| {
            let tx = conn.transaction()?;
            let of = (row.agent.as_deref(), row.answers.as_deref());
            store::add_turn(&tx, &row.chat, &row.turn, &row.trace, of, opened)?;
            let (chat, turn, picks) = (&row.chat, &row.turn, picks.as_deref());
            let mut message = match replying.is_reply() {
                true => store::add_reply(&tx, chat, turn, &said, picks, &replying)?,
                false => store::add_person_message(&tx, chat, turn, &said, picks)?,
            };
            if let Some(app) = &app {
                gathr::from_app::mark(&tx, &mut message, app)?;
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            let read = replies::read_as(&tx, &id, &said)?;
            tx.commit()?;
            Ok::<_, gathr::Error>((message, read))
        });
        let saved = match saved.await {
            Some(saved) => saved.map_err(|e| e.to_string()),
            None => Err("the database didn't answer".to_string()),
        };
        let (message, read) = saved
            .map_err(|why| self.fail_send(&ids, format!("A user message wasn't saved: {why}")))?;
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        self.record(
            Event::point("gathr.chat.send")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .ok()
                .attr("gathr.message.id", message["id"].clone())
                .attr("gathr.turn.id", ids.turn.clone())
                .attr("gathr.message.chars", text.chars().count())
                .attr("gathr.reply.to", json!(target.reply_to))
                .attr("gathr.send.agents", agents),
        );
        let queued = Queued {
            ids,
            text: read,
            held: None,
        };
        let ids = &queued.ids;
        let reference = gathr::ids::reference(&ids.trace).to_string();
        let id = message["id"].clone();
        let answer = |status: &str| json!({ "turn": ids.turn, "status": status, "ref": reference, "message": id });
        match self.enqueue(&queued).await {
            Ok(status) => Ok(answer(status)),
            Err(why) => Err(Failed {
                why,
                trace: ids.trace.clone(),
            }),
        }
    }
}
