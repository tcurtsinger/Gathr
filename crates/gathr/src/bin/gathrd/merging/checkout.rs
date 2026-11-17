//! After a merge GitHub confirmed, the project's own folder is brought to it, a fast-forward or
//! nothing (0067). Never at the cost of the person's work: git refuses what would overwrite it,
//! and the person is told once in the chat why the folder wasn't updated.

use std::path::Path;
use std::sync::Arc;

use gathr::event::Event;
use gathr::platform::env::Vars;
use serde_json::json;

use super::{By, Project, ancestor, default_branch, git};
use crate::chats::Chats;

/// Fast-forwards the folder to `base` on origin, or tells the person in the chat why not.
pub async fn update(
    chats: &Arc<Chats>,
    by: &By,
    project: &Project,
    pr: i64,
    base: &str,
    vars: &Vars,
) {
    let event = Event::point("gathr.merge.checkout")
        .trace(&by.trace)
        .chat(&by.chat)
        .attr("gathr.project.id", project.id.clone())
        .attr("pr", pr)
        .attr("base", base.to_string());
    let (kind, why) = match fast_forward(&project.folder, base, vars).await {
        Ok(commit) => {
            chats.writer.record(event.ok().attr("commit", commit));
            return;
        }
        Err(not) => not,
    };
    let next = match kind {
        "not_default" => "",
        _ => " Update it yourself with git pull --ff-only once it's safe.",
    };
    let line = format!(
        "Merged #{pr}, but didn't update the project folder: {why}.{next} (ref {})",
        gathr::ids::reference(&by.trace)
    );
    let chat = by.chat.clone();
    let said = crate::landing::db(chats, move |c| gathr::store::say(c, &chat, &line)).await;
    let event = event.error(kind).attr("why", why);
    let event = match said {
        Ok(message) => {
            chats.publish(json!({ "type": "message", "chat": by.chat, "message": message }));
            event
        }
        Err(e) => event.attr("unsaid", e),
    };
    chats.writer.record_now(event).await;
}

/// The folder fast-forwarded to `base` on origin: the commit it's at. Otherwise the kind of
/// reason and the reason, in the person's words.
async fn fast_forward(
    repo: &Path,
    base: &str,
    vars: &Vars,
) -> Result<String, (&'static str, String)> {
    // A squash is made on GitHub: the fetch before the merge doesn't have it.
    git(repo, &["fetch", "--quiet", "origin"], vars)
        .await
        .map_err(|why| ("fetch", format!("fetching from origin failed ({why})")))?;
    let default = default_branch(repo, vars)
        .await
        .map_err(|why| ("default", why))?;
    if base != default {
        let why = format!("it went into {base}, not the default branch, {default}");
        return Err(("not_default", why));
    }
    if let Some(doing) = under_way(repo, vars).await {
        return Err(("under_way", format!("a {doing} is under way in it")));
    }
    let on = git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"], vars).await;
    let on = on.map(|b| b.trim().to_string()).unwrap_or_default();
    if on != base {
        let why = match on.is_empty() {
            true => "it isn't on a branch".to_string(),
            false => format!("it's on {on}, not {base}"),
        };
        return Err(("branch", why));
    }
    let origin = format!("refs/remotes/origin/{base}");
    if !ancestor(repo, "HEAD", &origin, vars).await {
        let why = format!("its {base} has commits that aren't on origin");
        return Err(("local_commits", why));
    }
    // Autostash off whatever the person's config says: on, it stashes their change, moves, and
    // leaves conflict markers in a file the merge touched (Vale).
    let args = ["merge", "--ff-only", "--no-autostash", "--quiet", &origin];
    if let Err(why) = git(repo, &args, vars).await {
        return Err(("refused", refused(repo, &origin, why, vars).await));
    }
    let at = git(repo, &["rev-parse", "HEAD"], vars).await;
    Ok(at.map(|c| c.trim().to_string()).unwrap_or_default())
}

/// Why git refused the fast-forward, naming the person's changed files the merge touches: git's
/// own words list them only in its language.
async fn refused(repo: &Path, origin: &str, why: String, vars: &Vars) -> String {
    let touched = git(repo, &["diff", "--name-only", "HEAD", origin], vars).await;
    let changed = git(repo, &["diff", "--name-only", "HEAD"], vars).await;
    let (touched, changed) = (touched.unwrap_or_default(), changed.unwrap_or_default());
    let both: Vec<&str> = (changed.lines())
        .filter(|f| touched.lines().any(|t| t == *f))
        .collect();
    match both.is_empty() {
        true => format!("git refused to fast-forward it ({why})"),
        false => format!(
            "it has changes to {} that the merge would overwrite",
            both.join(", ")
        ),
    }
}

/// What git is in the middle of in the folder, if anything: a merge, rebase, cherry-pick or revert.
async fn under_way(repo: &Path, vars: &Vars) -> Option<&'static str> {
    for (path, doing) in [
        ("MERGE_HEAD", "merge"),
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        // A pick of several commits, or a revert, keeps only this once a conflict is committed (Vale).
        ("sequencer", "cherry-pick or revert"),
    ] {
        let at = git(repo, &["rev-parse", "--git-path", path], vars).await;
        if at.is_ok_and(|at| repo.join(at.trim()).exists()) {
            return Some(doing);
        }
    }
    None
}
