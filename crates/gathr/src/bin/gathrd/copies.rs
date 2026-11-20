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
