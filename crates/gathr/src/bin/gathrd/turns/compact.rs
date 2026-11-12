//! Compact, in the agent panel's More menu (compaction plan, K4): a turn of its own in the agent's
//! queue, so it waits for the turn under way and the engine's background commands, and a turn sent
//! meanwhile waits for it. Claude Code runs `/compact` in an engine started with its slash commands,
//! which agents' own engines keep off; Codex compacts the thread it has open, as a turn of its own.
//! The turn holds no ask: one saved queued with none is a compaction when the service starts again.

use gathr::claude::frames::TurnEnd;
use gathr::event::Event;
use gathr::ids::{now_us, ulid};
use gathr::store;
use serde_json::{Value, json};

use super::{Ids, Queued, Task};
use crate::chats::Failed;

/// What Claude Code is sent; Codex is asked with its own request instead.
pub(super) const COMMAND: &str = "/compact";

impl Task {
    /// Compacts the agent's conversation in `chat` now, or queues it: its turn and status, or why
    /// it can't.
    pub(super) async fn compact(&mut self, chat: String, trace: String) -> Result<Value, Failed> {
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace,
            agent: self.agent.clone(),
            task: None,
            answers: None,
            person: true,
        };
        let queued = Queued {
            ids: ids.clone(),
            text: COMMAND.into(),
            held: None,
        };
        let refused = |why: String| Failed {
            why,
            trace: ids.trace.clone(),
        };
        if let Some(why) = self.cannot_compact(&queued).await {
            let event = Event::point("gathr.context.compact").trace(&ids.trace);
            self.record(
                event
                    .chat(&ids.chat)
                    .error("refused")
                    .attr("why", why.clone()),
            );
            return Err(refused(why));
        }
        let row = ids.clone();
        let added = self.chats.writer.call(move |conn| {
            let by = (row.agent.as_deref(), None, None);
            store::add_turn(conn, &row.chat, &row.turn, &row.trace, by)
        });
        if let Some(Err(e)) = added.await {
            return Err(refused(e.to_string()));
        }
        self.compactions.insert(ids.turn.clone());
        let ahead = self.queue.iter().any(|q| self.holder(&q.ids).is_none());
        let busy = self.turn.is_some() || self.closing.is_some() || self.background > 0;
        let status = match busy || ahead {
            true => {
                self.queue.push_back(queued);
                self.set_turn(&ids, "queued", None).await;
                "queued"
            }
            false => match self.start(&queued).await {
                Ok(status) => status,
                Err(why) => {
                    self.compactions.remove(&ids.turn);
                    self.set_turn(&ids, "failed", Some(&why)).await;
                    return Err(refused(why));
                }
            },
        };
        let reference = gathr::ids::reference(&ids.trace).to_string();
        Ok(json!({ "turn": ids.turn, "status": status, "ref": reference }))
    }

    /// Why the conversation can't compact: there's none yet, or it's compacting already.
    async fn cannot_compact(&self, queued: &Queued) -> Option<String> {
        let chat = &queued.ids.chat;
        let compacting = |ids: &Ids| &ids.chat == chat && self.compactions.contains(&ids.turn);
        let running = self.turn.as_ref().map(|t| &t.ids);
        if running
            .into_iter()
            .chain(self.queue.iter().map(|q| &q.ids))
            .any(compacting)
        {
            return Some("its conversation is compacting already".into());
        }
        // A first turn under way or waiting saves its conversation before the compaction runs.
        let running = self.turn.as_ref().map(|t| &t.ids);
        if running
            .into_iter()
            .chain(self.queue.iter().map(|q| &q.ids))
            .any(|ids| &ids.chat == chat)
        {
            return None;
        }
        let assigned = match self.assignment(queued).await {
            Ok(Some(assigned)) => assigned,
            Ok(None) => return Some("there's no account to run it on".into()),
            Err(why) => return Some(why),
        };
        let session = self.chats.writer.call(move |conn| {
            let agent = assigned.agent.as_deref();
            store::engine_session(conn, &assigned.chat, agent, &assigned.account)
        });
        match session.await {
            Some(Ok(Some(_))) => None,
            Some(Ok(None)) => Some("there's no conversation to compact yet".into()),
            Some(Err(e)) => Some(e.to_string()),
            None => Some("the database didn't answer".into()),
        }
    }

    pub(super) async fn write_compact(&mut self, turn: &str) -> std::io::Result<()> {
        match self.driver.compact() {
            Some(frame) => self.write(&frame, Some(turn)).await,
            None => Ok(()),
        }
    }

    /// Why a compaction failed, as its turn says: "Couldn't compact Ann's conversation: …".
    pub(super) async fn not_compacted(&self, ids: &Ids, end: &TurnEnd) -> String {
        let said = end.message.clone().filter(|m| !m.trim().is_empty());
        let said = said.unwrap_or_else(|| "the engine ended without compacting it".into());
        let agent = ids.agent.clone().unwrap_or_default();
        let found = self
            .chats
            .writer
            .call(move |c| gathr::agents::get(c, &agent));
        let name = match found.await {
            Some(Ok(Some(agent))) => agent.name,
            _ => "the agent".into(),
        };
        format!("Couldn't compact {name}'s conversation: {said}")
    }
}
