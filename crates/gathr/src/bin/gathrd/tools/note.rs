//! `note`: an agent keeps a line in Memory (`gathr::memory`), about the person, its chat's project or
//! its own work. Its answer is the line the chat shows under the agent's message: the step keeps it.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::memory::{self, Place};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use rusqlite::OptionalExtension;
use serde_json::json;

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum About {
    /// How the person likes to work: every agent gets it.
    Person,
    /// A lasting fact about this chat's project: whoever works in it gets it.
    Project,
    /// A lesson about doing your work: it stays with you in every project.
    Work,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Note {
    about: About,
    /// The note: one line, at most 200 characters.
    text: String,
    /// The exact text of a note there that this one takes the place of.
    replaces: Option<String>,
}

#[tool_router(router = note_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "Keep one short line that will still matter in later chats: about the person, this project, \
                       or your own work. Agents get these notes when they start. To change a note, give its text \
                       as replaces."
    )]
    async fn note(
        &self,
        Parameters(ask): Parameters<Note>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        let data = self.chats.data.path().to_path_buf();
        let kept = self
            .chats
            .writer
            .call(move |c| {
                let agent = agent.ok_or_else(|| Error::Refused("You're not an agent here.".into()))?;
                let (place, about) = match ask.about {
                    About::Person => (Place::You, "about you".to_string()),
                    About::Project => {
                        let project: Option<(String, String)> = c
                            .query_row(
                                "SELECT p.id, p.name FROM chats c JOIN projects p ON p.id = c.project_id
                                 WHERE c.id = ?1",
                                [&chat],
                                |r| Ok((r.get(0)?, r.get(1)?)),
                            )
                            .optional()?;
                        let (id, name) = project
                            .ok_or_else(|| Error::Refused("This chat has no project.".into()))?;
                        (Place::Project(id), format!("about {name}"))
                    }
                    About::Work => (Place::Agent(agent.clone()), "for own work".to_string()),
                };
                let path = memory::file(c, &memory::root(c, &data)?, &place)?;
                let by = gathr::agents::name_of(c, &agent)?;
                let said = match &ask.replaces {
                    Some(was) => {
                        let note = memory::replace(&path, was, &ask.text, &by)?;
                        format!("Changed a note {about}:\n{}", note.text)
                    }
                    None => {
                        let note = memory::add(&path, &ask.text, &by)?;
                        format!("Noted {about}:\n{}", note.text)
                    }
                };
                Ok::<_, Error>(said)
            })
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())));
        if kept.is_ok() {
            self.chats.publish(json!({ "type": "memory" }));
        }
        let said = kept.map_err(|e| e.to_string());
        Ok(self.respond(&caller, "note", started, said).await)
    }
}
