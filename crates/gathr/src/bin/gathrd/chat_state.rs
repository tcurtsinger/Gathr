//! Each chat's state for the chat list (0040): whether a turn of it runs, and whether it needs the
//! person. Worked out on the writer's thread, so changes go out in the order they happened, and
//! sent live as `chat_state` only when it differs from what the windows were last told.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use gathr::Error;
use gathr::event::Event;
use gathr::store::{self, ChatState};
use rusqlite::Connection;
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

/// Notes, on the writer's connection, each chat whose state a write may have changed: a turn added
/// or moved, a question asked or answered. Temporary triggers note it in the same transaction, so
/// whichever code made the change, it's noted, and a change rolled back notes nothing.
pub fn watch(conn: &Connection) -> Result<(), Error> {
    conn.execute_batch(
        "CREATE TEMP TABLE touched (chat_id TEXT PRIMARY KEY);
         CREATE TEMP TRIGGER turn_added AFTER INSERT ON turns
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER turn_moved AFTER UPDATE OF status ON turns
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER question_asked AFTER INSERT ON questions
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;
         CREATE TEMP TRIGGER question_answered AFTER UPDATE OF answered_ts ON questions
             BEGIN INSERT OR IGNORE INTO touched VALUES (NEW.chat_id); END;",
    )?;
    Ok(())
}

/// Runs after each write: names each chat it noted on `changed`, to have its state worked out again.
pub fn after_write(
    changed: mpsc::UnboundedSender<String>,
) -> impl FnMut(&mut Connection) + Send + 'static {
    move |conn| {
        let taken = (|| -> Result<Vec<String>, Error> {
            let mut stmt = conn.prepare_cached("SELECT chat_id FROM temp.touched")?;
            let chats: Vec<String> = stmt
                .query_map([], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            if !chats.is_empty() {
                conn.execute("DELETE FROM temp.touched", [])?;
            }
            Ok(chats)
        })();
        match taken {
            Ok(chats) => chats.into_iter().for_each(|chat| {
                let _ = changed.send(chat);
            }),
            Err(e) => {
                let event = Event::point("gathr.chat.state").trace(&gathr::ids::trace_id());
                let _ = event
                    .error("sqlite")
                    .attr("why", e.to_string())
                    .insert(conn);
            }
        }
    }
}

/// Works out each chat the writer names, for as long as the service runs.
pub async fn run(chats: Arc<crate::chats::Chats>, mut named: mpsc::UnboundedReceiver<String>) {
    while let Some(chat) = named.recv().await {
        // One write can name a chat many times over.
        let mut all = BTreeSet::from([chat]);
        while let Ok(chat) = named.try_recv() {
            all.insert(chat);
        }
        for chat in all {
            changed(&chats, &chat, &gathr::ids::trace_id()).await;
        }
    }
}

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
    /// Told whether any chat works.
    pub awake: Arc<crate::awake::KeepAwake>,
}

#[derive(Default)]
struct Known {
    /// Turns with an approval waiting, each with its chat: approvals are in no table.
    asking: HashMap<String, String>,
    /// What the windows were last told of each chat that works, needs the person or has a mark.
    sent: HashMap<String, ChatState>,
}

impl States {
    pub fn new(live: broadcast::Sender<Value>, awake: Arc<crate::awake::KeepAwake>) -> States {
        States {
            live,
            known: Mutex::default(),
            awake,
        }
    }

    /// Notes whether the turn has an approval waiting.
    pub fn asking(&self, turn: &str, chat: &str, open: bool) {
        let mut known = self.known.lock().expect("never poisoned");
        if open {
            known.asking.insert(turn.to_string(), chat.to_string());
        } else {
            known.asking.remove(turn);
        }
    }

    /// Whether an approval waits in the chat.
    pub fn asks(&self, chat: &str) -> bool {
        Self::asked(&self.known.lock().expect("never poisoned"), chat)
    }

    fn asked(known: &Known, chat: &str) -> bool {
        known.asking.values().any(|c| c == chat)
    }

    /// Adds the approvals waiting to the list's rows, as `chat_state` counts them.
    pub fn on_list(&self, chats: &mut Value) {
        let known = self.known.lock().expect("never poisoned");
        for chat in chats.as_array_mut().into_iter().flatten() {
            // An archived chat's row carries no state at all.
            let listed = chat.get("needs").is_some();
            if listed && Self::asked(&known, chat["id"].as_str().unwrap_or("")) {
                chat["needs"] = true.into();
                chat["mark"] = "needs_you".into();
            }
        }
    }

    /// Works the chat's state out again and tells every window when it changed.
    pub fn changed(&self, conn: &Connection, chat: &str) -> Result<(), Error> {
        let mut known = self.known.lock().expect("never poisoned");
        let state = match store::chat_state(conn, chat)? {
            Some(state) if Self::asked(&known, chat) => state.asked(),
            Some(state) => state,
            None => ChatState::default(),
        };
        if known.sent.get(chat).copied().unwrap_or_default() == state {
            return Ok(());
        }
        match state == ChatState::default() {
            false => known.sent.insert(chat.to_string(), state),
            true => known.sent.remove(chat),
        };
        self.awake.working(known.sent.values().any(|s| s.working));
        let ChatState {
            working,
            needs,
            mark,
        } = state;
        let update = json!({ "type": "chat_state", "chat": chat, "working": working, "needs": needs,
            "mark": mark });
        let _ = self.live.send(update);
        Ok(())
    }

    /// At start: the windows load each chat's state from the list, so later changes are told from
    /// what the list says now.
    pub fn recover(&self, conn: &Connection) -> Result<(), Error> {
        let chats = store::chats(conn, false)?;
        let mut known = self.known.lock().expect("never poisoned");
        for chat in chats.as_array().into_iter().flatten() {
            let mark = ["needs_you", "cant_run", "working"]
                .into_iter()
                .find(|m| chat["mark"] == *m);
            let state = ChatState {
                working: chat["working"] == true,
                needs: chat["needs"] == true,
                mark,
            };
            if let (Some(id), false) = (chat["id"].as_str(), state == ChatState::default()) {
                known.sent.insert(id.to_string(), state);
            }
        }
        self.awake.working(known.sent.values().any(|s| s.working));
        Ok(())
    }
}
