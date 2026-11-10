//! One agent's task: it owns the agent's Claude Code and runs its turns one at a time, across all
//! its chats, each chat in its own session (docs/decisions/0001-engine-interface.md, 0010
//! Sessions). Turns sent to no agent get a task per chat, on the chats' account.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gathr::agents;
use gathr::claude::engine::{Engine, Line};
use gathr::claude::login::{self, Login};
use gathr::event::Event;
use gathr::ids::{now_us, span_id, ulid};
use gathr::store;
use serde_json::{Value, json};
use tokio::sync::{mpsc, oneshot};

use crate::chats::{Chats, Cmd, Failed};

mod driver;
mod ended;
mod given;
mod holds;
mod process;
mod pushes;
mod reading;
mod records;
mod workspace;

pub use process::CLOSE_GRACE;

/// How often a turn waiting for sign-in looks at the login again.
const SIGN_IN_POLL: Duration = Duration::from_secs(2);
/// Where the login isn't in a file (macOS), a refused login is tried again after this long.
const REFUSED_RETRY: Duration = Duration::from_secs(60);

/// What a turn is: its chat, its id, the trace its reference finds, and the agent it's for.
#[derive(Clone)]
struct Ids {
    chat: String,
    turn: String,
    trace: String,
    agent: Option<String>,
    /// The task it works on (0010).
    task: Option<i64>,
    /// The turn it answers, when Gathr started it (F8b).
    answers: Option<String>,
    /// The person wrote its ask: a pause never holds it (0010).
    person: bool,
}

#[derive(Clone)]
struct Queued {
    ids: Ids,
    text: String,
    /// What holds it back: a pause, usage, or the missing engine (`claude` or `codex`).
    held: Option<&'static str>,
}

/// Who the engine runs as and where: the agent, its account's folder, model, effort (0026) and
/// approvals (0029),
/// read when a turn starts so a saved change applies from the agent's next turn, the chat whose
/// session it runs, and that chat's roles, which its briefing names (0028).
#[derive(Clone, PartialEq)]
struct Assignment {
    agent: Option<String>,
    account: String,
    engine: String,
    folder: PathBuf,
    model: Option<String>,
    effort: Option<String>,
    approvals: gathr::agents::Approvals,
    chat: String,
    team: Vec<(String, gathr::team::Role)>,
}

