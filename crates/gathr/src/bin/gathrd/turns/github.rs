//! What a turn opens with from GitHub, read as the `pr` and `ci_failures` tools read it, so the agent
//! needn't ask: the failed jobs of the CI that woke it, at the commit that failed; and on its task's
//! PR, its branch, head and files, unless its conversation was told that head already. Bounded: a
//! read that takes too long is dropped, said why, and the turn goes on without it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use gathr::event::Event;
use gathr::sessions::Conversation;
use rusqlite::OptionalExtension;
use serde::Deserialize;

use super::session::task_of;
use super::{Ids, Task};
use crate::github;

/// The longest a turn's start waits on GitHub.
const LIMIT: Duration = Duration::from_secs(20);

/// A CI failure the turn opens with: the commit, the repository its wait named (none: the folder's
/// own) and the folder its looks ran in (none: the project's).
#[derive(Deserialize)]
struct Failed {
    commit: String,
    repo: Option<String>,
    cwd: Option<String>,
}

impl Task {
    /// `forgot`: the conversation is new, or a compaction summed it up, so it was told no head.
    pub(super) async fn github_opening(
        &mut self,
        ids: &Ids,
        choice: &Conversation,
        forgot: bool,
    ) -> Option<String> {
        let task = ids.task.or(task_of(choice));
        let conversation = match task_of(choice) {
            Some(task) => format!("task {task}"),
            None => format!("latest in {}", ids.chat),
        };
        if forgot {
            self.session
                .told_heads
                .retain(|(c, _), _| *c != conversation);
        }
        let event = Event::point("gathr.github.read")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .attr("op", "opening")
            .attr("gathr.turn.id", ids.turn.clone())
            .attr("gen_ai.agent.id", ids.agent.clone());
        let turn = ids.turn.clone();
        let read = self.chats.writer.call(move |c| {
            let failed: Option<String> = c
                .query_row("SELECT failed_ci FROM turns WHERE id = ?1", [&turn], |r| {
                    r.get(0)
                })
                .optional()?
                .flatten();
            let on = match task {
                Some(task) => gathr::tasks::get(c, task)?,
                None => None,
            };
            Ok::<_, gathr::Error>((failed, on.and_then(|t| Some((t.pr?, t.pr_head)))))
        });
        let (failed, pr) = match read.await {
            Some(Ok(found)) => found,
            failed => {
                self.record(event.error("sqlite").attr("why", format!("{failed:?}")));
                return None;
            }
        };
        let failed: Vec<Failed> = failed
            .and_then(|f| serde_json::from_str(&f).ok())
            .unwrap_or_default();
        let told = |pr: i64| self.session.told_heads.get(&(conversation.clone(), pr));
        let pr = pr.filter(|(pr, head)| head.is_none() || told(*pr) != head.as_ref());
        if failed.is_empty() && pr.is_none() {
            return None;
        }
        let folder = match github::folder_of(&self.chats, &ids.chat).await {
            Ok(folder) => folder,
            Err(why) => {
                self.record(event.error("sqlite").attr("why", why));
                return None;
            }
        };
        let pr = pr.zip(folder.clone());
        let started = Instant::now();
        let reads = async {
            let mut said = Vec::new();
            for f in &failed {
                let Some(dir) = f.cwd.clone().map(PathBuf::from).or(folder.clone()) else {
                    continue;
                };
                let at = github::failed_at((&dir, f.repo.as_deref()), &f.commit).await;
                said.extend(at.map_err(|why| format!("{}: {why}", f.commit))?);
            }
            let mut head = None;
            if let Some(((pr, _), dir)) = &pr {
                let (now, changes) = github::changes(dir, *pr)
                    .await
                    .map_err(|why| format!("#{pr}: {why}"))?;
                if told(*pr) != Some(&now) {
                    said.push(changes);
                }
                head = Some((*pr, now));
            }
            Ok::<_, String>((said, head))
        };
        let limit = limit();
        let read = match tokio::time::timeout(limit, reads).await {
            Ok(read) => read,
            Err(_) => Err(format!("GitHub took over {} s", limit.as_secs_f64())),
        };
        let ms = started.elapsed().as_millis() as i64;
        match read {
            Ok((said, head)) => {
                if let Some((pr, head)) = head {
                    self.session.told_heads.insert((conversation, pr), head);
                }
                let text = said.join("\n\n");
                let event = event.ok().attr("chars", text.chars().count());
                self.record(event.attr("ms", ms));
                (!text.is_empty()).then_some(text)
            }
            Err(why) => {
                let event = event.error("github").attr("why", why);
                self.record(event.attr("ms", ms));
                None
            }
        }
    }
}

/// `GATHR_OPENING_GITHUB_MS` shortens the limit, in dev and tests only.
fn limit() -> Duration {
    let shorter = cfg!(debug_assertions)
        .then(|| std::env::var("GATHR_OPENING_GITHUB_MS").ok()?.parse().ok())
        .flatten();
    shorter.map_or(LIMIT, Duration::from_millis)
}
