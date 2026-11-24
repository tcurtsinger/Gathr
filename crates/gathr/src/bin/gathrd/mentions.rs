//! `@Name` in an agent's words wakes that member when its turn ends (0081): into its running turn in
//! the chat, joined to its turn queued there, or a turn of its own answering the sender's, which a
//! pause holds. Each counts as a hand-off; past the limit the chat pauses. Then the team's check, and
//! once the chat is quiet, the hand-back to a member whose board line still says doing.

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

/// Wakes each member the ended turn's words name, when it ended `done`. Apart from the agent's own
/// task, which calls it: delivery waits on the member's task, which may be waiting on this one. The
/// lead's answer is then checked (0080), once whoever was woken has its turn queued; and with nothing
/// queued or running, each member whose board line still says doing gets its turn back (`nudge`).
pub fn wake(chats: Arc<Chats>, ended: Ended, done: bool) {
    tokio::spawn(async move {
        if done {
            woken(&chats, &ended).await;
        }
        let (chat, trace) = (ended.chat.clone(), ended.trace.clone());
        let round = chats
            .writer
            .call(move |c| gathr::open_check::settle(c, &chat, &trace));
        match round.await {
            Some(Ok(starts)) => chats.start(starts),
            Some(Err(e)) => {
                let event = Event::point("gathr.check.round")
                    .trace(&ended.trace)
                    .chat(&ended.chat)
                    .error("sqlite");
                chats.writer.record(event.attr("why", e.to_string()));
            }
            None => {}
        }
        hand_back(&chats, &ended).await;
    });
}

async fn hand_back(chats: &Arc<Chats>, ended: &Ended) {
    let asks = chats.states.asks(&ended.chat);
    let (chat, trace) = (ended.chat.clone(), ended.trace.clone());
    let handed = chats
        .writer
        .call(move |c| gathr::nudge::hand_back(c, &chat, asks, &trace));
    match handed.await {
        Some(Ok(starts)) => chats.start(starts),
        Some(Err(e)) => {
            let event = Event::point("gathr.turn.nudge")
                .trace(&ended.trace)
                .chat(&ended.chat)
                .error("sqlite");
            chats.writer.record(event.attr("why", e.to_string()));
        }
        None => {}
    }
}

async fn woken(chats: &Arc<Chats>, ended: &Ended) {
    let (turn, trace) = (ended.turn.clone(), ended.trace.clone());
    let found = chats
        .writer
        .call(move |c| {
            let tx = c.transaction()?;
            let found = gathr::handoffs::mentions(&tx, &turn, &trace)?;
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
    if let Some(members) = &mentions.joined {
        let seats: Vec<_> = members
            .iter()
            .map(|m| json!({ "agent": m.agent, "role": m.role }))
            .collect();
        let joined = Event::point("gathr.chat.members")
            .trace(&ended.trace)
            .chat(&ended.chat)
            .ok();
        chats.writer.record(joined.attr("members", seats));
        chats.publish(json!({ "type": "members", "chat": ended.chat, "members": members }));
    }
    crate::team::stayed(chats, &ended.chat, mentions.stays);
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
