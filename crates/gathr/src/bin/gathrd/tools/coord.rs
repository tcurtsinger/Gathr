//! Tasks and waits as tools (0010): the only way an agent changes them. Each acts as the calling
//! engine's agent in its chat, and a refusal says why (S68).

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::tasks::{self, Finish, Give, Kind, Said, Start, State, Took};
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
    /// A short title, at most 80 characters: the details go in `why`.
    what: String,
    /// Why, the details, and anything the owner needs to know.
    why: Option<String>,
    /// Task numbers that must be done before this one starts.
    after: Option<Vec<i64>>,
    /// `work` (the default) or `review`.
    kind: Option<String>,
    /// The PR it's about: merged settles the task, closed cancels it.
    pr: Option<i64>,
    /// True for a task that isn't part of your own: your task doesn't wait on it, and you hear back
    /// on it by itself. Leave it out for work your own task needs.
    in_passing: Option<bool>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct FinishTask {
    /// `done`, `left` (part is left: say what in `note`) or `blocked` (say on what in `note`, or
    /// name the task in `on_task`).
    how: String,
    /// Your task's number. Without it: the task this turn works on, or your one open task here.
    task: Option<i64>,
    /// The result, what's left, or why you're blocked.
    note: Option<String>,
    /// Blocked until this existing task is done, instead of a new one.
    on_task: Option<i64>,
    /// The PR your work is in: the task settles when it merges.
    pr: Option<i64>,
    /// For a review: the commit you reviewed. For work: a commit to look at.
    commit: Option<String>,
    /// For work: a file to look at.
    file: Option<String>,
    /// For a review that is done, when Gathr asks for your sign-off (the PR changes tests or check
    /// settings, or brings the project its first checks): yes once you checked they still test
    /// the code.
    signed_off: Option<bool>,
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
    /// With `pr`: until that PR is up to date with its base branch (it has every commit the base
    /// has), instead of until it merges or closes. Rather than a shell check for the same.
    up_to_date: Option<bool>,
    /// Until this command exits 0, run every few minutes in your folder by `sh` (Git Bash on
    /// Windows, never PowerShell or cmd): write it for that shell. One read-only command alone
    /// (`gh pr view`, `gh pr checks`, `git log`, `git rev-parse`) runs without
    /// asking the person; anything else asks first.
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
        description = "Give a chat member a task. It starts when its `after` tasks finish. You hear back when every task you gave \
            this turn is settled, and your own task waits on them. Set `in_passing` for a task that isn't part of your own work. \
            Check the returned tasks and PRs for duplicates."
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
                    in_passing: ask.in_passing.unwrap_or(false),
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
        description = "Report your task: done (say what to look at: a PR, a commit, a file or a note), left (say what is \
                       left) or blocked (say on what). Any task of yours, from any turn. While a turn runs on the task the \
                       report applies when that turn ends; otherwise at once. A task with open waits or PRs stays held until \
                       they end. A review reported done is the word to land its PR: Gathr merges it once its checks pass, and \
                       the answer says where it stands. Don't wait on its CI or call merge."
    )]
    async fn finish_task(
        &self,
        Parameters(ask): Parameters<FinishTask>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let agent = caller.agent.clone().unwrap_or_default();
        let chat = caller.chat.clone();
        let current = caller.on().and_then(|o| o.task);
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        let finished = self
            .db(move |c| {
                let how = Finish::named(&ask.how).ok_or_else(|| {
                    Error::Refused(format!("{} isn't one of done, left or blocked", ask.how))
                })?;
                let task = match ask.task.or(current) {
                    Some(task) => task,
                    None => only_open(c, &chat, &agent)?,
                };
                // A tool acts in its caller's chat: an agent's task in another chat isn't reported here.
                if !tasks::get(c, task)?.is_some_and(|t| t.chat == chat) {
                    return Err(Error::Refused(format!(
                        "there's no task #{task} in this chat"
                    )));
                }
                let said = Said {
                    note: ask.note.as_deref(),
                    on_task: ask.on_task,
                    pr: ask.pr,
                    commit: ask.commit.as_deref(),
                    file: ask.file.as_deref(),
                    signed_off: ask.signed_off == Some(true),
                };
                let (t, took, starts) = tasks::finish(c, &agent, task, how, &said, &trace)?;
                // Not a cancelled review's: nothing changed, and an older one may still stand.
                let review = t.kind == Kind::Review
                    && how == Finish::Done
                    && t.pr.is_some()
                    && took != Took::Cancelled;
                Ok((
                    taken(&t, took, &ask.how),
                    starts,
                    review.then_some(t.number),
                ))
            })
            .await;
        let finished = match finished {
            Ok((mut said, starts, review)) => {
                self.chats.start(starts);
                // A done review is the word to land its PR: Gathr looks at once (0049).
                if let Some(task) = review
                    && let Some(landing) =
                        crate::landing::recorded(&self.chats, &caller, task).await
                {
                    said = format!("{said}\n{landing}");
                }
                Ok(said)
            }
            Err(why) => Err(why),
        };
        Ok(self
            .respond(&caller, "finish_task", started, finished)
            .await)
    }

    #[tool(description = "Cancel a task you gave and end its waits.")]
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
                let done = tasks::get(c, ask.task)?.is_some_and(|t| t.state == State::Done);
                let said = match done {
                    true => format!(
                        "Task #{} had the person's answer already, so it's done, not cancelled. What waited on it carries on.",
                        ask.task
                    ),
                    false => format!("Task #{} was cancelled.", ask.task),
                };
                Ok((said, starts))
            })
            .await;
        Ok(self.answer(&caller, "cancel_task", started, done).await)
    }

    #[tool(description = "Restart your failed, stopped, blocked or cancelled task.")]
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
                Ok((format!("Task #{} was restarted.", ask.task), starts))
            })
            .await;
        Ok(self.answer(&caller, "restart_task", started, done).await)
    }

    #[tool(
        description = "This chat's open tasks (number, owner, state, what), your own tasks that stopped as left, \
                          blocked, failed or stopped, and your own waits with their ids."
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
                // Its own stopped tasks too: it can still report on them (0047). And its own
                // reviews that went out of date: it records them again on the same task.
                let mine = |t: &tasks::Task| {
                    let stopped =
                        matches!(t.state, State::Blocked | State::Failed | State::Stopped);
                    let stale = t.kind == Kind::Review && t.state == State::Done && t.outdated;
                    t.owner.is_some() && t.owner == agent && (stopped || stale)
                };
                let open = tasks::list(c, &chat)?
                    .into_iter()
                    .filter(|t| !t.state.settled() || mine(t));
                let mut lines: Vec<String> = open.map(|t| line(c, &t)).collect::<Result<_, _>>()?;
                if lines.is_empty() {
                    lines.push("No open tasks.".into());
                }
                for w in waits::in_chat(c, &chat, agent.as_deref())? {
                    if w.state == "open" {
                        let parts = w.parts.len() as i64;
                        let name = tasks::wait_name(c, &w.kind, w.target.as_deref(), parts)?;
                        let mut line = format!("Wait {}: {name}. Next: {}", w.id, w.then_do);
                        // What it is seen to wait on, and a look that can't be made: neither
                        // waits unseen (G101).
                        if let Some(saw) = w.saw() {
                            line.push_str(&format!(". Last look: {saw}"));
                        }
                        if let Some((failed, error)) = w.failing() {
                            let looks = if failed == 1 { "look" } else { "looks" };
                            line.push_str(&format!(". The last {failed} {looks} failed: {error}"));
                        }
                        lines.push(line);
                    }
                }
                Ok((lines.join("\n"), Vec::new()))
            })
            .await;
        Ok(self.answer(&caller, "list_tasks", started, listed).await)
    }

    #[tool(
        description = "Wait for an agent, CI, a PR to merge or close or to be up to date with its base, or a check to exit 0. \
            All conditions must be met. Gathr wakes you here on your task when they are met, or after 24 hours. Use this \
            instead of polling."
    )]
    async fn wait(
        &self,
        Parameters(ask): Parameters<WaitFor>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        if let Some(check) = &ask.check {
            let input = serde_json::json!({ "command": check });
            let mut stopping = self.stopping.clone();
            let allowed = tokio::select! {
                allowed = self.chats.ask_service(&caller, input, "check") => allowed,
                _ = stopping.wait_for(|stop| *stop) => false,
            };
            if !allowed {
                return Ok(self
                    .respond(
                        &caller,
                        "wait",
                        started,
                        Err("the wait check was not allowed".into()),
                    )
                    .await);
            }
        }
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
                let pr_kind = match ask.up_to_date {
                    Some(true) if ask.pr.is_none() => {
                        return Err(Error::Refused(
                            "up_to_date goes with pr: the PR to keep up to date".into(),
                        ));
                    }
                    Some(true) => "up_to_date",
                    _ => "pr",
                };
                parts.extend(ask.pr.map(|pr| part(pr_kind, pr.to_string())));
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
    pub(super) async fn db<T: Send + 'static>(
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
    pub(super) async fn answer(
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
    let owner = tasks::owner_name(conn, t.owner.as_deref())?;
    let reason = t
        .reason
        .as_ref()
        .map_or(String::new(), |r| format!(" ({r})"));
    let state = t.state.as_str();
    // A review says which commit it is of, and where its PR is now once that has moved past it (G42).
    let short = |commit: &str| commit.get(..7).unwrap_or(commit).to_string();
    let review = match (&t.reviewed, t.kind) {
        (Some(reviewed), Kind::Review) => {
            let at = short(reviewed);
            match (t.outdated, t.pr, &t.pr_head) {
                (true, Some(pr), Some(head)) => {
                    format!(
                        " [reviewed at {at}, out of date: #{pr} is at {}]",
                        short(head)
                    )
                }
                (true, ..) => format!(" [reviewed at {at}, out of date]"),
                _ => format!(" [reviewed at {at}]"),
            }
        }
        _ => String::new(),
    };
    Ok(format!(
        "#{} {owner} {state}{reason}{review}: {}",
        t.number, t.what
    ))
}

/// The caller's one open task in the chat, for a report from a turn on no task that names none.
fn only_open(conn: &Connection, chat: &str, agent: &str) -> Result<i64, Error> {
    let open: Vec<tasks::Task> = tasks::list(conn, chat)?
        .into_iter()
        .filter(|t| t.owner.as_deref() == Some(agent) && !t.state.settled())
        .collect();
    match open.as_slice() {
        [one] => Ok(one.number),
        [] => Err(Error::Refused(
            "This turn works on no task, and you have no open task here. Give the task's number in `task`.".into(),
        )),
        several => {
            let listed: Vec<String> = several
                .iter()
                .map(|t| format!("#{} {}", t.number, t.what))
                .collect();
            Err(Error::Refused(format!(
                "This turn works on no task, and you have {} open here. Say which in `task`: {}",
                several.len(),
                listed.join("; ")
            )))
        }
    }
}

/// What Gathr says back to a report.
fn taken(t: &tasks::Task, took: Took, how: &str) -> String {
    let n = t.number;
    let reason = |lead: &str| {
        let reason = t.reason.as_ref();
        reason.map_or(String::new(), |r| format!("{lead}{r}"))
    };
    match took {
        Took::Kept => {
            format!("Recorded: {how}. It applies when the turn running on task #{n} ends.")
        }
        Took::Applied => format!(
            "Recorded: {how}. Task #{n} is {}{}.",
            t.state.as_str(),
            reason(": ")
        ),
        Took::Already => format!(
            "Already recorded: task #{n} is done{}. Its giver or the person can restart it.",
            reason(": ")
        ),
        Took::Cancelled => format!("Task #{n} was cancelled{}. Nothing changed.", reason(": ")),
    }
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
    // A chat outside a project works in a plain folder: nothing there for GitHub to list, and
    // `gh` may take its whole limit to say so (#120's CI, 30 s on Windows). Never long either way:
    // the list only helps the giver.
    if !caller.cwd.join(".git").exists() {
        return "Open PRs: none here (not a git repository).".to_string();
    }
    let listed = crate::coord::github(&args, &caller.cwd);
    match tokio::time::timeout(std::time::Duration::from_secs(10), listed).await {
        Ok(Ok(listed)) if listed.trim().is_empty() => "Open PRs: none.".to_string(),
        Ok(Ok(listed)) => format!("Open PRs: {}", listed.trim().trim_end_matches(';')),
        Ok(Err(why)) => format!("Open PRs weren't listed: {why}"),
        Err(_) => "Open PRs weren't listed: GitHub didn't answer in 10 s".to_string(),
    }
}
