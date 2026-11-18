//! The person's like (0057): "approved" or "I agree". On an agent's message it reaches that agent at
//! once, as a short message from the person would; on a bubble holding a waiting question with a pick
//! it is that pick; on a plan waiting for the person it is Approve. Each act goes through the path its
//! click takes, so it is checked as that click is. A message's like acts once: given again after it
//! was taken back, it is the mark alone.

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};

use crate::Error;
use crate::event::Event;
use crate::tasks::person::{self, Answer, Form};
use crate::tasks::{Kind, Start, State};

/// The person as a giver: an agent's id is 16 hex characters, so it never clashes.
pub const PERSON: &str = "person";
/// How much of the liked message the agent reads.
const WORDS_MOST: usize = 80;

/// What a like did: the reaction as history shows it, what it did, and what follows.
pub struct Given {
    pub reaction: Value,
    /// `delivered`, `answered`, `approved` or `marked`; `standing` when one already stood.
    pub did: &'static str,
    /// The turns the answer or the approval starts.
    pub starts: Vec<Start>,
    pub deliver: Option<Delivery>,
}

/// A like the agent reads: who, the message it is on, and the line.
pub struct Delivery {
    pub agent: String,
    pub liked: String,
    pub line: String,
}

/// The message a like is on: Gathr's copy of a teammate's message stands for the message it carries.
struct Target {
    id: String,
    turn: Option<String>,
    author: Option<String>,
    role: String,
    kind: String,
    body: String,
    ts: i64,
}

/// What the author reads when the person likes its message `id` of `time`.
pub fn line(id: &str, time: &str, body: &str) -> String {
    let first = crate::text::first_line(body).unwrap_or_default();
    let words = crate::text::cut(first, WORDS_MOST, false);
    format!(
        "The person liked your message {id} of {time}: \"{words}\". A like means they approve it or agree. \
         If it answers something you were waiting on, carry on. Otherwise end your turn without replying."
    )
}

/// The person likes `message` in `chat`; `questions` are the waiting questions its bubble holds.
pub fn give(
    conn: &mut Connection,
    chat: &str,
    message: &str,
    questions: &[i64],
    trace: &str,
) -> Result<Given, Error> {
    crate::archive::refuse_archived(conn, chat)?;
    let target = target(conn, chat, message)?;
    if let Some(reaction) = standing(conn, &target.id)? {
        return Ok(Given {
            reaction,
            did: "standing",
            starts: Vec::new(),
            deliver: None,
        });
    }
    let version = version_of(conn, &target.id)?;
    if target.kind != "text" && (version.is_some() || questions.is_empty()) {
        return Err(refused(format!(
            "{message} is a step, not a message: a like goes on an agent's words."
        )));
    }
    let first = !ever_given(conn, chat, &target.id)?;
    let (did, starts, deliver) = match (first, version) {
        (false, _) => ("marked", Vec::new(), None),
        (true, Some((plan, n))) => {
            let decided = crate::plans::approve(conn, plan, n, trace)?;
            ("approved", decided.starts, None)
        }
        (true, None) if !questions.is_empty() => {
            let answers = picks(conn, chat, &target, questions)?;
            let sent = person::answer(conn, &answers, trace)?;
            if let Some((n, why)) = sent.refused.first() {
                return Err(refused(format!("#{n}: {why}")));
            }
            ("answered", sent.starts, None)
        }
        (true, None) => match deliverable(conn, chat, &target)? {
            Some(agent) => {
                let time = crate::ids::utc(target.ts);
                let line = line(&target.id, &time, &target.body);
                let liked = target.id.clone();
                (
                    "delivered",
                    Vec::new(),
                    Some(Delivery { agent, liked, line }),
                )
            }
            None => ("marked", Vec::new(), None),
        },
    };
    let reaction = mark(conn, chat, &target.id, did, trace)?;
    Ok(Given {
        reaction,
        did,
        starts,
        deliver,
    })
}

/// Takes the person's like off `message`: the mark goes, and nothing it did is undone. The message it
/// was on, when one stood.
pub fn take_back(
    conn: &Connection,
    chat: &str,
    message: &str,
    trace: &str,
) -> Result<Option<String>, Error> {
    let target = target(conn, chat, message)?;
    let removed = conn.execute(
        "DELETE FROM reactions WHERE message_id = ?1 AND agent_id = ?2",
        params![target.id, PERSON],
    )?;
    if removed > 0 {
        Event::point("gathr.message.reaction.remove")
            .trace(trace)
            .chat(chat)
            .ok()
            .attr("message", target.id.as_str())
            .attr("agent", PERSON)
            .insert(conn)?;
    }
    Ok((removed > 0).then_some(target.id))
}

