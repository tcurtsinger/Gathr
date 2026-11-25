//! The chat's terminals for its agents (`gathr::terminal`): `terminal_run` starts a command that keeps
//! running (a dev server, a watcher) in a terminal tab of the agent's own, `terminal_list` and
//! `terminal_read` see every terminal in the chat, the person's and other agents' too, and
//! `terminal_send` types in one. Run and send ask as the agent's Approvals say, as its own commands do
//! (`ASKED_TOOLS`).

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use axum::http::request::Parts;
use gathr::terminal::Info;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::Caller;
use super::server::{Tools, caller_of};

/// How long `terminal_run` waits for what a command first prints, and for it to go quiet.
const FIRST: Duration = Duration::from_secs(15);
const QUIET: Duration = Duration::from_millis(1500);
/// What one read gives at most: its last lines and bytes.
const LINES: usize = 200;
const BYTES: usize = 16 * 1024;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Run {
    /// The command, as typed in the shell (PowerShell on Windows).
    command: String,
    /// Where it runs: your working folder unless set; a folder of this chat.
    folder: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Reading {
    /// The terminal's id.
    id: String,
    /// Its last lines: 100 unless set, at most 200.
    lines: Option<usize>,
    /// Only what it printed after this mark, from an earlier read.
    since: Option<u64>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Typed {
    /// The terminal's id.
    id: String,
    /// What to type. Enter follows unless enter is false.
    text: Option<String>,
    enter: Option<bool>,
    /// Ctrl+C: stops what runs in it.
    stop: Option<bool>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct List {}

#[tool_router(router = terminal_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Run a command that keeps running (a dev server, a watcher, a long build) in a terminal \
                       tab of this chat the person can watch. Answers with its id and what it first printed; it \
                       keeps running after your turn. Use your own shell for commands that just finish."
    )]
    async fn terminal_run(
        &self,
        Parameters(ask): Parameters<Run>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let said = self.run(&caller, ask).await;
        Ok(self.respond(&caller, "terminal_run", started, said).await)
    }

    #[tool(
        description = "This chat's terminals: the person's, yours and other agents', each with its id, who \
                       started it, its folder and whether it runs."
    )]
    async fn terminal_list(
        &self,
        Parameters(_): Parameters<List>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let list = self.state.terminals.list(&caller.chat);
        let said = self.listed(&caller, &list).await;
        Ok(self.respond(&caller, "terminal_list", started, said).await)
    }

    #[tool(
        description = "Read what a terminal of this chat printed, as plain text: to check a dev server, a \
                       failed build, or what the person ran. Gives a mark; pass it as since to read only what's new."
    )]
    async fn terminal_read(
        &self,
        Parameters(ask): Parameters<Reading>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let said = self.ours(&caller, &ask.id).map(|info| {
            let (text, mark) = self
                .state
                .terminals
                .since(&info.id, ask.since)
                .unwrap_or_default();
            let lines = ask.lines.unwrap_or(100).clamp(1, LINES);
            format!("{}\n{}", state_of(&info, mark), last(&text, lines))
        });
        Ok(self.respond(&caller, "terminal_read", started, said).await)
    }

    #[tool(description = "Type in a terminal of this chat, or stop what runs in it (Ctrl+C).")]
    async fn terminal_send(
        &self,
        Parameters(ask): Parameters<Typed>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let said = self.ours(&caller, &ask.id).and_then(|info| {
            let terminals = &self.state.terminals;
            let typed = match (ask.stop, ask.text) {
                (Some(true), _) => "\u{3}".to_string(),
                (_, Some(text)) if ask.enter != Some(false) => format!("{text}\r"),
                (_, Some(text)) => text,
                (_, None) => return Err("Give text or stop".into()),
            };
            terminals
                .input(&info.id, typed.as_bytes())
                .map_err(|e| e.to_string())?;
            Ok(format!(
                "Sent. Read terminal {} for what it printed.",
                info.id
            ))
        });
        Ok(self.respond(&caller, "terminal_send", started, said).await)
    }
}

