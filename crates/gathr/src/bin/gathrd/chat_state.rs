//! Each chat's state for the chat list (0040): whether a turn of it runs, and whether it needs the
//! person. Worked out on the writer's thread, so changes go out in the order they happened, and
//! sent live as `chat_state` only when it differs from what the windows were last told.

use std::collections::HashMap;
use std::sync::Mutex;

use gathr::Error;
use gathr::event::Event;
use gathr::store;
use rusqlite::Connection;
use serde_json::{Value, json};
use tokio::sync::broadcast;

/// Tells every window when the chat started or stopped working, or needing the person. A failure
/// is recorded on `trace`, so the turn's or the request's reference finds it.
pub async fn changed(chats: &crate::chats::Chats, chat: &str, trace: &str) {
    let (states, id) = (chats.states.clone(), chat.to_string());
    let told = chats.writer.call(move |conn| states.changed(conn, &id));
    if let Some(Err(e)) = told.await {
        let event = Event::point("gathr.chat.state").trace(trace).chat(chat);
        let event = event.error("sqlite").attr("why", e.to_string());
        chats.writer.record_now(event).await;
    }
}

pub struct States {
    live: broadcast::Sender<Value>,
    known: Mutex<Known>,
}

#[derive(Default)]
struct Known {
    /// Turns with an approval waiting, each with its chat: approvals are in no table.
    asking: HashMap<String, String>,
    /// What the windows were last told of each chat that works or needs the person.
    sent: HashMap<String, (bool, bool)>,
}

impl States {
    pub fn new(live: broadcast::Sender<Value>) -> States {
        States {
            live,
            known: Mutex::default(),
        }
    }

    /// Notes whether the turn has an approval waiting.
    pub fn asking(&self, turn: &str, chat: &str, open: bool) {
        let mut known = self.known.lock().expect("never poisoned");
        match open {
            true => known.asking.insert(turn.to_string(), chat.to_string()),
            false => known.asking.remove(turn),
        };
    }

    /// Whether an approval waits in the chat.
    fn asked(known: &Known, chat: &str) -> bool {
        known.asking.values().any(|c| c == chat)
    }

    /// Adds the approvals waiting to the list's rows, as `chat_state` counts them.
    pub fn on_list(&self, chats: &mut Value) {
        let known = self.known.lock().expect("never poisoned");
        for chat in chats.as_array_mut().into_iter().flatten() {
            if chat["needs"] == false && Self::asked(&known, chat["id"].as_str().unwrap_or("")) {
                chat["needs"] = true.into();
            }
        }
    }

    /// Works the chat's state out again and tells every window when it changed.
    pub fn changed(&self, conn: &Connection, chat: &str) -> Result<(), Error> {
        let mut known = self.known.lock().expect("never poisoned");
        let (working, needs) = match store::chat_state(conn, chat)? {
            Some((working, needs)) => (working, needs || Self::asked(&known, chat)),
            None => (false, false),
        };
        if known.sent.get(chat).copied().unwrap_or_default() == (working, needs) {
            return Ok(());
        }
        match working || needs {
            true => known.sent.insert(chat.to_string(), (working, needs)),
            false => known.sent.remove(chat),
        };
        let update =
            json!({ "type": "chat_state", "chat": chat, "working": working, "needs": needs });
        let _ = self.live.send(update);
        Ok(())
    }

    /// At start: the windows load each chat's state from the list, so later changes are told from
    /// what the list says now.
    pub fn recover(&self, conn: &Connection) -> Result<(), Error> {
        let chats = store::chats(conn, false)?;
        let mut known = self.known.lock().expect("never poisoned");
        for chat in chats.as_array().into_iter().flatten() {
            let state = (chat["working"] == true, chat["needs"] == true);
            if let (Some(id), true) = (chat["id"].as_str(), state.0 || state.1) {
                known.sent.insert(id.to_string(), state);
            }
        }
        Ok(())
    }
}
