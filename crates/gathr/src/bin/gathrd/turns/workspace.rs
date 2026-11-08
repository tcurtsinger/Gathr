//! Where an engine runs and with what environment (0028): in a project chat, the agent's own
//! worktree, made the first time; elsewhere the chat's folder. Either way with the environment a
//! new sign-in would give (G23).

use std::path::PathBuf;

use gathr::event::Event;
use gathr::platform::env::{self, Fresh};
use gathr::worktree;
use rusqlite::OptionalExtension;

use super::{Assignment, Task};

pub(super) struct Workspace {
    pub cwd: PathBuf,
    /// The project folder, when the engine works in a worktree of it.
    pub project: Option<String>,
    pub env: Fresh,
}

impl Task {
    pub(super) async fn workspace(
        &self,
        assigned: &Assignment,
        trace: &str,
    ) -> Result<Workspace, String> {
        let fresh = tokio::task::spawn_blocking(|| env::fresh(std::env::vars_os().collect()))
            .await
            .map_err(|e| format!("the environment wasn't read: {e}"))?;
        let (chat, agent) = (assigned.chat.clone(), assigned.agent.clone());
        let found = self
            .chats
            .writer
            .call(move |conn| {
                let project: Option<String> = conn
                    .query_row("SELECT project FROM chats WHERE id = ?1", [&chat], |r| {
                        r.get(0)
                    })
                    .optional()?
                    .flatten();
                let name: Option<String> = match &agent {
                    Some(id) => conn
                        .query_row("SELECT name FROM agents WHERE id = ?1", [id], |r| r.get(0))
                        .optional()?,
                    None => None,
                };
                Ok::<_, gathr::Error>(project.zip(agent.zip(name)))
            })
            .await;
        let in_project = match found {
            Some(Ok(found)) => found,
            Some(Err(e)) => return Err(format!("the chat's project wasn't read: {e}")),
            None => return Err("the chat's project wasn't read: the database didn't answer".into()),
        };
        let Some((project, (agent, name))) = in_project else {
            let cwd = self.chats.data.path().join("chats").join(&assigned.chat);
            return Ok(Workspace {
                cwd,
                project: None,
                env: fresh,
            });
        };
        let folder = worktree::folder(self.chats.data.path(), &assigned.chat, &agent);
        let branch = worktree::branch(&name, &agent, &assigned.chat);
        let made = worktree::ensure(project.as_ref(), &folder, &branch, &fresh.vars).await;
        let event = Event::point("gathr.worktree.add")
            .trace(trace)
            .chat(&assigned.chat)
            .attr("gen_ai.agent.id", agent.clone())
            .attr("path", folder.display().to_string())
            .attr("project", project.clone());
        match made {
            Ok(None) => {}
            Ok(Some(made)) => self.record(
                event
                    .ok()
                    .attr("branch", made.branch)
                    .attr("base", made.base)
                    .attr("fetch_failed", made.fetch_failed),
            ),
            Err(why) => {
                self.record(event.error("worktree").attr("why", why.clone()));
                return Err(format!("the agent's worktree wasn't made: {why}"));
            }
        }
        let hooks = worktree::hooks_folder(self.chats.data.path(), &assigned.chat, &agent);
        if let Err(why) = worktree::guard_pushes(&folder, &hooks, &fresh.vars).await {
            let event = Event::point("gathr.approvals.push_guard")
                .trace(trace)
                .chat(&assigned.chat)
                .error("push_guard")
                .attr("gen_ai.agent.id", agent.clone())
                .attr("why", why.clone());
            self.record(event);
            return Err(format!("the worktree's push guard wasn't set: {why}"));
        }
        Ok(Workspace {
            cwd: folder,
            project: Some(project),
            env: fresh,
        })
    }
}
