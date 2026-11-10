//! Where an engine runs and with what environment (0028, 0034): an agent works in its own folder
//! in the agent folder, in a worktree of its own in a project chat, made the first time, with a
//! folder for the engine's temporary files that lasts as long as the engine. A turn to no agent
//! runs in the chat's folder in the data folder, as before. Either way with the environment a new
//! sign-in would give (G23).

use std::path::{Path, PathBuf};

use gathr::event::Event;
use gathr::folders::{self, Made, Place};
use gathr::platform::env::{self, Fresh};
use gathr::worktree;
use rusqlite::{Connection, OptionalExtension};

use super::{Assignment, Task};

const IN_DATA: &str = "it leads into Gathr's data folder";

pub(super) struct Workspace {
    pub cwd: PathBuf,
    /// The project folder, when the engine works in a worktree of it.
    pub project: Option<String>,
    pub env: Fresh,
    /// The agent's own folder and its project's Shared; none for a turn to no agent.
    pub place: Option<Place>,
    /// Of those, the ones it writes in without asking: never one that leads into the data folder.
    pub beside: Vec<PathBuf>,
    /// `cwd` is where Gathr put the agent's work before the agent folder, in the data folder.
    pub from_before: bool,
    /// Where its temporary files go (TEMP, TMP, TMPDIR).
    pub scratch: Option<PathBuf>,
}

/// What the database says about where the agent works in the chat.
struct Found {
    project: Option<String>,
    name: String,
    place: Place,
    /// The folder it was given before, recorded when it was made.
    recorded: Option<PathBuf>,
    /// It has a session in the chat from before the agent folder: a session resumes only in the
    /// folder it was made in, so it keeps that one.
    resumes: bool,
}