struct Running {
    ids: Ids,
    span: String,
    sent_us: i64,
    /// Text streamed since the last whole block: what a stopped turn keeps.
    partial: String,
    /// `partial`'s length in UTF-16 units, as the window counts it: each delta says where it starts,
    /// so a window that read `partial` in a chat's history skips what that already held.
    partial_units: usize,
    streamed: bool,
    /// Tool calls under way: their message, name and start.
    tools: HashMap<String, (String, String, i64)>,
    /// Questions Claude Code asked and nobody has answered: each tool, its input and why it asks.
    approvals: HashMap<String, (String, Value, Option<String>)>,
    /// Service-side asks (pushes and Codex wait checks): where each answer goes.
    service_approvals: HashMap<String, (&'static str, oneshot::Sender<bool>)>,
    /// The person allowed a command that pushes, and it hasn't ended: its push asks no twice.
    push_allowed: bool,
    stopping: bool,
    /// Why Gathr stopped it, when Gathr did: the stopped turn says so.
    stop_why: Option<String>,
    signed_out: bool,
    /// Tools it has started: a turn cut off after one never runs again from its ask (F10).
    tools_started: u32,
    /// The engine said the account is out of usage during it.
    limited: bool,
    /// When it last showed progress, and whether it was flagged for showing none.
    progress_at: Instant,
    stalled: bool,
    /// The account's renewal gate, held until the provider first answers (0016).
    gate: login::Hold,
}

struct Task {
    chats: Arc<Chats>,
    /// The agent this task runs; `None` for a chat's turns sent to no agent.
    agent: Option<String>,
    engine: Option<Engine>,
    driver: driver::Driver,
    /// The engine's token for Gathr's tools and its briefing, while it runs.
    pass: Option<crate::tools::Pass>,
    /// The file the engine runs, so an engine on Gathr's copy moves when it updates (0025).
    program: Option<PathBuf>,
    /// What the engine runs as, while it runs.
    assigned: Option<Assignment>,
    /// The task whose folder holds the engine's temporary files (0034).
    scratch_task: Option<i64>,
    lines: Option<mpsc::UnboundedReceiver<Line>>,
    /// The engine's last stderr line: when it exits mid-turn, that's usually why (0019).
    last_stderr: Option<String>,
    turn: Option<Running>,
    queue: VecDeque<Queued>,
    /// The account folder whose login the engine refused, its login file then, and when: held
    /// as signed out until the file changes, so a refused login doesn't start an engine every poll.
    refused: Option<(PathBuf, Option<std::time::SystemTime>, Instant)>,
    /// Messages sent into a turn under way that the engine hasn't read yet: their id and text.
    unread: Vec<(String, String)>,
    /// An engine closed the gentle way that hasn't exited yet; nothing new starts until it has.
    closing: Option<process::Closing>,
    /// Account removals waiting for the closing engine to exit.
    leaving: Vec<oneshot::Sender<()>>,
    /// When the engine last finished a turn or started.
    idle_since: Instant,
    /// The agent was deleted: its engine closes as soon as it's free.
    retired: bool,
    times: process::Times,
    /// When the account's usage hold ends, while the next turn waits on it.
    usage_wait: Option<Instant>,
    usage_until_us: Option<i64>,
    /// Commands the engine runs in the background: it isn't idle while any do.
    background: usize,
    /// The turn (and task) of the last background command that ended: the engine's next turn of
    /// its own carries on from it.
    woke_from: Option<(String, Option<i64>)>,
}

pub async fn run(
    chats: Arc<Chats>,
    agent: Option<String>,
    mut commands: mpsc::UnboundedReceiver<Cmd>,
) {
    let mut task = Task {
        chats,
        agent,
        engine: None,
        driver: Default::default(),
        pass: None,
        program: None,
        assigned: None,
        scratch_task: None,
        lines: None,
        last_stderr: None,
        turn: None,
        queue: VecDeque::new(),
        refused: None,
        unread: Vec::new(),
        closing: None,
        leaving: Vec::new(),
        idle_since: Instant::now(),
        retired: false,
        times: process::Times::from_env(),
        usage_wait: None,
        usage_until_us: None,
        background: 0,
        woke_from: None,
    };
    // An interval, not a sleep in the select: a sleep restarts on every command, so a window
    // asking for the chat more often than this would hold a waiting message forever.
    let mut sign_in_poll = tokio::time::interval(SIGN_IN_POLL);
    sign_in_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut stall_look = tokio::time::interval((task.times.stall / 4).min(Duration::from_secs(30)));
    stall_look.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let free = task.turn.is_none() && task.closing.is_none();
        // A usage hold has its own timer: polling it would only ask again.
        let waiting = free && !task.queue.is_empty() && task.usage_wait.is_none();
        let idle = free && task.engine.is_some() && task.queue.is_empty() && task.background == 0;
        let idle_at = match task.retired {
            true => tokio::time::Instant::now(),
            false => tokio::time::Instant::from_std(task.idle_since + task.times.idle_close),
        };
        let idle_why = match task.retired {
            true => "its agent was deleted",
            false => "it was idle",
        };
        let hold_after = task.times.hold_after;
        let usage_at = task.usage_wait.map(tokio::time::Instant::from_std);
        tokio::select! {
            command = commands.recv() => match command {
                Some(Cmd::Close { done }) => {
                    task.shut_down("the service is stopping").await;
                    let _ = done.send(());
                    return;
                }
                None => {
                    task.shut_down("the agent's task ended").await;
                    return;
                }
                Some(command) => task.handle(command).await,
            },
            line = next(&mut task.lines) => match line {
                Some(line) => task.on_line(line).await,
                None => task.on_exit().await,
            },
            closed = process::closing(&mut task.closing, hold_after) => match closed {
                Some(code) => task.on_closed(code).await,
                None => task.on_held(),
            },
            _ = tokio::time::sleep_until(idle_at), if idle => task.close(idle_why).await,
            _ = sign_in_poll.tick(), if waiting => task.try_next().await,
            _ = tokio::time::sleep_until(usage_at.unwrap_or_else(tokio::time::Instant::now)), if usage_at.is_some() => {
                task.usage_wait = None;
                task.mark_holds().await;
                task.try_next().await;
            }
            _ = stall_look.tick(), if task.turn.is_some() => {
                task.check_stall();
                task.check_task().await;
            }
        }
    }
}

