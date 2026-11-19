//! The person's words reaching every other agent they picked (0028): Gathr's copy of the message that
//! carries them, to each agent's task, one after another. A copy that can't go is said in its row of
//! the answer, with why and the reference; the others still go.

use gathr::words::Opened;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::api::{State, Trace};
use crate::chats::Cmd;

/// Gathr's copy of the message `of` to each agent in `lines`, with what it reads: each agent's row,
/// `{agent, turn, status}`, as its task answered.
pub async fn give(
    s: &State,
    t: &Trace,
    chat: &str,
    (of, opened): (&str, Opened),
    lines: Vec<(String, String)>,
) -> Vec<Value> {
    let mut rows = Vec::new();
    for (agent, line) in lines {
        let (reply, told) = oneshot::channel();
        let copy = Cmd::Copy {
            chat: chat.to_string(),
            of: of.to_string(),
            line,
            opened,
            trace: t.0.clone(),
            reply,
        };
        let _ = s.chats.task(chat, Some(&agent)).send(copy);
        let mut row = match told.await {
            Ok(Ok(turn)) => turn,
            Ok(Err(failed)) => {
                let reference = gathr::ids::reference(&failed.trace);
                json!({ "status": "failed", "why": failed.why, "ref": reference })
            }
            Err(_) => json!({ "status": "failed", "why": "the agent's task ended" }),
        };
        row["agent"] = json!(agent);
        rows.push(row);
    }
    rows
}

/// What each agent in `others` reads of the person's `message`: Gathr's line, then their words.
pub async fn lines(
    s: &State,
    message: &str,
    others: Vec<String>,
) -> Result<Vec<(String, String)>, String> {
    let message = message.to_string();
    let read = s.writer.call(move |conn| {
        let mut lines = Vec::new();
        for agent in others {
            let line = gathr::replies::copy_for(conn, &message, &agent)?;
            lines.push((agent, line));
        }
        Ok::<_, gathr::Error>(lines)
    });
    match read.await {
        Some(Ok(lines)) => Ok(lines),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

/// A Discuss with other agents picked in its words (0028): its question, its chat, and who else gets
/// the words beside the question's asker.
pub struct Discussed {
    task: i64,
    chat: String,
    asker: Option<String>,
    others: Vec<String>,
}

/// Each Discuss in `answers` with picks, routed as a message to the asker with those picks is: a pick
/// outside a project's team, or one too many joining, refuses it all before anything is saved.
pub async fn discussed(
    s: &State,
    t: &Trace,
    answers: &[gathr::tasks::person::Answer],
) -> Result<Vec<Discussed>, crate::routes::Fail> {
    let mut found = Vec::new();
    for a in answers.iter().filter(|a| a.discuss && !a.to.is_empty()) {
        let task = a.task;
        let sql = "SELECT chat_id, giver FROM tasks WHERE number = ?1";
        let read = s
            .writer
            .call(move |conn| conn.query_row(sql, [task], |r| Ok((r.get(0)?, r.get(1)?))));
        // A question that isn't there is refused with the answer, as it is without picks.
        let Some(Ok((chat, asker))) = read.await else {
            continue;
        };
        let (chat, asker): (String, Option<String>) = (chat, asker);
        let (_, others) =
            crate::routes::addressee(s, t, &chat, asker.clone(), a.to.clone()).await?;
        found.push(Discussed {
            task,
            chat,
            asker,
            others,
        });
    }
    Ok(found)
}

/// Gathr's copy of the asker's line that carries each Discuss to each other agent picked in it, read
/// as a reply about the question. A Discuss refused gives nothing.
pub async fn give_discussed(
    s: &State,
    t: &Trace,
    all: Vec<Discussed>,
    refused: &[i64],
) -> Vec<Value> {
    let mut rows = Vec::new();
    for d in all.into_iter().filter(|d| !refused.contains(&d.task)) {
        let Discussed {
            task,
            chat,
            asker,
            others,
        } = d;
        let read = s.writer.call(move |conn| {
            let sql = "SELECT m.id, t.answer FROM tasks t JOIN messages m ON m.turn_id = t.heard_in
                       WHERE t.number = ?1 AND m.role = 'gathr' ORDER BY m.ts, m.rowid LIMIT 1";
            let (line, answer): (String, Option<String>) =
                conn.query_row(sql, [task], |r| Ok((r.get(0)?, r.get(1)?)))?;
            let answer: Value = answer
                .and_then(|a| serde_json::from_str(&a).ok())
                .unwrap_or_default();
            let words = answer["discussed"]
                .as_array()
                .and_then(|d| d.last())
                .cloned();
            let words = words
                .and_then(|d| d["text"].as_str().map(String::from))
                .unwrap_or_default();
            let target = gathr::replies::Target {
                reply_to: None,
                reply_task: Some(task),
            };
            let everyone: Vec<String> = asker.into_iter().chain(others.iter().cloned()).collect();
            let mut lines = Vec::new();
            for agent in &others {
                let said = gathr::replies::said_to(conn, &target, Some(agent), &everyone)?;
                let read = said.map_or(words.clone(), |said| format!("{said}\n\n{words}"));
                lines.push((agent.clone(), read));
            }
            Ok::<_, gathr::Error>((line, lines))
        });
        match read.await {
            Some(Ok((line, lines))) => {
                rows.extend(give(s, t, &chat, (&line, Opened::Reply), lines).await)
            }
            Some(Err(e)) => {
                rows.push(json!({ "task": task, "status": "failed", "why": e.to_string() }))
            }
            None => rows.push(
                json!({ "task": task, "status": "failed", "why": "the database didn't answer" }),
            ),
        }
    }
    rows
}
