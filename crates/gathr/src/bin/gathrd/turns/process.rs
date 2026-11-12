//! The engine's life: found, started in its chat's session, closed the gentle way. A close never
//! kills (0001, 0010 Sessions): an engine still there after 30 s holds the next turn and shows as
//! a hold on its agent. Only shutdown kills, after its grace period.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use gathr::claude::command::{self, Session};
use gathr::claude::engine::{Closed, Engine};
use gathr::event::Event;
use gathr::ids::now_us;
use gathr::redact::redact;
use gathr::store::{self, RawFrame};
use serde_json::json;

use super::{Assignment, Task};
use crate::tools::{Pass, Place, briefing};

/// How long an engine gets at shutdown to exit on its own before it's killed (0001).
pub const CLOSE_GRACE: Duration = Duration::from_secs(10);
const HOLD_AFTER: Duration = Duration::from_secs(30);
const IDLE_CLOSE: Duration = Duration::from_secs(600);

/// When a close shows as a hold, and when an idle engine closes.
#[derive(Clone, Copy)]
pub struct Times {
    pub hold_after: Duration,
    pub idle_close: Duration,
    /// A turn with no progress this long (twice that during a tool) is flagged, never stopped.
    pub stall: Duration,
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
            stall: ms("GATHR_STALL_AFTER_MS", Duration::from_secs(600)),
        }
    }
}

