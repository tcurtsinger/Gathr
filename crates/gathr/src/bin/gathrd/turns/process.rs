//! The engine's life: found, started in its chat's session, closed the gentle way. A close never
//! kills (0001, 0010 Sessions): an engine still there after 30 s holds the next turn and shows as
//! a hold on its agent. Only shutdown kills, after its grace period.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gathr::claude::command::{self, Session};
use gathr::claude::engine::{Closed, Engine};
use gathr::claude::frames;
use gathr::event::Event;
use gathr::ids::now_us;
use gathr::redact::redact;
use gathr::store::{self, RawFrame};
use serde_json::json;

use super::{Assignment, Task};
use crate::tools::{Pass, briefing};

/// How long an engine gets at shutdown to exit on its own before it's killed (0001).
pub const CLOSE_GRACE: Duration = Duration::from_secs(10);
const HOLD_AFTER: Duration = Duration::from_secs(30);
const IDLE_CLOSE: Duration = Duration::from_secs(600);

/// When a close shows as a hold, and when an idle engine closes.
#[derive(Clone, Copy)]
pub struct Times {
    pub hold_after: Duration,
    pub idle_close: Duration,
}

impl Times {
    /// The decisions' times; `GATHR_HOLD_AFTER_MS` and `GATHR_IDLE_CLOSE_MS` shorten them for
    /// tests and dev only.
    pub fn from_env() -> Times {
        let ms = |name: &str, default: Duration| {
            let set = std::env::var(name).ok().and_then(|v| v.parse().ok());
            set.map_or(default, Duration::from_millis)
        };
        Times {
            hold_after: ms("GATHR_HOLD_AFTER_MS", HOLD_AFTER),
            idle_close: ms("GATHR_IDLE_CLOSE_MS", IDLE_CLOSE),
        }
    }
}

/// An engine whose input is closed, waiting for its own exit.
pub struct Closing {
    engine: Engine,
    /// Its token and briefing: they end when it has exited, not when it's asked to.
    pass: Option<Pass>,
    pub folder: PathBuf,
    chat: String,
    why: String,
    since: Instant,
    held: bool,
}

/// The closing engine's exit code once it exits, or `None` when it reaches the hold time first.
pub async fn closing(closing: &mut Option<Closing>, hold_after: Duration) -> Option<Option<i32>> {
    let Some(c) = closing else {
        return std::future::pending().await;
    };
    if c.held {
        return Some(c.engine.wait().await);
    }
    let at = tokio::time::Instant::from_std(c.since + hold_after);
    tokio::time::timeout_at(at, c.engine.wait()).await.ok()
}

impl Task {
    /// Claude Code's path, found at every start so a new copy applies at once.
    pub(super) async fn find_engine(&self, trace: &str) -> Result<PathBuf, String> {
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

    /// Starts the engine in the agent's session for the chat, resumed when it has one.
    pub(super) async fn spawn(
        &mut self,
        program: &Path,
        assigned: Assignment,
        trace: &str,
    ) -> Result<(), String> {
        let a = assigned.clone();
        let session = self
            .chats
            .writer
            .call(move |conn| store::engine_session(conn, &a.chat, a.agent.as_deref(), &a.account))
            .await
            .and_then(Result::ok)
            .flatten();
        let new_id = gathr::ids::uuid();
        let given = command::Assigned {
            model: assigned.model.as_deref(),
            effort: assigned.effort.as_deref(),
            approvals: assigned.approvals,
        };
        let hook = format!(
            "http://127.0.0.1:{}{}",
            self.chats.port,
            crate::tools::HOOK_PATH
        );
        let mut args = match &session {
            Some(id) => command::args(Session::Resume(id), given, Some(&hook)),
            None => command::args(Session::New(&new_id), given, Some(&hook)),
        };
        let folder = assigned.folder.as_path();
        let place = self.workspace(&assigned, trace).await?;
        let cwd = place.cwd;
        for dir in [folder, &cwd] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
        }
        let worktree = place.project.map(|project| (cwd.clone(), project));
        let pass = self.pass_for(&assigned, worktree, &cwd, trace).await?;
        let url = format!("http://127.0.0.1:{}/mcp", self.chats.port);
        args.extend(command::given(&url, &pass.briefing));
        let (env_from, env_why) = (place.env.from, place.env.why);
        let mut env = command::env(place.env.vars.into_iter(), folder);
        env.push((command::TOKEN_VAR.into(), pass.token.clone().into()));
        let started = Instant::now();
        match Engine::start(program, &args, env, &cwd) {
            Ok((engine, lines)) => {
                self.pass = Some(pass);
                self.record(
                    Event::point("gathr.engine.start")
                        .trace(trace)
                        .chat(&assigned.chat)
                        .ok()
                        .attr("process.pid", engine.pid)
                        .attr("path", program.display().to_string())
                        .attr("session", session.clone().unwrap_or(new_id))
                        .attr("resumed", session.is_some())
                        .attr("gen_ai.agent.id", assigned.agent.clone())
                        .attr("gathr.account", assigned.account.clone())
                        .attr("gen_ai.request.model", assigned.model.clone())
                        .attr("gathr.effort", assigned.effort.clone())
                        .attr("gathr.approvals", assigned.approvals.as_str())
                        .attr("cwd", cwd.display().to_string())
                        .attr("gathr.env.from", env_from)
                        .attr("gathr.env.why", env_why),
                );
                self.engine = Some(engine);
                self.program = Some(program.to_path_buf());
                self.assigned = Some(assigned);
                self.lines = Some(lines);
                self.last_stderr = None;
                self.idle_since = started;
                Ok(())
            }
            Err(e) => {
                let why = format!("Claude Code didn't start from {}: {e}", program.display());
                let event = Event::point("gathr.engine.start")
                    .trace(trace)
                    .chat(&assigned.chat)
                    .error("engine_start")
                    .attr("why", why.clone())
                    .attr("path", program.display().to_string());
                self.record(event);
                Err(why)
            }
        }
    }

