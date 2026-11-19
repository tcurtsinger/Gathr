//! `@Name` in an agent's words wakes that member when its turn ends (0081), as a `message` does: into
//! its running turn in the chat, joined to its turn queued there, or a turn of its own answering the
//! sender's, which a pause holds. Each counts as a hand-off; past the limit the chat pauses.

use std::sync::Arc;

use gathr::event::Event;
use serde_json::json;
use tokio::sync::oneshot;

use crate::chats::{Chats, Cmd};

/// A turn that ended: its chat, its id and the trace its reference finds.
pub struct Ended {
    pub chat: String,
    pub turn: String,
    pub trace: String,
}

/// Wakes each member the ended turn's words name, but those in `skip`: whoever the caller hands the
/// words to itself. Apart from the agent's own task, which calls it: delivery waits on the member's
/// task, which may be waiting on this one.
pub fn wake(chats: Arc<Chats>, ended: Ended, skip: Vec<String>) {
    tokio::spawn(async move { woken(&chats, &ended, skip).await });
}

async fn woken(chats: &Arc<Chats>, ended: &Ended, skip: Vec<String>) {
    let (turn, trace) = (ended.turn.clone(), ended.trace.clone());
    let found = chats
        .writer
        .call(move |c| {
            let tx = c.transaction()?;
            let found = gathr::handoffs::mentions(&tx, &turn, &skip, &trace)?;
            tx.commit()?;
            Ok::<_, gathr::Error>(found)
        })
        .await;
    let event = || {
        Event::point("gathr.message.mention")
            .trace(&ended.trace)
            .chat(&ended.chat)
            .attr("gathr.turn.id", ended.turn.clone())
    };
    let mentions = match found {
        Some(Ok(mentions)) => mentions,
        failed => {
            let why = match failed {
                Some(Err(e)) => e.to_string(),
                _ => "the database didn't answer".to_string(),
            };
            return chats
                .writer
                .record(event().error("sqlite").attr("why", why));
        }
    };
    let from = mentions.from.clone();
    for named in mentions.named {
        let told = named.told;
        let delivered = deliver(
            chats,
            (&ended.chat, &named.to),
            told,
            &ended.turn,
            &named.message,
        )
        .await;
        let event = event().ok().attr("from", from.clone()).attr("to", named.to);
        let event = event
            .attr("message", named.message)
            .attr("depth", named.depth);
        chats.writer.record(event.attr("delivered", delivered));
    }
    let Some(over) = mentions.over else {
        return;
    };
    chats.publish(json!({ "type": "message", "chat": ended.chat, "message": over.line }));
    let event = event().ok().attr("from", from).attr("to", over.to);
    let event = event
        .attr("message", over.message)
        .attr("depth", over.depth);
    chats.writer.record(event.attr("delivered", "paused"));
    let limit = Event::point("gathr.handoff.limit")
        .trace(&ended.trace)
        .chat(&ended.chat)
        .ok();
    chats
        .writer
        .record(limit.attr("limit", over.limit).attr("depth", over.depth));
    chats.pauses_changed().await;
}

/// Hands `ask` to the member's own task in the chat, as Gathr's copy of the message `copy_of`, on a
/// turn answering `answers` if it starts one: where it went (`in_turn`, `joined`, `queued`, a hold, or
/// `failed`).
pub async fn deliver(
    chats: &Arc<Chats>,
    (chat, to): (&str, &str),
    ask: String,
    answers: &str,
    copy_of: &str,
) -> &'static str {
    let (reply, answer) = oneshot::channel();
    let command = Cmd::Message {
        chat: chat.to_string(),
        ask,
        answers: answers.to_string(),
        copy_of: copy_of.to_string(),
        reply,
    };
    let _ = chats.task(chat, Some(to)).send(command);
    answer.await.unwrap_or("failed")
}
