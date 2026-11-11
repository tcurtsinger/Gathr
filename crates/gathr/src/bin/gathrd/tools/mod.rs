//! Gathr's tools for agents: one MCP server at `/mcp` (docs/decisions/0005-mcp-server.md). Each
//! running engine gets a token of its own, so a tool knows which agent calls it and in which
//! chat, and the token ends when the engine exits.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub mod briefing;
mod checks;
mod coord;
mod hook;
mod message;
mod person;
mod server;

pub use hook::{HOOK_PATH, PUSH_PATH, routes as hook_routes};
pub use server::service;

/// Who a tool call comes from: the engine's agent and chat, and the trace of the turn it's on.
#[derive(Clone)]
pub struct Caller {
    pub agent: Option<String>,
    pub chat: String,
    /// The folder the engine runs in: its worktree in a project's chat.
    pub cwd: PathBuf,
    /// Where it writes without asking: `cwd`, its own folder and its project's Shared (0029,
    /// 0034). Gathr asks before a write anywhere else.
    pub folders: Vec<PathBuf>,
    /// Its temporary files' folder (TEMP), while it runs.
    pub scratch: Option<PathBuf>,
    /// `cwd` is where Gathr put its work in the data folder before the agent folder (0034).
    pub from_before: bool,
    turn: Arc<Mutex<Option<OnTurn>>>,
}

/// The engine's turn: its trace, its id and the task it works on.
#[derive(Clone)]
pub struct OnTurn {
    pub trace: String,
    pub turn: String,
    pub task: Option<i64>,
}

impl Caller {
    /// The trace of the engine's turn, or of the last one it ran.
    pub fn trace(&self) -> Option<String> {
        self.on().map(|on| on.trace)
    }

    /// The engine's turn, or the last one it ran.
    pub fn on(&self) -> Option<OnTurn> {
        self.turn.lock().expect("never poisoned").clone()
    }
}

/// The tokens of the engines running now.
#[derive(Default)]
pub struct Passes(Mutex<HashMap<String, Caller>>);

impl Passes {
    pub fn caller(&self, token: &str) -> Option<Caller> {
        self.0.lock().expect("never poisoned").get(token).cloned()
    }

    /// Whether a running engine works in `folder` or has its temporary files there, or in a
    /// folder that holds it: it isn't removed then (0034).
    pub fn using(&self, folder: &Path) -> bool {
        let all = self.0.lock().expect("never poisoned");
        let mut used = all
            .values()
            .flat_map(|c| std::iter::once(&c.cwd).chain(&c.scratch));
        used.any(|p| p.starts_with(folder) || folder.starts_with(p))
    }
}

/// Where an engine works: the folder it runs in, where it writes without asking, its temporary
/// files' folder.
pub struct Place {
    pub cwd: PathBuf,
    pub folders: Vec<PathBuf>,
    pub scratch: Option<PathBuf>,
    pub from_before: bool,
}

/// What one engine was given: its token and its briefing's file. Both end when it's dropped,
/// which is when the engine has exited.
pub struct Pass {
    pub token: String,
    pub briefing: PathBuf,
    pub briefing_chars: usize,
    turn: Arc<Mutex<Option<OnTurn>>>,
    passes: Arc<Passes>,
}

impl Pass {
    /// A new token for an engine about to start as `agent` in `chat` at `place`, with its
    /// briefing written where the engine reads it and the agent's own tools don't look: the data
    /// folder.
    pub fn new(
        passes: &Arc<Passes>,
        folder: PathBuf,
        (agent, chat, place): (Option<String>, &str, Place),
        briefing: &str,
    ) -> std::io::Result<Pass> {
        std::fs::create_dir_all(&folder)?;
        let token = gathr::ids::token();
        let file = folder.join(format!("{}.md", gathr::ids::uuid()));
        std::fs::write(&file, briefing)?;
        let turn = Arc::new(Mutex::new(None));
        let caller = Caller {
            agent,
            chat: chat.to_string(),
            cwd: place.cwd,
            folders: place.folders,
            scratch: place.scratch,
            from_before: place.from_before,
            turn: turn.clone(),
        };
        let mut all = passes.0.lock().expect("never poisoned");
        all.insert(token.clone(), caller);
        Ok(Pass {
            token,
            briefing: file,
            briefing_chars: briefing.chars().count(),
            turn,
            passes: passes.clone(),
        })
    }

    /// The engine's turn from now, so its tool calls are recorded on that turn's trace, and a task
    /// it gives was given in that turn.
    pub fn on_turn(&self, on: OnTurn) {
        *self.turn.lock().expect("never poisoned") = Some(on);
    }
}

impl Drop for Pass {
    fn drop(&mut self) {
        self.passes
            .0
            .lock()
            .expect("never poisoned")
            .remove(&self.token);
        let _ = std::fs::remove_file(&self.briefing);
    }
}