    /// The engine's token and briefing, written before it starts (0005, 0021).
    async fn pass_for(
        &self,
        assigned: &Assignment,
        worktree: Option<(PathBuf, String)>,
        cwd: &Path,
        trace: &str,
    ) -> Result<Pass, String> {
        let (chat, agent) = (assigned.chat.clone(), assigned.agent.clone());
        let facts = self
            .chats
            .writer
            .call(move |conn| briefing::facts(conn, &chat, agent.as_deref(), worktree))
            .await;
        let facts = match facts {
            Some(read) => read.map_err(|e| e.to_string()),
            None => Err("the database didn't answer".to_string()),
        };
        let made = facts.and_then(|facts| {
            let folder = self.chats.data.path().join("briefings");
            let text = briefing::text(&facts);
            let (agent, chat) = (assigned.agent.clone(), &assigned.chat);
            let caller = (agent, chat.as_str(), cwd);
            Pass::new(&self.chats.passes, folder, caller, &text).map_err(|e| e.to_string())
        });
        made.map_err(|e| {
            let why = format!("the engine's briefing wasn't written: {e}");
            let event = Event::point("gathr.engine.start")
                .trace(trace)
                .chat(&assigned.chat)
                .error("briefing")
                .attr("why", why.clone());
            self.record(event);
            why
        })
    }

    /// The version Gathr's copy moved to, when the engine runs an older copy: called between
    /// turns only, so a turn is never cut short and a renewal, which happens inside one, never is.
    pub(super) fn copy_moved(&self) -> Option<String> {
        let running = self.program.as_ref().filter(|_| self.engine.is_some())?;
        let copies = self.chats.data.path().join("engines");
        let current = self
            .chats
            .copy()
            .filter(|c| running.starts_with(&copies) && c != running)?;
        let version = current.parent()?.file_name()?;
        Some(version.to_string_lossy().into_owned())
    }

    /// The gentle close (0001): interrupt a turn under way, close stdin, and let the engine exit
    /// on its own. Nothing new starts until it has.
    pub(super) async fn close(&mut self, why: &str) {
        let Some(mut engine) = self.engine.take() else {
            return;
        };
        self.lines = None;
        if self.turn.as_ref().is_some_and(|t| !t.stopping) {
            let _ = engine.write(&frames::interrupt("close")).await;
        }
        self.keep_what_was_shown().await;
        self.requeue_unread().await;
        engine.close_input();
        let assigned = self.assigned.as_ref();
        self.closing = Some(Closing {
            engine,
            pass: self.pass.take(),
            folder: assigned.map(|a| a.folder.clone()).unwrap_or_default(),
            chat: assigned.map(|a| a.chat.clone()).unwrap_or_default(),
            why: why.to_string(),
            since: Instant::now(),
            held: false,
        });
    }

    /// The closing engine exited: the next turn may start.
    pub(super) async fn on_closed(&mut self, code: Option<i32>) {
        let Some(mut closed) = self.closing.take() else {
            return;
        };
        // Its token and briefing end now, before anything queued starts the next engine.
        drop(closed.pass.take());
        let mut event = Event::point("gathr.engine.close")
            .chat(&closed.chat)
            .ok()
            .attr("process.pid", closed.engine.pid)
            .attr("process.exit.code", code)
            .attr("why", closed.why.clone());
        event.duration_us = Some(closed.since.elapsed().as_micros() as i64);
        self.record(event);
        if closed.held {
            self.show_hold(&closed.chat, None);
        }
        for done in self.leaving.drain(..) {
            let _ = done.send(());
        }
        self.try_next().await;
    }

