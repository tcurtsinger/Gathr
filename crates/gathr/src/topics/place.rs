//! An agent's choice of where its turn's words show, with `post` (0079): checked, kept on the turn
//! (`turns.placed`) with an event, and overridden by what needs the person: a plan or a question.

use rusqlite::{Connection, OptionalExtension};

use crate::Error;
use crate::event::Event;
use crate::replies::{self, Target};

/// What a choice did: the answer to the agent, and the turn's topic now.
pub struct Chosen {
    pub said: String,
    pub topic: Option<String>,
}

fn refused(why: impl Into<String>) -> Error {
    Error::Refused(why.into())
}

/// Puts `turn`'s words where `to` says: `room`, `person`, `#<task>` or a message's id.
pub fn choose(
    conn: &Connection,
    chat: &str,
    turn: &str,
    to: &str,
    trace: &str,
) -> Result<Chosen, Error> {
    crate::archive::refuse_archived(conn, chat)?;
    let sql = "SELECT status, result_of FROM turns WHERE id = ?1 AND chat_id = ?2";
    let found: Option<(String, Option<String>)> = conn
        .query_row(sql, [turn, chat], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    // A turn is queued until its engine reads it, but its tools already answer as its own.
    let result_of = match found {
        Some((status, result_of)) if status == "queued" || status == "running" => result_of,
        _ => return Err(refused("you have no turn running in this chat")),
    };
    if crate::team::members(conn, chat)?.len() < 2 {
        return Err(refused("post is for a team chat: this one has one agent."));
    }
    if result_of.is_some() {
        return Err(refused(
            "This turn is the result of the ask: it stays in the room.",
        ));
    }
    let (kind, placed) = match to.trim() {
        "room" => ("room", "main".to_string()),
        "person" => {
            let person = crate::reactions::persons(conn, turn)?;
            ("person", thread(conn, chat, turn, Some(person), None)?)
        }
        named => match named.strip_prefix('#').and_then(|n| n.parse().ok()) {
            Some(n) => ("task", thread(conn, chat, turn, None, Some(n))?),
            None => (
                "message",
                thread(conn, chat, turn, Some(named.into()), None)?,
            ),
        },
    };
    conn.execute(
        "UPDATE turns SET placed = ?2 WHERE id = ?1",
        [turn, &placed],
    )?;
    Event::point("gathr.turn.placed")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("turn", turn)
        .attr("to", kind)
        .attr("placed", placed.clone())
        .attr("words_before", words_before(conn, turn)?)
        .attr("overridden", false)
        .insert(conn)?;
    Ok(Chosen {
        said: format!("Your words this turn show {}.", shown_in(conn, &placed)?),
        topic: super::of_turn(conn, chat, turn)?,
    })
}

/// The thread of a message or task of the chat, as `m:<message>` or `t:<task>`: never one of this
/// turn's own messages, nor for a turn that holds what needs the person.
fn thread(
    conn: &Connection,
    chat: &str,
    turn: &str,
    message: Option<String>,
    task: Option<i64>,
) -> Result<String, Error> {
    let target = Target {
        reply_to: message,
        reply_task: task,
    };
    let checked = replies::check(conn, chat, &target)?;
    if let Some(id) = &checked.reply_to {
        // The person's own message is a main message even in the turn it opened; anything else of
        // this turn sits where the turn does, so its thread would be the turn itself.
        let sql = "SELECT turn_id = ?2 AND NOT (role = 'user' AND reply_to IS NULL AND reply_task IS NULL)
                   FROM messages WHERE id = ?1";
        let own: Option<bool> = conn.query_row(sql, [id, turn], |r| r.get(0))?;
        if own == Some(true) {
            return Err(refused(format!(
                "{id} is this turn's own message: choose another, or the room."
            )));
        }
    }
    if let Some(why) = needs_room(conn, chat, turn)? {
        return Err(refused(why));
    }
    Ok(match (checked.reply_task, checked.reply_to) {
        (Some(n), _) => format!("t:{n}"),
        (None, id) => format!("m:{}", id.unwrap_or_default()),
    })
}

/// Why `turn` stays in the room whatever its agent chooses: it wrote a plan, or asked the person.
fn needs_room(conn: &Connection, chat: &str, turn: &str) -> Result<Option<&'static str>, Error> {
    let wrote = "SELECT 1 FROM plan_versions v JOIN plans p ON p.id = v.plan_id
                 WHERE p.chat_id = ?1 AND v.turn_id = ?2 LIMIT 1";
    if conn
        .query_row(wrote, [chat, turn], |_| Ok(()))
        .optional()?
        .is_some()
    {
        return Ok(Some(
            "This turn holds a plan for the person: it stays in the room.",
        ));
    }
    let asked =
        "SELECT 1 FROM tasks WHERE chat_id = ?1 AND given_in = ?2 AND kind = 'person' LIMIT 1";
    if conn
        .query_row(asked, [chat, turn], |_| Ok(()))
        .optional()?
        .is_some()
    {
        return Ok(Some(
            "This turn asks the person a question: it stays in the room.",
        ));
    }
    Ok(None)
}

/// The turn's own words saved so far: what moves with the bubble though written before the choice.
fn words_before(conn: &Connection, turn: &str) -> Result<i64, Error> {
    let sql = "SELECT count(*) FROM messages
               WHERE turn_id = ?1 AND role = 'assistant' AND kind = 'text' AND to_agent IS NULL";
    Ok(conn.query_row(sql, [turn], |r| r.get(0))?)
}

/// Where `placed` shows, in words: "in the room", "in task #12's thread", "in the thread of Ann's
/// message".
fn shown_in(conn: &Connection, placed: &str) -> Result<String, Error> {
    if let Some(n) = placed.strip_prefix("t:") {
        return Ok(format!("in task #{n}'s thread"));
    }
    let Some(id) = placed.strip_prefix("m:") else {
        return Ok("in the room".into());
    };
    let (role, name): (String, Option<String>) = conn.query_row(
        "SELECT m.role, a.name FROM messages m LEFT JOIN turns t ON t.id = m.turn_id
         LEFT JOIN agents a ON a.id = t.agent_id WHERE m.id = ?1",
        [id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    let whose = match (role.as_str(), name) {
        ("user", _) => "the person's".to_string(),
        ("gathr", _) => "Gathr's".to_string(),
        (_, Some(name)) => format!("{name}'s"),
        (_, None) => "an agent's".to_string(),
    };
    Ok(format!("in the thread of {whose} message"))
}

/// A plan written, or a question asked, in a turn whose agent chose a thread: the turn stays in the
/// room (a plan) or the question is its own main message (a question), and the event says the choice
/// was overridden by `by`. Whether one was.
pub fn overridden(
    conn: &Connection,
    chat: &str,
    turn: &str,
    trace: &str,
    by: &str,
) -> Result<bool, Error> {
    let sql = "SELECT placed FROM turns WHERE id = ?1";
    let placed: Option<String> = conn
        .query_row(sql, [turn], |r| r.get(0))
        .optional()?
        .flatten();
    let Some(placed) = placed.filter(|p| p != "main") else {
        return Ok(false);
    };
    let to = match placed.starts_with("t:") {
        true => "task",
        false => "message",
    };
    Event::point("gathr.turn.placed")
        .trace(trace)
        .chat(chat)
        .ok()
        .attr("turn", turn)
        .attr("to", to)
        .attr("placed", placed)
        .attr("words_before", words_before(conn, turn)?)
        .attr("overridden", true)
        .attr("by", by)
        .insert(conn)?;
    Ok(true)
}
