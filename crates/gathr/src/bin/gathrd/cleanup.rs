//! Gathr removes what it made for agents' work (0034): a task's temporary files once the task is
//! done or cancelled, a chat's worktrees and folders once it's archived or deleted; branches stay.
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

/// One sweep at a time: an archive's and the looks' would remove the same folder at once.
static SWEEPING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

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
    if found.is_empty() {
        return Vec::new();
    }
    let env = crate::coord::fresh().await;
    let mut left = Vec::new();
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
        let removed = remove(chats, &clean, &env).await;
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

/// Removes one folder, or why not: the error kind and the reason.
async fn remove(chats: &Chats, clean: &Clean, env: &Vars) -> Result<(), (&'static str, String)> {
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
    if clean.kind != "worktree" {
        return match std::fs::remove_dir_all(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(("left", format!("{} wasn't deleted: {e}", path.display())))
            }
            _ => Ok(()),
        };
    }
    if clean.chat_kept && path.exists() {
        let why = match worktree::changed(path, env).await {
            Ok(false) => None,
            Ok(true) => Some(format!(
                "{} has changes not committed: it stays until they're committed or the chat is deleted",
                path.display()
            )),
            Err(e) => Some(format!("{}'s changes weren't read: {e}", path.display())),
        };
        if let Some(why) = why {
            return Err(("changes", why));
        }
    }
    let project = clean.project.as_deref().map(Path::new);
    worktree::remove(project, path, env)
        .await
        .map_err(|why| ("left", why))
}