    /// The closing engine hasn't exited in the hold time: it holds the next turn, never forced,
    /// since it might be renewing its login (0010).
    pub(super) fn on_held(&mut self) {
        let Some(closing) = self.closing.as_mut() else {
            return;
        };
        closing.held = true;
        let (chat, pid) = (closing.chat.clone(), closing.engine.pid);
        // On each waiting turn's trace, so its reference finds why it hasn't started (0019).
        let waiting: Vec<_> = self.queue.iter().map(|q| q.ids.clone()).collect();
        match waiting.is_empty() {
            true => self.record(self.hold_event()),
            false => waiting.iter().for_each(|ids| self.trace_hold(ids)),
        }
        let since = now_us() - self.times.hold_after.as_micros() as i64;
        let hold = json!({ "why": "Claude Code hasn't exited", "pid": pid, "since_us": since });
        self.show_hold(&chat, Some(hold));
    }

    /// The hold, recorded on a turn that waits behind it, when the closing engine is held.
    pub(super) fn trace_hold(&self, ids: &super::Ids) {
        if self.closing.as_ref().is_some_and(|c| c.held) {
            self.record(self.hold_event().trace(&ids.trace));
        }
    }

    fn hold_event(&self) -> Event {
        let closing = self.closing.as_ref();
        let why = closing.map(|c| c.why.clone()).unwrap_or_default();
        Event::point("gathr.engine.hold")
            .chat(&closing.map(|c| c.chat.clone()).unwrap_or_default())
            .error("held")
            .attr("process.pid", closing.map(|c| c.engine.pid))
            .attr(
                "why",
                format!("Claude Code hasn't exited since its close: {why}"),
            )
            .attr("gen_ai.agent.id", self.agent.clone())
    }

    fn show_hold(&self, chat: &str, hold: Option<serde_json::Value>) {
        if let Some(agent) = &self.agent {
            self.chats.set_hold(agent, hold.clone());
        }
        let event = json!({ "type": "hold", "agent": self.agent, "hold": hold });
        self.publish(chat, event);
    }

    /// At shutdown: every engine closes the gentle way, killed only after the grace period.
    pub(super) async fn shut_down(&mut self, why: &str) {
        self.close(why).await;
        let Some(closing) = self.closing.take() else {
            return;
        };
        let (chat, pid) = (closing.chat.clone(), closing.engine.pid);
        let event = match closing.engine.close(CLOSE_GRACE).await {
            Closed::Exited(code) => Event::point("gathr.engine.close")
                .ok()
                .attr("process.exit.code", code),
            Closed::Killed => Event::point("gathr.engine.close").error("killed").attr(
                "killed_because",
                "it hadn't exited 10 s after its input closed, at shutdown",
            ),
        };
        let event = event
            .chat(&chat)
            .attr("process.pid", pid)
            .attr("why", closing.why);
        self.record_now(event).await;
        for done in self.leaving.drain(..) {
            let _ = done.send(());
        }
    }

    /// The engine's output ended: it's exiting.
    pub(super) async fn on_exit(&mut self) {
        self.lines = None;
        if let Some(turn) = self.turn.as_mut() {
            turn.gate.release();
        }
        let chat = self
            .assigned
            .as_ref()
            .map(|a| a.chat.clone())
            .unwrap_or_default();
        let code = match self.engine.as_mut() {
            Some(engine) => tokio::time::timeout(CLOSE_GRACE, engine.wait())
                .await
                .ok()
                .flatten(),
            None => None,
        };
        // One that closed its output but stays is waited for as a close, never killed.
        if self.engine.as_mut().is_some_and(|e| e.exited().is_none()) {
            self.close("it closed its output but didn't exit").await;
        }
        self.engine = None;
        self.pass = None;
        let event = Event::point("gathr.engine.exit")
            .chat(&chat)
            .attr("process.exit.code", code);
        let last_stderr = self.last_stderr.take();
        self.keep_what_was_shown().await;
        self.requeue_unread().await;
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
                .trace(&turn.ids.trace)
                .error("exited")
                .attr("why", why.clone()),
        );
        self.set_turn(&turn.ids, "failed", Some(&why)).await;
        self.try_next().await;
    }

    pub(super) async fn write(
        &mut self,
        frame: &serde_json::Value,
        turn: Option<&str>,
    ) -> std::io::Result<()> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("Claude Code isn't running"))?;
        let pid = engine.pid;
        engine.write(frame).await?;
        self.raw(turn, pid, "in", &frame.to_string()).await;
        Ok(())
    }

    /// One frame in or out of the engine, recorded under the chat it runs in (0011).
    pub(super) async fn raw(
        &self,
        turn: Option<&str>,
        pid: u32,
        direction: &'static str,
        line: &str,
    ) {
        let chat = self
            .assigned
            .as_ref()
            .map(|a| a.chat.clone())
            .unwrap_or_default();
        let (turn, line) = (turn.map(String::from), line.to_string());
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
}
