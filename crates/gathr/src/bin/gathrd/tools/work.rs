//! Work an AI app outside Gathr sends the person's team, and the answer it reads (`tools::outside`). A message is sent
//! as the person's own send is, through the same route, marked as from the app (`gathr::from_app`); nothing is pushed
//! to the app: it reads when it asks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::{Extension, Path, State as Extract};
use gathr::from_app;
use rmcp::schemars;
use rusqlite::OptionalExtension;
use serde_json::{Value, json};

use super::outside::db;
use crate::api::{State, Trace};
use crate::routes::NewMessage;

/// The most an app sends at once.
const WORDS_MOST: usize = 8_000;
/// How long `send` waits for the agent's turn to end. Codex gives a tool 60 s by default; Claude Code gives one over
/// standard input and output far longer. But `gathr connect` passes each message on with the one HTTP client
/// (`gathr::http`), which gives up on an answer after 30 s: so a little under that.
const WAIT: Duration = Duration::from_secs(25);

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct SendTo {
    /// What to say, as the person would: at most 8,000 characters.
    words: String,
    /// A project's name: a new chat in it, to its lead.
    project: Option<String>,
    /// A chat's id, from an earlier send or read: to go on there.
    chat: Option<String>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct ReadChat {
    /// A chat's id. Leave it out to list the person's chats.
    pub chat: Option<String>,
}

/// Sends the words, then waits a while for the turn they start to end: how the chat stands then, or that the agent is
/// still working.
pub async fn send(s: &Arc<State>, trace: &str, ask: SendTo, app: &str) -> Result<String, String> {
    if ask.words.trim().is_empty() {
        return Err("Give the words to send.".into());
    }
    let count = ask.words.chars().count();
    if count > WORDS_MOST {
        return Err(format!("The words are {count} characters: 8,000 at most."));
    }
    let t = Trace(trace.to_string());
    let chat = match (ask.project, ask.chat) {
        (Some(_), Some(_)) => return Err("Give a project or a chat, not both.".into()),
        (None, Some(chat)) => kept_chat(s, chat.trim(), true).await?,
        (Some(project), None) => {
            let id = project_id(s, project.trim()).await?;
            new_chat(s, &t, json!({ "project": id })).await?
        }
        (None, None) => new_chat(s, &t, json!({ "assistant": true })).await?,
    };
    let mut body: NewMessage =
        serde_json::from_value(json!({ "text": ask.words })).map_err(|e| e.to_string())?;
    body.app = Some(app.to_string());
    let sent = crate::routes::send(
        Extract(s.clone()),
        Extension(t),
        Path(chat.clone()),
        Json(body),
    );
    let Json(sent) = sent.await.map_err(|f| sentence(f.why()))?;
    let (turn, agent) = (
        sent["turn"].as_str().unwrap_or_default().to_string(),
        sent["turns"][0]["agent"].clone(),
    );
    let deadline = Instant::now() + WAIT;
    loop {
        let id = turn.clone();
        let status: Option<String> = db(s, move |c| {
            let sql = "SELECT status FROM turns WHERE id = ?1";
            Ok(c.query_row(sql, [&id], |r| r.get(0)).optional()?)
        })
        .await?;
        if matches!(
            status.as_deref(),
            Some("done" | "stopped" | "failed") | None
        ) {
            return read(s, &chat, app).await;
        }
        if Instant::now() >= deadline {
            let agent = agent.as_str().unwrap_or_default().to_string();
            let name = db(s, move |c| gathr::agents::name(c, &agent)).await?;
            let name = name.unwrap_or_else(|| "The agent".into());
            return Ok(format!(
                "{name} is still working in chat {chat}. Call read with this chat for the answer."
            ));
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

/// How the chat stands, the agents' words since the app's last message there, and what it waits for, if anything.
pub async fn read(s: &Arc<State>, chat: &str, app: &str) -> Result<String, String> {
    kept_chat(s, chat, false).await?;
    let (approvals, _) = s.chats.open(chat).await;
    let (id, app) = (chat.to_string(), app.to_string());
    let (standing, said, waits) = db(s, move |c| {
        let approval = match approvals.first() {
            Some(asked) => {
                let agent = asked["agent"].as_str().unwrap_or_default();
                let who = gathr::agents::name(c, agent)?.unwrap_or_else(|| "An agent".into());
                Some(format!(
                    "An approval: {who} asks to use {}.",
                    asked["tool"].as_str().unwrap_or("a tool")
                ))
            }
            None => None,
        };
        let standing = from_app::standing(c, &id, approval.is_some())?;
        let waits = match approval {
            Some(approval) => Some(approval),
            None => from_app::waits(c, &id)?,
        };
        Ok((standing, from_app::said_since(c, &id, &app)?, waits))
    })
    .await?;
    let mut lines = vec![format!("Chat {chat}: {standing}.")];
    lines.extend(said);
    if let Some(waits) = waits.filter(|_| standing == from_app::WAITING) {
        lines.push(format!(
            "It waits for the person. {waits} They answer it in Gathr."
        ));
    }
    Ok(lines.join("\n\n"))
}

/// The person's chats that aren't archived, the assistant's first: each with its id, title, project and how it stands.
pub async fn chats(s: &Arc<State>) -> Result<String, String> {
    let mut listed = db(s, |c| gathr::store::chats(c, false)).await?;
    s.chats.states.on_list(&mut listed);
    let rows = db(s, move |c| {
        let projects: HashMap<String, String> = (gathr::projects::list(c)?.into_iter())
            .map(|p| (p.id, p.name))
            .collect();
        let mut rows = Vec::new();
        for chat in listed.as_array().into_iter().flatten() {
            let id = chat["id"].as_str().unwrap_or_default();
            let project = chat["project"].as_str().and_then(|p| projects.get(p));
            let state = from_app::standing(c, id, chat["needs"] == true)?;
            let row =
                json!({ "chat": id, "title": chat["title"], "project": project, "state": state });
            rows.push((chat["assistant"] != true, row));
        }
        rows.sort_by_key(|(not_assistant, _)| *not_assistant);
        Ok(rows.into_iter().map(|(_, row)| row).collect::<Vec<Value>>())
    })
    .await?;
    serde_json::to_string_pretty(&rows).map_err(|e| e.to_string())
}

/// The chat, refused in plain words when there's none by that id or, for a send, when it is archived.
async fn kept_chat(s: &State, chat: &str, sending: bool) -> Result<String, String> {
    let id = chat.to_string();
    let archived: Option<bool> = db(s, move |c| {
        let sql = "SELECT archived_ts IS NOT NULL FROM chats WHERE id = ?1";
        Ok(c.query_row(sql, [&id], |r| r.get(0)).optional()?)
    })
    .await?;
    match archived {
        None => Err(format!(
            "There's no chat {chat}: read with no chat lists the person's chats."
        )),
        Some(true) if sending => Err(format!(
            "Chat {chat} is archived: send without a chat, or to a project, to start a new one."
        )),
        Some(_) => Ok(chat.to_string()),
    }
}

/// The project called `name`, by its id.
async fn project_id(s: &State, name: &str) -> Result<String, String> {
    let projects = db(s, |c| gathr::projects::list(c)).await?;
    let found = projects
        .into_iter()
        .find(|p| p.name.eq_ignore_ascii_case(name));
    found.map(|p| p.id).ok_or_else(|| {
        format!("There's no project called {name}: look lists the person's projects.")
    })
}

/// A chat made as the window makes one: in a project, or the assistant's own, made at its first message.
async fn new_chat(s: &Arc<State>, t: &Trace, asked: Value) -> Result<String, String> {
    let body = serde_json::from_value(asked).map_err(|e| e.to_string())?;
    let made = crate::team::create(Extract(s.clone()), Extension(t.clone()), Json(body)).await;
    match made {
        Ok(Json(chat)) => Ok(chat["id"].as_str().unwrap_or_default().to_string()),
        Err(f) if f.why() == "no agents yet" => {
            Err("The person has no agents yet: add one with add_agent first.".into())
        }
        Err(f) => Err(sentence(f.why())),
    }
}

/// A route's reason as a sentence: "the message is empty" is "The message is empty."
fn sentence(why: &str) -> String {
    let mut chars = why.trim_end_matches('.').chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => "It failed.".into(),
    }
}
