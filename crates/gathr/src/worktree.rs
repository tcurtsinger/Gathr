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

/// The branch a new worktree takes: the agent's name, the chat and the start of the agent's id,
/// such as `gathr/orin-x4k2pq-85613e`. Names that read the same here ("Build A", "Build-A") are
/// told apart by the id.
pub fn branch(name: &str, agent: &str, chat: &str) -> String {
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
    let id: String = agent.chars().take(6).collect();
    format!("gathr/{slug}-{}-{id}", chat[tail..].to_lowercase())
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

/// Where Gathr's git hooks for an agent's worktree are written.
pub fn hooks_folder(data: &Path, chat: &str, agent: &str) -> PathBuf {
    data.join("hooks").join(chat).join(agent)
}

const HOOK: &str = include_str!("git-hook.sh");

/// Hooks git runs from a hooks folder; Gathr's stands in for each the project has, so the
/// project's own still run.
const HOOK_NAMES: &[&str] = &[
    "applypatch-msg",
    "pre-applypatch",
    "post-applypatch",
    "pre-commit",
    "pre-merge-commit",
    "prepare-commit-msg",
    "commit-msg",
    "post-commit",
    "pre-rebase",
    "post-checkout",
    "post-merge",
    "post-rewrite",
    "reference-transaction",
    "pre-auto-gc",
    "post-index-change",
];

/// Makes every `git push` from the worktree at `folder` wait for the person's answer (0029): a
/// `pre-push` hook in `hooks`, set for this worktree alone (`extensions.worktreeConfig`), so the
/// project's own checkout and other worktrees keep theirs. The project's hooks folder is kept as
/// `gathr.hooks`, and Gathr's runs the project's hook of each name after its own part.
pub async fn guard_pushes(folder: &Path, hooks: &Path, env: &Vars) -> Result<(), String> {
    let pattern = r"^(core\.hookspath|gathr\.hooks)$";
    let set = git(
        env,
        folder,
        &["config", "--worktree", "--get-regexp", pattern],
        LOCAL_LIMIT,
    )
    .await
    .unwrap_or_default();
    let value = |key: &str| {
        set.lines()
            .find_map(|l| l.strip_prefix(key)?.strip_prefix(' '))
            .map(PathBuf::from)
    };
    let (own, kept) = match value("gathr.hooks") {
        Some(own) => (own, true),
        None => {
            // Before Gathr's is set: the folder git runs this worktree's hooks from now.
            let path = ["rev-parse", "--path-format=absolute", "--git-path", "hooks"];
            (
                PathBuf::from(git(env, folder, &path, LOCAL_LIMIT).await?),
                false,
            )
        }
    };
    write_hooks(hooks, &own).map_err(|e| format!("{} wasn't written: {e}", hooks.display()))?;
    let pointed = value("core.hookspath").as_deref() == Some(hooks);
    if kept && pointed {
        return Ok(());
    }
    // Without the extension, `--worktree` writes the project's shared config: every checkout's.
    let on = ["config", "extensions.worktreeConfig", "true"];
    git(env, folder, &on, LOCAL_LIMIT).await?;
    let (own, hooks) = (own.to_string_lossy(), hooks.to_string_lossy());
    for (key, value) in [("gathr.hooks", own), ("core.hooksPath", hooks)] {
        let set = ["config", "--worktree", key, &value];
        git(env, folder, &set, LOCAL_LIMIT).await?;
    }
    Ok(())
}

/// Gathr's hook as `pre-push`, and under the name of each hook the project has in `own`.
fn write_hooks(hooks: &Path, own: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(hooks)?;
    let names = HOOK_NAMES.iter().filter(|name| own.join(name).is_file());
    for name in std::iter::once(&"pre-push").chain(names) {
        let file = hooks.join(name);
        std::fs::write(&file, HOOK)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o755))?;
        }
    }
    Ok(())
}

/// Runs git in `dir` with `env`, never asking for a password: its output, or the line of what it
/// said that gives the reason. Past `limit` it's stopped.
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
        // Progress lines ("Preparing worktree …") come first; the reason is its `fatal:` line.
        let lines: Vec<&str> = said
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let reason = lines
            .iter()
            .find(|l| l.starts_with("fatal:") || l.starts_with("error:"));
        let said = reason.or(lines.last()).copied().unwrap_or("");
        return Err(format!("{what} failed ({}): {said}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
