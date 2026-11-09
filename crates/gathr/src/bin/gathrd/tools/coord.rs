//! Tasks and waits as tools (0010): the only way an agent changes them. Each acts as the calling
//! engine's agent in its chat, and a refusal says why (S68).

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::tasks::{self, Finish, Give, Kind, Said, Start};
use gathr::waits::{self, Leave, Part};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use rusqlite::Connection;

use super::Caller;
use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct GiveTask {
    /// The agent who does it, by name: one of this chat's.
    to: String,
    /// What to do, in one line (at most 200 characters).
    what: String,
    /// Why, and anything the owner needs to know.
    why: Option<String>,
    /// Task numbers that must be done before this one starts.
    after: Option<Vec<i64>>,
    /// `work` (the default) or `review`.
    kind: Option<String>,
    /// The PR it's about: merged settles the task, closed cancels it.
    pr: Option<i64>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct FinishTask {
    /// `done`, `left` (part is left: say what) or `blocked` (say on whom).
    how: String,
    /// Your task's number: the one this turn works on unless set.
    task: Option<i64>,
    /// The result, what's left, or why you're blocked.
    note: Option<String>,
    /// Blocked until this existing task is done, instead of a new one.
    on_task: Option<i64>,
    /// The PR your work is in: the task settles when it merges.
    pr: Option<i64>,
    /// For a review: the commit you reviewed.
    commit: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct TaskNumber {
    task: i64,
    /// Why (for a cancel).
    why: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct WaitFor {
    /// Until this agent of the chat, by name, has nothing running or queued here. Not for an
    /// agent on a task you gave: you hear back on that already.
    agent: Option<String>,
    /// Until CI on a PR (`#54`) or a commit has finished. A PR is followed to its newest commit.
    ci: Option<String>,
    /// Until this PR merges or closes.
    pr: Option<i64>,
    /// Until this shell command exits 0, run every few minutes in your folder.
    check: Option<String>,
    /// What you'll do then: you're told it when woken.
    then: String,
    /// `owner/name`, when the PR or commit isn't in your folder's repository.
    repo: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct WaitId {
    id: String,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Nothing {}

#[tool_router(router = coord_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Give one of this chat's agents a task. It starts when its `after` tasks \
                          are done; you hear back once every task you gave in this turn is \
                          settled. The answer lists open tasks and PRs, so a duplicate shows."
    )]
    async fn give_task(
        &self,
        Parameters(ask): Parameters<GiveTask>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let on = caller.on();
        let chat = caller.chat.clone();
        let giver = caller.agent.clone();
        let given = self
            .db(move |c| {
                let kind = ask.kind.as_deref().unwrap_or("work");
                let kind = Kind::named(kind).ok_or_else(|| {
                    Error::Refused(format!("{kind} isn't a kind: work or review"))
                })?;
                let owner = agent_named(c, &chat, &ask.to)?;
                let g = Give {
                    chat: &chat,
                    giver: giver.as_deref(),
                    owner: &owner,
                    given_in: on.as_ref().map(|o| o.turn.as_str()),
                    parent: on.as_ref().and_then(|o| o.task),
                    kind,
                    what: &ask.what,
                    why: ask.why.as_deref(),
                    after: ask.after.as_deref().unwrap_or_default(),
                    pr: ask.pr,
                };
                let trace = on
                    .as_ref()
                    .map_or_else(gathr::ids::trace_id, |o| o.trace.clone());
                let (task, starts) = tasks::give(c, &g, &trace)?;
                let others = tasks::list(c, &chat)?
                    .into_iter()
                    .filter(|t| t.number != task.number && !t.state.settled())
                    .map(|t| line(c, &t))
                    .collect::<Result<Vec<_>, _>>()?;
                let state = match task.state {
                    tasks::State::Waiting => format!("waiting on {}", numbers(&task.after)),
                    state => state.as_str().to_string(),
                };
                let said = format!("Task #{} for {}: {state}.", task.number, ask.to);
                Ok((said, others, starts))
            })
            .await;
        let answer = match given {
            Ok((said, others, starts)) => {
                self.chats.start(starts);
                let mut lines = vec![said];
                if !others.is_empty() {
                    lines.push(format!("Open in this chat: {}", others.join("; ")));
                }
                lines.push(open_prs(&caller).await);
                Ok(lines.join("\n"))
            }
            Err(why) => Err(why),
        };
        Ok(self.respond(&caller, "give_task", started, answer).await)
    }

    #[tool(
        description = "Say how your task stands: done, left (part is left) or blocked. It \
                          applies when this turn ends; a task with a wait or PR still open is \
                          held on it, and settles when it ends."
    )]
    async fn finish_task(
        &self,
        Parameters(ask): Parameters<FinishTask>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let agent = caller.agent.clone().unwrap_or_default();
        let current = caller.on().and_then(|o| o.task);
        let finished = self
            .db(move |c| {
                let how = Finish::named(&ask.how).ok_or_else(|| {
                    Error::Refused(format!("{} isn't one of done, left or blocked", ask.how))
                })?;
                let task = ask.task.or(current).ok_or_else(|| {
                    Error::Refused(
                        "this turn works on no task, so there's nothing to finish".into(),
                    )
                })?;
                let said = Said {
                    note: ask.note.as_deref(),
                    on_task: ask.on_task,
                    pr: ask.pr,
                    commit: ask.commit.as_deref(),
                };
                let t = tasks::finish(c, &agent, task, how, &said)?;
                Ok((
                    format!("Task #{}: {} when this turn ends.", t.number, ask.how),
                    Vec::new(),
                ))
            })
            .await;
        Ok(self.answer(&caller, "finish_task", started, finished).await)
    }

    #[tool(description = "Cancel a task you gave: nobody is to do it any more. Its waits end.")]
    async fn cancel_task(
        &self,
        Parameters(ask): Parameters<TaskNumber>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (by, trace) = (caller.agent.clone(), trace_of(&caller));
        let done = self
            .db(move |c| {
                let why = ask.why.as_deref().unwrap_or("cancelled by its giver");
                let starts = tasks::cancel(c, by.as_deref(), ask.task, why, &trace)?;
                Ok((format!("Task #{} is cancelled.", ask.task), starts))
            })
            .await;
        Ok(self.answer(&caller, "cancel_task", started, done).await)
    }

    #[tool(description = "Start a task you gave again after it failed, stopped or was blocked.")]
    async fn restart_task(
        &self,
        Parameters(ask): Parameters<TaskNumber>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (by, trace) = (caller.agent.clone(), trace_of(&caller));
        let done = self
            .db(move |c| {
                let starts = tasks::restart(c, by.as_deref(), ask.task, &trace)?;
                Ok((format!("Task #{} starts again.", ask.task), starts))
            })
            .await;
        Ok(self.answer(&caller, "restart_task", started, done).await)
    }

    #[tool(
        description = "This chat's open tasks (number, owner, state, what) and your own waits \
                          with their ids."
    )]
    async fn list_tasks(
        &self,
        Parameters(_): Parameters<Nothing>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        let listed = self
            .db(move |c| {
                let open = tasks::list(c, &chat)?
                    .into_iter()
                    .filter(|t| !t.state.settled());
                let mut lines: Vec<String> = open.map(|t| line(c, &t)).collect::<Result<_, _>>()?;
                if lines.is_empty() {
                    lines.push("No open tasks.".into());
                }
                for w in waits::in_chat(c, &chat, agent.as_deref())? {
                    if w.state == "open" {
                        let parts = w.parts.len() as i64;
                        let name = tasks::wait_name(c, &w.kind, w.target.as_deref(), parts)?;
                        lines.push(format!(
                            "Your wait {}: on {name}, then: {}",
                            w.id, w.then_do
                        ));
                    }
                }
                Ok((lines.join("\n"), Vec::new()))
            })
            .await;
        Ok(self.answer(&caller, "list_tasks", started, listed).await)
    }

    #[tool(
        description = "Wait instead of looping: Gathr wakes you here, on your task, when an \
                          agent is done, CI finishes, a PR merges or closes, or a check exits 0. \
                          Give several to wait for all of them. After a day you're woken anyway."
    )]
    async fn wait(
        &self,
        Parameters(ask): Parameters<WaitFor>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (on, trace) = (caller.on(), trace_of(&caller));
        let (chat, agent) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let cwd = caller.cwd.display().to_string();
        let left = self
            .db(move |c| {
                let part = |kind: &str, target: String| Part {
                    kind: kind.into(),
                    target,
                    repo: ask.repo.clone(),
                };
                let mut parts = Vec::new();
                if let Some(name) = &ask.agent {
                    parts.push(part("agent", agent_named(c, &chat, name)?));
                }
                parts.extend(ask.ci.clone().map(|ci| part("ci", ci)));
                parts.extend(ask.pr.map(|pr| part("pr", pr.to_string())));
                parts.extend(ask.check.clone().map(|check| part("check", check)));
                let leave = Leave {
                    chat: &chat,
                    agent: &agent,
                    task: on.as_ref().and_then(|o| o.task),
                    left_in: on.as_ref().map(|o| o.turn.as_str()),
                    cwd: Some(&cwd),
                    then_do: &ask.then,
                    parts,
                };
                let wait = waits::leave(c, &leave, &trace)?;
                let said = format!("Waiting ({}). You'll be woken here.", wait.id);
                Ok((said, Vec::new()))
            })
            .await;
        Ok(self.answer(&caller, "wait", started, left).await)
    }

    #[tool(description = "End one of your waits by its id, from list_tasks.")]
    async fn end_wait(
        &self,
        Parameters(ask): Parameters<WaitId>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let trace = trace_of(&caller);
        let ended = self
            .db(move |c| {
                waits::end(c, &chat, &agent, &ask.id, &trace)?;
                Ok((format!("Wait {} ended.", ask.id), Vec::new()))
            })
            .await;
        Ok(self.answer(&caller, "end_wait", started, ended).await)
    }
}