/// The message in this chat, as a like reads it, or why it takes none.
fn target(conn: &Connection, chat: &str, message: &str) -> Result<Target, Error> {
    let found = conn
        .query_row(
            "SELECT m.id, m.turn_id, t.agent_id, m.role, m.kind, m.body, m.ts FROM messages c
             JOIN messages m ON m.id = COALESCE(c.copy_of, c.id)
             LEFT JOIN turns t ON t.id = m.turn_id
             WHERE c.id = ?1 AND c.chat_id = ?2",
            [message, chat],
            |r| {
                Ok(Target {
                    id: r.get(0)?,
                    turn: r.get(1)?,
                    author: r.get(2)?,
                    role: r.get(3)?,
                    kind: r.get(4)?,
                    body: r.get(5)?,
                    ts: r.get(6)?,
                })
            },
        )
        .optional()?;
    let target =
        found.ok_or_else(|| refused(format!("There's no message {message} in this chat.")))?;
    match target.role.as_str() {
        "assistant" => Ok(target),
        "user" => Err(refused(format!(
            "{message} is your own message: a like goes on an agent's words."
        ))),
        _ => Err(refused(format!(
            "{message} is Gathr's line: a like goes on an agent's words."
        ))),
    }
}

/// The person's like already on the message, as history shows it.
fn standing(conn: &Connection, message: &str) -> Result<Option<Value>, Error> {
    let sql = "SELECT created_ts FROM reactions WHERE message_id = ?1 AND agent_id = ?2";
    let given: Option<i64> = conn
        .query_row(sql, [message, PERSON], |r| r.get(0))
        .optional()?;
    Ok(given.map(|ts| reaction(message, ts)))
}

fn reaction(message: &str, ts: i64) -> Value {
    json!({ "message": message, "agent": PERSON, "time": crate::ids::utc(ts) })
}

/// Whether the person liked the message before: its act is done, whatever came of it.
fn ever_given(conn: &Connection, chat: &str, message: &str) -> Result<bool, Error> {
    let sql = "SELECT EXISTS (SELECT 1 FROM events WHERE name = 'gathr.message.reaction'
               AND conversation_id = ?1 AND attrs ->> '$.agent' = ?2 AND attrs ->> '$.message' = ?3)";
    Ok(conn.query_row(sql, [chat, PERSON, message], |r| r.get(0))?)
}

/// The plan and version number the message is, if it is a version of a plan.
fn version_of(conn: &Connection, message: &str) -> Result<Option<(i64, i64)>, Error> {
    let sql = "SELECT plan_id, n FROM plan_versions WHERE message_id = ?1";
    Ok(conn
        .query_row(sql, [message], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?)
}

/// Each question answered with its asker's pick, or why the like can't answer it. Checked before
/// any is answered, so a refused like answers none.
fn picks(
    conn: &Connection,
    chat: &str,
    target: &Target,
    questions: &[i64],
) -> Result<Vec<Answer>, Error> {
    let mut answers = Vec::new();
    for &n in questions {
        let t = crate::tasks::get(conn, n)?.filter(|t| t.chat == chat && t.kind == Kind::Person);
        let t = t.ok_or_else(|| refused(format!("There's no question #{n} in this chat.")))?;
        if t.given_in != target.turn || t.giver.is_none() || t.giver != target.author {
            return Err(refused(format!(
                "Question #{n} wasn't asked in this message."
            )));
        }
        if t.state != State::Ready {
            return Err(refused(format!(
                "Question #{n} is already {}.",
                t.state.as_str()
            )));
        }
        let pick = t.choices.first().filter(|_| t.form == Some(Form::Choice));
        let pick = pick.ok_or_else(|| {
            refused(format!(
                "Question #{n} has no recommended choice: answer it in the question."
            ))
        })?;
        answers.push(Answer {
            task: n,
            pick: vec![pick.label.clone()],
            ..Answer::default()
        });
    }
    Ok(answers)
}

/// The agent the like reaches: the author, while it is one of the chat's agents.
fn deliverable(conn: &Connection, chat: &str, target: &Target) -> Result<Option<String>, Error> {
    let Some(author) = &target.author else {
        return Ok(None);
    };
    let sql = "SELECT EXISTS (SELECT 1 FROM chat_members WHERE chat_id = ?1 AND agent_id = ?2)";
    let member: bool = conn.query_row(sql, [chat, author], |r| r.get(0))?;
    let gone = crate::agents::is_removed(conn, author)?;
    Ok((member && !gone).then(|| author.clone()))
}

/// Puts the person's like on the message, with what it did.
fn mark(
    conn: &Connection,
    chat: &str,
    message: &str,
    did: &str,
    trace: &str,
) -> Result<Value, Error> {
    let ts = crate::ids::now_us();
    conn.execute(
        "INSERT INTO reactions (message_id, agent_id, chat_id, created_ts) VALUES (?1, ?2, ?3, ?4)",
        params![message, PERSON, chat, ts],
    )?;
    Event::point("gathr.message.reaction")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("message", message)
        .attr("agent", PERSON)
        .attr("did", did)
        .insert(conn)?;
    Ok(reaction(message, ts))
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}
