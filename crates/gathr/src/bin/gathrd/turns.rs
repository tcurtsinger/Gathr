//! One chat's task: it owns the chat's Claude Code and runs its turns one at a time
//! (docs/first-slice.md, docs/decisions/0001-engine-interface.md).

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::Duration;

use gathr::claude::command::{self, Session};
use gathr::claude::engine::{Closed, Engine, Line};
use gathr::claude::frames::{self, Frame, TurnEnd};
use gathr::claude::login::{self, Login};
use gathr::event::Event;
use gathr::ids::{now_us, span_id, ulid};
use gathr::redact::redact;
use gathr::store::{self, RawFrame};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use crate::chats::{Chats, Cmd, Failed};

/// How long a closing engine gets to exit on its own before it's killed (0001).
const CLOSE_GRACE: Duration = Duration::from_secs(10);
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
    approvals: HashMap<String, Value>,
    stopping: bool,
    signed_out: bool,
}

struct Task {
    chats: Arc<Chats>,
    chat: String,
    engine: Option<Engine>,
    lines: Option<mpsc::UnboundedReceiver<Line>>,
    /// The engine's last stderr line: when it exits mid-turn, that's usually why (0019).
    last_stderr: Option<String>,
    turn: Option<Running>,
    queue: VecDeque<Queued>,
    /// The login file as it was when the engine refused it, and when: held as signed out
    /// until the file changes, so a refused login doesn't start an engine every poll.
    refused: Option<(Option<std::time::SystemTime>, std::time::Instant)>,
}

/// Where the login isn't in a file (macOS), a refused login is tried again after this long.
const REFUSED_RETRY: Duration = Duration::from_secs(60);