fn found(
    conn: &Connection,
    data: &Path,
    (chat, agent): (&str, &str),
) -> Result<Option<Found>, gathr::Error> {
    let project: Option<String> = conn
        .query_row("SELECT project FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let name: Option<String> = conn
        .query_row("SELECT name FROM agents WHERE id = ?1", [agent], |r| {
            r.get(0)
        })
        .optional()?;
    let Some(name) = name else {
        return Ok(None);
    };
    let resumes = conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM engine_sessions WHERE chat_id = ?1 AND agent_key = ?2)",
        [chat, agent],
        |r| r.get(0),
    )?;
    Ok(Some(Found {
        project,
        name,
        place: folders::place(conn, data, chat, agent)?,
        recorded: folders::workspace(conn, chat, agent)?,
        resumes,
    }))
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
        let data = self.chats.data.path();
        let chat = assigned.chat.clone();
        let Some(agent) = assigned.agent.clone() else {
            let cwd = data.join("chats").join(&chat);
            if self.shut(&cwd, true, (&chat, None), trace) {
                return Err(format!("{} wasn't used: {IN_DATA}", cwd.display()));
            }
            return Ok(Workspace {
                cwd,
                project: None,
                env: fresh,
                place: None,
                beside: Vec::new(),
                from_before: true,
                scratch: None,
            });
        };
        let (c, a, d) = (chat.clone(), agent.clone(), data.to_path_buf());
        let read = self
            .chats
            .writer
            .call(move |conn| found(conn, &d, (&c, &a)))
            .await;
        let found = match read {
            Some(Ok(Some(found))) => found,
            Some(Ok(None)) => return Err("its agent was deleted".into()),
            Some(Err(e)) => return Err(format!("the agent's folder wasn't read: {e}")),
            None => return Err("the agent's folder wasn't read: the database didn't answer".into()),
        };
        let cwd = found
            .recorded
            .clone()
            .unwrap_or_else(|| match &found.project {
                _ if !found.resumes => folders::new_workspace(&found.place, &chat),
                Some(_) => worktree::old_folder(data, &chat, &agent),
                None => data.join("chats").join(&chat),
            });
        let scratch = folders::scratch(&found.place, &chat);
        let from_before = folders::from_before(data, &chat, Some(&agent), &cwd);
        // Nothing is made or given in the data folder, however the agent folder leads there.
        let refused = |dir: &Path, old: bool| self.shut(dir, old, (&chat, Some(&agent)), trace);
        let must = [
            (&found.place.root, false),
            (&cwd, from_before),
            (&scratch, false),
        ];
        if let Some((dir, _)) = must.into_iter().find(|&(dir, old)| refused(dir, old)) {
            return Err(format!("{} wasn't used: {IN_DATA}", dir.display()));
        }
        let beside = [&found.place.agent, &found.place.shared]
            .into_iter()
            .filter(|dir| !refused(dir, false))
            .cloned()
            .collect();
        let kind = match found.project {
            Some(_) => "worktree",
            None => "folder",
        };
        let made = [(cwd.as_path(), kind), (&scratch, "scratch")];
        let project = found.project.as_deref();
        self.record_made(&made, (&chat, &agent), project).await?;
        let mut dirs = vec![&found.place.agent, &found.place.shared, &scratch];
        if project.is_none() {
            dirs.push(&cwd);
        }
        for dir in dirs {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("{} wasn't made: {e}", dir.display()))?;
        }
        if let Some(project) = project {
            let who = (agent.as_str(), found.name.as_str(), chat.as_str());
            self.ensure_worktree(project, &cwd, who, &fresh, trace)
                .await?;
        }
        Ok(Workspace {
            cwd,
            project: found.project,
            env: fresh,
            place: Some(found.place),
            beside,
            from_before,
            scratch: Some(scratch),
        })
    }

    /// Whether `dir` holds Gathr's data folder or leads into it through a link: never made or
    /// given to an agent (0034). Recorded with the turn's reference when it does.
    fn shut(
        &self,
        dir: &Path,
        from_before: bool,
        (chat, agent): (&str, Option<&str>),
        trace: &str,
    ) -> bool {
        let data = self.chats.data.path();
        if !gathr::approvals::shut(dir, data, from_before) {
            return false;
        }
        let event = Event::point("gathr.folder.refused")
            .trace(trace)
            .chat(chat)
            .error("in_data")
            .attr("gen_ai.agent.id", agent.map(String::from))
            .attr("path", dir.display().to_string())
            .attr("why", IN_DATA);
        self.record(event);
        true
    }

    /// Records each folder before it's made, so Gathr removes it later (0034).
    async fn record_made(
        &self,
        made: &[(&Path, &'static str)],
        (chat, agent): (&str, &str),
        project: Option<&str>,
    ) -> Result<(), String> {
        let rows: Vec<(PathBuf, &'static str)> = made
            .iter()
            .map(|(path, kind)| (path.to_path_buf(), *kind))
            .collect();
        let (chat, agent) = (chat.to_string(), agent.to_string());
        let project = project.map(String::from);
        let saved = self
            .chats
            .writer
            .call(move |conn| {
                for (path, kind) in &rows {
                    let row = Made {
                        path,
                        kind,
                        chat: &chat,
                        agent: Some(&agent),
                        project: project.as_deref().filter(|_| *kind == "worktree"),
                    };
                    folders::made(conn, &row)?;
                }
                Ok::<_, gathr::Error>(())
            })
            .await;
        match saved {
            Some(Ok(())) => Ok(()),
            Some(Err(e)) => Err(format!("the agent's folders weren't recorded: {e}")),
            None => Err("the agent's folders weren't recorded: the database didn't answer".into()),
        }
    }

    /// The agent's worktree at `folder`, made if it isn't there, with its pushes guarded (0029).
    async fn ensure_worktree(
        &self,
        project: &str,
        folder: &Path,
        (agent, name, chat): (&str, &str, &str),
        fresh: &Fresh,
        trace: &str,
    ) -> Result<(), String> {
        let branch = worktree::branch(name, agent, chat);
        let made = worktree::ensure(project.as_ref(), folder, &branch, &fresh.vars).await;
        let event = Event::point("gathr.worktree.add")
            .trace(trace)
            .chat(chat)
            .attr("gen_ai.agent.id", agent.to_string())
            .attr("path", folder.display().to_string())
            .attr("project", project.to_string());
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
        let hooks = worktree::hooks_folder(self.chats.data.path(), chat, agent);
        if let Err(why) = worktree::guard_pushes(folder, &hooks, &fresh.vars).await {
            let event = Event::point("gathr.approvals.push_guard")
                .trace(trace)
                .chat(chat)
                .error("push_guard")
                .attr("gen_ai.agent.id", agent.to_string())
                .attr("why", why.clone());
            self.record(event);
            return Err(format!("the worktree's push guard wasn't set: {why}"));
        }
        Ok(())
    }
}
