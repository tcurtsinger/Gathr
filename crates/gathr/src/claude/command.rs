//! How Claude Code is started: its arguments and an environment built, not inherited as is
//! (docs/decisions/0001-engine-interface.md, 0002-login-folders.md, 0021-no-prompting-needed.md).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Tools turned off for every agent, each with its reason (0001).
pub const DENIED_TOOLS: &[(&str, &str)] = &[
    (
        "CronCreate",
        "a wake-up lives only as long as the process, which Gathr closes",
    ),
    ("CronDelete", "goes with CronCreate"),
    ("CronList", "goes with CronCreate"),
    (
        "ScheduleWakeup",
        "a wake-up lives only as long as the process; coming back later is Gathr's job (0010)",
    ),
    // Offered only once the folder is signed in to a claude.ai account: they come with the
    // account and act outside Gathr. Agents get only what Gathr gives them (0024).
    ("PublishPlugin", "comes with the claude.ai account (0024)"),
    (
        "PushNotification",
        "comes with the claude.ai account (0024)",
    ),
    ("SendToProject", "comes with the claude.ai account (0024)"),
    (
        "SendToProjectContext",
        "comes with the claude.ai account (0024)",
    ),
    (
        "ShareOnboardingGuide",
        "comes with the claude.ai account (0024)",
    ),
];

/// Every tool Claude Code 2.1.318 offered in `system/init` with the flags below, looked at.
/// A tool missing here is one nobody has looked at yet: the driver records it.
pub const KNOWN_TOOLS: &[&str] = &[
    "Task",
    "AskUserQuestion",
    "Bash",
    "DesignSync",
    "Edit",
    "EnterPlanMode",
    "EnterWorktree",
    "ExitPlanMode",
    "ExitWorktree",
    "Glob",
    "Grep",
    "ListAgents",
    "Monitor",
    "NotebookEdit",
    "PowerShell",
    "Read",
    "ReportFindings",
    "SendMessage",
    "Skill",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskStop",
    "TaskUpdate",
    "ToolSearch",
    "WebFetch",
    "WebSearch",
    "Workflow",
    "Write",
];

/// Reads `AGENTS.md` next to any `CLAUDE.md`, in every project (0021). No auto-memory, which
/// would load Claude Code's own notes for the folder, and no claude.ai connectors: the account
/// brings them with the login, and `--strict-mcp-config` isn't documented to cover them (0024).
/// Passed as `--settings`, so nothing is written into the account's folder.
const SETTINGS: &str = r#"{"autoMemoryEnabled":false,"disableClaudeAiConnectors":true,"pluginConfigs":{"cc-plugin-agents-md@builtin":{"options":{"instructionFiles":"claude-md-and-agents-md"}}}}"#;

/// A new session gets the id Gathr chose; a known one is resumed.
pub enum Session<'a> {
    New(&'a str),
    Resume(&'a str),
}

/// The agent's model and effort (0026); `None` leaves each to Claude Code's own default.
#[derive(Clone, Copy, Default)]
pub struct Assigned<'a> {
    pub model: Option<&'a str>,
    pub effort: Option<&'a str>,
}

pub fn args(session: Session<'_>, assigned: Assigned<'_>) -> Vec<String> {
    let denied: Vec<&str> = DENIED_TOOLS.iter().map(|(name, _)| *name).collect();
    let mut args: Vec<String> = [
        "-p",
        "--input-format",
        "stream-json",
        "--output-format",
        "stream-json",
        "--verbose",
        "--include-partial-messages",
        "--replay-user-messages",
        // Without it a foreground sub-agent's text is dropped, so what it said can't be shown.
        "--forward-subagent-text",
        "--permission-prompt-tool",
        "stdio",
        // Agents get only what Gathr gives them (0024), as SerenityOS starts Claude Code: no
        // skills or commands, no MCP servers but Gathr's, and no user settings, which is where
        // plugins (synced from claude.ai too), their hooks and the person's sub-agents come from.
        "--disable-slash-commands",
        "--strict-mcp-config",
        "--setting-sources",
        "project,local",
        "--settings",
        SETTINGS,
        "--disallowedTools",
    ]
    .map(String::from)
    .into();
    args.push(denied.join(","));
    match session {
        Session::New(id) => args.extend(["--session-id".into(), id.into()]),
        Session::Resume(id) => args.extend(["--resume".into(), id.into()]),
    }
    // Each value in the same argument as its switch, so one starting with `-` is never a switch.
    if let Some(model) = assigned.model {
        args.push(format!("--model={model}"));
    }
    if let Some(effort) = assigned.effort {
        args.push(format!("--effort={effort}"));
    }
    args
}

