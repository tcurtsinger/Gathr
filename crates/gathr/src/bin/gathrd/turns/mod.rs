//! One chat's task: it owns the chat's Claude Code and runs its turns one at a time
//! (docs/first-slice.md, docs/decisions/0001-engine-interface.md).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use gathr::claude::command::{self, Session};
use gathr::claude::engine::{Closed, Engine, Line};
use gathr::claude::frames;
use gathr::claude::login::{self, Login};
use gathr::event::Event;
use gathr::ids::{now_us, span_id, ulid};
use gathr::redact::redact;
use gathr::store::{self, RawFrame};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::chats::{Chats, Cmd, Failed};

mod reading;

/// How long a closing engine gets to exit on its own before it's killed (0001).
pub const CLOSE_GRACE: Duration = Duration::from_secs(10);
/// How often a chat waiting for sign-in looks at the login again.
const SIGN_IN_POLL: Duration = Duration::from_secs(2);

#[derive(Clone)]
struct Queued {
    turn: String,
    trace: String,
    text: String,
}

struct Running {
    turn: String,
    trace: String,
    span: String,
    sent_us: i64,
    /// Text streamed since the last whole block: what a stopped turn keeps.
    partial: String,
    streamed: bool,
    /// Tool calls under way: their message, name and start.
    tools: HashMap<String, (String, String, i64)>,
    /// Questions Claude Code asked and nobody has answered: each tool and its input.
    approvals: HashMap<String, (String, Value)>,
    stopping: bool,
    signed_out: bool,
    /// The account's renewal gate, held until the provider first answers (0016).
    gate: login::Hold,
}

struct Task {
    chats: Arc<Chats>,
    chat: String,
    engine: Option<Engine>,
    /// The file the engine runs, so a chat on Gathr's copy moves when it updates (0025).
    program: Option<std::path::PathBuf>,
    /// The account folder the engine runs in.
    folder: Option<std::path::PathBuf>,
    lines: Option<mpsc::UnboundedReceiver<Line>>,
    /// The engine's last stderr line: when it exits mid-turn, that's usually why (0019).
    last_stderr: Option<String>,
    turn: Option<Running>,
    queue: VecDeque<Queued>,
    /// The login file as it was when the engine refused it, and when: held as signed out
    /// until the file changes, so a refused login doesn't start an engine every poll.
    refused: Option<(Option<std::time::SystemTime>, std::time::Instant)>,
    /// Messages sent into a turn under way that the engine hasn't read yet: their id and text.
    unread: Vec<(String, String)>,
}

/// Where the login isn't in a file (macOS), a refused login is tried again after this long.
const REFUSED_RETRY: Duration = Duration::from_secs(60);

