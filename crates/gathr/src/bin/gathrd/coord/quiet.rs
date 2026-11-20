//! The service's part in noticing a chat where nothing moves (0052): it runs the rule
//! (`gathr::quiet`) for a chat after every write that may have moved it and at every look, with
//! what only it knows of the chat, and starts the turns the rule calls for.

use std::collections::BTreeSet;
use std::sync::Arc;

use gathr::event::Event;
use gathr::ids::{now_us, trace_id};
use gathr::quiet::{self, Times};
use gathr::tasks::Start;
use tokio::sync::mpsc;

use crate::chats::Chats;

/// After how much stuck time the person is told, unless `GATHR_QUIET_TELL_MS` says (dev and tests
/// only).
const TELL_AFTER_US: i64 = 300_000_000;

fn times() -> Times {
    let ms = std::env::var("GATHR_QUIET_TELL_MS").ok();
    let ms = ms.and_then(|ms| ms.parse::<i64>().ok());
    Times {
        tell_after_us: ms.map_or(TELL_AFTER_US, |ms| ms * 1000),
        look_us: super::every().as_micros() as i64,
    }
}

/// How long a version's round has before the version reaches the person anyway, unless
/// `GATHR_PLAN_CHECK_MS` says (dev and tests only).
const PLAN_CHECK_US: i64 = 600_000_000;

fn plan_check_us() -> i64 {
    let ms = std::env::var("GATHR_PLAN_CHECK_MS").ok();
    let ms = ms.and_then(|ms| ms.parse::<i64>().ok());
    ms.map_or(PLAN_CHECK_US, |ms| ms * 1000)
}

/// Looks at one chat now. A room chat has none of this: its check runs from each turn's end
/// (`mentions.rs`).
pub async fn check(chats: &Arc<Chats>, chat: &str) {
    let trace = trace_id();
    if room(chats, chat).await {
        return crate::chat_state::changed(chats, chat, &trace).await;
    }
    let live = super::live_now(chats, chat);
    let id = chat.to_string();
    let t = trace.clone();
    let checked = chats
        .writer
        .call(move |conn| quiet::check(conn, &id, &live, now_us(), times(), &t));
    started(chats, checked.await, ("gathr.quiet.check", chat, &trace));
    // The lead's answer is checked in the open once the chat is quiet (0080).
    let (id, t) = (chat.to_string(), trace.clone());
    let round = chats
        .writer
        .call(move |conn| gathr::open_check::settle(conn, &id, &t));
    started(chats, round.await, ("gathr.check.round", chat, &trace));
    // After what the write committed, the giver's hear-back included: a repair's "Fixed" (0061).
    let (id, t) = (chat.to_string(), trace.clone());
    let fixed = chats
        .writer
        .call(move |conn| gathr::asks::fixed(conn, &id, &t));
    if let Some(Err(e)) = fixed.await {
        let event = Event::point("gathr.ask.fixed")
            .trace(&trace)
            .chat(chat)
            .error("sqlite");
        chats.writer.record(event.attr("why", e.to_string()));
    }
    // A version whose check runs late reaches the person (0062).
    let (id, t) = (chat.to_string(), trace.clone());
    let late = chats
        .writer
        .call(move |conn| gathr::plans::overdue(conn, &id, now_us(), plan_check_us(), &t));
    if let Some(Err(e)) = late.await {
        let event = Event::point("gathr.plan.checked")
            .trace(&trace)
            .chat(chat)
            .error("sqlite");
        chats.writer.record(event.attr("why", e.to_string()));
    }
    // Told, or no longer stuck, the chat needs the person or stops needing them (0040).
    crate::chat_state::changed(chats, chat, &trace).await;
}

/// Looks at every chat that owes work or is in a stuck stretch: once at each of the service's looks.
pub async fn look(chats: &Arc<Chats>) {
    match chats.writer.call(|conn| quiet::chats(conn)).await {
        Some(Ok(owing)) => {
            for chat in owing {
                check(chats, &chat).await;
            }
        }
        Some(Err(e)) => {
            let event = Event::point("gathr.quiet.check").trace(&trace_id());
            let event = event.error("sqlite").attr("why", e.to_string());
            chats.writer.record(event);
        }
        None => {}
    }
}

/// Looks at each chat a write may have moved, as the writer names them.
pub async fn run(chats: Arc<Chats>, mut changed: mpsc::UnboundedReceiver<String>) {
    while let Some(chat) = changed.recv().await {
        // One write can name a chat many times over, and a turn's end names it again.
        let mut named = BTreeSet::from([chat]);
        while let Ok(chat) = changed.try_recv() {
            named.insert(chat);
        }
        for chat in named {
            check(&chats, &chat).await;
        }
    }
}

/// A turn of `agent`'s has made no progress for `quiet_ms`: its chat's lead is told, on the turn's
/// own trace, so its reference finds the telling.
pub async fn silent(
    chats: Arc<Chats>,
    chat: String,
    agent: String,
    task: Option<i64>,
    quiet_ms: i64,
    trace: String,
) {
    if room(&chats, &chat).await {
        return;
    }
    let (id, t) = (chat.clone(), trace.clone());
    let told = chats
        .writer
        .call(move |conn| quiet::silent_turn(conn, &id, &agent, task, quiet_ms, &t));
    started(
        &chats,
        told.await,
        ("gathr.turn.silent_told", &chat, &trace),
    );
}

/// Whether `chat` is a room chat; one that can't be read is taken for an old one.
async fn room(chats: &Chats, chat: &str) -> bool {
    let id = chat.to_string();
    let read = chats.writer.call(move |c| gathr::room::is_room(c, &id));
    matches!(read.await, Some(Ok(true)))
}

/// Queues the turns a change called for; a failure is recorded with its chat.
pub(crate) fn started(
    chats: &Arc<Chats>,
    changed: Option<Result<Vec<Start>, gathr::Error>>,
    (name, chat, trace): (&str, &str, &str),
) {
    match changed {
        Some(Ok(starts)) => chats.start(starts),
        Some(Err(e)) => {
            let event = Event::point(name).trace(trace).chat(chat).error("sqlite");
            chats.writer.record(event.attr("why", e.to_string()));
        }
        None => {}
    }
}
