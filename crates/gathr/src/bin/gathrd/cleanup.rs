//! Gathr removes what it made for agents' work (0034): an engine's temporary files once it has
//! closed, a chat's worktrees once it's archived (Restore checks them out again from their
//! branches, which stay), and all of its folders once it's deleted. A task's finished PR also
//! removes its recorded worktrees and local branch, once clean and no longer in use (0043 C10).
//! Never a folder a running engine or wait check uses, nor, while the chat is only archived, a
//! worktree with changes not committed: each is skipped, recorded once with a reference, and tried
//! again at the next sweep, which runs with the service's looks (0010).

use std::path::Path;
use std::sync::Arc;

use gathr::event::Event;
use gathr::folders::{self, Clean};
use gathr::platform::env::Vars;
use gathr::worktree;

use crate::chats::Chats;

mod prs;
pub use prs::queue;

/// One sweep at a time, and none while an engine's folders are made: an archive's and the looks'
/// would remove the same folder at once, and a sweep could remove a folder made again meanwhile.
static SWEEPING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Held while one folder's removal is decided and done, and by Restore: a chat restored while a
/// sweep reads its worktree keeps it, as the sweep reads again whether it goes once it holds this.
static REMOVING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Holds removals off while a chat is restored.
pub async fn restoring() -> tokio::sync::MutexGuard<'static, ()> {
    REMOVING.lock().await
}

/// Holds sweeps off while an engine's folders are made, until its token says it uses them.
pub async fn hold() -> tokio::sync::MutexGuard<'static, ()> {
    SWEEPING.lock().await
}

/// The agent's engine in `chat` has closed: its temporary files go, at once unless in use.
pub async fn engine_closed(chats: &Arc<Chats>, chat: &str, agent: &str) {
    let (c, a) = (chat.to_string(), agent.to_string());
    let due = chats
        .writer
        .call(move |conn| folders::engine_closed(conn, &c, &a));
    if let Some(Ok(n)) = due.await
        && n > 0
    {
        let (chats, chat) = (chats.clone(), chat.to_string());
        tokio::spawn(async move { sweep(&chats, Some(&chat)).await });
    }
}

/// Removes what goes now, of `chat` or of every chat: what's left, each with why and its reference.
pub async fn sweep(chats: &Arc<Chats>, chat: Option<&str>) -> Vec<String> {
    let _one = SWEEPING.lock().await;
    let only = chat.map(String::from);
    let found = chats
        .writer
        .call(move |conn| folders::to_clean(conn, only.as_deref()))
        .await;
    let found = match found {
        Some(Ok(found)) => found,
        Some(Err(e)) => {
            let event = Event::point("gathr.cleanup.sweep").error("sqlite");
            chats.writer.record(event.attr("why", e.to_string()));
            return Vec::new();
        }
        None => return Vec::new(),
    };
    let env = crate::coord::fresh().await;
    let mut left = prs::sweep(chats, &env, chat).await;
    if found.is_empty() {
        return left;
    }
    for clean in found {
        // Not on the chat's own record: a delete removes that, and the reference must still find it.
        let event = |name: &str| {
            Event::point(name)
                .trace(&clean.trace)
                .attr("gathr.chat.id", clean.chat.clone())
                .attr("gen_ai.agent.id", clean.agent.clone())
                .attr("path", clean.path.display().to_string())
                .attr("kind", clean.kind.clone())
        };
        let mut removed = blocked(chats, &clean, &env).await;
        let _removing = REMOVING.lock().await;
        match still_due(chats, &clean.path).await {
            Ok(false) => continue,
            Ok(true) => {}
            Err(why) => removed = Err(("left", why)),
        }
        if removed.is_ok() {
            removed = delete(&clean, &env).await;
        }
        let (path, kept) = (clean.path.clone(), clean.chat_kept);
        if let Err((kind, why)) = removed {
            if clean.skipped.as_deref() != Some(kind) {
                let skip = event("gathr.cleanup.skip").error(kind);
                chats.writer.record(skip.attr("why", why.clone()));
                let _ = chats
                    .writer
                    .call(move |conn| folders::skipped(conn, &path, kind))
                    .await;
            }
            let reference = gathr::ids::reference(&clean.trace);
            left.push(format!("{why} (ref {reference})"));
            continue;
        }
        chats.writer.record(event("gathr.cleanup.remove").ok());
        let _ = chats
            .writer
            .call(move |conn| folders::removed(conn, &path, kept))
            .await;
    }
    left
}

/// Whether the folder still goes, read again under `REMOVING`.
async fn still_due(chats: &Chats, path: &Path) -> Result<bool, String> {
    let at = path.to_path_buf();
    match chats
        .writer
        .call(move |conn| folders::still_due(conn, &at))
        .await
    {
        Some(Ok(due)) => Ok(due),
        Some(Err(e)) => Err(format!("{} wasn't checked again: {e}", path.display())),
        None => Err(format!(
            "{} wasn't checked again: the database didn't answer",
            path.display()
        )),
    }
}

/// Why one folder can't go yet: the error kind and the reason.
async fn blocked(chats: &Chats, clean: &Clean, env: &Vars) -> Result<(), (&'static str, String)> {
    let path = &clean.path;
    let checked = clean.kind != "scratch"
        && (chats.checking().into_iter())
            .any(|(chat, agent)| chat == clean.chat && clean.agent.as_ref() == Some(&agent));
    if checked || chats.passes.using(path) {
        let why = format!(
            "{} is in use by a running engine or wait check",
            path.display()
        );
        return Err(("in_use", why));
    }
    // A folder that leads into the data folder now (its agent folder became a link) is never
    // removed: it would remove what's there.
    let data = chats.data.path();
    let agent = clean.agent.as_deref();
    let from_before = folders::from_before(data, &clean.chat, agent, path);
    if gathr::approvals::shut(path, data, from_before) {
        let why = format!("{} leads into Gathr's data folder", path.display());
        return Err(("in_data", why));
    }
    if clean.kind != "worktree" || !clean.chat_kept || !path.exists() {
        return Ok(());
    }
    match worktree::changed(path, env).await {
        Ok(false) => Ok(()),
        Ok(true) => Err((
            "changes",
            format!(
                "{} has changes not committed: it stays until they're committed or the chat is deleted",
                path.display()
            ),
        )),
        Err(e) => Err((
            "changes",
            format!("{}'s changes weren't read: {e}", path.display()),
        )),
    }
}

/// Removes one folder, or why not.
async fn delete(clean: &Clean, env: &Vars) -> Result<(), (&'static str, String)> {
    let path = &clean.path;
    if clean.kind != "worktree" {
        return match std::fs::remove_dir_all(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(("left", format!("{} wasn't deleted: {e}", path.display())))
            }
            _ => Ok(()),
        };
    }
    let project = clean.project.as_deref().map(Path::new);
    worktree::remove(project, path, env)
        .await
        .map_err(|why| ("left", why))
}
