//! Why an agent's queued turns haven't started, said for whoever asks what a chat waits on (0050).
//! A pause, an update, usage and a missing engine are saved on the turn. The rest lives only here,
//! in the agent's one queue across all its chats: the turn it is behind, an engine that is closing,
//! a message its engine hasn't read yet, a compaction waiting for background commands.

use std::collections::HashMap;

use serde_json::json;

use super::{Ids, Task};

/// Each queued turn's chat and why it waits.
pub(super) type Said = HashMap<String, (String, String)>;

impl Task {
    /// Tells the chats why each queued turn waits, when that has changed since it last did, and
    /// each chat it changed for that its answer may have.
    pub(super) fn say_waits(&mut self) {
        let now = self.queued_whys();
        if now == self.said_waits {
            return;
        }
        let was = std::mem::replace(&mut self.said_waits, now);
        let mut chats: Vec<&String> = Vec::new();
        for (turn, (chat, why)) in &was {
            if self.said_waits.get(turn).map(|(_, now)| now) != Some(why) {
                self.chats.say_queued(turn, None);
                chats.push(chat);
            }
        }
        for (turn, said) in &self.said_waits {
            if was.get(turn) != Some(said) {
                self.chats.say_queued(turn, Some(said.clone()));
                chats.push(&said.0);
            }
        }
        chats.sort();
        chats.dedup();
        for chat in chats {
            self.chats
                .publish(json!({ "type": "waiting", "chat": chat }));
        }
    }

    /// Why each queued turn that nothing saved holds hasn't started. The one next to start has no
    /// reason to give.
    fn queued_whys(&self) -> Said {
        let behind = |ids: &Ids, of: &Ids, what: &str| {
            let on = of.task.map_or(String::new(), |n| format!(" on #{n}"));
            let place = match of.chat == ids.chat {
                true => "",
                false => " in another chat",
            };
            format!("behind this agent's {what}{on}{place}")
        };
        let mut whys = Said::new();
        let mut first: Option<&Ids> = None;
        for q in &self.queue {
            // A pause or an update is saved on the turn, and holds no other turn back.
            if self.holder(&q.ids).is_some() {
                continue;
            }
            // So is usage or a missing engine, but the turns after it wait for it.
            if q.held.is_some() {
                first = first.or(Some(&q.ids));
                continue;
            }
            let compaction = self.background > 0 && self.compactions.contains(&q.ids.turn);
            let why = if let Some(running) = &self.turn {
                behind(&q.ids, &running.ids, "turn")
            } else if self.closing.is_some() {
                "its engine is closing".to_string()
            } else if self.awaiting_read() {
                "its engine hasn't yet read a message sent into its last turn".to_string()
            } else if let Some(ahead) = first {
                behind(&q.ids, ahead, "queued turn")
            } else if compaction {
                "it compacts the conversation once this agent's background commands end".to_string()
            } else {
                first = Some(&q.ids);
                continue;
            };
            first = first.or(Some(&q.ids));
            whys.insert(q.ids.turn.clone(), (q.ids.chat.clone(), why));
        }
        whys
    }
}
