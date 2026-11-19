//! What a turn opens with from GitHub, read as the `pr` and `ci_failures` tools read it, so the agent
//! needn't ask: the failed jobs of the CI that woke it, at the commit that failed; and on its task's
//! PR, its branch, head and files, unless its conversation was told that head already.

use gathr::event::Event;
use gathr::sessions::Conversation;
use rusqlite::OptionalExtension;

use super::session::task_of;
use super::{Ids, Task};
use crate::github;

impl Task {
    /// `forgot`: the conversation is new, or a compaction summed it up, so it was told no head.
    pub(super) async fn github_opening(
        &mut self,
        ids: &Ids,
        choice: &Conversation,
        forgot: bool,
    ) -> Option<String> {
        let task = task_of(choice);
        if let Some(task) = task.filter(|_| forgot) {
            self.session.told_heads.remove(&task);
        }
        let turn = ids.turn.clone();
        let read = self.chats.writer.call(move |c| {
            let failed: Option<String> = c
                .query_row("SELECT failed_at FROM turns WHERE id = ?1", [&turn], |r| {
                    r.get(0)
                })
                .optional()?
                .flatten();
            let on = match task {
                Some(task) => gathr::tasks::get(c, task)?,
                None => None,
            };
            Ok::<_, gathr::Error>((failed, on.and_then(|t| Some((t.number, t.pr?, t.pr_head)))))
        });
        let event = Event::point("gathr.github.read")
            .trace(&ids.trace)
            .chat(&ids.chat)
            .attr("op", "opening")
            .attr("gathr.turn.id", ids.turn.clone())
            .attr("gen_ai.agent.id", ids.agent.clone());
        let (failed, pr) = match read.await {
            Some(Ok(found)) => found,
            failed => {
                self.record(event.error("sqlite").attr("why", format!("{failed:?}")));
                return None;
            }
        };
        let pr = pr.filter(|(task, _, head)| {
            head.is_none() || self.session.told_heads.get(task) != head.as_ref()
        });
        if failed.is_none() && pr.is_none() {
            return None;
        }
        let repo = match github::folder_of(&self.chats, &ids.chat).await {
            Ok(Some(repo)) => repo,
            Ok(None) => return None,
            Err(why) => {
                self.record(event.error("sqlite").attr("why", why));
                return None;
            }
        };
        let started = std::time::Instant::now();
        let mut said = Vec::new();
        for commit in failed.iter().flat_map(|f| f.split_whitespace()) {
            match github::failed_at(&repo, commit).await {
                Ok(failures) => said.extend(failures),
                Err(why) => {
                    let event = event.clone().attr("head", commit.to_string());
                    self.record(event.error("github").attr("why", why));
                }
            }
        }
        if let Some((task, pr, _)) = pr {
            match github::changes(&repo, pr).await {
                Ok((head, changes)) => {
                    if self.session.told_heads.insert(task, head.clone()) != Some(head) {
                        said.push(changes);
                    }
                }
                Err(why) => {
                    let event = event.clone().attr("pr", pr);
                    self.record(event.error("github").attr("why", why));
                }
            }
        }
        let text = said.join("\n\n");
        let event = event.ok().attr("chars", text.chars().count());
        self.record(event.attr("ms", started.elapsed().as_millis() as i64));
        (!text.is_empty()).then_some(text)
    }
}
