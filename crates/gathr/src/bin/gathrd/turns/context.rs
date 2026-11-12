//! How full the agent's context is in its chat (compaction plan, K1): each main-session call's
//! prompt and its model's window, saved, then sent live and shown on the chat's members.

use gathr::event::Event;
use serde_json::{Value, json};

use super::Task;

impl Task {
    pub(super) async fn context_filled(&mut self, used: i64, model: Option<String>) {
        if let Some(model) = model {
            self.ran_on(model).await;
        }
        self.save_context(move |c, chat, agent| gathr::context::filled(c, chat, agent, used))
            .await;
    }

    /// The window of the model the agent's calls run on; Codex lists one, unnamed: that one.
    pub(super) async fn context_window(&mut self, windows: Vec<(Option<String>, i64)>) {
        let own = match windows.as_slice() {
            [(None, window)] => Some(*window),
            _ => windows
                .iter()
                .find(|(model, _)| model.is_some() && *model == self.context_model)
                .map(|(_, window)| *window),
        };
        let set = self.assigned.as_ref().map(super::Assignment::model_key);
        if let (Some(window), Some(set)) = (own, set) {
            let ran = self.context_model.clone();
            let save = move |c: &_, chat: &str, agent: &str| {
                gathr::context::window(c, chat, agent, window, &set, ran.as_deref())
            };
            self.save_context(save).await;
        }
    }

    /// The engine named the model the agent's calls run on. When its compaction point was for
    /// another (a Latest or the engine's default moved), no window seen on that one gives a point
    /// again, and the engine closes before its next turn, to start without it.
    pub(super) async fn ran_on(&mut self, model: String) {
        // Claude Code names a message it wrote itself `<synthetic>`: no model ran.
        if model.is_empty() || model.starts_with('<') {
            return;
        }
        let was = self.point_model.take_if(|was| *was != model);
        if let (Some(was), Some(agent), Some(assigned)) =
            (was, self.agent.clone(), self.assigned.as_ref())
        {
            self.model_moved = true;
            let (set, ran) = (assigned.model_key(), model.clone());
            let mut event = Event::point("gathr.context.moved").chat(&assigned.chat);
            if let Some(turn) = &self.turn {
                event = event.trace(&turn.ids.trace);
            }
            event = event.attr("gen_ai.agent.id", agent.clone());
            let forget =
                move |c: &mut rusqlite::Connection| gathr::context::moved(c, &agent, &set, &ran);
            let event = match self.chats.writer.call(forget).await {
                Some(Ok(())) => event.ok(),
                failed => event.error("sqlite").attr("why", format!("{failed:?}")),
            };
            self.record(event.attr("from", was).attr("to", model.clone()));
        }
        self.context_model = Some(model);
    }

    /// The engine compacted the conversation: the fill after it, when said, and a quiet line in the
    /// turn it ran in (compaction plan, point 6).
    pub(super) async fn compacted(
        &mut self,
        trigger: String,
        before: Option<i64>,
        after: Option<i64>,
    ) {
        // Compacted, the agent may be told again as the next point nears (K3).
        if let (Some(agent), Some(chat)) = (
            self.agent.clone(),
            self.assigned.as_ref().map(|a| a.chat.clone()),
        ) {
            let reset =
                move |c: &mut rusqlite::Connection| gathr::context::untold(c, &chat, &agent);
            let _ = self.chats.writer.call(reset).await;
        }
        if let Some(after) = after {
            self.save_context(move |c, chat, agent| gathr::context::filled(c, chat, agent, after))
                .await;
        }
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        turn.compacted = true;
        let ids = turn.ids.clone();
        // Claude Code says how its compaction began; Codex doesn't, so Compact says so (K4).
        let trigger = match self.compactions.contains(&ids.turn) {
            true => "manual".to_string(),
            false => trigger,
        };
        let mut event = Event::point("gathr.context.compact")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .ok();
        event = event.attr("trigger", trigger);
        if let (Some(before), Some(after)) = (before, after) {
            event = event.attr("before", before).attr("after", after);
        }
        self.record(event);
        let Some(agent) = self.agent.clone() else {
            return;
        };
        let line = self
            .chats
            .writer
            .call(move |c| gathr::context::compacted_line(c, &agent, before, after))
            .await;
        let Some(Ok(line)) = line else {
            let why = format!("the line wasn't written: {line:?}");
            let event = Event::point("gathr.context.compact")
                .trace(&ids.trace)
                .chat(&ids.chat);
            return self.record(event.error("sqlite").attr("why", why));
        };
        if let Ok(message) = self.db_message(&ids, "gathr", "text", &line).await {
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        }
    }

    /// Whether the turn starting now is told its conversation nears where it compacts (K3): once,
    /// at 75% of the point, again only after a compaction or a failed turn. Recorded when it is.
    pub(super) async fn near_compaction(&self, ids: &super::Ids) -> bool {
        let set = self.assigned.as_ref().map(super::Assignment::model_key);
        let (Some(agent), Some(set)) = (ids.agent.clone(), set) else {
            return false;
        };
        let chat = ids.chat.clone();
        let told = self
            .chats
            .writer
            .call(move |c| gathr::context::tell_soon(c, &chat, &agent, &set));
        let told = told.await.and_then(Result::ok).unwrap_or(false);
        if told {
            let event = Event::point("gathr.context.soon")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .ok();
            self.record(event.attr("gen_ai.agent.id", ids.agent.clone()));
        }
        told
    }

    /// A failed turn may not have saved what it was told to: it's told again.
    pub(super) async fn tell_again(&self, ids: &super::Ids) {
        let (Some(agent), chat) = (ids.agent.clone(), ids.chat.clone()) else {
            return;
        };
        let reset = move |c: &mut rusqlite::Connection| gathr::context::untold(c, &chat, &agent);
        let _ = self.chats.writer.call(reset).await;
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