/// The engine's own token for Gathr's tools, in its environment: never in its arguments, which any
/// process on the computer can read (0005).
pub const TOKEN_VAR: &str = "GATHR_AGENT_TOKEN";

/// Gathr's tools at `url` with the engine's token, allowed without asking, and the briefing in
/// `briefing`, a file for the same reason as the token (0005, 0021).
pub fn given(url: &str, briefing: &Path) -> Vec<String> {
    let auth = format!("Bearer ${{{TOKEN_VAR}}}");
    let servers = serde_json::json!({ "mcpServers": { "gathr": {
        "type": "http", "url": url, "headers": { "Authorization": auth } } } });
    vec![
        "--mcp-config".into(),
        servers.to_string(),
        "--allowedTools".into(),
        "mcp__gathr".into(),
        "--append-system-prompt-file".into(),
        briefing.display().to_string(),
    ]
}

/// The parent's environment without anything that picks a credential, a provider or another
/// Claude Code session, plus the account's folder.
pub fn env(
    inherited: impl Iterator<Item = (OsString, OsString)>,
    folder: &Path,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = inherited.filter(|(name, _)| keep(name)).collect();
    env.push(("CLAUDE_CONFIG_DIR".into(), folder.into()));
    env.push(("CLAUDE_CODE_ENABLE_TODO_TOOLS".into(), "1".into()));
    // Gathr's copy changes only through its own checks (0025); `claude update` still works.
    env.push(("DISABLE_AUTOUPDATER".into(), "1".into()));
    env
}

/// Drops every `ANTHROPIC_*` and `CLAUDE*` variable (selectors, keys, tokens, base URLs, and a
/// parent Claude Code's own session) and Bedrock's bearer token.
fn keep(name: &OsString) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    !(name.starts_with("ANTHROPIC_")
        || name.starts_with("CLAUDE")
        || name == "AWS_BEARER_TOKEN_BEDROCK")
}

/// Stands in for Gathr's copy in dev and tests: the replay engine, or a missing file for a
/// forced failure. Settings has no path (0026).
pub const PATH_VAR: &str = "GATHR_CLAUDE_PATH";

/// The engine's path: `GATHR_CLAUDE_PATH`, else Gathr's own copy (0025), else `claude` on the
/// PATH.
pub fn find(copy: Option<PathBuf>) -> Result<PathBuf, String> {
    let set = std::env::var_os(PATH_VAR).filter(|p| !p.is_empty());
    if let Some(path) = set.map(PathBuf::from) {
        return match path.is_file() {
            true => Ok(path),
            false => Err(format!(
                "Claude Code's path {} isn't a file",
                path.display()
            )),
        };
    }
    if let Some(copy) = copy {
        return Ok(copy);
    }
    crate::platform::find_on_path("claude")
        .ok_or_else(|| "Claude Code isn't installed yet, and claude isn't on the PATH".to_string())
}

/// The oldest Claude Code all the switches above were proven on (0024). Its changelog has
/// older ones lacking `--forward-subagent-text` (2.1.211), `AGENTS.md` (2.1.277), or keeping user
/// settings out under `--setting-sources` (2.1.314).
const MIN_VERSION: [u32; 3] = [2, 1, 318];

/// How long `--version` may take: Claude Code answers in tens of milliseconds, and a launcher
/// that hangs would otherwise hold the chat's Send and Stop with it.
const VERSION_WAIT: std::time::Duration = std::time::Duration::from_secs(5);