pub async fn run(chats: Arc<Chats>, chat: String, mut commands: mpsc::UnboundedReceiver<Cmd>) {
    let mut task = Task {
        chats,
        chat,
        engine: None,
        program: None,
        folder: None,
        lines: None,
        last_stderr: None,
        turn: None,
        queue: VecDeque::new(),
        refused: None,
        unread: Vec::new(),
    };
    // An interval, not a sleep in the select: a sleep restarts on every command, so a window
    // asking for the chat more often than this would hold a waiting message forever.
    let mut sign_in_poll = tokio::time::interval(SIGN_IN_POLL);
    sign_in_poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        let waiting = task.turn.is_none() && !task.queue.is_empty();
        tokio::select! {
            command = commands.recv() => match command {
                Some(Cmd::Close { done }) => {
                    task.close("the service is stopping").await;
                    let _ = done.send(());
                    return;
                }
                None => {
                    task.close("the chat's task ended").await;
                    return;
                }
                Some(command) => task.handle(command).await,
            },
            line = next(&mut task.lines) => match line {
                Some(line) => task.on_line(line).await,
                None => task.on_exit().await,
            },
            _ = sign_in_poll.tick(), if waiting => task.try_next().await,
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
            Cmd::Send { text, trace, reply } => {
                let _ = reply.send(self.send(text, trace).await);
            }
            Cmd::Stop { reply } => {
                let _ = reply.send(self.stop().await);
            }
            Cmd::Approve {
                request_id,
                allow,
                reply,
            } => {
                let _ = reply.send(self.approve(&request_id, allow).await);
            }
            Cmd::Approvals { reply } => {
                let _ = reply.send(self.open_approvals());
            }
            Cmd::Resume(w) => {
                self.queue.push_back(Queued {
                    turn: w.turn,
                    trace: w.trace,
                    text: w.text,
                });
                self.try_next().await;
            }
            Cmd::Leave { folder, done } => {
                self.leave(&folder).await;
                let _ = done.send(());
            }
            Cmd::Close { .. } => unreachable!("handled in run"),
        }
    }

    async fn send(&mut self, text: String, trace: String) -> Result<Value, Failed> {
        if let Some(running) = self.turn.as_ref().filter(|t| !t.stopping) {
            // G13: into the turn under way. Claude Code reads it after the step it's on and
            // says so (`Frame::Read`), as Esc-free typing does in its terminal.
            let (turn, trace) = (running.turn.clone(), running.trace.clone());
            let message = self.db_message(&turn, "user", "text", &text).await;
            if let Err(e) = self.write(&frames::user(&text), Some(&turn)).await {
                return Err(self.fail_send(&trace, format!("Claude Code didn't take it: {e}")));
            }
            let id = message["id"].as_str().unwrap_or_default().to_string();
            self.unread.push((id, text));
            self.publish(json!({ "type": "message", "message": message }));
            self.record(Event::point("gathr.turn.steer").trace(&trace).ok());
            return Ok(json!({ "turn": turn, "status": "running", "steered": true }));
        }
        let queued = Queued {
            turn: ulid(now_us()),
            trace,
            text,
        };
        let (chat, turn, trace) = (self.chat.clone(), queued.turn.clone(), queued.trace.clone());
        let added = self
            .chats
            .writer
            .call(move |conn| store::add_turn(conn, &chat, &turn, &trace))
            .await;
        if let Some(Err(e)) = added {
            return Err(self.fail_send(&queued.trace, e.to_string()));
        }
        let message = self
            .db_message(&queued.turn, "user", "text", &queued.text)
            .await;
        self.publish(json!({ "type": "message", "message": message }));
        self.record(
            Event::point("gathr.chat.send")
                .trace(&queued.trace)
                .ok()
                .attr("gathr.message.id", message["id"].clone())
                .attr("gathr.turn.id", queued.turn.clone())
                .attr("gathr.message.chars", queued.text.chars().count()),
        );
        let reference = gathr::ids::reference(&queued.trace).to_string();
        let answer =
            |status: &str| json!({ "turn": queued.turn, "status": status, "ref": reference });
        if self.turn.is_some() || !self.queue.is_empty() {
            self.queue.push_back(queued.clone());
            self.set_turn(&queued.turn, &queued.trace, "queued", None)
                .await;
            return Ok(answer("queued"));
        }
        match self.start(&queued).await {
            Ok(status) => Ok(answer(status)),
            Err(why) => {
                self.set_turn(&queued.turn, &queued.trace, "failed", Some(&why))
                    .await;
                Err(Failed {
                    why,
                    trace: queued.trace,
                })
            }
        }
    }

    /// Starts the turns waiting, in order, while nothing runs.
    async fn try_next(&mut self) {
        while self.turn.is_none() {
            let Some(next) = self.queue.front().cloned() else {
                return;
            };
            match self.start(&next).await {
                Ok("running") => {
                    self.queue.pop_front();
                }
                Ok(_) => return,
                Err(why) => {
                    self.queue.pop_front();
                    self.set_turn(&next.turn, &next.trace, "failed", Some(&why))
                        .await;
                }
            }
        }
    }

    /// Sends a turn to the engine, starting it first if needed: `running`, or
    /// `waiting_sign_in` (it stays first in the queue), or why it couldn't.
    async fn start(&mut self, next: &Queued) -> Result<&'static str, String> {
        if let Some(version) = self.copy_moved() {
            let why = format!("Gathr's copy of Claude Code is now {version}");
            self.close(&why).await;
        }
        let folder = self.chats.account_folder();
        if self.engine.is_some() && self.folder != folder {
            self.close("the chats moved to another account").await;
        }
        // A wrong path fails the send at once, signed in or not (hand check 8).
        let program = match self.engine {
            Some(_) => None,
            None => Some(self.find_engine(&next.trace).await?),
        };
        // No account yet waits like a signed-out one: adding one and signing it in sends it.
        let Some(folder) = folder else {
            return Ok(self.wait_for_sign_in(next).await);
        };
        if self.login(&folder).await == Login::SignedOut {
            return Ok(self.wait_for_sign_in(next).await);
        }
        let gate = self.chats.gate(&folder).pass().await;
        if let Some(program) = program {
            self.spawn(&program, &folder, &next.trace).await?;
        }
        self.open_turn(&next.turn, &next.trace, gate);
        if let Err(e) = self
            .write(&frames::user(&next.text), Some(&next.turn))
            .await
        {
            self.turn = None;
            return Err(format!("Claude Code didn't take the message: {e}"));
        }
        self.set_turn(&next.turn, &next.trace, "running", None)
            .await;
        Ok("running")
    }

    async fn wait_for_sign_in(&mut self, next: &Queued) -> &'static str {
        if self.queue.front().map(|q| &q.turn) != Some(&next.turn) {
            self.queue.push_front(next.clone());
        }
        self.set_turn(&next.turn, &next.trace, "waiting_sign_in", None)
            .await;
        "waiting_sign_in"
    }

    /// Lets go of an account being removed: its engine closes and a turn under way stops.
    async fn leave(&mut self, folder: &std::path::Path) {
        if self.engine.is_none() || self.folder.as_deref() != Some(folder) {
            return;
        }
        self.close("its account is being removed").await;
        if let Some(running) = self.turn.take() {
            self.set_turn(&running.turn, &running.trace, "stopped", None)
                .await;
        }
    }

    /// The turn under way from now: its span starts here.
    fn open_turn(&mut self, turn: &str, trace: &str, gate: login::Hold) {
        let span = span_id();
        self.record(
            Event::point("invoke_agent claude")
                .trace(trace)
                .span("start", &span)
                .attr("gen_ai.operation.name", "invoke_agent")
                .attr("gathr.turn.id", turn),
        );
        self.turn = Some(Running {
            turn: turn.to_string(),
            trace: trace.to_string(),
            span,
            sent_us: now_us(),
            partial: String::new(),
            streamed: false,
            tools: HashMap::new(),
            approvals: HashMap::new(),
            stopping: false,
            signed_out: false,
            gate,
        });
    }

    /// A turn the engine began itself, to answer a message sent into the last one.
    async fn begin_read_turn(&mut self, message: &str) {
        let (turn, trace) = (ulid(now_us()), gathr::ids::trace_id());
        let (chat, id, t, tr) = (
            self.chat.clone(),
            message.to_string(),
            turn.clone(),
            trace.clone(),
        );
        let saved = self
            .chats
            .writer
            .call(move |conn| {
                store::add_turn(conn, &chat, &t, &tr)?;
                store::move_message(conn, &id, &t)
            })
            .await;
        if let Some(Err(e)) = saved {
            let event = Event::point("gathr.turn.read")
                .trace(&trace)
                .error("sqlite");
            self.record(event.attr("why", e.to_string()));
        }
        self.open_turn(&turn, &trace, login::Hold::default());
        self.set_turn(&turn, &trace, "running", None).await;
    }

    async fn login(&mut self, folder: &std::path::Path) -> Login {
        let file = login::credentials(folder);
        if let Some((stamp, when)) = self.refused {
            let unchanged = login_stamp(&file) == stamp;
            let file_based = stamp.is_some() || !cfg!(target_os = "macos");
            if unchanged && (file_based || when.elapsed() < REFUSED_RETRY) {
                return Login::SignedOut;
            }
            self.refused = None;
        }
        self.chats.login_status(folder).await.0
    }

    /// Claude Code's path, found at every start so a new copy applies at once.
    async fn find_engine(&self, trace: &str) -> Result<std::path::PathBuf, String> {
        let program = command::find(self.chats.copy()).inspect_err(|why| {
            self.record(
                Event::point("gathr.engine.start")
                    .trace(trace)
                    .error("engine_not_found")
                    .attr("why", why.clone()),
            );
        })?;
        // An engine too old for the switches it's given fails here by its version, not later
        // with whatever it prints about an unknown switch.
        let path = program.clone();
        let (version, usable) = tokio::task::spawn_blocking(move || command::version(&path))
            .await
            .unwrap_or_else(|e| {
                (
                    None,
                    Err(format!("Claude Code's version check stopped: {e}")),
                )
            });
        usable.map(|()| program.clone()).inspect_err(|why| {
            self.record(
                Event::point("gathr.engine.start")
                    .trace(trace)
                    .error("engine_version")
                    .attr("why", why.clone())
                    .attr("version", version)
                    .attr("path", program.display().to_string()),
            );
        })
    }

    async fn spawn(
        &mut self,
        program: &std::path::Path,
        folder: &std::path::Path,
        trace: &str,
    ) -> Result<(), String> {
        let chat = self.chat.clone();
        let session = self
            .chats
            .writer
            .call(move |conn| store::engine_session(conn, &chat))
            .await
            .and_then(Result::ok)
            .flatten();
        let new_id = gathr::ids::uuid();
        let args = match &session {
            Some(id) => command::args(Session::Resume(id)),
            None => command::args(Session::New(&new_id)),
        };
        let cwd = self.chats.data.path().join("chats").join(&self.chat);
        for dir in [folder, &cwd] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
        }
        let env = command::env(std::env::vars_os(), folder);
        match Engine::start(program, &args, env, &cwd) {
            Ok((engine, lines)) => {
                self.record(
                    Event::point("gathr.engine.start")
                        .trace(trace)
                        .chat(&self.chat)
                        .ok()
                        .attr("process.pid", engine.pid)
                        .attr("path", program.display().to_string())
                        .attr("session", session.clone().unwrap_or(new_id))
                        .attr("resumed", session.is_some()),
                );
                self.engine = Some(engine);
                self.program = Some(program.to_path_buf());
                self.folder = Some(folder.to_path_buf());
                self.lines = Some(lines);
                self.last_stderr = None;
                Ok(())
            }
            Err(e) => {
                let why = format!("Claude Code didn't start from {}: {e}", program.display());
                let event = Event::point("gathr.engine.start")
                    .trace(trace)
                    .chat(&self.chat)
                    .error("engine_start")
                    .attr("why", why.clone())
                    .attr("path", program.display().to_string());
                self.record(event);
                Err(why)
            }
        }
    }

    async fn stop(&mut self) -> Value {
        let mut stopped = Vec::new();
        for queued in std::mem::take(&mut self.queue) {
            self.set_turn(&queued.turn, &queued.trace, "stopped", None)
                .await;
            stopped.push(queued.turn);
        }
        let Some(running) = self.turn.as_mut().filter(|t| !t.stopping) else {
            return json!({ "stopped": stopped });
        };
        running.stopping = true;
        let (turn, trace) = (running.turn.clone(), running.trace.clone());
        // Shown at once; the turn's row and end event follow when the engine says it ended.
        self.publish(
            json!({ "type": "turn", "chat": self.chat, "turn": turn, "status": "stopped" }),
        );
        self.record(
            Event::point("gathr.turn.stop")
                .trace(&trace)
                .chat(&self.chat)
                .ok(),
        );
        let request = frames::interrupt(&format!("stop-{}", span_id()));
        if let Err(e) = self.write(&request, Some(&turn)).await {
            let event = Event::point("gathr.turn.stop")
                .trace(&trace)
                .error("engine_write")
                .attr("why", e.to_string());
            self.record(event);
        }
        stopped.push(turn);
        json!({ "stopped": stopped })
    }

    async fn approve(&mut self, request_id: &str, allow: bool) -> bool {
        let Some(input) = self
            .turn
            .as_mut()
            .and_then(|t| t.approvals.remove(request_id))
            .map(|(_, input)| input)
        else {
            return false;
        };
        let turn = self.turn.as_ref().map(|t| t.turn.clone());
        let answer = frames::approval(request_id, allow, &input);
        self.write(&answer, turn.as_deref()).await.is_ok()
    }

    /// The questions still open, as the live socket sent them, for a window that opens the chat later.
    fn open_approvals(&self) -> Value {
        let Some(turn) = self.turn.as_ref() else {
            return json!([]);
        };
        let open = turn.approvals.iter().map(|(request_id, (tool, input))| {
            json!({ "turn": turn.turn, "request_id": request_id, "tool": tool, "input": input })
        });
        Value::Array(open.collect())
    }

    /// The engine's output ended: it has exited.
    async fn on_exit(&mut self) {
        self.lines = None;
        if let Some(turn) = self.turn.as_mut() {
            turn.gate.release();
        }
        let code = match self.engine.take() {
            Some(engine) => match engine.close(CLOSE_GRACE).await {
                Closed::Exited(code) => code,
                Closed::Killed => None,
            },
            None => None,
        };
        let event = Event::point("gathr.engine.exit")
            .chat(&self.chat)
            .attr("process.exit.code", code);
        let last_stderr = self.last_stderr.take();
        self.keep_what_was_shown().await;
        let Some(turn) = self.turn.take() else {
            self.record(event);
            return self.try_next().await;
        };
        let code = code.map_or("none".into(), |c| c.to_string());
        let mut why = format!("Claude Code exited during the turn (exit code {code})");
        if let Some(line) = last_stderr {
            why = format!("{why}: {}", redact(line.trim()));
        }
        // On the turn's trace, so its reference finds why (0019).
        self.record(
            event
                .trace(&turn.trace)
                .error("exited")
                .attr("why", why.clone()),
        );
        self.set_turn(&turn.turn, &turn.trace, "failed", Some(&why))
            .await;
        self.try_next().await;
    }

    /// The gentle close (0001): interrupt a turn under way, close stdin, wait for the exit.
    /// The version Gathr's copy moved to, when the engine runs an older copy: called between
    /// turns only, so a turn is never cut short and a renewal, which happens inside one, never is.
    fn copy_moved(&self) -> Option<String> {
        let running = self.program.as_ref().filter(|_| self.engine.is_some())?;
        let copies = self.chats.data.path().join("engines");
        let current = self
            .chats
            .copy()
            .filter(|c| running.starts_with(&copies) && c != running)?;
        let version = current.parent()?.file_name()?;
        Some(version.to_string_lossy().into_owned())
    }

    async fn close(&mut self, why: &str) {
        let Some(mut engine) = self.engine.take() else {
            return;
        };
        self.lines = None;
        if self.turn.as_ref().is_some_and(|t| !t.stopping) {
            let _ = engine.write(&frames::interrupt("close")).await;
        }
        let pid = engine.pid;
        self.keep_what_was_shown().await;
        let event = match engine.close(CLOSE_GRACE).await {
            Closed::Exited(code) => Event::point("gathr.engine.close")
                .ok()
                .attr("process.exit.code", code),
            Closed::Killed => Event::point("gathr.engine.close").error("killed").attr(
                "killed_because",
                "it hadn't exited 10 s after its input closed",
            ),
        };
        self.record_now(
            event
                .chat(&self.chat)
                .attr("process.pid", pid)
                .attr("why", why),
        )
        .await;
    }

    async fn write(&mut self, frame: &Value, turn: Option<&str>) -> std::io::Result<()> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Claude Code isn't running"))?;
        let pid = engine.pid;
        engine.write(frame).await?;
        self.raw(turn, pid, "in", &frame.to_string()).await;
        Ok(())
    }

    async fn raw(&self, turn: Option<&str>, pid: u32, direction: &'static str, line: &str) {
        let (chat, turn, line) = (self.chat.clone(), turn.map(String::from), line.to_string());
        let written = self
            .chats
            .writer
            .call(move |conn| {
                let frame = RawFrame {
                    chat: &chat,
                    turn: turn.as_deref(),
                    pid,
                    direction,
                    line: &line,
                };
                store::add_raw(conn, &frame)
            })
            .await;
        if let Some(Err(e)) = written {
            self.record(
                Event::point("gathr.engine.raw")
                    .error("sqlite")
                    .attr("why", e.to_string()),
            );
        }
    }

    async fn db_message(
        &self,
        turn: &str,
        role: &'static str,
        kind: &'static str,
        body: &str,
    ) -> Value {
        let (chat, turn, body) = (self.chat.clone(), turn.to_string(), body.to_string());
        let added = self
            .chats
            .writer
            .call(move |conn| store::add_message(conn, &chat, &turn, role, kind, &body))
            .await;
        match added {
            Some(Ok(message)) => message,
            Some(Err(e)) => {
                self.record(
                    Event::point("gathr.chat.message")
                        .error("sqlite")
                        .attr("why", e.to_string()),
                );
                Value::Null
            }
            None => Value::Null,
        }
    }

    async fn set_turn(&self, turn: &str, trace: &str, status: &str, why: Option<&str>) {
        let (id, status_owned, why_owned) =
            (turn.to_string(), status.to_string(), why.map(String::from));
        let saved = self
            .chats
            .writer
            .call(move |conn| store::set_turn(conn, &id, &status_owned, why_owned.as_deref()))
            .await;
        if let Some(Err(e)) = saved {
            self.record(
                Event::point("gathr.turn.status")
                    .trace(trace)
                    .error("sqlite")
                    .attr("why", e.to_string()),
            );
        }
        let reference = gathr::ids::reference(trace);
        self.publish(
            json!({ "type": "turn", "chat": self.chat, "turn": turn, "status": status,
            "why": why, "ref": reference }),
        );
    }

    fn fail_send(&self, trace: &str, why: String) -> Failed {
        self.record(
            Event::point("gathr.chat.send")
                .trace(trace)
                .chat(&self.chat)
                .error("send")
                .attr("why", why.clone()),
        );
        Failed {
            why,
            trace: trace.to_string(),
        }
    }

    fn publish(&self, mut event: Value) {
        if let Some(object) = event.as_object_mut() {
            object
                .entry("chat")
                .or_insert_with(|| self.chat.clone().into());
        }
        self.chats.publish(event);
    }

    fn record(&self, event: Event) {
        let event = match event.conversation_id {
            Some(_) => event,
            None => event.chat(&self.chat),
        };
        self.chats.writer.record(event);
    }

    async fn record_now(&self, event: Event) {
        self.chats.writer.record_now(event).await;
    }
}
