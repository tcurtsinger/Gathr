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

/// Reads `AGENTS.md` next to any `CLAUDE.md`, in every project (0021). Passed as `--settings`,
/// so nothing is written into the account's folder.
const SETTINGS: &str = r#"{"pluginConfigs":{"cc-plugin-agents-md@builtin":{"options":{"instructionFiles":"claude-md-and-agents-md"}}}}"#;

/// A new session gets the id Gathr chose; a known one is resumed.
pub enum Session<'a> {
    New(&'a str),
    Resume(&'a str),
}

pub fn args(session: Session<'_>) -> Vec<String> {
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
        "--strict-mcp-config",
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
    args
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

/// The setting that holds Claude Code's path; none means `claude` on the PATH (0016).
pub const PATH_SETTING: &str = "engines.claude.path";

/// The engine's path: the one set in Settings, else `claude` on the PATH.
pub fn find(setting: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = setting.filter(|p| !p.trim().is_empty()) {
        let path = PathBuf::from(path.trim());
        return match path.is_file() {
            true => Ok(path),
            false => Err(format!(
                "Claude Code's path {} isn't a file",
                path.display()
            )),
        };
    }
    crate::platform::find_on_path("claude").ok_or_else(|| {
        "claude wasn't found on the PATH: set its path in Settings, Engines".to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
                "CLAUDE_CODE_ENABLE_TODO_TOOLS"
            ]
        );
        assert_eq!(env[2].1, folder.as_os_str());
    }
}
