//! The conversation each turn runs in: the agent's latest in the chat. An engine resumes it when it
//! opens again, and a refused resume opens a new one.

use gathr::event::Event;
use gathr::redact::redact;

use super::{Assignment, Ids, Queued, Task};

/// The conversation the running engine was opened in.
#[derive(Default)]
pub(super) struct Session {
    /// The id it resumed, or the one its first Init reported (N3).
    pub id: Option<String>,
    /// It resumed `id` and no Init has come yet: a turn that fails now was refused the id (N8).
    resuming: bool,
    /// The turn put back after a refused resume: a second refusal fails it.
    requeued: Option<String>,
}

impl Session {
    /// The engine said its session.
    pub fn init(&mut self, id: &str) {
        self.id.get_or_insert_with(|| id.to_string());
        self.resuming = false;
    }
}

impl Task {
    /// The session a start resumes: the agent's latest in the chat.
    pub(super) async fn resumes(&self, assigned: &Assignment) -> Option<String> {
        let a = assigned.clone();
        let read = move |conn: &mut rusqlite::Connection| {
            gathr::store::engine_session(conn, &a.chat, a.agent.as_deref(), &a.account)
        };
        self.chats
            .writer
            .call(read)
            .await
            .and_then(Result::ok)
            .flatten()
    }

    /// The engine started in `resumed` (none: a new one).
    pub(super) fn opened(&mut self, resumed: Option<String>) {
        self.session.resuming = resumed.is_some();
        self.session.id = resumed;
    }

    /// What a turn is sent before its ask, never saved: what the chat said that the agent hasn't
    /// heard (0081). A new conversation, or one a compaction summed up, hears the chat's last lines.
    pub(super) async fn opening(&mut self, next: &Queued, fresh: bool, text: String) -> (String, bool) {
        if self.compactions.contains(&next.ids.turn) {
            return (text, false);
        }
        let new = fresh && !self.session.resuming;
        let summed = self.summed(None, &next.ids).await;
        let heard = self.heard(&next.ids, new || summed).await;
        let said = [heard, Some(text)].into_iter().flatten();
        (said.collect::<Vec<_>>().join("\n\n"), summed)
    }

    /// Read the pending marker, or set it after compaction and clear it after a restored send.
    pub(super) async fn summed(&self, mark: Option<bool>, ids: &Ids) -> bool {
        let (Some(session), Some(assigned)) = (self.session.id.clone(), self.assigned.as_ref())
        else {
            return false;
        };
        let (chat, agent, account) = (
            assigned.chat.clone(),
            assigned.agent.clone().unwrap_or_default(),
            assigned.account.clone(),
        );
        let read = self
            .chats
            .writer
            .call(move |c| {
                if let Some(mark) = mark {
                    gathr::sessions::summed(c, &chat, &agent, &account, &session, mark)?;
                    Ok(mark)
                } else {
                    gathr::sessions::is_summed(c, &chat, &agent, &account, &session)
                }
            })
            .await;
        match read {
            Some(Ok(summed)) => summed,
            failed => {
                self.record(
                    Event::point("gathr.context.summed")
                        .trace(&ids.trace)
                        .chat(&assigned.chat)
                        .error("sqlite")
                        .attr("why", format!("{failed:?}")),
                );
                false
            }
        }
    }

    /// The turn failed before any Init on an engine that resumed a conversation: the engine
    /// refused it (N8). This agent's rows holding it go, the engine closes, and the turn goes back
    /// first in the queue, once, to start new. Whether it did.
    pub(super) async fn refused(&mut self, ids: &Ids, why: &str) -> bool {
        let session = self.session.id.clone().filter(|_| self.session.resuming);
        let again = self.session.requeued.as_deref() == Some(ids.turn.as_str());
        let Some(session) = session.filter(|_| !again) else {
            return false;
        };
        self.session.resuming = false;
        self.session.requeued = Some(ids.turn.clone());
        let (chat, agent, id) = (ids.chat.clone(), ids.agent.clone(), session.clone());
        let forget = move |c: &mut rusqlite::Connection| {
            gathr::sessions::forget(c, &chat, agent.as_deref(), &id)
        };
        self.saved_row(self.chats.writer.call(forget).await);
        let event = Event::point("gathr.engine.resume_failed")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .error("resume_failed")
            .attr("session", session)
            .attr("why", redact(why.trim()));
        self.record(event);
        // The refused engine is exiting, or ended its turn and waits for input: the retry must not
        // be written into it. Its close ending starts the next turn.
        self.close("it couldn't resume its conversation").await;
        if !self.queue.iter().any(|q| q.ids.turn == ids.turn) {
            let text = self.text_of(&ids.turn).await;
            let ids = ids.clone();
            self.queue.push_front(Queued {
                ids,
                text,
                held: None,
            });
        }
        self.set_turn(ids, "queued", None).await;
        true
    }

    /// A row write that failed, recorded as Init's own write is.
    fn saved_row(&self, written: Option<Result<(), gathr::Error>>) {
        if let Some(Err(e)) = written {
            let event = Event::point("gathr.engine.session").error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
    }
}
