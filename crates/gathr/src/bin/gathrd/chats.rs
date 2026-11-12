//! Chats with Claude Code: one task per agent owns its engine and runs its turns, one at a time
//! across its chats ([`crate::turns`], 0010 Sessions). Turns sent to no agent get a task per chat.
//! This keeps the tasks, starts them when a message arrives or the service starts, and asks every
//! task for what concerns a whole chat (stop, approvals) or an account (leave).

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gathr::claude::copies::Copies;
use gathr::claude::login::{self, Gate, Login};
use gathr::data::DataDir;
use gathr::event::Event;
use gathr::pauses::Pause;
use gathr::store;
use gathr::writer::Writer;
use serde_json::Value;
use tokio::sync::{broadcast, mpsc, oneshot};

use crate::turns;

pub enum Cmd {
    Send {
        chat: String,
        text: String,
        /// The agent it's sent to; none runs on the chats' account with the engine's defaults.
        agent: Option<String>,
        /// The request's trace: the turn's too, so one reference finds both.
        trace: String,
        reply: oneshot::Sender<Result<Value, Failed>>,
    },
    /// Stops the chat's turns: the task's queued ones, and its running one if it's the chat's.
    Stop {
        chat: String,
        reply: oneshot::Sender<Vec<String>>,
    },
    Approve {
        chat: String,
        request_id: String,
        allow: bool,
        reply: oneshot::Sender<bool>,
    },
    /// A service action (push or Codex wait check), waiting for its answer (0029).
    AskService {
        kind: &'static str,
        chat: String,
        input: Value,
        reply: oneshot::Sender<bool>,
    },
    /// The chat's approvals still waiting for an answer, and its reply still streaming.
    Open {
        chat: String,
        reply: oneshot::Sender<(Vec<Value>, Option<Value>)>,
    },
    /// Compacts the agent's conversation in the chat, after its running turn and background
    /// commands (K4): the compaction's turn, or why it can't.
    Compact {
        chat: String,
        trace: String,
        reply: oneshot::Sender<Result<Value, Failed>>,
    },
    /// The agent was deleted: its waiting turns fail and its engine closes once it's free.
    Retire,
    /// Takes over a turn left waiting by the last run.
    Resume(store::Waiting),
    /// A turn Gathr starts for the agent: a task's, a wake-up or a hear-back (0010).
    Start(gathr::tasks::Start),
    /// Another agent's message to this one (plans/agent-messages.md): into the turn it runs in the
    /// chat, else into one queued there, else a turn of its own. Answers which it was.
    Message {
        chat: String,
        /// What the agent is told: who wrote, and their words.
        ask: String,
        /// The sender's turn, which a turn of its own answers.
        answers: String,
        /// The sender's message, which what the agent is told is a copy of.
        copy_of: String,
        reply: oneshot::Sender<&'static str>,
    },
    /// Closes the task's engine if it runs in this account folder, answering once it has exited;
    /// the queue stays.
    Leave {
        folder: PathBuf,
        done: oneshot::Sender<()>,
    },
    /// A pause or a usage hold changed: what it held may start, or now waits.
    Kick,
    /// The person's Stop now (0010): the turns in the scope are interrupted, their background
    /// commands ended, and a carry-on per agent waits for Restart.
    StopNow {
        scope: String,
        target: String,
        /// Who stopped it, as the carry-on says: Stop now, or an update.
        by: String,
        /// An update saved every carry-on before it stopped anything: none is saved here.
        carried: bool,
        done: oneshot::Sender<()>,
    },
    /// The chat was archived, or with close deleted (F11): its turns and background commands end,
    /// and with close its engines close, answering once each has exited.
    EndChat {
        chat: String,
        close: bool,
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
    /// The service's port, where engines reach Gathr's tools.
    pub port: u16,
    /// The tokens of the engines running now (0005).
    pub passes: Arc<crate::tools::Passes>,
    /// Everything live, to every client's WebSocket (0004).
    pub live: broadcast::Sender<Value>,
    /// Each chat's state in the chat list, and what the windows were last told of it (0040).
    pub states: Arc<crate::chat_state::States>,
    /// One renewal gate per account folder, for every start on it: chats, sign-ins, model asks.
    gates: Mutex<HashMap<PathBuf, Gate>>,
    /// Each agent's task by `agent:<id>`, and each chat's for turns to no agent by `chat:<id>`.
    tasks: Mutex<HashMap<String, mpsc::UnboundedSender<Cmd>>>,
    /// Agents whose engine hasn't exited 30 s after its close: what each is held on (0010).
    holds: Mutex<HashMap<String, Value>>,
    /// Chats run on the oldest account until agents pick theirs (0026); none until one is added.
    account: Mutex<Option<String>>,
    /// Accounts being removed, never picked even while their rows remain.
    removing: Mutex<HashSet<String>>,
    picking: tokio::sync::Mutex<()>,
    /// The pauses in force, as saved (0010); every queue reads them before it starts a turn.
    pauses: Mutex<Vec<Pause>>,
    /// Wait checks running now: each one's chat and agent. A pause stays `pausing` while one runs.
    checking: Mutex<Vec<(String, String)>>,
    /// The version Gathr is updating to: the turns Gathr queued wait while it's set (0032).
    updating: Mutex<Option<String>>,
}

impl Chats {
    pub fn new(
        writer: Writer,
        data: DataDir,
        port: u16,
        (live, states): (broadcast::Sender<Value>, Arc<crate::chat_state::States>),
    ) -> Arc<Chats> {
        Arc::new(Chats {
            writer,
            data,
            port,
            passes: Arc::default(),
            live,
            states,
            gates: Mutex::default(),
            tasks: Mutex::default(),
            holds: Mutex::default(),
            account: Mutex::default(),
            removing: Mutex::default(),
            picking: tokio::sync::Mutex::default(),
            pauses: Mutex::default(),
            checking: Mutex::default(),
            updating: Mutex::default(),
        })
    }

