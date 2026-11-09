//! `ask_person` (F9, 0010): the one tool for anything an agent needs from the person. From the chat's
//! Supervisor it lands on the person's Tasks list; from a member it goes to the Supervisor first,
//! who checks it. Files named with it are copied when it's given.

use std::path::{Path, PathBuf};
use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::tasks::person::{self, Ask, Choice, File, Form};
use gathr::tasks::{self, Kind};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};

use super::server::{Tools, caller_of};

/// The most files with one task, and the largest: pages and plans, not builds.
const FILES_MOST: usize = 5;
const FILE_BYTES_MOST: u64 = 10 * 1024 * 1024;

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AskPerson {
    /// The question, or what the person is to do, in one line (at most 200 characters).
    what: Option<String>,
    /// Why, and what the person needs to know to answer.
    why: Option<String>,
    /// `done` (something to do), `yesno`, `choice` (one), `choices` (any number) or `text`.
    /// Unset: `choice` with choices, `text` for a question ending in ?, else `done`.
    form: Option<String>,
    /// For a choice: your pick first, each with a few words on it.
    choices: Option<Vec<ChoiceIn>>,
    /// Files the person reads beside it (a design's pages, a plan), as paths in your folder.
    files: Option<Vec<String>>,
    /// Supervisor only: the member's request this files. Unset, the request this turn works on.
    for_task: Option<i64>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ChoiceIn {
    label: String,
    detail: Option<String>,
}

#[tool_router(router = person_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Ask the person: a decision, a pick between designs, or something only they \
                       can do. It goes on their Tasks list (from a member, to the Supervisor \
                       first, who checks it). You're woken here with the answer."
    )]
    async fn ask_person(
        &self,
        Parameters(ask): Parameters<AskPerson>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let folder = self
            .chats
            .data
            .path()
            .join("task-files")
            .join(gathr::ids::uuid());
        let (paths, cwd, to) = (
            ask.files.clone().unwrap_or_default(),
            caller.cwd.clone(),
            folder.clone(),
        );
        let copied = tokio::task::spawn_blocking(move || copy_files(&paths, &cwd, &to)).await;
        let files = match copied.unwrap_or_else(|e| Err(e.to_string())) {
            Ok(files) => files,
            Err(why) => {
                let _ = std::fs::remove_dir_all(&folder);
                return Ok(self.respond(&caller, "ask_person", started, Err(why)).await);
            }
        };
        let on = caller.on();
        let (chat, giver) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let trace = on
            .as_ref()
            .map_or_else(gathr::ids::trace_id, |o| o.trace.clone());
        let asked = self
            .db(move |c| {
                let form = match ask.form.as_deref() {
                    Some(name) => Some(Form::named(name).ok_or_else(|| {
                        Error::Refused(format!(
                            "{name} isn't a form: done, yesno, choice, choices or text"
                        ))
                    })?),
                    None => None,
                };
                let current = on.as_ref().and_then(|o| o.task);
                let request = match (ask.for_task, current) {
                    (Some(n), _) => Some(n),
                    (None, Some(n)) => tasks::get(c, n)?
                        .filter(|t| t.kind == Kind::PersonRequest)
                        .map(|t| t.number),
                    (None, None) => None,
                };
                let choices = ask.choices.unwrap_or_default();
                let a = Ask {
                    chat: &chat,
                    giver: &giver,
                    given_in: on.as_ref().map(|o| o.turn.as_str()),
                    parent: current,
                    what: ask.what.as_deref(),
                    why: ask.why.as_deref(),
                    form,
                    choices: choices
                        .into_iter()
                        .map(|c| Choice {
                            label: c.label,
                            detail: c.detail,
                        })
                        .collect(),
                    files,
                    request,
                };
                let (task, starts) = person::ask(c, a, &trace)?;
                let said = match &task.owner {
                    None => format!(
                        "Task #{} is on the person's Tasks list. You'll be woken here with the answer.",
                        task.number
                    ),
                    Some(lead) => format!(
                        "It went to {} (Supervisor) as task #{}: they check it and pass the person's \
                         answer back to you.",
                        tasks::name_of(c, lead)?,
                        task.number
                    ),
                };
                Ok((said, starts))
            })
            .await;
        if asked.is_err() {
            let _ = std::fs::remove_dir_all(&folder);
        }
        Ok(self.answer(&caller, "ask_person", started, asked).await)
    }
}

/// Copies the named files, each inside the engine's folder, into `to`: the person reads what the
/// agent meant even after its worktree moves on.
fn copy_files(paths: &[String], cwd: &Path, to: &Path) -> Result<Vec<File>, String> {
    if paths.len() > FILES_MOST {
        return Err(format!("at most {FILES_MOST} files"));
    }
    let root = cwd
        .canonicalize()
        .map_err(|e| format!("your folder can't be read: {e}"))?;
    let mut files = Vec::new();
    for (i, path) in paths.iter().enumerate() {
        let found: PathBuf = root.join(path);
        let found = found
            .canonicalize()
            .map_err(|_| format!("{path} isn't there"))?;
        let meta = std::fs::metadata(&found).map_err(|e| format!("{path}: {e}"))?;
        if !found.starts_with(&root) || !meta.is_file() {
            return Err(format!("{path} isn't a file in your folder"));
        }
        if meta.len() > FILE_BYTES_MOST {
            return Err(format!("{path} is over 10 MB"));
        }
        let name = found
            .file_name()
            .map_or_else(|| format!("file-{i}"), |n| n.to_string_lossy().into_owned());
        let copy = to.join(format!("{i}-{name}"));
        std::fs::create_dir_all(to)
            .and_then(|_| std::fs::copy(&found, &copy))
            .map_err(|e| format!("{path} wasn't copied: {e}"))?;
        files.push(File {
            name,
            size: meta.len(),
            path: copy.display().to_string(),
        });
    }
    Ok(files)
}
