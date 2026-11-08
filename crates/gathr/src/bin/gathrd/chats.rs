//! Chats with Claude Code: one task per chat owns its engine and its turns ([`crate::turns`]);
//! this keeps them, and starts them when a message arrives or the service starts.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gathr::claude::copies::Copies;
use gathr::claude::login::{self, Gate, Login};
use gathr::data::DataDir;
use gathr::event::Event;
use gathr::store;
use gathr::writer::Writer;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::turns;

/// Chats run on this account until agents pick theirs: the one from before accounts had names.
const CHAT_ACCOUNT: &str = "main";

pub enum Cmd {
    Send {
        text: String,
        /// The request's trace: the turn's too, so one reference finds both.
        trace: String,
        reply: oneshot::Sender<Result<Value, Failed>>,
    },
    Stop {
        reply: oneshot::Sender<Value>,
    },
    Approve {
        request_id: String,
        allow: bool,
        reply: oneshot::Sender<bool>,
    },
    /// The approvals still waiting for an answer.
    Approvals {
        reply: oneshot::Sender<Value>,
    },
    /// Takes over a turn left waiting by the last run.
    Resume(store::Waiting),
    Close {
        done: oneshot::Sender<()>,
    },
}

/// A send that failed: why, and the trace its reference points to.
pub struct Failed {
    pub why: String,
    pub trace: String,
}

pub struct Chats {
    pub writer: Writer,
    pub data: DataDir,
    /// Everything live, to every client's WebSocket (0004).
    pub live: broadcast::Sender<Value>,
    /// One renewal gate per account folder, for every start on it: chats, sign-ins, model asks.
    gates: Mutex<HashMap<PathBuf, Gate>>,
    tasks: Mutex<HashMap<String, mpsc::UnboundedSender<Cmd>>>,
}

impl Chats {
    pub fn new(writer: Writer, data: DataDir) -> Arc<Chats> {
        Arc::new(Chats {
            writer,
            data,
            live: broadcast::channel(4096).0,
            gates: Mutex::default(),
            tasks: Mutex::default(),
        })
    }

    /// The chats' account's Claude folder.
    pub fn account_folder(&self) -> PathBuf {
        gathr::accounts::folder(&self.data, CHAT_ACCOUNT, "claude")
    }

    /// The account folder's renewal gate (0002).
    pub fn gate(&self, folder: &Path) -> Gate {
        let mut gates = self.gates.lock().expect("the gates lock is never poisoned");
        gates
            .entry(folder.to_path_buf())
            .or_insert_with(|| Gate::new(folder))
            .clone()
    }

    /// The chat's task, started if it isn't running.
    pub fn task(self: &Arc<Self>, chat: &str) -> mpsc::UnboundedSender<Cmd> {
        let mut tasks = self.tasks.lock().expect("the chats lock is never poisoned");
        if let Some(tx) = tasks.get(chat).filter(|tx| !tx.is_closed()) {
            return tx.clone();
        }
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(turns::run(self.clone(), chat.to_string(), rx));
        tasks.insert(chat.to_string(), tx.clone());
        tx
    }

    /// Picks up turns left waiting when the service last stopped.
    pub async fn take_over(self: &Arc<Self>) {
        let waiting = self.writer.call(|conn| store::take_over(conn)).await;
        match waiting {
            Some(Ok(waiting)) => {
                for turn in waiting {
                    let _ = self.task(&turn.chat.clone()).send(Cmd::Resume(turn));
                }
            }
            Some(Err(e)) => {
                let event = Event::point("gathr.chats.take_over")
                    .error("sqlite")
                    .attr("why", e.to_string());
                self.writer.record(event);
            }
            None => {}
        }
    }

    /// Closes every chat's engine the gentle way, at shutdown.
    pub async fn close_all(&self) {
        let tasks: Vec<_> = self
            .tasks
            .lock()
            .expect("the chats lock is never poisoned")
            .drain()
            .map(|(_, tx)| tx)
            .collect();
        let mut waits = Vec::new();
        for tx in tasks {
            let (done, wait) = oneshot::channel();
            if tx.send(Cmd::Close { done }).is_ok() {
                waits.push(wait);
            }
        }
        for wait in waits {
            let _ = wait.await;
        }
    }

    /// An account folder's sign-in state and expiry, off the async threads: on macOS it may run
    /// `claude auth status` (0016).
    pub async fn login_status(&self, folder: &Path) -> (Login, Option<i64>) {
        let folder = folder.to_path_buf();
        let (copy, now_ms) = (self.copy(), gathr::ids::now_us() / 1000);
        tokio::task::spawn_blocking(move || login::status(&folder, copy, now_ms))
            .await
            .unwrap_or((Login::Unknown, None))
    }

    /// Gathr's own copy of Claude Code, the one every start uses outside dev and tests (0025).
    pub fn copy(&self) -> Option<PathBuf> {
        Copies::new(&self.data).program()
    }

    pub fn publish(&self, event: Value) {
        let _ = self.live.send(event);
    }
}
