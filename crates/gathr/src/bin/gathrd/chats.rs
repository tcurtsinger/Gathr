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
    /// Closes the chat's engine if it runs in this account folder; the chat's queue stays.
    Leave {
        folder: PathBuf,
        done: oneshot::Sender<()>,
    },
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
    /// Chats run on the oldest account until agents pick theirs (0026); none until one is added.
    account: Mutex<Option<String>>,
}

impl Chats {
    pub fn new(writer: Writer, data: DataDir) -> Arc<Chats> {
        Arc::new(Chats {
            writer,
            data,
            live: broadcast::channel(4096).0,
            gates: Mutex::default(),
            tasks: Mutex::default(),
            account: Mutex::default(),
        })
    }

    /// The chats' account's Claude folder, if there's an account.
    pub fn account_folder(&self) -> Option<PathBuf> {
        let account = self.account.lock().expect("never poisoned").clone()?;
        Some(gathr::accounts::folder(&self.data, &account, "claude"))
    }

    /// Picks the chats' account again: the oldest, other than `except` (one being removed).
    /// A new account has none of the old one's Claude Code sessions, so chats start new ones.
    pub async fn pick_account(&self, except: Option<&str>) {
        let except = except.map(str::to_string);
        let picked = self
            .writer
            .call(move |conn| {
                let listed = gathr::accounts::list(conn)?;
                let next = listed.into_iter().find(|a| Some(&a.id) != except.as_ref());
                Ok::<_, gathr::Error>(next.map(|a| a.id))
            })
            .await;
        let Some(Ok(next)) = picked else { return };
        let before = std::mem::replace(
            &mut *self.account.lock().expect("never poisoned"),
            next.clone(),
        );
        if before.is_some() && before != next {
            self.writer
                .call(|conn| store::forget_engine_sessions(conn))
                .await;
        }
    }

    /// Closes every chat engine running in `folder` the gentle way; the chats carry on.
    pub async fn leave(&self, folder: &Path) {
        let tasks: Vec<_> = self
            .tasks
            .lock()
            .expect("the chats lock is never poisoned")
            .values()
            .cloned()
            .collect();
        let mut waits = Vec::new();
        for tx in tasks {
            let (done, wait) = oneshot::channel();
            let folder = folder.to_path_buf();
            if tx.send(Cmd::Leave { folder, done }).is_ok() {
                waits.push(wait);
            }
        }
        for wait in waits {
            let _ = wait.await;
        }
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