impl Tools {
    async fn run(&self, caller: &Caller, ask: Run) -> Result<String, String> {
        let agent = caller.agent.clone().ok_or("You're not an agent here.")?;
        let folder = match ask.folder {
            Some(folder) => self.allowed(caller, Path::new(&folder)).await?,
            None => caller.cwd.clone(),
        };
        let command = ask.command.trim().to_string();
        if command.is_empty() {
            return Err("Give a command".into());
        }
        let vars = gathr::run::fresh().await;
        let terminals = self.state.terminals.clone();
        let chat = caller.chat.clone();
        let place = folder.clone();
        let opened = tokio::task::spawn_blocking(move || {
            terminals.open(&chat, &place, Some(&agent), Some(&command), vars)
        });
        let id = opened
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;
        // What it first prints: until it goes quiet, ends, or a while passes.
        let terminals = &self.state.terminals;
        let (mut seen, mut quiet_since) = (0, Instant::now());
        let begun = Instant::now();
        while begun.elapsed() < FIRST {
            tokio::time::sleep(Duration::from_millis(250)).await;
            let Some(info) = terminals.info(&id) else {
                break;
            };
            let printed = terminals.since(&id, None).map_or(0, |(t, _)| t.len());
            if printed != seen {
                (seen, quiet_since) = (printed, Instant::now());
            }
            if !info.running || (seen > 0 && quiet_since.elapsed() >= QUIET) {
                break;
            }
        }
        let info = terminals.info(&id).ok_or("The terminal went")?;
        let (text, mark) = terminals.since(&id, None).unwrap_or_default();
        Ok(format!(
            "Terminal {id} in {}.\n{}\n{}",
            folder.display(),
            state_of(&info, mark),
            last(&text, 40)
        ))
    }

    /// A folder of the chat's (its tree's, or one the agent writes in), as asked.
    async fn allowed(&self, caller: &Caller, asked: &Path) -> Result<PathBuf, String> {
        let (chat, data) = (caller.chat.clone(), self.chats.data.path().to_path_buf());
        let roots = self
            .chats
            .writer
            .call(move |c| gathr::files::tree(c, &data, &chat).map(|t| t.roots()))
            .await
            .unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string())?;
        let asked = gathr::folders::real(asked);
        let inside = roots
            .iter()
            .chain(&caller.folders)
            .any(|r| asked.starts_with(gathr::folders::real(r)));
        match inside && asked.is_dir() {
            true => Ok(asked),
            false => Err(format!("{} isn't a folder of this chat", asked.display())),
        }
    }

    /// The chat's terminal by its id.
    fn ours(&self, caller: &Caller, id: &str) -> Result<Info, String> {
        let info = self.state.terminals.info(id.trim());
        info.filter(|i| i.chat == caller.chat)
            .ok_or_else(|| format!("No terminal {id} in this chat. terminal_list names them."))
    }

    async fn listed(&self, caller: &Caller, list: &[Info]) -> Result<String, String> {
        if list.is_empty() {
            return Ok("No terminals in this chat.".into());
        }
        let chat = caller.chat.clone();
        let members = self
            .chats
            .writer
            .call(move |c| gathr::team::members(c, &chat))
            .await
            .unwrap_or_else(|| Err(gathr::Error::Other("the database didn't answer".into())))
            .map_err(|e| e.to_string())?;
        let lines = list.iter().map(|t| {
            let by = match &t.agent {
                None => "the person".to_string(),
                Some(a) if Some(a) == caller.agent.as_ref() => "you".to_string(),
                Some(a) => members
                    .iter()
                    .find(|m| m.agent == *a)
                    .map_or("an agent".into(), |m| m.name.clone()),
            };
            let what = t
                .command
                .as_deref()
                .map(|c| format!(", runs `{c}`"))
                .unwrap_or_default();
            let ran = if t.running { "running" } else { "ended" };
            format!("{}: by {by} in {}{what}, {ran}", t.id, t.folder.display())
        });
        Ok(lines.collect::<Vec<_>>().join("\n"))
    }
}

fn state_of(info: &Info, mark: u64) -> String {
    let ran = match (info.running, info.code) {
        (true, _) => "It runs.".to_string(),
        (false, Some(code)) => format!("It ended, code {code}."),
        (false, None) => "It ended.".to_string(),
    };
    format!("{ran} Mark {mark}.")
}

/// The last `lines` of what it printed, within `BYTES`, keys and secrets hidden.
fn last(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.trim_end().lines().collect();
    let mut kept = all[all.len().saturating_sub(lines)..].join("\n");
    if kept.len() > BYTES {
        let cut = kept.len() - BYTES;
        let at = (cut..kept.len())
            .find(|&i| kept.is_char_boundary(i))
            .unwrap_or(cut);
        kept = kept[at..].to_string();
    }
    match kept.is_empty() {
        true => "(nothing printed)".into(),
        false => gathr::redact::redact(&kept),
    }
}
