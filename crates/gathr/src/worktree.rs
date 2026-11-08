//! Each agent's own git worktree of its chat's project (0028, S18), adapted from the owner's
//! SerenityOS `worktrees.ts`.
//!
//! Two agents in one folder step on each other: one's half-made change is in the other's tests.
//! So each agent in a project chat works in a worktree of its own, on a branch of its own, made
//! the first time it needs one, from the project's remote default branch fetched then. Nothing
//! here removes one: work left in it is the agent's, kept until someone takes it.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::platform::env::Vars;

/// How long a fetch may take before the worktree starts from what was fetched last.
const FETCH_LIMIT: Duration = Duration::from_secs(120);
const LOCAL_LIMIT: Duration = Duration::from_secs(30);

/// Where an agent's worktree for a chat lives.
pub fn folder(data: &Path, chat: &str, agent: &str) -> PathBuf {
    data.join("worktrees").join(chat).join(agent)
}

/// The branch a new worktree takes: the agent's name and the chat, such as `gathr/orin-x4k2pq`.
pub fn branch(name: &str, chat: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        match c.is_ascii_alphanumeric() {
            true => slug.push(c),
            false if !slug.ends_with('-') => slug.push('-'),
            false => {}
        }
    }
    let slug: String = slug.trim_matches('-').chars().take(30).collect();
    let slug = if slug.is_empty() {
        "agent".into()
    } else {
        slug
    };
    let tail = chat.len().saturating_sub(6);
    format!("gathr/{slug}-{}", chat[tail..].to_lowercase())
}

/// A worktree just made: its branch, the commit it started from, and why the fetch before it
/// failed, if it did (it then starts from what was fetched last).
pub struct Made {
    pub branch: String,
    pub base: String,
    pub fetch_failed: Option<String>,
}

/// The top folder of the git repository `dir` is in: what a chat's project folder is saved as.
pub async fn top_level(dir: &Path, env: &Vars) -> Result<PathBuf, String> {
    if !dir.is_dir() {
        return Err(format!("{} isn't a folder", dir.display()));
    }
    let top = git(env, dir, &["rev-parse", "--show-toplevel"], LOCAL_LIMIT).await;
    let top = top.map_err(|why| format!("{} isn't in a git repository: {why}", dir.display()))?;
    Ok(PathBuf::from(top))
}

/// The agent's worktree at `folder`, made from `project` on `branch` if it isn't there yet:
/// `None` when it was. A branch already there is checked out again, never made afresh.
pub async fn ensure(
    project: &Path,
    folder: &Path,
    branch: &str,
    env: &Vars,
) -> Result<Option<Made>, String> {
    if folder.join(".git").exists() {
        return Ok(None);
    }
    if !project.is_dir() {
        return Err(format!("the project folder {} is gone", project.display()));
    }
    let remotes = git(env, project, &["remote"], LOCAL_LIMIT).await?;
    let origin = remotes.lines().any(|r| r == "origin");
    let fetch_failed = match origin {
        true => git(env, project, &["fetch", "--quiet", "origin"], FETCH_LIMIT)
            .await
            .err(),
        false => None,
    };
    let remote_head = match origin {
        true => git(
            env,
            project,
            &["rev-parse", "--verify", "--quiet", "origin/HEAD^{commit}"],
            LOCAL_LIMIT,
        )
        .await
        .ok(),
        false => None,
    };
    let base = match remote_head {
        Some(base) => base,
        None => git(
            env,
            project,
            &["rev-parse", "--verify", "HEAD^{commit}"],
            LOCAL_LIMIT,
        )
        .await
        .map_err(|_| {
            "the project has no commits yet, so a worktree has nothing to start from".to_string()
        })?,
    };
    // A worktree folder deleted by hand stays registered until pruned, and blocks its branch.
    git(env, project, &["worktree", "prune"], LOCAL_LIMIT).await?;
    if let Some(parent) = folder.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("{} wasn't made: {e}", parent.display()))?;
    }
    let path = folder.to_string_lossy();
    let reference = format!("refs/heads/{branch}");
    let exists = git(
        env,
        project,
        &["rev-parse", "--verify", "--quiet", &reference],
        LOCAL_LIMIT,
    )
    .await;
    let add: Vec<&str> = match exists {
        Ok(_) => vec!["worktree", "add", &path, branch],
        Err(_) => vec!["worktree", "add", "-b", branch, &path, &base],
    };
    git(env, project, &add, LOCAL_LIMIT).await?;
    Ok(Some(Made {
        branch: branch.to_string(),
        base,
        fetch_failed,
    }))
}

/// Runs git in `dir` with `env`, never asking for a password: its output, or the first line of
/// what it said went wrong. Past `limit` it's stopped.
async fn git(env: &Vars, dir: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let mut command = std::process::Command::new("git");
    command
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .stdin(std::process::Stdio::null());
    crate::platform::no_window(&mut command);
    let mut command = tokio::process::Command::from(command);
    command.kill_on_drop(true);
    let what = format!("git {}", args.first().unwrap_or(&""));
    let out = tokio::time::timeout(limit, command.output())
        .await
        .map_err(|_| format!("{what} didn't finish in {} s", limit.as_secs()))?
        .map_err(|e| format!("git didn't start: {e}"))?;
    if !out.status.success() {
        let said = String::from_utf8_lossy(&out.stderr);
        let said = said
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim();
        return Err(format!("{what} failed ({}): {said}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
