//! The catch on the runner (0063 I2a, I2b): a held turn's worktree is looked at when it starts and
//! when it ends, and what it changed is said in its turn and told in its agent's next opening. A
//! look runs git as Gathr's other read-only looks do, never in the agent's shell, and a look that
//! fails is recorded and finds nothing: it never fails the turn.

use std::time::Instant;

use gathr::event::Event;
use gathr::hold::look::{self, Ending, Failed, ToLook};
use gathr::platform::env::{self, Vars};
use serde_json::json;

use super::{Ids, Task};

impl Task {
    /// A turn the runner starts: its look, then what the agent's last held turn changed, told
    /// before Gathr's text while this turn is held (I2b.5).
    pub(super) async fn held_start(&self, ids: &Ids, text: String) -> String {
        self.look_before(ids).await;
        let caller = self
            .pass
            .as_ref()
            .and_then(|p| self.chats.passes.caller(&p.token));
        let (Some(agent), Some(caller)) = (ids.agent.clone(), caller) else {
            return text;
        };
        let (turn, chat) = (ids.turn.clone(), ids.chat.clone());
        let told = move |c: &mut rusqlite::Connection| {
            look::untold(c, &turn, &chat, &agent, &caller.place())
        };
        match self.chats.writer.call(told).await {
            Some(Ok(Some(words))) => format!("{words}\n\n{text}"),
            Some(Ok(None)) => text,
            failed => {
                let event = Event::point("gathr.hold.told")
                    .trace(&ids.trace)
                    .chat(&ids.chat);
                self.record(event.error("sqlite").attr("why", format!("{failed:?}")));
                text
            }
        }
    }

    /// A held turn's start (I2a.1): the agent's turns a stop cut off are looked at first, against
    /// the worktree now (I2a.7), then this one's start is kept.
    pub(super) async fn look_before(&self, ids: &Ids) {
        let Some(agent) = ids.agent.clone() else {
            return;
        };
        let (chat, turn) = (ids.chat.clone(), ids.turn.clone());
        let pending = move |c: &mut rusqlite::Connection| look::pending(c, &chat, &agent, &turn);
        match self.chats.writer.call(pending).await {
            Some(Ok(cut)) => {
                for turn in cut {
                    self.look_at_end(&turn).await;
                }
            }
            failed => self.unread(ids, "start", format!("{failed:?}")),
        }
        let turn = ids.turn.clone();
        let l = match self
            .chats
            .writer
            .call(move |c| look::to_look(c, &turn))
            .await
        {
            Some(Ok(Some(l))) => l,
            Some(Ok(None)) => return,
            failed => return self.unread(ids, "start", format!("{failed:?}")),
        };
        let started = Instant::now();
        let env = git_env().await;
        let event = looked(&l, "start");
        match look::seen(&l.worktree, &env).await {
            Ok(seen) => {
                let entries = seen.entries.len();
                let kept = l.clone();
                let saved = self
                    .chats
                    .writer
                    .call(move |c| look::before(c, &kept, &seen));
                match saved.await {
                    Some(Ok(())) => self.record(timed(event, entries, started)),
                    failed => self.record(event.error("sqlite").attr("why", format!("{failed:?}"))),
                }
            }
            Err(Failed::TooMany(n)) => {
                self.record(event.error("too_many").attr("gathr.hold.entries", n))
            }
            Err(Failed::Git(why)) => self.record(event.error("git").attr("why", why)),
        }
    }

    /// A turn's end (I2a.2): its look, after its end record, its task's move and its live status.
    pub(super) async fn look_after(&self, ids: &Ids) {
        self.look_at_end(&ids.turn).await;
    }

    /// The end look of a held turn with a start record, asking the rule again: a turn released
    /// mid-turn reports nothing.
    async fn look_at_end(&self, turn: &str) {
        let t = turn.to_string();
        let (l, start) = match self.chats.writer.call(move |c| look::ending(c, &t)).await {
            Some(Ok(Some(Ending::Look(l, start)))) => (l, start),
            Some(Ok(Some(Ending::Released(l)))) => {
                let event = looked(&l, "end").ok().attr("gathr.hold.released", true);
                return self.record(event);
            }
            Some(Ok(None)) => return,
            failed => {
                let event = Event::point("gathr.hold.look").attr("gathr.turn.id", turn);
                let event = event.attr("gathr.hold.at", "end").error("sqlite");
                return self.record(event.attr("why", format!("{failed:?}")));
            }
        };
        let started = Instant::now();
        let env = git_env().await;
        let event = looked(&l, "end");
        let (found, entries) = match look::changes(&l.worktree, &env, &start).await {
            Ok((changed, entries)) => (Ok(changed), entries),
            Err(why) => (Err(why), 0),
        };
        let (kept, changed) = (l.clone(), found.as_ref().ok().cloned());
        let saved = self
            .chats
            .writer
            .call(move |c| look::after(c, &kept, changed.as_ref()));
        match (saved.await, found) {
            (Some(Ok(line)), Ok(_)) => {
                self.record(timed(event, entries, started));
                if let Some(message) = line {
                    self.publish(&l.chat, json!({ "type": "message", "message": message }));
                }
            }
            (Some(Ok(_)), Err(why)) => self.record(event.error("git").attr("why", why)),
            (failed, _) => self.record(event.error("sqlite").attr("why", format!("{failed:?}"))),
        }
    }

    /// A start look that couldn't read the database.
    fn unread(&self, ids: &Ids, at: &str, why: String) {
        let event = Event::point("gathr.hold.look")
            .trace(&ids.trace)
            .chat(&ids.chat);
        let event = event
            .attr("gathr.turn.id", ids.turn.as_str())
            .attr("gathr.hold.at", at);
        self.record(event.error("sqlite").attr("why", why));
    }
}

/// `gathr.hold.look` for the turn, at its `start` or `end`.
fn looked(l: &ToLook, at: &str) -> Event {
    Event::point("gathr.hold.look")
        .trace(&l.trace)
        .chat(&l.chat)
        .attr("gathr.turn.id", l.turn.as_str())
        .attr("gen_ai.agent.id", l.agent.as_str())
        .attr("gathr.hold.at", at)
}

/// A look that worked: how many entries it read and how long it took (I2a.6).
fn timed(event: Event, entries: usize, started: Instant) -> Event {
    let ms = started.elapsed().as_millis() as u64;
    let event = event.ok().attr("gathr.hold.entries", entries);
    event.attr("gathr.hold.duration_ms", ms)
}

/// The environment the looks run git with.
async fn git_env() -> Vars {
    let read = tokio::task::spawn_blocking(|| env::fresh(std::env::vars_os().collect()).vars);
    look::env(read.await.unwrap_or_default())
}