    /// The account a turn sent to no agent runs on, if there's one.
    pub fn account(&self) -> Option<String> {
        self.account.lock().expect("never poisoned").clone()
    }

    /// Marks an account as being removed, or kept after all; either way the chats pick again.
    pub async fn removing(&self, id: &str, removing: bool) {
        {
            let mut set = self.removing.lock().expect("never poisoned");
            match removing {
                true => set.insert(id.to_string()),
                false => set.remove(id),
            };
        }
        self.pick_account().await;
    }

    /// The accounts being removed now.
    pub fn being_removed(&self) -> HashSet<String> {
        self.removing.lock().expect("never poisoned").clone()
    }

    /// An account removed: its row is gone, so it needs no mark any more.
    pub fn removed(&self, id: &str) {
        self.removing.lock().expect("never poisoned").remove(id);
    }

    /// Picks the chats' account again: the oldest not being removed.
    pub async fn pick_account(&self) {
        // One pick at a time: one that listed before a removal began must not land after it.
        let _one = self.picking.lock().await;
        let except = self.removing.lock().expect("never poisoned").clone();
        let picked = self
            .writer
            .call(move |conn| {
                let listed = gathr::accounts::list(conn)?;
                let next = listed.into_iter().find(|a| !except.contains(&a.id));
                Ok::<_, gathr::Error>(next.map(|a| a.id))
            })
            .await;
        let Some(Ok(next)) = picked else { return };
        *self.account.lock().expect("never poisoned") = next;
    }

    /// Closes every engine running in `folder` the gentle way, once each has exited; the chats
    /// carry on.
    pub async fn leave(&self, folder: &Path) {
        self.ask_all(|done| Cmd::Leave {
            folder: folder.to_path_buf(),
            done,
        })
        .await;
    }

    /// Stops the chat's turns, whichever agents they're for: the turns stopped.
    pub async fn stop(&self, chat: &str) -> Vec<String> {
        let chat = chat.to_string();
        let stopped = self.ask_all(|reply| Cmd::Stop {
            chat: chat.clone(),
            reply,
        });
        stopped.await.into_iter().flatten().collect()
    }

    /// Answers an approval the chat's running turn asked: false when none is open.
    pub async fn approve(&self, chat: &str, request_id: &str, allow: bool) -> bool {
        let answered = self.ask_all(|reply| Cmd::Approve {
            chat: chat.to_string(),
            request_id: request_id.to_string(),
            allow,
            reply,
        });
        answered.await.into_iter().any(|a| a)
    }

    /// The chat's open approvals and its replies still streaming, one per agent at work, for a
    /// window that opens it later.
    pub async fn open(&self, chat: &str) -> (Vec<Value>, Vec<Value>) {
        let open = self.ask_all(|reply| Cmd::Open {
            chat: chat.to_string(),
            reply,
        });
        let (approvals, streaming): (Vec<_>, Vec<_>) = open.await.into_iter().unzip();
        let approvals = approvals.into_iter().flatten().collect();
        (approvals, streaming.into_iter().flatten().collect())
    }

