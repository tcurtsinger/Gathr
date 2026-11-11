//! Finished PR work uses the same sweep locks, path guards and events as 0034.
use crate::chats::Chats;
use gathr::event::Event;
use gathr::folders::{
    self,
    prs::{self, Job},
};
use gathr::platform::env::Vars;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

type Blocked = (&'static str, String, PathBuf);

/// Only the base repository's branch: a fork's identically named local branch isn't its PR.
pub fn queue(
    conn: &rusqlite::Connection,
    chat: &str,
    view: &Value,
    trace: &str,
) -> Result<(), gathr::Error> {
    let text = |key: &str| view[key].as_str().unwrap_or_default();
    let (branch, head, base) = (text("headRefName"), text("headRefOid"), text("baseRefName"));
    if view["isCrossRepository"].as_bool() != Some(false)
        || branch.is_empty()
        || base.is_empty()
        || branch == base
        || head.is_empty()
    {
        return Ok(());
    }
    prs::queue(conn, chat, branch, head, trace)
}

pub async fn sweep(chats: &Arc<Chats>, env: &Vars, only: Option<&str>) -> Vec<String> {
    let only = only.map(String::from);
    let found = chats
        .writer
        .call(move |c| prs::pending(c, only.as_deref()))
        .await;
    let jobs = match found {
        Some(Ok(jobs)) => jobs,
        Some(Err(error)) => {
            chats.writer.record(
                Event::point("gathr.cleanup.sweep")
                    .error("sqlite")
                    .attr("why", error.to_string()),
            );
            return Vec::new();
        }
        None => return Vec::new(),
    };
    let mut left = Vec::new();
    for job in jobs {
        let _removing = super::REMOVING.lock().await;
        if let Err((kind, why, path)) = remove(chats, &job, env).await {
            left.push(format!("{why} (ref {})", gathr::ids::reference(&job.trace)));
            let skipped = format!("{}:{kind}", path.display());
            if job.skipped.as_ref() != Some(&skipped) {
                let target = if path == Path::new(&job.project) {
                    "branch"
                } else {
                    "worktree"
                };
                chats.writer.record(
                    event(&job, "gathr.cleanup.skip", &path, target)
                        .error(kind)
                        .attr("why", why.clone()),
                );
                let (j, s) = (job, skipped);
                let _ = chats.writer.call(move |c| prs::skipped(c, &j, &s)).await;
            }
        } else {
            chats.writer.record(
                event(
                    &job,
                    "gathr.cleanup.remove",
                    Path::new(&job.project),
                    "branch",
                )
                .ok(),
            );
            let _ = chats.writer.call(move |c| prs::finished(c, &job)).await;
        }
    }
    left
}

fn event(job: &Job, name: &str, path: &Path, kind: &str) -> Event {
    Event::point(name)
        .trace(&job.trace)
        .attr("gathr.chat.id", job.chat.clone())
        .attr("path", path.display().to_string())
        .attr("kind", kind)
        .attr("branch", job.branch.clone())
}

async fn remove(chats: &Arc<Chats>, job: &Job, env: &Vars) -> Result<(), Blocked> {
    let repo = Path::new(&job.project);
    let failure = |why| ("left", why, repo.to_path_buf());
    let reference = format!("refs/heads/{}", job.branch);
    git(repo, &["check-ref-format", &reference], env)
        .await
        .map_err(failure)?;
    let job_ref = reference.clone();
    let first_tip = tip(repo, &reference, env).await.map_err(failure)?;
    if let Some(tip) = first_tip
        && tip != job.head
    {
        return Err((
            "changes",
            format!(
                "{} moved past the finished PR's head; its newer commits stay",
                job.branch
            ),
            repo.to_path_buf(),
        ));
    }
    let lookup = job.clone();
    let Some(Ok(paths)) = chats.writer.call(move |c| prs::worktrees(c, &lookup)).await else {
        return Err(failure("the recorded worktrees weren't read".into()));
    };
    for clean in paths {
        if !clean.path.join(".git").exists() {
            continue;
        }
        let branch = git(&clean.path, &["symbolic-ref", "--quiet", "HEAD"], env).await;
        if branch.as_deref() != Ok(job_ref.as_str()) {
            continue;
        }
        let protect = folders::Clean {
            chat_kept: true,
            ..clean.clone()
        };
        super::blocked(chats, &protect, env)
            .await
            .map_err(|(kind, why)| (kind, why, clean.path.clone()))?;
        let head = git(&clean.path, &["rev-parse", "HEAD"], env)
            .await
            .map_err(failure)?;
        if head != job.head {
            return Err((
                "changes",
                "the worktree moved past the finished PR's head".into(),
                clean.path.clone(),
            ));
        }
        // Git gets the final say on dirtiness too; no force or remove_dir_all fallback.
        let path = clean.path.to_string_lossy();
        if let Err(why) = git(repo, &["worktree", "remove", "--", &path], env).await {
            let kind = match gathr::worktree::changed(&clean.path, env).await {
                Ok(true) => "changes",
                _ => "left",
            };
            return Err((kind, why, clean.path.clone()));
        }
        chats.writer.record(
            event(job, "gathr.cleanup.remove", &clean.path, "worktree")
                .ok()
                .attr("gen_ai.agent.id", clean.agent.clone()),
        );
        let (path, kept) = (clean.path, clean.chat_kept);
        let _ = chats
            .writer
            .call(move |c| folders::removed(c, &path, kept))
            .await;
    }
    let registered = git(repo, &["worktree", "list", "--porcelain"], env)
        .await
        .map_err(failure)?;
    if registered
        .lines()
        .any(|l| l == format!("branch {reference}"))
    {
        return Err((
            "in_use",
            "the PR branch is still checked out; it stays".into(),
            repo.to_path_buf(),
        ));
    }
    if tip(repo, &reference, env).await.map_err(failure)?.is_some() {
        // Compare-and-delete: a branch moved since the look is never deleted.
        git(repo, &["update-ref", "-d", &reference, &job.head], env)
            .await
            .map_err(failure)?;
    }
    Ok(())
}

async fn tip(repo: &Path, reference: &str, env: &Vars) -> Result<Option<String>, String> {
    let ran = gathr::run::run(
        Path::new("git"),
        &["rev-parse", "--verify", "--quiet", reference],
        repo,
        env,
        Duration::from_secs(30),
    )
    .await?;
    match ran.code {
        Some(0) => Ok(Some(ran.out.trim().to_string())),
        Some(1) => Ok(None),
        _ => Err(ran.reason().to_string()),
    }
}

async fn git(repo: &Path, args: &[&str], env: &Vars) -> Result<String, String> {
    let ran = gathr::run::run(Path::new("git"), args, repo, env, Duration::from_secs(30)).await?;
    if ran.code == Some(0) {
        Ok(ran.out.trim().to_string())
    } else {
        Err(format!("git {} failed: {}", args[0], ran.reason()))
    }
}