/// An engine whose input is closed, waiting for its own exit.
pub struct Closing {
    engine: Engine,
    /// Its token and briefing: they end when it has exited, not when it's asked to.
    pass: Option<Pass>,
    pub folder: PathBuf,
    pub chat: String,
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
    pub(super) async fn find_engine(&self, kind: &str, trace: &str) -> Result<PathBuf, String> {
        let found = if kind == "codex" {
            gathr::codex::command::find(gathr::codex::copies::storage(&self.chats.data).program())
        } else {
            command::find(self.chats.copy())
        };
        let program = found.inspect_err(|why| {
            self.record(
                Event::point("gathr.engine.start")
                    .trace(trace)
                    .error("engine_not_found")
                    .attr("why", why.clone()),
            );
        })?;
        if kind == "codex" {
            return Ok(program);
        }
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
        let codex = assigned.engine == "codex";
        let mut args = match &session {
            Some(id) => command::args(Session::Resume(id), given, Some(&hook)),
            None => command::args(Session::New(&new_id), given, Some(&hook)),
        };
        let folder = assigned.folder.as_path();
        let making = crate::cleanup::hold().await;
        let place = self.workspace(&assigned, trace).await?;
        let cwd = place.cwd;
        for dir in [folder, &cwd] {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
        }
        // Beside the folder it runs in: its own folder and Shared, where it writes without asking.
        let beside = place.beside;
        let worktree = place.project.map(|project| (cwd.clone(), project));
        let given = Place {
            cwd: cwd.clone(),
            folders: std::iter::once(cwd.clone()).chain(beside.clone()).collect(),
            scratch: place.scratch.clone(),
            from_before: place.from_before,
        };
        let pass = self.pass_for(&assigned, worktree, given, trace).await?;
        drop(making);
        let url = format!("http://127.0.0.1:{}/mcp", self.chats.port);
        let point = self.compact_at(&assigned).await;
        let compact_at = point.as_ref().map(|(at, _)| *at);
        let first = if codex {
            args = gathr::codex::command::args(
                Some(&url),
                assigned.approvals == gathr::agents::Approvals::Auto,
            );
            let opening = gathr::codex::rpc::Opening {
                resume: session.clone(),
                cwd: cwd.display().to_string(),
                model: assigned.model.clone(),
                effort: assigned.effort.clone(),
                policy: gathr::codex::command::policy(assigned.approvals),
                instructions: Some(
                    std::fs::read_to_string(&pass.briefing).map_err(|e| e.to_string())?,
                ),
                writable: beside.iter().map(|d| d.display().to_string()).collect(),
                compact_at,
                compact_for: point.as_ref().map(|(_, model)| model.clone()),
            };
            let (driver, first) = super::driver::Driver::codex(opening);
            self.driver = driver;
            Some(first)
        } else {
            self.driver = Default::default();
            args.extend(command::given(&url, &pass.briefing, &crate::tools::names()));
            if assigned.slash {
                args.retain(|a| a != command::NO_SLASH);
            }
            args.extend(command::folders(&beside));
            None
        };
        let (env_from, env_why) = (place.env.from, place.env.why);
        let mut env = if codex {
            gathr::codex::command::env(place.env.vars.into_iter(), folder)
        } else {
            command::env(place.env.vars.into_iter(), folder)
        };
        if let Some(scratch) = &place.scratch {
            command::scratch(&mut env, scratch);
        }
        // Claude Code takes 100K at least (its docs, 2026-11-11).
        if let (false, Some(at)) = (codex, compact_at) {
            let at = at.max(100_000).to_string();
            env.push((command::COMPACT_VAR.into(), at.into()));
        }
        env.push((command::TOKEN_VAR.into(), pass.token.clone().into()));
        let push = format!(
            "http://127.0.0.1:{}{}",
            self.chats.port,
            crate::tools::PUSH_PATH
        );
        env.push((command::PUSH_VAR.into(), push.into()));
        let started = Instant::now();
        let ours = place.place.as_ref();
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
                        .attr("gathr.folder.scratch", shown(place.scratch.as_deref()))
                        .attr(
                            "gathr.folder.agent",
                            passed(&beside, ours.map(|p| &p.agent)),
                        )
                        .attr(
                            "gathr.folder.shared",
                            passed(&beside, ours.map(|p| &p.shared)),
                        )
                        .attr("gathr.env.from", env_from)
                        .attr("gathr.env.why", env_why),
                );
                self.engine = Some(engine);
                self.program = Some(program.to_path_buf());
                self.assigned = Some(assigned);
                self.lines = Some(lines);
                self.last_stderr = None;
                self.point_model = point.map(|(_, model)| model);
                self.model_moved = false;
                // Named again by this engine: a window is never kept under an earlier one's model.
                self.context_model = None;
                self.idle_since = started;
                if let Some(first) = first {
                    self.write(&first, None).await.map_err(|e| e.to_string())?;
                }
                Ok(())
            }
            Err(e) => {
                let why = format!(
                    "{} didn't start from {}: {e}",
                    if codex { "Codex" } else { "Claude Code" },
                    program.display()
                );
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
        place: Place,
        trace: &str,
    ) -> Result<Pass, String> {
        let (chat, agent) = (assigned.chat.clone(), assigned.agent.clone());
        let own = place
            .folders
            .get(1..3)
            .map(|f| (f[0].clone(), f[1].clone()));
        let facts = self
            .chats
            .writer
            .call(move |conn| briefing::facts(conn, &chat, agent.as_deref(), worktree, own))
            .await;
        let facts = match facts {
            Some(read) => read.map_err(|e| e.to_string()),
            None => Err("the database didn't answer".to_string()),
        };
        let made = facts.and_then(|facts| {
            let folder = self.chats.data.path().join("briefings");
            let text = briefing::text(&facts);
            let (agent, chat) = (assigned.agent.clone(), &assigned.chat);
            let caller = (agent, chat.as_str(), place);
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
        let current = if self.engine_name() == "codex" {
            gathr::codex::copies::storage(&self.chats.data).program()
        } else {
            self.chats.copy()
        };
        let current = current.filter(|c| running.starts_with(&copies) && c != running)?;
        let version = current.strip_prefix(copies).ok()?.components().nth(1)?;
        let version = version.as_os_str();
        Some(version.to_string_lossy().into_owned())
    }

    /// Where the agent's conversation in `chat` compacts (Auto-compact, K3) and the model that
    /// point is for: read at each start, so a changed setting applies from the engine's next start.
    async fn compact_at(&self, assigned: &Assignment) -> Option<(i64, String)> {
        let (chat, agent) = (assigned.chat.clone(), self.agent.clone()?);
        let set = assigned.model_key();
        let found = self
            .chats
            .writer
            .call(move |c| gathr::context::point_for(c, &chat, &agent, &set));
        found.await.and_then(Result::ok).flatten()
    }

    /// The gentle close (0001): interrupt a turn under way, close stdin, and let the engine exit
    /// on its own. Nothing new starts until it has.
    pub(super) async fn close(&mut self, why: &str) {
        let Some(mut engine) = self.engine.take() else {
            return;
        };
        self.flush_raw().await;
        self.lines = None;
        if self.turn.as_ref().is_some_and(|t| !t.stopping)
            && let Some(request) = self.driver.interrupt("close")
        {
            let _ = engine.write(&request).await;
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
        self.engine_gone(closed.chat.clone()).await;
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
        let hold = json!({ "why": format!("{} hasn't exited", self.engine_label()), "pid": pid, "since_us": since });
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
                format!(
                    "{} hasn't exited since its close: {why}",
                    self.engine_label()
                ),
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
        // Its temporary files go at the next start's first sweep.
        if let Some(agent) = self.agent.clone() {
            let due = move |conn: &mut rusqlite::Connection| {
                gathr::folders::engine_closed(conn, &chat, &agent)
            };
            let _ = self.chats.writer.call(due).await;
        }
        for done in self.leaving.drain(..) {
            let _ = done.send(());
        }
    }

    /// The engine's output ended: it's exiting.
    pub(super) async fn on_exit(&mut self) {
        self.flush_raw().await;
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
        if self.closing.is_none() {
            self.engine_gone(chat.clone()).await;
        }
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
        let mut why = format!(
            "{} exited during the turn (exit code {code})",
            self.engine_label()
        );
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

    pub(super) async fn write_user(
        &mut self,
        text: &str,
        turn: Option<&str>,
    ) -> std::io::Result<()> {
        match self.driver.user(text) {
            Some(frame) => self.write(&frame, turn).await,
            None => Ok(()),
        }
    }

    pub(super) async fn write(
        &mut self,
        frame: &serde_json::Value,
        turn: Option<&str>,
    ) -> std::io::Result<()> {
        let engine = self
            .engine
            .as_mut()
            .ok_or_else(|| std::io::Error::other("The engine isn't running"))?;
        let pid = engine.pid;
        engine.write(frame).await?;
        self.raw(turn, pid, "in", &frame.to_string()).await;
        Ok(())
    }

    /// Writes the held lines in their order, their stream fragments redacted joined
    /// (`redact::stream`). Lines that aren't frames go as they came, redacted as text.
    pub(super) async fn flush_raw(&mut self) {
        let held = std::mem::take(&mut self.held_raw);
        let parse = |(_, _, dir, line): &(Option<String>, u32, &str, String)| {
            (*dir == "out")
                .then(|| serde_json::from_str(line).ok())
                .flatten()
        };
        let parsed: Vec<Option<serde_json::Value>> = held.iter().map(parse).collect();
        let mut frames: Vec<serde_json::Value> = parsed
            .iter()
            .map(|v| v.clone().unwrap_or_default())
            .collect();
        gathr::redact::stream::redact_run(&mut frames);
        for ((turn, pid, dir, line), (frame, was)) in held.iter().zip(frames.iter().zip(&parsed)) {
            let text = match was {
                Some(_) => frame.to_string(),
                None => line.clone(),
            };
            self.raw(turn.as_deref(), *pid, dir, &text).await;
        }
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

fn shown(path: Option<&Path>) -> Option<String> {
    path.map(|p| p.display().to_string())
}

/// One of the agent's folders, if it was given to the engine.
fn passed(beside: &[PathBuf], dir: Option<&PathBuf>) -> Option<String> {
    shown(dir.filter(|d| beside.contains(d)).map(PathBuf::as_path))
}
