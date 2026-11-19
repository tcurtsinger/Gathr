//! A reply the person typed in a thread (0076): the message or task it answers, checked when it is
//! sent, kept on the reply (`messages.reply_to`, `reply_task`), and told to the agent in one line of
//! Gathr's before the person's words.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;

use crate::Error;

/// How much of the message replied to the agent reads.
const WORDS_MOST: usize = 200;

/// What a reply answers: a message, or a task (a task's message has no message of its own).
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Target {
    pub reply_to: Option<String>,
    pub reply_task: Option<i64>,
}

impl Target {
    pub fn is_reply(&self) -> bool {
        self.reply_to.is_some() || self.reply_task.is_some()
    }
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// The target, checked against `chat`: a text message of it (Gathr's copy of a message stands for
/// the message it carries, as a like's does) or a task of it. Anything else is refused with why.
pub fn check(conn: &Connection, chat: &str, target: &Target) -> Result<Target, Error> {
    if let (Some(_), Some(_)) = (&target.reply_to, target.reply_task) {
        return Err(refused(
            "A reply answers one message or one task, not both.",
        ));
    }
    if let Some(n) = target.reply_task {
        let sql = "SELECT 1 FROM tasks WHERE number = ?1 AND chat_id = ?2";
        let found: Option<i64> = conn
            .query_row(sql, params![n, chat], |r| r.get(0))
            .optional()?;
        return match found {
            Some(_) => Ok(target.clone()),
            None => Err(refused(format!("No task #{n} in this chat."))),
        };
    }
    let Some(id) = &target.reply_to else {
        return Ok(Target::default());
    };
    let found: Option<(String, String)> = conn
        .query_row(
            "SELECT m.id, m.kind FROM messages c JOIN messages m ON m.id = COALESCE(c.copy_of, c.id)
             WHERE c.id = ?1 AND c.chat_id = ?2",
            [id, chat],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    match found {
        None => Err(refused(format!("No message {id} in this chat."))),
        Some((_, kind)) if kind != "text" => Err(refused(format!(
            "{id} is a step, not a message: a reply answers a message's words."
        ))),
        Some((original, _)) => Ok(Target {
            reply_to: Some(original),
            reply_task: None,
        }),
    }
}

/// The person's saved `message` as its agent reads it: Gathr's line naming what it answers, then
/// their words; their words alone when it answers nothing.
pub fn read_as(conn: &Connection, message: &str, body: &str) -> Result<String, Error> {
    Ok(match line(conn, message)? {
        Some(line) => format!("{line}\n\n{body}"),
        None => body.to_string(),
    })
}

/// Gathr's line before the reply `message`, for the agent of the turn it is in: none for a message
/// that answers nothing.
pub fn line(conn: &Connection, message: &str) -> Result<Option<String>, Error> {
    let found: Option<(Option<String>, Option<i64>, Option<String>)> = conn
        .query_row(
            "SELECT m.reply_to, m.reply_task, t.agent_id FROM messages m
             LEFT JOIN turns t ON t.id = m.turn_id WHERE m.id = ?1",
            [message],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?;
    let Some((to, task, reader)) = found else {
        return Ok(None);
    };
    let whose = |agent: Option<String>| -> Result<String, Error> {
        Ok(match agent {
            Some(a) if Some(&a) == reader.as_ref() => "your".to_string(),
            Some(a) => match crate::agents::name(conn, &a)? {
                Some(name) => format!("{name}'s"),
                None => "an agent's".to_string(),
            },
            None => String::new(),
        })
    };
    if let Some(n) = task {
        let sql = "SELECT what, owner FROM tasks WHERE number = ?1";
        let task: Option<(String, Option<String>)> = conn
            .query_row(sql, [n], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()?;
        let Some((what, owner)) = task else {
            return Ok(None);
        };
        let whose = match whose(owner)? {
            w if w.is_empty() => String::new(),
            w => format!("{w} "),
        };
        return Ok(Some(format!(
            "The person replies about {whose}task #{n} \"{what}\"."
        )));
    }
    let Some(to) = to else {
        return Ok(None);
    };
    let sql = "SELECT m.role, m.body, m.ts, t.agent_id FROM messages m
               LEFT JOIN turns t ON t.id = m.turn_id WHERE m.id = ?1";
    let target: Option<(String, String, i64, Option<String>)> = conn
        .query_row(sql, [&to], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .optional()?;
    let Some((role, body, ts, author)) = target else {
        return Ok(None);
    };
    let whose = match role.as_str() {
        "user" => "their own".to_string(),
        "gathr" => "Gathr's".to_string(),
        _ => match whose(author)? {
            w if w.is_empty() => "an agent's".to_string(),
            w => w,
        },
    };
    let words = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let words = crate::text::cut(&words, WORDS_MOST, false);
    let time = crate::ids::utc(ts);
    Ok(Some(format!(
        "The person replies to {whose} message (message {to}, {time}): \"{words}\""
    )))
}

/// The line before a queued turn's opening reply, for a turn taken over after a restart.
pub fn of_turn(conn: &Connection, turn: &str) -> Result<Option<String>, Error> {
    let sql = "SELECT id FROM messages WHERE turn_id = ?1 AND role IN ('user', 'gathr')
               ORDER BY ts, rowid LIMIT 1";
    let opening: Option<String> = conn.query_row(sql, [turn], |r| r.get(0)).optional()?;
    match opening {
        Some(id) => line(conn, &id),
        None => Ok(None),
    }
}
