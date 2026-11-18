//! Which conversation each turn runs in (0055): a member's task in its own, kept in
//! `task_sessions`; every other turn in the agent's latest. An engine holds one conversation, so a
//! turn on another closes it first, the gentle way.

use gathr::event::Event;
use gathr::redact::redact;
use gathr::sessions::{Conversation, Key};
use serde_json::json;

use super::{Assignment, Ids, Queued, Task};

/// The conversation the running engine was opened in.
#[derive(Default)]
pub(super) struct Session {
    /// The id it resumed, or the one its first Init reported (N3).
    id: Option<String>,
    /// The task it was opened for: each Init writes that task's row (N5).
    pub task: Option<i64>,
    /// It resumed `id` and no Init has come yet: a turn that fails now was refused the id (N8).
    resuming: bool,
    /// The turn put back after a refused resume: a second refusal fails it.
    requeued: Option<String>,
}

impl Session {
    /// The engine said its session: the task whose row takes it (N5).
    pub fn init(&mut self, id: &str) -> Option<i64> {
        self.id.get_or_insert_with(|| id.to_string());
        self.resuming = false;
        self.task
    }
}

pub(super) fn task_of(choice: &Conversation) -> Option<i64> {
    match choice {
        Conversation::Latest => None,
        Conversation::New(task) | Conversation::Resume { task, .. } => Some(*task),
    }
}

impl Task {
    /// The turn's key and the conversation it gives (N1, N2). With no account yet, or a turn the
    /// database can't place, it runs on Latest, as every turn did before.
    pub(super) async fn conversation(
        &self,
        ids: &Ids,
        assigned: Option<&Assignment>,
    ) -> (Key, Conversation) {
        let latest = (Key::Latest, Conversation::Latest);
        let (Some(agent), Some(assigned)) = (ids.agent.clone(), assigned) else {
            return latest;
        };
        let (turn, account) = (ids.turn.clone(), assigned.account.clone());
        let chose = self.chats.writer.call(move |conn| {
            let key = gathr::sessions::want(conn, &turn)?;
            let chose = gathr::sessions::choose(conn, &agent, &account, key.clone())?;
            Ok::<_, gathr::Error>((key, chose))
        });
        match chose.await {
            Some(Ok(found)) => found,
            failed => {
                self.saved_row(failed.map(|r| r.map(|_| ())));
                latest
            }
        }
    }

    /// A running engine closes when the turn is on another conversation than the one it opened
    /// (N3): the turn waits, first in the queue, with its status. Under a background command, which
    /// the close would end, the turn runs where the engine is (N4).
    pub(super) async fn switch(
        &mut self,
        next: &Queued,
        choice: &Conversation,
    ) -> Option<&'static str> {
        let other = match choice {
            Conversation::Latest => false,
            Conversation::New(_) => true,
            Conversation::Resume { session, .. } => self.session.id.as_ref() != Some(session),
        };
        if self.engine.is_none() || !other {
            return None;
        }
        if self.background == 0 {
            self.close("the next turn is on another task").await;
            return Some(self.hold_back(next, "queued").await);
        }
        let open = self.session.id.clone();
        let event = Event::point("gathr.session.kept")
            .trace(&next.ids.trace)
            .chat(&next.ids.chat)
            .ok()
            .attr("gathr.task.id", task_of(choice))
            .attr("session", open.clone())
            .attr("why", "background");
        self.record(event);
        // A new task's later turns come back to the conversation that holds its first; a resumed
        // task's row stays its own.
        if let (Conversation::New(task), Some(open), Some(agent), Some(a)) =
            (choice, open, next.ids.agent.clone(), self.assigned.as_ref())
        {
            let (task, chat, account) = (*task, a.chat.clone(), a.account.clone());
            let set = move |c: &mut rusqlite::Connection| {
                gathr::sessions::set(c, task, &agent, &chat, &account, &open)
            };
            self.saved_row(self.chats.writer.call(set).await);
        }
        None
    }

    /// The session a start resumes: the chosen task's, none for a new one, or the agent's latest
    /// in the chat, as before.
    pub(super) async fn resumes(
        &self,
        assigned: &Assignment,
        choice: &Conversation,
    ) -> Option<String> {
        match choice {
            Conversation::New(_) => None,
            Conversation::Resume { session, .. } => Some(session.clone()),
            Conversation::Latest => {
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
        }
    }

    /// The engine started in `resumed` (none: a new one): what it was opened for, and the context
    /// shown follows a task's conversation (N7). Latest leaves it as it is.
    pub(super) async fn opened(&mut self, choice: &Conversation, resumed: Option<String>) {
        self.session.resuming = resumed.is_some();
        self.session.id = resumed;
        self.session.task = task_of(choice);
        let from = match choice {
            Conversation::Latest => return,
            Conversation::New(_) => None,
            Conversation::Resume { task, session, .. } => Some((*task, session.clone())),
        };
        let (Some(agent), Some(chat)) = (self.agent.clone(), self.assigned.as_ref()) else {
            return;
        };
        let chat = chat.chat.clone();
        let (c, a) = (chat.clone(), agent.clone());
        let switched = move |conn: &mut rusqlite::Connection| {
            let from = from
                .as_ref()
                .map(|(task, session)| (*task, session.as_str()));
            gathr::context::switched(conn, &c, &a, from)
        };
        match self.chats.writer.call(switched).await {
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

    /// A new conversation's first turn is sent the opening before its ask (N6), never saved.
    pub(super) async fn opening(
        &self,
        next: &Queued,
        choice: &Conversation,
        fresh: bool,
        text: String,
    ) -> String {
        let text = self.held_start(&next.ids, text).await;
        let (true, Conversation::New(task), Some(agent)) = (fresh, choice, next.ids.agent.clone())
        else {
            return text;
        };
        let (task, ask) = (*task, next.text.clone());
        let made = self
            .chats
            .writer
            .call(move |c| gathr::tasks::opening(c, task, &agent, &ask))
            .await;
        let event = Event::point("gathr.session.opening")
            .trace(&next.ids.trace)
            .chat(&next.ids.chat)
            .attr("gathr.task.id", task);
        let Some(Ok((opening, count))) = made else {
            self.record(event.error("sqlite").attr("why", format!("{made:?}")));
            return text;
        };
        let chars = opening.chars().count();
        let event = event.ok().attr("gathr.task.count", count);
        self.record(event.attr("gathr.message.chars", chars));
        format!("{opening}\n\n{text}")
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
            .attr("gathr.task.id", self.session.task)
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
