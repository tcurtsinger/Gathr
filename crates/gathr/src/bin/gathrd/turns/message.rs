//! Another agent's message to this one (plans/agent-messages.md): it wakes the agent at most once.
//! Into the turn the agent runs in the chat, read as the person's message sent into a turn is;
//! else into its turn queued there that hasn't started; else one turn of its own, answering the
//! sender's, which a pause holds as it holds every turn Gathr queues.

use gathr::tasks::Start;
use serde_json::json;

use super::{Ids, Task};

impl Task {
    pub(super) async fn take_message(
        &mut self,
        chat: String,
        ask: String,
        answers: String,
    ) -> &'static str {
        let running = self
            .turn
            .as_ref()
            .filter(|t| !t.stopping && t.ids.chat == chat);
        if let Some(ids) = running.map(|t| t.ids.clone()) {
            let Ok(message) = self.db_message(&ids, "gathr", "text", &ask).await else {
                return "failed";
            };
            if self.write_user(&ask, Some(&ids.turn)).await.is_err() {
                return "failed";
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id, ask));
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            return "in_turn";
        }
        let waiting = self
            .queue
            .iter_mut()
            .find(|q| q.ids.chat == chat && !q.ids.person);
        if let Some(queued) = waiting {
            queued.text = format!("{}\n\n{ask}", queued.text);
            let ids = queued.ids.clone();
            let Ok(message) = self.db_message(&ids, "gathr", "text", &ask).await else {
                return "failed";
            };
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
        let Some(Ok(start)) = saved else {
            return "failed";
        };
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
}
