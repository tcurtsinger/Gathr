//! A chat archived or deleted (F11, 0010): the database part already ended its tasks, waits and
//! queued turns, so each agent's queue drops that chat's turns, interrupts the one it runs there and
//! ends its engine's background commands by the engine's own stop. For a delete, the engine also
//! closes the gentle way, so the chat's worktrees can go once it has exited.

use gathr::claude::frames;
use gathr::ids::span_id;
use tokio::sync::oneshot;

use super::Task;

impl Task {
    pub(super) async fn end_chat(&mut self, chat: &str, close: bool, done: oneshot::Sender<()>) {
        self.queue.retain(|q| q.ids.chat != chat);
        let here = self.assigned.as_ref().is_some_and(|a| a.chat == chat);
        if here && self.engine.is_some() {
            let (c, agent) = (chat.to_string(), self.agent.clone());
            let commands = self
                .chats
                .writer
                .call(move |conn| gathr::background::of(conn, &c, agent.as_deref()));
            for (id, _, _) in commands.await.and_then(Result::ok).unwrap_or_default() {
                let request = frames::stop_task(&format!("end-chat-{}", span_id()), &id);
                let _ = self.write(&request, None).await;
            }
            self.interrupt(chat).await;
            if close {
                self.close("its chat was deleted").await;
                if let Some(running) = self.turn.take() {
                    self.set_turn(&running.ids, "stopped", Some("its chat was deleted"))
                        .await;
                }
            }
        }
        match &self.closing {
            Some(closing) if close && closing.chat == chat => self.leaving.push(done),
            _ => _ = done.send(()),
        }
    }
}
