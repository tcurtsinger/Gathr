//! The one way the service runs a short program and reads what it printed: git, gh and a wait's
//! check. Never asks for a password, has no window, and is stopped past its time limit.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use crate::platform::env::Vars;

/// How a program ended: its exit code (none when a signal ended it) and what it printed.
pub struct Ran {
    pub code: Option<i32>,
    pub out: String,
    pub err: String,
}

impl Ran {
    /// The line of what it said that gives the reason: a `fatal:` or `error:` line, else the last.
    pub fn reason(&self) -> &str {
        let lines: Vec<&str> = self
            .err
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let found = lines
            .iter()
            .find(|l| l.starts_with("fatal:") || l.starts_with("error:"));
        found.or(lines.last()).copied().unwrap_or("")
    }
}

/// Runs `program` in `dir` with exactly `env`: how it ended, or why it didn't start or finish.
pub async fn run(
    program: &Path,
    args: &[&str],
    dir: &Path,
    env: &Vars,
    limit: Duration,
) -> Result<Ran, String> {
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::platform::no_window(&mut command);
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    let what = format!("{name} {}", args.first().unwrap_or(&""));
    let child = tokio::process::Command::from(command)
        .spawn()
        .map_err(|e| format!("{name} didn't start: {e}"))?;
    let mut running = Running(child.id(), program);
    let out = tokio::time::timeout(limit, child.wait_with_output())
        .await
        .map_err(|_| format!("{what} didn't finish in {} s", limit.as_secs()))?
        .map_err(|e| format!("{name} wasn't waited for: {e}"))?;
    running.0 = None;
    Ok(Ran {
        code: out.status.code(),
        out: String::from_utf8_lossy(&out.stdout).trim().to_string(),
        err: String::from_utf8_lossy(&out.stderr).to_string(),
    })
}

/// A program still running when its run is dropped (past its limit, or its wait ended) is ended
/// with everything it started: a child left behind keeps the output pipe open, and on Windows the
/// service can't exit until that child does.
struct Running<'a>(Option<u32>, &'a Path);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        if let Some(pid) = self.0 {
            crate::platform::end_tree(pid, self.1);
        }
    }
}
