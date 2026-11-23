//! A reply the person typed under a message (0076): the message it answers, checked when it is sent,
//! kept on the reply (`messages.reply_to`), and told to the agent in one line of Gathr's before the
//! person's words.

use rusqlite::{Connection, OptionalExtension};
use serde::Deserialize;

use crate::Error;

/// How much of the message replied to the agent reads.
const WORDS_MOST: usize = 200;

/// What a reply answers.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct Target {
    pub reply_to: Option<String>,
}

impl Target {
    pub fn is_reply(&self) -> bool {
        self.reply_to.is_some()
    }
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// The target, checked against `chat`: a text message of it (Gathr's copy of a message stands for
/// the message it carries, as a like's does). Anything else is refused with why.
pub fn check(conn: &Connection, chat: &str, target: &Target) -> Result<Target, Error> {
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
        }),
    }
}

/// The person's saved `message` as its agent reads it: Gathr's line naming what it answers and who
/// else got it, then their words; their words alone when it answers nothing and went to one agent.
pub fn read_as(conn: &Connection, message: &str, body: &str) -> Result<String, Error> {
    Ok(match line(conn, message)? {
        Some(line) => format!("{line}\n\n{body}"),
        None => body.to_string(),
    })
}

/// The person's `message` as `reader`, another agent they picked, reads it in Gathr's copy (0028).
pub fn copy_for(conn: &Connection, message: &str, reader: &str) -> Result<String, Error> {
    let Some(sent) = sent(conn, message)? else {
        return Err(Error::Refused(format!(
            "No message {message} of the person's."
        )));
    };
    let words = conn.query_row("SELECT body FROM messages WHERE id = ?1", [message], |r| {
        r.get::<_, String>(0)
    })?;
    Ok(
        match said_to(conn, &sent.target, Some(reader), &sent.everyone)? {
            Some(line) => format!("{line}\n\n{words}"),
            None => words,
        },
    )
}

/// Gathr's line before the person's `message`, for the agent of the turn it is in: none for a
/// message that answers nothing and went to that agent alone, or one that isn't the person's. A
/// scheduled run's words get the line that says which schedule it is, a project's brief the line that says who wrote it,
/// and words an AI app sent the line that names the app.
pub fn line(conn: &Connection, message: &str) -> Result<Option<String>, Error> {
    if let Some(line) = crate::schedules::line(conn, message)? {
        return Ok(Some(line));
    }
    if let Some(line) = crate::proposals::line(conn, message)? {
        return Ok(Some(line));
    }
    if let Some(line) = crate::from_app::line(conn, message)? {
        return Ok(Some(line));
    }
    match sent(conn, message)? {
        Some(sent) => said_to(conn, &sent.target, sent.agent.as_deref(), &sent.everyone),
        None => Ok(None),
    }
}

/// What the person's message answers, the agent it was sent to, and everyone it went to: that
/// agent, then each picked agent in the order the names stand.
struct Sent {
    target: Target,
    agent: Option<String>,
    everyone: Vec<String>,
}

fn sent(conn: &Connection, message: &str) -> Result<Option<Sent>, Error> {
    let found = conn
        .query_row(
            "SELECT m.reply_to, t.agent_id, m.picks FROM messages m
             LEFT JOIN turns t ON t.id = m.turn_id WHERE m.id = ?1 AND m.role = 'user'",
            [message],
            |r| {
                let target = Target {
                    reply_to: r.get(0)?,
                };
                let agent: Option<String> = r.get(1)?;
                let picks: Option<String> = r.get(2)?;
                let mut everyone: Vec<String> = agent.iter().cloned().collect();
                for picked in crate::picks::agents(picks.as_deref()) {
                    if !everyone.contains(&picked) {
                        everyone.push(picked);
                    }
                }
                Ok(Sent {
                    target,
                    agent,
                    everyone,
                })
            },
        )
        .optional()?;
    Ok(found)
}

/// Gathr's line for `reader` before the person's words: what the reply `target` answers, then who
/// else of `everyone` got it; none when it answers nothing and went to `reader` alone.
fn said_to(
    conn: &Connection,
    target: &Target,
    reader: Option<&str>,
    everyone: &[String],
) -> Result<Option<String>, Error> {
    let mut names = vec!["you".to_string()];
    for agent in everyone.iter().filter(|a| Some(a.as_str()) != reader) {
        names.push(crate::agents::name(conn, agent)?.unwrap_or_else(|| "an agent".into()));
    }
    let to = (names.len() > 1).then(|| listed(&names));
    Ok(match (answered(conn, target, reader)?, to) {
        (Some(line), Some(to)) => Some(format!("{line} They sent it to {to}.")),
        (Some(line), None) => Some(line),
        (None, Some(to)) => Some(format!("The person sent this to {to}.")),
        (None, None) => None,
    })
}

/// "you and Bo", "you, Ann and Bo".
fn listed(names: &[String]) -> String {
    match names.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", rest.join(", ")),
        None => String::new(),
    }
}
/// What the reply `target` answers, in words for `reader`: none for a message that answers nothing.
fn answered(
    conn: &Connection,
    target: &Target,
    reader: Option<&str>,
) -> Result<Option<String>, Error> {
    let to = target.reply_to.clone();
    let reader = reader.map(String::from);
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