    /// Whether a `git push` from the engine of `caller` may go on: its agent's task asks the
    /// person and answers once they have. No task running for it: no.
    pub async fn ask_push(&self, caller: &crate::tools::Caller, input: Value) -> bool {
        self.ask_service(caller, input, "push").await
    }

    /// A service-side action, approved on the engine's active turn before it can be saved/run.
    pub async fn ask_service(
        &self,
        caller: &crate::tools::Caller,
        input: Value,
        kind: &'static str,
    ) -> bool {
        let key = match &caller.agent {
            Some(agent) => format!("agent:{agent}"),
            None => format!("chat:{}", caller.chat),
        };
        let task = self
            .tasks
            .lock()
            .expect("never poisoned")
            .get(&key)
            .cloned();
        let (reply, answer) = oneshot::channel();
        let chat = caller.chat.clone();
        let asked = task.is_some_and(|t| {
            t.send(Cmd::AskService {
                kind,
                chat,
                input,
                reply,
            })
            .is_ok()
        });
        asked && answer.await.unwrap_or(false)
    }

    /// Sends every task a command and waits for every answer.
    async fn ask_all<T>(&self, command: impl Fn(oneshot::Sender<T>) -> Cmd) -> Vec<T> {
        let tasks: Vec<_> = self
            .tasks
            .lock()
            .expect("the chats lock is never poisoned")
            .values()
            .cloned()
            .collect();
        let mut waits = Vec::new();
        for tx in tasks {
            let (reply, wait) = oneshot::channel();
            if tx.send(command(reply)).is_ok() {
                waits.push(wait);
            }
        }
        let mut answers = Vec::new();
        for wait in waits {
            answers.extend(wait.await);
        }
        answers
    }

    /// Lets a deleted agent's task go: its engine closes the gentle way, without waiting to idle.
    pub fn retire(&self, agent: &str) {
        let tasks = self.tasks.lock().expect("the chats lock is never poisoned");
        if let Some(task) = tasks.get(&format!("agent:{agent}")) {
            let _ = task.send(Cmd::Retire);
        }
    }

    /// What the agent's engine is held on, or null.
    pub fn hold(&self, agent: &str) -> Value {
        let holds = self.holds.lock().expect("never poisoned");
        holds.get(agent).cloned().unwrap_or(Value::Null)
    }

    pub fn set_hold(&self, agent: &str, hold: Option<Value>) {
        let mut holds = self.holds.lock().expect("never poisoned");
        match hold {
            Some(hold) => holds.insert(agent.to_string(), hold),
            None => holds.remove(agent),
        };
    }

    /// The account folder's renewal gate (0002).
    pub fn gate(&self, folder: &Path) -> Gate {
        let mut gates = self.gates.lock().expect("the gates lock is never poisoned");
        gates
            .entry(folder.to_path_buf())
            .or_insert_with(|| Gate::new(folder))
            .clone()
    }

    /// The task that runs the turn: its agent's, or the chat's for a turn sent to no agent.
    pub fn task(self: &Arc<Self>, chat: &str, agent: Option<&str>) -> mpsc::UnboundedSender<Cmd> {
        let key = match agent {
            Some(agent) => format!("agent:{agent}"),
            None => format!("chat:{chat}"),
        };
        let mut tasks = self.tasks.lock().expect("the chats lock is never poisoned");
        if let Some(tx) = tasks.get(&key).filter(|tx| !tx.is_closed()) {
            return tx.clone();
        }
        let (tx, rx) = mpsc::unbounded_channel();
        let agent = agent.map(String::from);
        tokio::spawn(turns::run(self.clone(), agent, rx));
        tasks.insert(key, tx.clone());
        tx
    }

    /// Queues the turns Gathr starts, each on its agent's task.
    pub fn start(self: &Arc<Self>, starts: Vec<gathr::tasks::Start>) {
        for start in starts {
            let task = self.task(&start.chat, Some(&start.agent));
            let _ = task.send(Cmd::Start(start));
        }
    }