pub async fn run(chats: Arc<Chats>, chat: String, mut commands: mpsc::UnboundedReceiver<Cmd>) {
    let mut task = Task {
        chats,
        chat,
        engine: None,
        lines: None,
        last_stderr: None,
        turn: None,
        queue: VecDeque::new(),
        refused: None,
    };
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
            _ = tokio::time::sleep(SIGN_IN_POLL), if waiting => task.try_next().await,
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
            Cmd::Resume(w) => {
                self.queue.push_back(Queued {
                    turn: w.turn,
                    trace: w.trace,
                    text: w.text,
                });
                self.try_next().await;
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
        // A wrong path fails the send at once, signed in or not (hand check 8).
        let program = match self.engine {
            Some(_) => None,
            None => Some(self.find_engine(&next.trace).await?),
        };
        if self.login(program.as_deref()).await == Login::SignedOut {
            if self.queue.front().map(|q| &q.turn) != Some(&next.turn) {
                self.queue.push_front(next.clone());
            }
            self.set_turn(&next.turn, &next.trace, "waiting_sign_in", None)
                .await;
            return Ok("waiting_sign_in");
        }
        self.chats.gate.pass().await;
        if let Some(program) = program {
            self.spawn(&program, &next.trace).await?;
        }
        let span = span_id();
        self.turn = Some(Running {
            turn: next.turn.clone(),
            trace: next.trace.clone(),
            span: span.clone(),
            sent_us: now_us(),
            partial: String::new(),
            streamed: false,
            tools: HashMap::new(),
            approvals: HashMap::new(),
            stopping: false,
            signed_out: false,
        });
        if let Err(e) = self
            .write(&frames::user(&next.text), Some(&next.turn))
            .await
        {
            self.turn = None;
            return Err(format!("Claude Code didn't take the message: {e}"));
        }
        self.record(
            Event::point("invoke_agent claude")
                .trace(&next.trace)
                .span("start", &span)
                .attr("gen_ai.operation.name", "invoke_agent")
                .attr("gathr.turn.id", next.turn.clone()),
        );
        self.set_turn(&next.turn, &next.trace, "running", None)
            .await;
        Ok("running")
    }

    async fn login(&mut self, program: Option<&std::path::Path>) -> Login {
        let folder = self.chats.account_folder();
        let file = login::credentials(&folder);
        if let Some((stamp, when)) = self.refused {
            let unchanged = login_stamp(&file) == stamp;
            let file_based = stamp.is_some() || !cfg!(target_os = "macos");
            if unchanged && (file_based || when.elapsed() < REFUSED_RETRY) {
                return Login::SignedOut;
            }
            self.refused = None;
        }
        let (state, _) = login::read(&file, now_us() / 1000);
        if cfg!(target_os = "macos") && state == Login::SignedOut {
            let program = match program {
                Some(program) => program.to_path_buf(),
                None => match command::find(self.chats.path_setting().await.as_deref()) {
                    Ok(program) => program,
                    Err(_) => return state,
                },
            };
            return login::probe(&program, command::env(std::env::vars_os(), &folder)).await;
        }
        state
    }

    /// Claude Code's path, from Settings at every start so a change applies at once (S26).
    async fn find_engine(&self, trace: &str) -> Result<std::path::PathBuf, String> {
        let setting = self.chats.path_setting().await;
        command::find(setting.as_deref()).inspect_err(|why| {
            self.record(
                Event::point("gathr.engine.start")
                    .trace(trace)
                    .error("engine_not_found")
                    .attr("why", why.clone())
                    .attr("path_setting", setting.clone()),
            );
        })
    }

    async fn spawn(&mut self, program: &std::path::Path, trace: &str) -> Result<(), String> {
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
        let folder = self.chats.account_folder();
        let cwd = self.chats.data.path().join("chats").join(&self.chat);
        for dir in [&folder, &cwd] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
        }
        let env = command::env(std::env::vars_os(), &folder);
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
        else {
            return false;
        };
        let turn = self.turn.as_ref().map(|t| t.turn.clone());
        let answer = frames::approval(request_id, allow, &input);
        self.write(&answer, turn.as_deref()).await.is_ok()
    }

    async fn on_line(&mut self, line: Line) {
        let pid = self.engine.as_ref().map_or(0, |e| e.pid);
        let turn = self.turn.as_ref().map(|t| t.turn.clone());
        let (direction, text) = match &line {
            Line::Out(text) => ("out", text),
            Line::Err(text) => ("err", text),
        };
        // Written, redacted, before it's acted on (0011).
        self.raw(turn.as_deref(), pid, direction, text).await;
        let Line::Out(text) = line else {
            self.record(
                Event::point("gathr.engine.stderr")
                    .chat(&self.chat)
                    .attr("line", text.clone()),
            );
            if !text.trim().is_empty() {
                self.last_stderr = Some(text.clone());
            }
            return;
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(value) => {
                for frame in frames::read(&value) {
                    self.on_frame(frame).await;
                }
            }
            Err(e) => self.record(
                Event::point("gathr.engine.frame")
                    .chat(&self.chat)
                    .error("malformed")
                    .attr("why", e.to_string()),
            ),
        }
    }

    async fn on_frame(&mut self, frame: Frame) {
        let chat = self.chat.clone();
        match frame {
            Frame::Init {
                session_id, tools, ..
            } => {
                self.check_tools(&tools);
                let saved = self
                    .chats
                    .writer
                    .call(move |conn| store::set_engine_session(conn, &chat, &session_id))
                    .await;
                if let Some(Err(e)) = saved {
                    self.record(
                        Event::point("gathr.engine.session")
                            .error("sqlite")
                            .attr("why", e.to_string()),
                    );
                }
            }
            Frame::TextDelta(text) => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.partial.push_str(&text);
                let first = !std::mem::replace(&mut turn.streamed, true);
                let (id, trace, sent) = (turn.turn.clone(), turn.trace.clone(), turn.sent_us);
                self.publish(json!({ "type": "delta", "chat": chat, "turn": id, "text": text }));
                if first {
                    let mut event = Event::point("gathr.turn.first_token").trace(&trace).ok();
                    event.duration_us = Some(now_us() - sent);
                    self.record(event);
                }
            }
            Frame::Text(text) => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.partial.clear();
                let id = turn.turn.clone();
                if !text.is_empty() {
                    let message = self.db_message(&id, "assistant", "text", &text).await;
                    self.publish(json!({ "type": "message", "message": message }));
                }
            }
            Frame::ToolStart { id, name } => self.tool_started(id, name).await,
            Frame::ToolEnd { id, ok } => {
                self.tool_ended(&id, if ok { "ok" } else { "error" }).await
            }
            Frame::Read(text) => {
                let turn = self.turn.as_ref().map(|t| t.turn.clone());
                self.publish(json!({ "type": "read", "chat": chat, "turn": turn, "text": text }));
            }
            Frame::ApprovalAsked {
                request_id,
                tool,
                input,
            } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.approvals.insert(request_id.clone(), input.clone());
                let id = turn.turn.clone();
                self.publish(json!({ "type": "approval", "chat": chat, "turn": id,
                    "request_id": request_id, "tool": tool, "input": input }));
            }
            Frame::EngineError { kind, message } => {
                let Some(turn) = self.turn.as_mut() else {
                    return;
                };
                turn.signed_out |= kind == "authentication_failed";
                let trace = turn.trace.clone();
                self.record(
                    Event::point("gathr.engine.error")
                        .trace(&trace)
                        .error(&kind)
                        .attr("message", message),
                );
            }
            Frame::TurnEnd(end) => self.turn_ended(end).await,
            // A sub-agent's frames stay in the raw record under their Agent call, never in the
            // agent's own messages or tools (friction ledger G24).
            Frame::Sub { .. } | Frame::ControlAnswer { .. } | Frame::Other(_) => {}
        }
    }

    /// Records a tool the engine offers that's denied, or that nobody has looked at (0001).
    fn check_tools(&self, tools: &[String]) {
        let denied: Vec<&String> = tools
            .iter()
            .filter(|t| command::DENIED_TOOLS.iter().any(|(d, _)| d == t))
            .collect();
        let unknown: Vec<&String> = tools
            .iter()
            .filter(|t| !command::KNOWN_TOOLS.contains(&t.as_str()))
            .collect();
        if !denied.is_empty() || !unknown.is_empty() {
            self.record(
                Event::point("gathr.engine.tools")
                    .chat(&self.chat)
                    .error("tools_changed")
                    .attr("denied_but_offered", json!(denied))
                    .attr("not_looked_at", json!(unknown)),
            );
        }
    }

    async fn tool_started(&mut self, id: String, name: String) {
        let Some(turn) = self.turn.as_ref() else {
            return;
        };
        let turn_id = turn.turn.clone();
        let body = json!({ "tool": name, "status": "running" }).to_string();
        let message = self.db_message(&turn_id, "assistant", "tool", &body).await;
        let message_id = message["id"].as_str().unwrap_or_default().to_string();
        self.publish(json!({ "type": "message", "message": message }));
        if let Some(turn) = self.turn.as_mut() {
            turn.tools.insert(id, (message_id, name, now_us()));
        }
    }

    async fn tool_ended(&mut self, id: &str, status: &str) {
        let Some(turn) = self.turn.as_mut() else {
            return;
        };
        let Some((message_id, name, started)) = turn.tools.remove(id) else {
            return;
        };
        let (trace, span) = (turn.trace.clone(), turn.span.clone());
        let duration_us = now_us() - started;
        let body = json!({ "tool": name, "status": status, "duration_ms": duration_us / 1000 });
        let (body_text, mid) = (body.to_string(), message_id.clone());
        self.chats
            .writer
            .call(move |conn| store::set_message_body(conn, &mid, &body_text))
            .await;
        self.publish(
            json!({ "type": "tool", "chat": self.chat, "message": message_id, "body": body }),
        );
        let mut event = Event::point(&format!("execute_tool {name}"))
            .trace(&trace)
            .chat(&self.chat)
            .span("end", &span_id())
            .attr("gen_ai.tool.name", name);
        event.parent_span_id = Some(span);
        event.duration_us = Some(duration_us);
        self.record(if status == "ok" {
            event.ok()
        } else {
            event.error("tool_error")
        });
    }

    async fn turn_ended(&mut self, end: TurnEnd) {
        let Some(mut turn) = self.turn.take() else {
            return;
        };
        if !turn.partial.is_empty() {
            let text = std::mem::take(&mut turn.partial);
            let message = self
                .db_message(&turn.turn, "assistant", "text", &text)
                .await;
            self.publish(json!({ "type": "message", "message": message }));
        }
        let open: Vec<String> = turn.tools.keys().cloned().collect();
        self.turn = Some(turn);
        for id in open {
            self.tool_ended(&id, "stopped").await;
        }
        let turn = self.turn.take().expect("put back above");
        let status = match () {
            _ if turn.signed_out => "waiting_sign_in",
            _ if turn.stopping => "stopped",
            _ if end.ok => "done",
            _ => "failed",
        };
        let why = (status == "failed").then(|| {
            end.message
                .clone()
                .or(end.reason.clone())
                .unwrap_or_else(|| end.subtype.clone())
        });
        let mut event = Event::point("invoke_agent claude")
            .trace(&turn.trace)
            .chat(&self.chat)
            .span("end", &turn.span)
            .attr("gathr.turn.status", status)
            .attr("gen_ai.usage.input_tokens", end.usage.input)
            .attr("gen_ai.usage.output_tokens", end.usage.output)
            .attr("gen_ai.usage.cache_read.input_tokens", end.usage.cache_read)
            .attr(
                "gen_ai.usage.cache_creation.input_tokens",
                end.usage.cache_write,
            )
            .attr("gathr.cost_usd", end.cost_usd)
            .attr(
                "gen_ai.response.finish_reasons",
                json!([end.reason.clone().unwrap_or(end.subtype.clone())]),
            );
        event.duration_us = Some(now_us() - turn.sent_us);
        event = match status {
            "failed" => event.error("turn_failed").attr("why", why.clone()),
            "waiting_sign_in" => event.error("signed_out"),
            _ => event.ok(),
        };
        self.record(event);
        if turn.signed_out {
            // The message waits for a sign-in (0002); the engine that failed is closed.
            self.queue.push_front(Queued {
                turn: turn.turn.clone(),
                trace: turn.trace.clone(),
                text: self.text_of(&turn.turn).await,
            });
            self.close("the login was refused").await;
            let file = login::credentials(&self.chats.account_folder());
            self.refused = Some((login_stamp(&file), std::time::Instant::now()));
        }
        self.set_turn(&turn.turn, &turn.trace, status, why.as_deref())
            .await;
        self.try_next().await;
    }

    async fn text_of(&self, turn: &str) -> String {
        let turn = turn.to_string();
        self.chats
            .writer
            .call(move |conn| {
                conn.query_row(
                    "SELECT body FROM messages WHERE turn_id = ?1 AND role = 'user' ORDER BY ts LIMIT 1",
                    [turn],
                    |r| r.get::<_, String>(0),
                )
            })
            .await
            .and_then(Result::ok)
            .unwrap_or_default()
    }

    /// The engine's output ended: it has exited.
    async fn on_exit(&mut self) {
        self.lines = None;
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
    async fn close(&mut self, why: &str) {
        let Some(mut engine) = self.engine.take() else {
            return;
        };
        self.lines = None;
        if self.turn.as_ref().is_some_and(|t| !t.stopping) {
            let _ = engine.write(&frames::interrupt("close")).await;
        }
        let pid = engine.pid;
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