/// Claude Code's version from `<program> --version`, and why Gathr can't run it if it can't
/// (with the version, when one was read). Blocks until it exits or `VERSION_WAIT` passes; the
/// service calls it off its async threads.
pub fn version(program: &Path) -> (Option<String>, Result<(), String>) {
    let mut command = std::process::Command::new(program);
    command
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null());
    crate::platform::no_window(&mut command);
    let unread = |e: String| {
        let why = format!(
            "Claude Code's version wasn't read from {}: {e}",
            program.display()
        );
        (None, Err(why))
    };
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => return unread(e.to_string()),
    };
    let mut stdout = child.stdout.take().expect("stdout is piped");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut answer = String::new();
        let _ = std::io::Read::read_to_string(&mut stdout, &mut answer);
        let _ = tx.send(answer);
    });
    // The deadline is on its exit, not its output: a launcher can close its output and hang.
    let deadline = std::time::Instant::now() + VERSION_WAIT;
    let timed_out = || unread(format!("it didn't answer in {} s", VERSION_WAIT.as_secs()));
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            // Safe to kill: `--version` only prints, it never renews a login.
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return timed_out();
            }
        }
    }
    let left = deadline.saturating_duration_since(std::time::Instant::now());
    match rx.recv_timeout(left) {
        Ok(answer) => usable(&answer),
        Err(_) => timed_out(),
    }
}

/// `2.1.318 (Claude Code)` is fine; an older one, or one that can't be read, isn't.
fn usable(answer: &str) -> (Option<String>, Result<(), String>) {
    let version = answer.split_whitespace().next().unwrap_or_default();
    let parts: Option<Vec<u32>> = version.split('.').map(|p| p.parse().ok()).collect();
    let min = MIN_VERSION.map(|n| n.to_string()).join(".");
    match parts {
        Some(parts) if parts.len() == 3 => {
            let fine = parts[..] >= MIN_VERSION[..];
            let why = format!("Claude Code {version} is too old: Gathr needs {min} or later");
            (
                Some(version.to_string()),
                if fine { Ok(()) } else { Err(why) },
            )
        }
        _ => (
            None,
            Err(format!(
                "Claude Code's version wasn't readable from {:?}",
                answer.trim()
            )),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_usable_from_the_oldest_proven_one_on_compared_by_number() {
        assert_eq!(
            usable("2.1.318 (Claude Code)\n"),
            (Some("2.1.318".into()), Ok(()))
        );
        assert_eq!(usable("2.10.0 (Claude Code)").1, Ok(()));
        let old = "Claude Code 2.1.183 is too old: Gathr needs 2.1.318 or later";
        assert_eq!(
            usable("2.1.183 (Claude Code)"),
            (Some("2.1.183".into()), Err(old.into()))
        );
        assert!(
            usable("2.1.99 (Claude Code)").1.is_err(),
            "2.1.99 compared as text"
        );
        for unreadable in ["", "2.1 beta"] {
            let (version, why) = usable(unreadable);
            assert!(version.is_none() && why.unwrap_err().contains("wasn't readable"));
        }
    }

    #[test]
    fn the_engine_gets_none_of_the_credentials_or_sessions_it_could_inherit() {
        let inherited = [
            "ANTHROPIC_API_KEY",
            "ANTHROPIC_BASE_URL",
            "ANTHROPIC_SOMETHING_NEW",
            "CLAUDE_CODE_OAUTH_TOKEN",
            "CLAUDE_CODE_USE_BEDROCK",
            "CLAUDE_CONFIG_DIR",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDECODE",
            "AWS_BEARER_TOKEN_BEDROCK",
            "PATH",
            "USERPROFILE",
        ]
        .map(|name| (OsString::from(name), OsString::from("x")));
        let folder = Path::new("data/accounts/main/claude");
        let env = env(inherited.into_iter(), folder);
        let names: Vec<String> = env
            .iter()
            .map(|(n, _)| n.to_string_lossy().into())
            .collect();
        assert_eq!(
            names,
            [
                "PATH",
                "USERPROFILE",
                "CLAUDE_CONFIG_DIR",
                "CLAUDE_CODE_ENABLE_TODO_TOOLS",
                "DISABLE_AUTOUPDATER"
            ]
        );
        assert_eq!(env[2].1, folder.as_os_str());
    }
}
