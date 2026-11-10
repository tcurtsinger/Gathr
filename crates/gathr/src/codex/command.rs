//! How Gathr starts `codex app-server` (0033): its switches, its environment and where it's found.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::agents::Approvals;

/// Stands in for Gathr's copy in dev and tests: the replay engine, or a missing file for a
/// forced failure.
pub const PATH_VAR: &str = "GATHR_CODEX_PATH";

/// What Codex's own setup would add that Gathr doesn't give (0024), and how it runs under Gathr.
/// Each is read back through `config/read` in the driver's tests.
pub const SWITCHES: &[(&str, &str)] = &[
    // ChatGPT apps (connectors) reach any folder signed in to the account.
    ("features.apps", "false"),
    ("features.plugins", "false"),
    ("features.hooks", "false"),
    ("skills.bundled.enabled", "false"),
    ("skills.include_instructions", "false"),
    ("notify", "[]"),
    // Gathr's copy changes only through its own checks (0025).
    ("check_for_update_on_startup", "false"),
    // The login stays in the account's folder, never the system's keyring (0002).
    ("cli_auth_credentials_store", "\"file\""),
    // Codex's recommended Windows sandbox: no administrator setup, no extra Windows users (0033).
    ("windows.sandbox", "\"mxc\""),
];

/// Gathr's tools may hold a call while the person answers (`ask_person`, a wait's check).
const TOOL_TIMEOUT_SEC: u32 = 3600;

/// `app-server` with Gathr's switches, and its tools at `mcp` when the engine runs a turn.
pub fn args(mcp: Option<&str>) -> Vec<String> {
    let mut args = vec!["app-server".to_string()];
    let mut set = |key: &str, value: String| {
        args.push("-c".into());
        args.push(format!("{key}={value}"));
    };
    for (key, value) in SWITCHES {
        set(key, value.to_string());
    }
    if let Some(url) = mcp {
        let quoted = |s: &str| serde_json::Value::from(s).to_string();
        set("mcp_servers.gathr.url", quoted(url));
        set("mcp_servers.gathr.bearer_token_env_var", quoted(TOKEN_VAR));
        set(
            "mcp_servers.gathr.default_tools_approval_mode",
            quoted("approve"),
        );
        set(
            "mcp_servers.gathr.tool_timeout_sec",
            TOOL_TIMEOUT_SEC.to_string(),
        );
    }
    args
}

pub use crate::claude::command::{PUSH_VAR, TOKEN_VAR};

/// The sandbox and approval policy for an agent's approvals (0029, 0033).
pub fn policy(approvals: Approvals) -> (&'static str, &'static str, &'static str) {
    match approvals {
        Approvals::Ask => ("read-only", "on-request", "user"),
        Approvals::Edits => ("workspace-write", "on-request", "user"),
        Approvals::Auto => ("workspace-write", "on-request", "auto_review"),
        Approvals::Full => ("danger-full-access", "never", "user"),
    }
}

/// The parent's environment without anything that picks a credential or another Codex's
/// folder, plus the account's folder.
pub fn env(
    inherited: impl Iterator<Item = (OsString, OsString)>,
    folder: &Path,
) -> Vec<(OsString, OsString)> {
    let mut env: Vec<(OsString, OsString)> = inherited.filter(|(name, _)| keep(name)).collect();
    env.push(("CODEX_HOME".into(), folder.into()));
    env
}

/// Drops `OPENAI_*` and `CODEX_*` (keys, tokens, base URLs, a parent's `CODEX_HOME`).
fn keep(name: &OsString) -> bool {
    let name = name.to_string_lossy().to_ascii_uppercase();
    !(name.starts_with("OPENAI_") || name.starts_with("CODEX_"))
}

/// The engine's path: `GATHR_CODEX_PATH`, else Gathr's own copy (0025), else `codex` on the PATH.
pub fn find(copy: Option<PathBuf>) -> Result<PathBuf, String> {
    let set = std::env::var_os(PATH_VAR).filter(|p| !p.is_empty());
    if let Some(path) = set.map(PathBuf::from) {
        return match path.is_file() {
            true => Ok(path),
            false => Err(format!("Codex's path {} isn't a file", path.display())),
        };
    }
    if let Some(copy) = copy {
        return Ok(copy);
    }
    crate::platform::find_on_path("codex")
        .ok_or_else(|| "Codex isn't installed yet, and codex isn't on the PATH".to_string())
}

/// The oldest Codex the switches and frames above were proven on.
const MIN_VERSION: [u32; 3] = [0, 166, 0];

/// Codex's version from `initialize`'s `userAgent` (`gathr/0.166.0 (…)`), and why Gathr can't run
/// it when it's too old or unreadable.
pub fn usable(user_agent: &str) -> Result<String, String> {
    let version = user_agent
        .split_once('/')
        .and_then(|(_, rest)| rest.split_whitespace().next())
        .unwrap_or_default();
    let parts: Vec<u32> = version
        .split(['.', '-'])
        .take(3)
        .map_while(|p| p.parse().ok())
        .collect();
    match parts.len() == 3 {
        true if parts.as_slice() >= MIN_VERSION.as_slice() => Ok(version.to_string()),
        true => Err(format!(
            "Codex {version} is too old: Gathr needs {}.{}.{} or later",
            MIN_VERSION[0], MIN_VERSION[1], MIN_VERSION[2]
        )),
        false => Err(format!("Codex's version wasn't read from \"{user_agent}\"")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_comes_from_the_user_agent_and_old_ones_are_refused() {
        let agent = "gathr/0.166.0 (Windows 10.0.26300; x86_64) xterm-256color (gathr; 0.0.0)";
        assert_eq!(usable(agent), Ok("0.166.0".into()));
        assert_eq!(
            usable("gathr/0.170.0-alpha.2 (Mac OS 26.0.0; arm64)"),
            Ok("0.170.0-alpha.2".into())
        );
        assert!(usable("gathr/0.165.9 (x)").unwrap_err().contains("too old"));
        assert!(usable("nothing").unwrap_err().contains("wasn't read"));
    }

    #[test]
    fn credentials_and_another_folder_never_reach_the_engine() {
        let parent = [
            ("OPENAI_API_KEY", "k"),
            ("CODEX_HOME", "x"),
            ("Codex_Access_Token", "t"),
            ("PATH", "p"),
        ];
        let env = env(
            parent.iter().map(|(k, v)| (k.into(), v.into())),
            Path::new("acct"),
        );
        let names: Vec<_> = env
            .iter()
            .map(|(k, _)| k.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["PATH", "CODEX_HOME"]);
    }
}