/// When the login file last changed; `None` when there's none.
fn login_stamp(file: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(file).and_then(|m| m.modified()).ok()
}

async fn next(lines: &mut Option<mpsc::UnboundedReceiver<Line>>) -> Option<Line> {
    match lines {
        Some(rx) => rx.recv().await,
        None => std::future::pending().await,
    }
}

impl Task {
    async fn handle(&mut self, command: Cmd) {
        match command {
            Cmd::Send {
                chat,
                text,
                agent,
                trace,
                reply,
            } => {
                let _ = reply.send(self.send(chat, text, agent, trace).await);
            }
            Cmd::Stop { chat, reply } => {
                let _ = reply.send(self.stop(&chat).await);
            }
            Cmd::Approve {
                chat,
                request_id,
                allow,
                reply,
            } => {
                let _ = reply.send(self.approve(&chat, &request_id, allow).await);
            }
            Cmd::AskService {
                kind,
                chat,
                input,
                reply,
            } => self.ask_service(kind, &chat, input, reply),
            Cmd::Open { chat, reply } => {
                let _ = reply.send((self.open_approvals(&chat), self.streaming(&chat).await));
            }
            Cmd::Resume(w) => {
                let ids = Ids {
                    chat: w.chat,
                    turn: w.turn,
                    trace: w.trace,
                    agent: w.agent,
                    task: w.task,
                    answers: w.answers,
                    person: w.person,
                };
                self.queue.push_back(Queued {
                    ids,
                    text: w.text,
                    held: None,
                });
                self.mark_holds().await;
                self.try_next().await;
            }
            Cmd::Start(start) => self.given(start).await,
            Cmd::Leave { folder, done } => self.leave(&folder, done).await,
            Cmd::Kick => {
                self.mark_holds().await;
                self.try_next().await;
            }
            Cmd::StopNow {
                scope,
                target,
                done,
            } => {
                self.stop_now(scope, target).await;
                let _ = done.send(());
            }
            Cmd::EndChat { chat, close, done } => self.end_chat(&chat, close, done).await,
            Cmd::Retire => self.retire().await,
            Cmd::Close { .. } => unreachable!("handled in run"),
        }
    }

