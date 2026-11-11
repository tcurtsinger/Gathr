//! How full the agent's context is in its chat (compaction plan, K1): each main-session call's
//! prompt and its model's window, saved, then sent live and shown on the chat's members.

use gathr::event::Event;
use serde_json::{Value, json};

use super::Task;

impl Task {
    pub(super) async fn context_filled(&mut self, used: i64, model: Option<String>) {
        if model.is_some() {
            self.context_model = model;
        }
        self.save_context(move |c, chat, agent| gathr::context::filled(c, chat, agent, used))
            .await;
    }

    /// The window of the model the agent's calls run on; with one model listed (Codex), that one.
    pub(super) async fn context_window(&mut self, windows: Vec<(Option<String>, i64)>) {
        let own = match windows.as_slice() {
            [(_, window)] => Some(*window),
            _ => windows
                .iter()
                .find(|(model, _)| model.is_some() && *model == self.context_model)
                .map(|(_, window)| *window),
        };
        if let Some(window) = own {
            self.save_context(move |c, chat, agent| gathr::context::window(c, chat, agent, window))
                .await;
        }
    }

    async fn save_context(
        &self,
        save: impl FnOnce(&rusqlite::Connection, &str, &str) -> Result<Value, gathr::Error>
        + Send
        + 'static,
    ) {
        let (Some(agent), Some(assigned)) = (self.agent.clone(), self.assigned.as_ref()) else {
            return;
        };
        let chat = assigned.chat.clone();
        let (c, a) = (chat.clone(), agent.clone());
        match self.chats.writer.call(move |conn| save(conn, &c, &a)).await {
            Some(Ok(context)) => self.publish(
                &chat,
                json!({ "type": "context", "agent": agent, "context": context }),
            ),
            failed => self.record(
                Event::point("gathr.context.fill")
                    .chat(&chat)
                    .error("sqlite")
                    .attr("why", format!("{failed:?}")),
            ),
        }
    }
}
