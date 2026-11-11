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
            .filter(|t| !t.stopping && t.ids.chat == chat);
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
        let waiting = self.queue.iter_mut().find(|q| q.ids.chat == chat);
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
        let held = self.holder(&ids).is_some();
        self.given(start).await;
        if held { "held" } else { "queued" }
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