    async fn send(
        &mut self,
        chat: String,
        text: String,
        agent: Option<String>,
        trace: String,
    ) -> Result<Value, Failed> {
        // Only into the same chat's turn: a message in another chat waits for the agent.
        let steered = self
            .turn
            .as_ref()
            .filter(|t| !t.stopping && t.ids.chat == chat && t.ids.agent == agent);
        if let Some(running) = steered {
            // G13: into the turn under way. Claude Code reads it after the step it's on and
            // says so (`Frame::Read`), as Esc-free typing does in its terminal.
            let ids = running.ids.clone();
            let message = self.db_message(&ids, "user", "text", &text).await;
            if let Err(e) = self.write_user(&text, Some(&ids.turn)).await {
                return Err(self.fail_send(&ids, format!("Claude Code didn't take it: {e}")));
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id, text));
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            let steer = Event::point("gathr.turn.steer").trace(&ids.trace);
            self.record(steer.chat(&ids.chat).ok());
            return Ok(json!({ "turn": ids.turn, "status": "running", "steered": true }));
        }
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace,
            agent,
            task: None,
            answers: None,
            person: true,
        };
        let queued = Queued {
            ids,
            text,
            held: None,
        };
        let row = queued.ids.clone();
        let added = self
            .chats
            .writer
            .call(move |conn| {
                store::add_turn(
                    conn,
                    &row.chat,
                    &row.turn,
                    &row.trace,
                    (row.agent.as_deref(), row.task, row.answers.as_deref()),
                )
            })
            .await;
        let ids = &queued.ids;
        if let Some(Err(e)) = added {
            return Err(self.fail_send(ids, e.to_string()));
        }
        let message = self.db_message(ids, "user", "text", &queued.text).await;
        self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        self.record(
            Event::point("gathr.chat.send")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .ok()
                .attr("gathr.message.id", message["id"].clone())
                .attr("gathr.turn.id", ids.turn.clone())
                .attr("gathr.message.chars", queued.text.chars().count()),
        );
        let reference = gathr::ids::reference(&ids.trace).to_string();
        let answer = |status: &str| json!({ "turn": ids.turn, "status": status, "ref": reference });
        match self.enqueue(&queued).await {
            Ok(status) => Ok(answer(status)),
            Err(why) => Err(Failed {
                why,
                trace: ids.trace.clone(),
            }),
        }
    }

    /// Starts the turn now, or queues it behind what the agent is doing: its status, or why it
    /// failed.
    async fn enqueue(&mut self, queued: &Queued) -> Result<&'static str, String> {
        let ids = &queued.ids;
        if self.holder(ids).is_some() {
            self.queue.push_back(queued.clone());
            self.mark_holds().await;
            return Ok("held");
        }
        let ahead = self.queue.iter().any(|q| self.holder(&q.ids).is_none());
        if self.turn.is_some() || self.closing.is_some() || ahead {
            self.queue.push_back(queued.clone());
            self.set_turn(ids, "queued", None).await;
            self.trace_hold(ids);
            return Ok("queued");
        }
        let started = self.start(queued).await;
        if let Err(why) = &started {
            self.set_turn(ids, "failed", Some(why)).await;
        }
        started
    }

    /// Starts the turns waiting, in order, while nothing runs or closes.
    async fn try_next(&mut self) {
        while self.turn.is_none() && self.closing.is_none() {
            // The first turn no pause holds: a paused chat's turns never hold another chat's.
            let Some(next) = self
                .queue
                .iter()
                .find(|q| self.holder(&q.ids).is_none())
                .cloned()
            else {
                return;
            };
            let taken = |q: &Queued| q.ids.turn != next.ids.turn;
            match self.start(&next).await {
                Ok("running" | "dropped") => self.queue.retain(taken),
                Ok(_) => return,
                Err(why) => {
                    self.queue.retain(taken);
                    self.set_turn(&next.ids, "failed", Some(&why)).await;
                }
            }
        }
    }

    /// Sends a turn to the engine, starting it first if needed: `running`; `queued` while
    /// another session's engine closes; `waiting_sign_in`; `dropped` when its task may no longer
    /// start (0010's gate); or why it couldn't. A turn that doesn't run stays first in the queue.
    async fn start(&mut self, next: &Queued) -> Result<&'static str, String> {
        if let Some(why) = self.gate(&next.ids).await {
            self.set_turn(&next.ids, "stopped", Some(&why)).await;
            return Ok("dropped");
        }
        if self.closing.is_some() || self.awaiting_read() {
            return Ok(self.hold_back(next, "queued").await);
        }
        if let Some(version) = self.copy_moved() {
            let why = format!("Gathr's engine copy is now {version}");
            self.close(&why).await;
            return Ok(self.hold_back(next, "queued").await);
        }
        let assigned = self.assignment(next).await?;
        if self.engine.is_some() && self.assigned != assigned {
            let why = match (&self.assigned, &assigned) {
                (Some(was), Some(now)) if was.chat != now.chat => {
                    "the next turn is in another chat"
                }
                (Some(was), Some(now)) if was.account != now.account => {
                    "the next turn runs on another account"
                }
                (Some(was), Some(now)) if was.team != now.team => "the chat's roles changed",
                _ => "the next turn runs with another model, effort or approvals",
            };
            self.close(why).await;
            return Ok(self.hold_back(next, "queued").await);
        }
        // Its temporary files are its task's, set when it starts; a turn on no task keeps them.
        if self.engine.is_some() && next.ids.task.is_some() && next.ids.task != self.scratch_task {
            self.close("the next turn is on another task").await;
            return Ok(self.hold_back(next, "queued").await);
        }
        let kind = if assigned.as_ref().is_some_and(|a| a.engine == "codex") {
            "codex"
        } else {
            "claude"
        };
        let (copy, override_var) = if kind == "codex" {
            (
                gathr::codex::copies::storage(&self.chats.data).program(),
                gathr::codex::command::PATH_VAR,
            )
        } else {
            (self.chats.copy(), gathr::claude::command::PATH_VAR)
        };
        // A first download holds the saved turn. A dev/test override that is wrong still fails.
        if self.engine.is_none()
            && copy.is_none()
            && std::env::var_os(override_var).is_none_or(|p| p.is_empty())
        {
            self.hold_back(next, "queued").await;
            if let Some(queued) = self.queue.iter_mut().find(|q| q.ids.turn == next.ids.turn) {
                queued.held = Some(kind);
            }
            self.save_held(&next.ids, Some(kind)).await;
            return Ok("queued");
        }
        if next.held.is_some_and(|h| matches!(h, "claude" | "codex")) {
            if let Some(queued) = self.queue.iter_mut().find(|q| q.ids.turn == next.ids.turn) {
                queued.held = None;
            }
            self.save_held(&next.ids, None).await;
        }
        // A wrong path fails the send at once, signed in or not (hand check 8).
        let program = match self.engine {
            Some(_) => None,
            None => Some(
                self.find_engine(
                    assigned.as_ref().map_or("claude", |a| a.engine.as_str()),
                    &next.ids.trace,
                )
                .await?,
            ),
        };
        // No account yet waits like a signed-out one: adding one and signing it in sends it.
        let Some(assigned) = assigned else {
            return Ok(self.hold_back(next, "waiting_sign_in").await);
        };
        if self.usage_held(&assigned.account).await {
            self.hold_on_usage(next).await;
            return Ok("queued");
        }
        if self.login(&assigned.folder).await == Login::SignedOut {
            return Ok(self.hold_back(next, "waiting_sign_in").await);
        }
        let gate = self.chats.gate(&assigned.folder).pass().await;
        if let Some(program) = program {
            self.spawn(&program, assigned, (next.ids.task, &next.ids.trace))
                .await?;
        }
        self.open_turn(next.ids.clone(), gate, next.text.chars().count());
        if let Err(e) = self.write_user(&next.text, Some(&next.ids.turn)).await {
            self.turn = None;
            return Err(format!(
                "{} didn't take the message: {e}",
                self.engine_label()
            ));
        }
        self.set_turn(&next.ids, "running", None).await;
        Ok("running")
    }

    /// Keeps the turn first in the queue, with `status`.
    async fn hold_back(&mut self, next: &Queued, status: &'static str) -> &'static str {
        let first = !self.queue.iter().any(|q| q.ids.turn == next.ids.turn);
        if first {
            self.queue.push_front(next.clone());
        }
        self.set_turn(&next.ids, status, None).await;
        if first {
            self.trace_hold(&next.ids);
        }
        status
    }

    /// Whether a message sent into the last turn is still to be read: the engine answers it with
    /// a turn of its own, so it goes before any other turn, and no timer gives up on it.
    fn awaiting_read(&self) -> bool {
        self.engine.is_some() && !self.unread.is_empty()
    }

    /// Messages sent into a turn that the engine will now never read become the chat's next
    /// turns, first in the queue, so each still gets its answer.
    pub(super) async fn requeue_unread(&mut self) {
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        for (at, (message, text)) in std::mem::take(&mut self.unread).into_iter().enumerate() {
            let ids = Ids {
                chat: chat.clone(),
                turn: ulid(now_us()),
                trace: gathr::ids::trace_id(),
                agent: self.agent.clone(),
                task: None,
                answers: None,
                person: true,
            };
            let row = ids.clone();
            let saved = self
                .chats
                .writer
                .call(move |conn| {
                    store::add_turn(
                        conn,
                        &row.chat,
                        &row.turn,
                        &row.trace,
                        (row.agent.as_deref(), row.task, row.answers.as_deref()),
                    )?;
                    store::move_message(conn, &message, &row.turn)
                })
                .await;
            if let Some(Ok(message)) = &saved {
                self.publish(&ids.chat, json!({ "type": "message", "message": message }));
            }
            if let Some(Err(e)) = saved {
                let event = Event::point("gathr.turn.requeue").trace(&ids.trace);
                self.record(event.error("sqlite").attr("why", e.to_string()));
            }
            self.queue.insert(
                at,
                Queued {
                    ids: ids.clone(),
                    text,
                    held: None,
                },
            );
            self.set_turn(&ids, "queued", None).await;
        }
    }

    /// The agent was deleted: its waiting turns fail, and its engine closes once it's free.
    async fn retire(&mut self) {
        self.retired = true;
        for queued in std::mem::take(&mut self.queue) {
            self.set_turn(&queued.ids, "failed", Some("its agent was deleted"))
                .await;
        }
    }

    /// What the turn runs as: its agent's account, model and effort as saved now; with no agent,
    /// the chats' account and the engine's defaults. `None` while there's no account.
    async fn assignment(&self, next: &Queued) -> Result<Option<Assignment>, String> {
        let on = |account: gathr::accounts::Account, agent, (model, effort, approvals), team| {
            Assignment {
                folder: account.folder(&self.chats.data),
                account: account.id,
                engine: account.engine,
                agent,
                model,
                effort,
                approvals,
                chat: next.ids.chat.clone(),
                team,
            }
        };
        let Some(id) = next.ids.agent.clone() else {
            let Some(id) = self.chats.account() else {
                return Ok(None);
            };
            return match self
                .chats
                .writer
                .call(move |conn| gathr::accounts::get(conn, &id))
                .await
            {
                Some(Ok(account)) => {
                    Ok(account.map(|a| on(a, None, (None, None, Default::default()), Vec::new())))
                }
                _ => Err("the account wasn't read".into()),
            };
        };
        let chat = next.ids.chat.clone();
        let read = move |conn: &mut rusqlite::Connection| {
            let team = gathr::team::members(conn, &chat)?;
            let team = team.into_iter().map(|m| (m.agent, m.role)).collect();
            let Some(agent) = agents::get(conn, &id)? else {
                return Ok(None);
            };
            let account = gathr::accounts::get(conn, &agent.account)?;
            Ok::<_, gathr::Error>(account.map(|account| (agent, account, team)))
        };
        match self.chats.writer.call(read).await {
            Some(Ok(Some((a, account, team)))) => {
                let given = (a.model, a.effort, a.approvals);
                Ok(Some(on(account, Some(a.id), given, team)))
            }
            Some(Ok(None)) => Err("its agent was deleted".into()),
            Some(Err(e)) => Err(format!("its agent wasn't read: {e}")),
            None => Err("its agent wasn't read: the database didn't answer".into()),
        }
    }

    /// Lets go of an account being removed: its engine closes, a turn under way stops, and
    /// `done` answers once the engine has exited.
    async fn leave(&mut self, folder: &std::path::Path, done: oneshot::Sender<()>) {
        let here = self.assigned.as_ref().map(|a| a.folder.as_path()) == Some(folder);
        if self.engine.is_some() && here {
            self.close("its account is being removed").await;
            if let Some(running) = self.turn.take() {
                self.set_turn(&running.ids, "stopped", None).await;
            }
        }
        match &self.closing {
            Some(closing) if closing.folder == folder => self.leaving.push(done),
            _ => _ = done.send(()),
        }
    }

    /// The turn under way from now: its span starts here.
    fn open_turn(&mut self, ids: Ids, gate: login::Hold, ask_chars: usize) {
        let span = span_id();
        let assigned = self.assigned.as_ref();
        if let Some(pass) = &self.pass {
            pass.on_turn(crate::tools::OnTurn {
                trace: ids.trace.clone(),
                turn: ids.turn.clone(),
                task: ids.task,
            });
        }
        // What Gathr gave the turn, in characters with a rough token estimate (0020: the engines
        // report totals only, so the parts are Gathr's own count, labelled an estimate).
        let briefing = self.pass.as_ref().map_or(0, |p| p.briefing_chars);
        let estimate = |chars: usize| chars.div_ceil(4);
        self.record(
            Event::point(&format!("invoke_agent {}", self.engine_name()))
                .attr("gathr.context.briefing.chars", briefing)
                .attr("gathr.context.briefing.tokens_estimate", estimate(briefing))
                .attr("gathr.context.ask.chars", ask_chars)
                .attr("gathr.context.ask.tokens_estimate", estimate(ask_chars))
                .attr("gathr.tools.off", crate::tools::briefing::off())
                .trace(&ids.trace)
                .chat(&ids.chat)
                .span("start", &span)
                .attr("gen_ai.operation.name", "invoke_agent")
                .attr("gen_ai.agent.id", ids.agent.clone())
                .attr(
                    "gen_ai.request.model",
                    assigned.and_then(|a| a.model.clone()),
                )
                .attr("gathr.effort", assigned.and_then(|a| a.effort.clone()))
                .attr("gathr.approvals", assigned.map(|a| a.approvals.as_str()))
                .attr("gathr.account", assigned.map(|a| a.account.clone()))
                .attr("gathr.turn.id", ids.turn.clone()),
        );
        self.turn = Some(Running {
            ids,
            span,
            sent_us: now_us(),
            partial: String::new(),
            partial_units: 0,
            streamed: false,
            tools: HashMap::new(),
            approvals: HashMap::new(),
            service_approvals: HashMap::new(),
            push_allowed: false,
            stopping: false,
            stop_why: None,
            signed_out: false,
            tools_started: 0,
            limited: false,
            progress_at: Instant::now(),
            stalled: false,
            gate,
        });
    }

    /// A turn the engine began itself, to answer a message sent into the last one.
    async fn begin_read_turn(&mut self, message: &str, ask_chars: usize) {
        let Some(chat) = self.assigned.as_ref().map(|a| a.chat.clone()) else {
            return;
        };
        let ids = Ids {
            chat,
            turn: ulid(now_us()),
            trace: gathr::ids::trace_id(),
            agent: self.agent.clone(),
            task: None,
            answers: None,
            person: true,
        };
        let (row, id) = (ids.clone(), message.to_string());
        let saved = self
            .chats
            .writer
            .call(move |conn| {
                store::add_turn(
                    conn,
                    &row.chat,
                    &row.turn,
                    &row.trace,
                    (row.agent.as_deref(), row.task, row.answers.as_deref()),
                )?;
                store::move_message(conn, &id, &row.turn)
            })
            .await;
        // An open window moves the message to its new turn, where its reply or failure shows.
        if let Some(Ok(message)) = &saved {
            self.publish(&ids.chat, json!({ "type": "message", "message": message }));
        }
        if let Some(Err(e)) = saved {
            let event = Event::point("gathr.turn.read")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
        self.open_turn(ids.clone(), login::Hold::default(), ask_chars);
        self.set_turn(&ids, "running", None).await;
    }

    fn engine_label(&self) -> &str {
        if self.engine_name() == "codex" {
            "Codex"
        } else {
            "Claude Code"
        }
    }

    fn engine_name(&self) -> &str {
        self.assigned
            .as_ref()
            .map_or("claude", |a| a.engine.as_str())
    }

    fn credentials(folder: &std::path::Path) -> PathBuf {
        if folder.file_name().is_some_and(|n| n == "codex") {
            gathr::codex::login::credentials(folder)
        } else {
            login::credentials(folder)
        }
    }

    async fn login(&mut self, folder: &std::path::Path) -> Login {
        let file = Self::credentials(folder);
        if let Some((_, stamp, when)) = self.refused.as_ref().filter(|r| r.0 == folder) {
            let (stamp, when) = (*stamp, *when);
            let unchanged = login_stamp(&file) == stamp;
            let file_based = stamp.is_some() || !cfg!(target_os = "macos");
            if unchanged && (file_based || when.elapsed() < REFUSED_RETRY) {
                return Login::SignedOut;
            }
            self.refused = None;
        }
        self.chats.login_status(folder).await.0
    }

    /// Stops the chat's queued turns, and its running one if it's this task's: their ids.
    async fn stop(&mut self, chat: &str) -> Vec<String> {
        let (theirs, others): (VecDeque<Queued>, _) = std::mem::take(&mut self.queue)
            .into_iter()
            .partition(|q| q.ids.chat == chat);
        self.queue = others;
        let mut stopped = Vec::new();
        for queued in theirs {
            self.set_turn(&queued.ids, "stopped", None).await;
            stopped.push(queued.ids.turn);
        }
        stopped.extend(self.interrupt(chat).await);
        stopped
    }

    /// Interrupts the chat's turn if it's this task's running one: its id.
    async fn interrupt(&mut self, chat: &str) -> Option<String> {
        let running = self
            .turn
            .as_mut()
            .filter(|t| !t.stopping && t.ids.chat == chat)?;
        running.stopping = true;
        let ids = running.ids.clone();
        // Shown at once; the turn's row and end event follow when the engine says it ended.
        self.publish(
            &ids.chat,
            json!({ "type": "turn", "turn": ids.turn, "status": "stopped", "agent": ids.agent }),
        );
        let event = Event::point("gathr.turn.stop").trace(&ids.trace);
        self.record(event.chat(&ids.chat).ok());
        let request = self.driver.interrupt(&format!("stop-{}", span_id()));
        if let Some(request) = request
            && let Err(e) = self.write(&request, Some(&ids.turn)).await
        {
            let event = Event::point("gathr.turn.stop")
                .trace(&ids.trace)
                .chat(&ids.chat)
                .error("engine_write")
                .attr("why", e.to_string());
            self.record(event);
        }
        Some(ids.turn)
    }

    async fn approve(&mut self, chat: &str, request_id: &str, allow: bool) -> bool {
        let Some(turn) = self.turn.as_mut().filter(|t| t.ids.chat == chat) else {
            return false;
        };
        let Some((tool, input, _)) = turn.approvals.remove(request_id) else {
            return false;
        };
        if let Some((kind, reply)) = turn.service_approvals.remove(request_id) {
            return self.answer_service(kind, request_id, allow, reply);
        }
        if allow && gathr::approvals::runs_push(&tool, &input) {
            turn.push_allowed = true;
        }
        let turn = turn.ids.turn.clone();
        let Some(answer) = self.driver.approval(request_id, allow, &input) else {
            return false;
        };
        self.write(&answer, Some(&turn)).await.is_ok()
    }

    /// The chat's questions still open, as the live socket sent them, for a window that opens
    /// the chat later.
    fn open_approvals(&self, chat: &str) -> Vec<Value> {
        let Some(turn) = self.turn.as_ref().filter(|t| t.ids.chat == chat) else {
            return Vec::new();
        };
        // A service action whose caller gave up has no one to answer.
        let open = turn.approvals.iter().filter(|(request_id, _)| {
            let push = turn.service_approvals.get(*request_id);
            push.is_none_or(|(_, reply)| !reply.is_closed())
        });
        let open = open.map(|(request_id, (tool, input, why))| {
            json!({ "turn": turn.ids.turn, "request_id": request_id, "tool": tool, "input": input,
                "why": why })
        });
        open.collect()
    }

    /// The chat's reply still streaming: the text since its last whole block, which isn't saved yet,
    /// with how many of the turn's messages are. Counted here, where nothing new is saved meanwhile,
    /// so whoever reads the chat after can tell a block saved since from the text still streaming.
    async fn streaming(&self, chat: &str) -> Option<Value> {
        let turn = self
            .turn
            .as_ref()
            .filter(|t| t.ids.chat == chat && !t.partial.is_empty())?;
        let (chat, id) = (chat.to_string(), turn.ids.turn.clone());
        let count = move |conn: &mut rusqlite::Connection| store::replies(conn, &chat, &id);
        let saved = self.chats.writer.call(count).await?.ok()?;
        Some(
            json!({ "turn": turn.ids.turn, "agent": turn.ids.agent, "text": turn.partial,
            "saved": saved }),
        )
    }
}
