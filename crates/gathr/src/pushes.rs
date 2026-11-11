//! Which of an agent's pushes ask the person (0029, amended by the owner's "Ask only for risky
//! pushes"): its worktree's `pre-push` hook passes git's refs here, and only a push to the
//! remote's default branch, one that rewrites or deletes a branch, or one to anywhere but the
//! worktree's origin asks. Read in the worktree itself, so Claude Code and Codex are alike.

use std::path::Path;
use std::time::Duration;

use crate::platform::env::Vars;

const LIMIT: Duration = Duration::from_secs(10);
/// `ls-remote` asks the remote itself, so it gets the time a fetch would.
const REMOTE_LIMIT: Duration = Duration::from_secs(30);

/// Why the push asks the person, or `None` for an ordinary push. `dir` is the worktree; `remote`
/// and `url` are git's two arguments to the hook; `refs` its lines on standard input
/// (`<local ref> <local sha> <remote ref> <remote sha>`). What can't be read asks.
pub async fn why_ask(
    dir: &Path,
    remote: &str,
    url: &str,
    refs: &[&str],
    env: &Vars,
) -> Option<String> {
    let origin = git(
        env,
        dir,
        &["remote", "get-url", "--push", "--all", "origin"],
        LIMIT,
    )
    .await;
    if !origin.is_ok_and(|urls| urls.lines().any(|u| u.trim() == url)) {
        return Some(format!("it pushes to {url}, not the worktree's origin"));
    }
    let mut default = None;
    for line in refs {
        let parts: Vec<&str> = line.split_whitespace().collect();
        let [_, local, remote_ref, remote_sha] = parts[..] else {
            return Some(format!("git gave a ref Gathr can't read: {line}"));
        };
        if zero(local) {
            return Some(format!("it deletes {remote_ref}"));
        }
        if default.is_none() {
            default = Some(default_branch(dir, remote, env).await);
        }
        match default.as_ref().expect("just read") {
            Some(branch) if *remote_ref == format!("refs/heads/{branch}") => {
                return Some(format!("it pushes to the default branch, {branch}"));
            }
            Some(_) => {}
            None => return Some("the remote's default branch isn't known".into()),
        }
        if !zero(remote_sha) {
            let ancestor = ["merge-base", "--is-ancestor", remote_sha, local];
            let ran = crate::run::run(Path::new("git"), &ancestor, dir, env, LIMIT).await;
            match ran.map(|r| r.code) {
                Ok(Some(0)) => {}
                Ok(Some(1)) => return Some(format!("it rewrites {remote_ref} (a force push)")),
                _ => {
                    let why = format!("it may rewrite {remote_ref}: its commit isn't known here");
                    return Some(why);
                }
            }
        }
    }
    None
}

fn zero(sha: &str) -> bool {
    sha.bytes().all(|b| b == b'0')
}

/// The remote's default branch: as the worktree's last fetch saw it, else as the remote says.
async fn default_branch(dir: &Path, remote: &str, env: &Vars) -> Option<String> {
    let head = format!("refs/remotes/{remote}/HEAD");
    let local = git(env, dir, &["symbolic-ref", "--quiet", &head], LIMIT).await;
    let prefix = format!("refs/remotes/{remote}/");
    if let Some(branch) = local
        .ok()
        .and_then(|r| r.trim().strip_prefix(&prefix).map(String::from))
    {
        return Some(branch);
    }
    let said = git(
        env,
        dir,
        &["ls-remote", "--symref", remote, "HEAD"],
        REMOTE_LIMIT,
    )
    .await;
    said.ok()?.lines().find_map(|line| {
        let target = line.strip_prefix("ref: refs/heads/")?;
        Some(target.split_whitespace().next()?.to_string())
    })
}

async fn git(env: &Vars, dir: &Path, args: &[&str], limit: Duration) -> Result<String, String> {
    let ran = crate::run::run(Path::new("git"), args, dir, env, limit).await?;
    match ran.code {
        Some(0) => Ok(ran.out),
        _ => Err(ran.reason().to_string()),
    }
}
