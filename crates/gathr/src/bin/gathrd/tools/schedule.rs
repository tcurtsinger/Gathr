//! `schedule` and `unschedule`: an agent keeps words that come into its chat at a set time, once or repeating, as a
//! message to it (`gathr::schedules`), or removes one made in its chat. `schedule`'s answer starts with the line the
//! chat shows under the agent's message, then the words, then the schedule's id for that line to open it.

use std::time::Instant;

use axum::http::request::Parts;
use gathr::Error;
use gathr::schedules::{self, Ask, when};
use rmcp::handler::server::tool::Extension;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::CallToolResult;
use rmcp::{ErrorData, schemars, tool, tool_router};
use serde_json::json;

use super::server::{Tools, caller_of};

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Schedule {
    /// What will arrive, as a message to you: up to 2,000 characters.
    words: String,
    /// HH:MM on this PC's clock.
    time: Option<String>,
    /// Repeat on these days: mon to sun.
    days: Option<Vec<String>>,
    /// Once, on this date: YYYY-MM-DD.
    date: Option<String>,
    /// Alone: once, this many minutes from now (1 to 1440).
    in_minutes: Option<i64>,
}

#[derive(serde::Deserialize, schemars::JsonSchema)]
pub struct Unschedule {
    /// The schedule's id, as schedule answered it.
    id: String,
}

#[tool_router(router = schedule_router, vis = "pub(super)")]
impl Tools {
    #[tool(
        description = "When the person asks for something at a set time or on repeat, or for you to come back to \
                       something later in this chat: keep words that arrive here then, as a message to you. With \
                       days it repeats; with date, or neither, it is once."
    )]
    async fn schedule(
        &self,
        Parameters(ask): Parameters<Schedule>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let (chat, agent) = (caller.chat.clone(), caller.agent.clone());
        let kept = self
            .chats
            .writer
            .call(move |c| {
                let agent = agent.ok_or_else(|| Error::Refused("You're not an agent here.".into()))?;
                let now = gathr::ids::now_us();
                let asked = Ask {
                    time: ask.time,
                    days: ask.days,
                    date: ask.date,
                    in_minutes: ask.in_minutes,
                };
                let when = asked.when(c, now)?;
                let made = schedules::add(c, (&chat, &agent), &ask.words, &when, now)?;
                let mut label = made.when.chars();
                let first = label.next().map(|l| l.to_lowercase().to_string()).unwrap_or_default();
                let words = ask.words.split_whitespace().collect::<Vec<_>>().join(" ");
                Ok::<_, Error>(format!(
                    "Scheduled {first}{}:\n{words}\nschedule {}\nIts id is {}. Next: {}. It is now {} on this PC.",
                    label.as_str(),
                    made.id,
                    made.id,
                    when::day_time(c, made.next)?,
                    when::now_words(c, now)?,
                ))
            })
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())));
        if kept.is_ok() {
            self.chats.publish(json!({ "type": "schedules" }));
        }
        let said = kept.map_err(|e| e.to_string());
        Ok(self.respond(&caller, "schedule", started, said).await)
    }

    #[tool(
        description = "Remove something scheduled in this chat, by its id. Answers what is still scheduled here."
    )]
    async fn unschedule(
        &self,
        Parameters(ask): Parameters<Unschedule>,
        Extension(parts): Extension<Parts>,
    ) -> Result<CallToolResult, ErrorData> {
        let caller = caller_of(&parts)?;
        let started = Instant::now();
        let chat = caller.chat.clone();
        let removed = self
            .chats
            .writer
            .call(move |c| {
                schedules::delete(c, ask.id.trim(), Some(&chat))?;
                Ok::<_, Error>(format!(
                    "Removed {}.\n{}",
                    ask.id.trim(),
                    schedules::in_chat(c, &chat)?
                ))
            })
            .await
            .unwrap_or_else(|| Err(Error::Other("the database didn't answer".into())));
        if removed.is_ok() {
            self.chats.publish(json!({ "type": "schedules" }));
        }
        let said = removed.map_err(|e| e.to_string());
        Ok(self.respond(&caller, "unschedule", started, said).await)
    }
}