type Done = (String, Vec<Start>);

impl Tools {
    /// Runs a change on the database: its answer, or why it was refused or failed.
    async fn db<T: Send + 'static>(
        &self,
        work: impl FnOnce(&mut Connection) -> Result<T, Error> + Send + 'static,
    ) -> Result<T, String> {
        match self.chats.writer.call(work).await {
            Some(Ok(done)) => Ok(done),
            Some(Err(e)) => Err(e.to_string()),
            None => Err("the database didn't answer".into()),
        }
    }

    /// Queues what the change starts, and answers.
    async fn answer(
        &self,
        caller: &Caller,
        tool: &str,
        started: Instant,
        done: Result<Done, String>,
    ) -> CallToolResult {
        let said = done.map(|(said, starts)| {
            self.chats.start(starts);
            said
        });
        self.respond(caller, tool, started, said).await
    }
}

fn trace_of(caller: &Caller) -> String {
    caller.trace().unwrap_or_else(gathr::ids::trace_id)
}

/// The chat's agent with this name, ignoring case.
fn agent_named(conn: &Connection, chat: &str, name: &str) -> Result<String, Error> {
    let members = gathr::team::members(conn, chat)?;
    let wanted = name.trim().trim_start_matches('@').to_lowercase();
    let found = members.iter().find(|m| m.name.to_lowercase() == wanted);
    match found {
        Some(m) => Ok(m.agent.clone()),
        None => {
            let names: Vec<&str> = members.iter().map(|m| m.name.as_str()).collect();
            Err(Error::Refused(format!(
                "no agent {name} in this chat: its agents are {}",
                names.join(", ")
            )))
        }
    }
}

fn line(conn: &Connection, t: &tasks::Task) -> Result<String, Error> {
    let owner = tasks::name_of(conn, &t.owner)?;
    let reason = t
        .reason
        .as_ref()
        .map_or(String::new(), |r| format!(" ({r})"));
    let state = t.state.as_str();
    Ok(format!("#{} {owner} {state}{reason}: {}", t.number, t.what))
}

fn numbers(after: &[i64]) -> String {
    let all: Vec<String> = after.iter().map(|n| format!("#{n}")).collect();
    all.join(", ")
}

/// The open PRs of the caller's repository, so a fix that's already under way shows (G74).
async fn open_prs(caller: &Caller) -> String {
    let args = [
        "pr",
        "list",
        "--state",
        "open",
        "--limit",
        "20",
        "--json",
        "number,title",
        "--template",
        "{{range .}}#{{.number}} {{.title}}; {{end}}",
    ];
    match crate::coord::github(&args, &caller.cwd).await {
        Ok(listed) if listed.trim().is_empty() => "Open PRs: none.".to_string(),
        Ok(listed) => format!("Open PRs: {}", listed.trim().trim_end_matches(';')),
        Err(why) => format!("Open PRs weren't listed: {why}"),
    }
}
