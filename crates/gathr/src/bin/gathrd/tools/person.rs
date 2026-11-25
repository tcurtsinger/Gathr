//! `ask_person` (F9, 0010): the one tool for anything an agent needs from the person. It stores a
//! question of its own (`gathr::questions`).

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct AskPerson {
    /// The question, or what the person is to do, in one line (at most 200 characters).
    what: Option<String>,
    /// Why, and what the person needs to know to answer.
    why: Option<String>,
    /// For a choice: your pick first, each with a few words on it.
    /// Don't number or letter the labels: Gathr letters a design's options itself.
    choices: Option<Vec<ChoiceIn>>,
    /// Files for the person to open with it: full paths, in this chat's folders.
    files: Option<Vec<String>>,
    /// To ask 2 to 4 things at once, answered together in one card: each with its own topic, what, why and
    /// choices. `what` is then a short title for them all. Ask related questions together, not one after another.
    questions: Option<Vec<PartIn>>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct PartIn {
    /// A word or two naming it, shown as its tab (at most 24 characters).
    topic: String,
    /// The question, in one line.
    what: String,
    /// Why, and what the person needs to know to answer.
    why: Option<String>,
    /// Your pick first, each with a few words on it.
    choices: Option<Vec<ChoiceIn>>,
    /// Files for the person to open with it: full paths, in this chat's folders.
    files: Option<Vec<String>>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ChoiceIn {
    label: String,
    detail: Option<String>,
}

#[tool_router(router = person_router, vis = "pub(super)")]
impl Tools {
    /// `what` with `why` below it, a line per file, and the choices. The window hears the chat's
    /// questions whole, and the chat now needs the person.
    #[tool(description = "Ask the person for input or action. The answer wakes you here.")]
    async fn ask_person(
        &self,
        Parameters(ask): Parameters<AskPerson>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (
            caller.chat.clone(),
            caller.agent.clone().unwrap_or_default(),
        );
        let what = ask.what.unwrap_or_default();
        let question = with_why(&what, ask.why.as_deref());
        let choices = choices_of(ask.choices);
        let (files, data) = (
            ask.files.unwrap_or_default(),
            self.chats.data.path().to_path_buf(),
        );
        // Several asked together: each with its own files, the call's files with the first.
        let parts: Option<Vec<_>> = ask.questions.map(|parts| {
            let mut parts: Vec<_> = parts
                .into_iter()
                .map(|p| {
                    let question = with_why(&p.what, p.why.as_deref());
                    (
                        p.topic,
                        question,
                        choices_of(p.choices),
                        p.files.unwrap_or_default(),
                    )
                })
                .collect();
            if let Some(first) = parts.first_mut() {
                first.3.extend(files.iter().cloned());
            }
            parts
        });
        let asked = self
            .chats
            .writer
            .call(move |c| {
                // Each file as the line a person's message names its files with.
                let roots = gathr::files::tree(c, &data, &chat)?.roots();
                let id = match parts {
                    Some(parts) => {
                        let mut all = Vec::new();
                        for (topic, question, choices, files) in parts {
                            let lines = gathr::files::attached(&roots, &files)?;
                            let question = gathr::files::with_lines(&question, &lines);
                            all.push(gathr::questions::Part {
                                topic,
                                question,
                                choices,
                            });
                        }
                        gathr::questions::ask_all(c, (&chat, &agent), &what, &all)?
                    }
                    None => {
                        let lines = gathr::files::attached(&roots, &files)?;
                        let question = gathr::files::with_lines(&question, &lines);
                        gathr::questions::ask(c, (&chat, &agent), &question, &choices)?
                    }
                };
                Ok::<_, Error>((id, gathr::questions::of_chat(c, &chat)?))
            })
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())));
        let said = asked.map(|(id, questions)| {
            let event = json!({ "type": "questions", "chat": caller.chat, "questions": questions });
            self.chats.publish(event);
            format!("Question {id} is with the person. Their answer wakes you here.")
        });
        let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
        crate::chat_state::changed(&self.chats, &caller.chat, &trace).await;
        let said = said.map_err(|e| e.to_string());
        Ok(self.respond(&caller, "ask_person", started, said).await)
    }
}

/// `what` with `why` below it, when there is one.
fn with_why(what: &str, why: Option<&str>) -> String {
    match why.map(str::trim) {
        Some(why) if !why.is_empty() => format!("{}\n\n{why}", what.trim()),
        _ => what.to_string(),
    }
}

fn choices_of(choices: Option<Vec<ChoiceIn>>) -> Vec<gathr::questions::Choice> {
    choices
        .unwrap_or_default()
        .into_iter()
        .map(|c| gathr::questions::Choice {
            label: c.label,
            detail: c.detail,
        })
        .collect()
}