    /// Picks up turns left waiting when the service last stopped.
    pub async fn take_over(self: &Arc<Self>) {
        let states = self.states.clone();
        let waiting = self.writer.call(move |conn| {
            let waiting = store::take_over(conn)?;
            states.recover(conn)?;
            Ok::<_, gathr::Error>(waiting)
        });
        let waiting = waiting.await;
        match waiting {
            Some(Ok(waiting)) => {
                for turn in waiting {
                    let task = self.task(&turn.chat, turn.agent.as_deref());
                    let _ = task.send(Cmd::Resume(turn));
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

    /// The pauses in force now.
    pub fn pauses(&self) -> Vec<Pause> {
        self.pauses.lock().expect("never poisoned").clone()
    }

    /// The version Gathr is updating to, while the turns it queued wait for it.
    pub fn updating(&self) -> Option<String> {
        self.updating.lock().expect("never poisoned").clone()
    }

    /// Holds the turns Gathr queued for an update to `version`, or lets them go (`None`).
    pub async fn set_updating(&self, version: Option<String>) {
        *self.updating.lock().expect("never poisoned") = version;
        self.pauses_changed().await;
    }

    /// The wait checks running now.
    pub fn checking(&self) -> Vec<(String, String)> {
        self.checking.lock().expect("never poisoned").clone()
    }

    /// A wait check for `agent` in `chat` started (`true`) or ended: an end may quiet a pause.
    pub async fn check_running(&self, chat: &str, agent: &str, running: bool) {
        let one = (chat.to_string(), agent.to_string());
        {
            let mut checking = self.checking.lock().expect("never poisoned");
            match running {
                true => checking.push(one),
                false => {
                    if let Some(at) = checking.iter().position(|c| *c == one) {
                        checking.remove(at);
                    }
                }
            }
        }
        if !running {
            self.settle_pauses(false).await;
        }
    }

    /// Returns once no wait check runs in the chat: one whose wait ended is killed within a look.
    pub async fn checks_ended(&self, chat: &str) {
        let running = || {
            let checking = self.checking.lock().expect("never poisoned");
            checking.iter().any(|(c, _)| c == chat)
        };
        while running() {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Reads the pauses again after a change, lets every queue look at what it holds, and says
    /// where things stand.
    pub async fn pauses_changed(&self) {
        if let Some(Ok(saved)) = self.writer.call(|c| gathr::pauses::list(c)).await {
            *self.pauses.lock().expect("never poisoned") = saved;
        }
        self.retry_waiting();
        self.settle_pauses(true).await;
    }

    /// Lets queued turns try again after a pause changes or an engine copy arrives.
    pub fn retry_waiting(&self) {
        let tasks: Vec<_> = self
            .tasks
            .lock()
            .expect("never poisoned")
            .values()
            .cloned()
            .collect();
        for task in tasks {
            let _ = task.send(Cmd::Kick);
        }
    }

    /// A pause in which nothing runs any more becomes `paused`, said once, and one a turn runs in
    /// again is `pausing` (0010). Sends the pauses live when one moved, or when `changed`.
    pub async fn settle_pauses(&self, changed: bool) {
        if self.pauses().is_empty() && !changed {
            return;
        }
        let (trace, checks) = (gathr::ids::trace_id(), self.checking());
        let settled = self.writer.call(move |c| {
            let quiet = gathr::pauses::settle(c, &checks, &trace)?;
            Ok::<_, gathr::Error>((
                quiet.len(),
                gathr::pauses::list(c)?,
                gathr::pauses::listed(c, &checks)?,
            ))
        });
        let Some(Ok((quiet, saved, mut listed))) = settled.await else {
            return;
        };
        *self.pauses.lock().expect("never poisoned") = saved;
        if quiet > 0 || changed {
            listed["type"] = "pauses".into();
            self.publish(listed);
        }
    }

    /// Stop now in a scope: every agent's queue stops what it runs there; `by` is who did.
    pub async fn stop_now(&self, scope: &str, target: &str, by: &str, carried: bool) {
        self.ask_all(|done| Cmd::StopNow {
            scope: scope.to_string(),
            target: target.to_string(),
            by: by.to_string(),
            carried,
            done,
        })
        .await;
    }

    /// Ends a chat's work in every agent's queue once it's archived or, with close, being deleted.
    pub async fn end_chat(&self, chat: &str, close: bool) {
        self.ask_all(|done| Cmd::EndChat {
            chat: chat.to_string(),
            close,
            done,
        })
        .await;
    }

    /// Closes every engine at shutdown: the gentle way, killed only after the grace period.
    pub async fn close_all(&self) {
        self.ask_all(|done| Cmd::Close { done }).await;
        self.tasks.lock().expect("never poisoned").clear();
    }

    /// An account folder's sign-in state and expiry, off the async threads: on macOS it may run
    /// `claude auth status` (0016).
    pub async fn login_status(&self, folder: &Path) -> (Login, Option<i64>) {
        let folder = folder.to_path_buf();
        if folder.file_name().is_some_and(|n| n == "codex") {
            return (gathr::codex::login::status(&folder), None);
        }
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
