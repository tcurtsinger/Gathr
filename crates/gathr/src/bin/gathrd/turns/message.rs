//! Another agent's message to this one (plans/agent-messages.md): it wakes the agent at most once.
//! Into the turn the agent runs in the chat, read as the person's message sent into a turn is;
//! else into its turn queued there that hasn't started; else one turn of its own, answering the
//! sender's, which a pause holds as it holds every turn Gathr queues.

use gathr::event::Event;
use gathr::tasks::Start;
use serde_json::json;

use super::{Ids, Task};

impl Task {
    pub(super) async fn take_message(
        &mut self,
        chat: String,
        ask: String,
        answers: String,
        copy_of: String,
    ) -> &'static str {
        let running = self
            .turn
            .as_ref()
            .filter(|t| !t.stopping && t.ids.chat == chat)
            .filter(|t| !self.compactions.contains(&t.ids.turn));
        if let Some(ids) = running.map(|t| t.ids.clone()) {
            let Ok(mut message) = self.db_message(&ids, "gathr", "text", &ask).await else {
                return "failed";
            };
            self.mark_copy(&ids, &mut message, &copy_of).await;
            if self.write_user(&ask, Some(&ids.turn)).await.is_err() {
                return "failed";
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id, ask));
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            return "in_turn";
        }
        let compactions = &self.compactions;
        let waiting = self
            .queue
            .iter_mut()
            .find(|q| q.ids.chat == chat && !compactions.contains(&q.ids.turn));
        if let Some(queued) = waiting {
            queued.text = format!("{}\n\n{ask}", queued.text);
            let ids = queued.ids.clone();
            let Ok(mut message) = self.db_message(&ids, "gathr", "text", &ask).await else {
                return "failed";
            };
            self.mark_copy(&ids, &mut message, &copy_of).await;
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            return "joined";
        }
        let Some(agent) = self.agent.clone() else {
            return "failed";
        };
        let (c, text) = (chat.clone(), ask.clone());
        let saved = self
            .chats
            .writer
            .call(move |conn| Start::queue(conn, (&c, &agent), (None, Some(&answers)), text))
            .await;
        let Some(Ok(mut start)) = saved else {
            return "failed";
        };
        let marking = Ids {
            chat: start.chat.clone(),
            turn: start.turn.clone(),
            trace: start.trace.clone(),
            agent: None,
            task: None,
            answers: None,
            person: false,
        };
        self.mark_copy(&marking, &mut start.message, &copy_of).await;
        let ids = Ids {
            chat: start.chat.clone(),
            turn: start.turn.clone(),
            trace: start.trace.clone(),
            agent: Some(start.agent.clone()),
            task: None,
            answers: start.answers.clone(),
            person: false,
        };
        // What holds it, by name: an update, or the pause of the app, the chat or the agent (G163).
        let pauses = self.chats.pauses();
        let pause = gathr::pauses::holding(&pauses, &ids.chat, ids.agent.as_deref());
        let held = match (self.holder(&ids), pause.map(|p| p.scope.as_str())) {
            (Some("update"), _) => "held_update",
            (Some(_), Some("app")) => "held_app",
            (Some(_), Some("agent")) => "held_agent",
            (Some(_), _) => "held",
            (None, _) => "queued",
        };
        self.given(start).await;
        held
    }

    /// Marks what the agent is told as a copy of the sender's message, saved and as sent live, so
    /// the chat shows the words once. Only how it's shown: a failure is recorded, never fails delivery.
    async fn mark_copy(&self, ids: &Ids, message: &mut serde_json::Value, copy_of: &str) {
        let (id, of) = (
            message["id"].as_str().unwrap_or_default().to_string(),
            copy_of.to_string(),
        );
        let marked = self
            .chats
            .writer
            .call(move |conn| {
                conn.execute("UPDATE messages SET copy_of = ?2 WHERE id = ?1", [&id, &of])
            })
            .await;
        let why = match marked {
            Some(Ok(_)) => return message["copy_of"] = json!(copy_of),
            Some(Err(e)) => e.to_string(),
            None => "the database didn't answer".to_string(),
        };
        self.record(
            Event::point("gathr.message.copy")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("sqlite")
                .attr("why", why),
        );
    }
}
