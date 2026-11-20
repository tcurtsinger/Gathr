//! Coordination in the service (0010): what a turn's start and end do to its task and to the waits
//! on its agent, and the loop that looks at waits on CI, PRs and checks, and at tasks' PRs.

use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use gathr::tasks::{self, Start};
use serde_json::{Value, json};
use tokio::sync::{broadcast, mpsc};

use crate::chats::Chats;

mod api;
mod looks;
mod plans;
pub mod quiet;

pub fn routes() -> axum::Router<Arc<crate::api::State>> {
    api::routes().merge(plans::routes())
}

/// How often waits are looked at, unless `GATHR_WAIT_EVERY_MS` says (dev and tests only).
const EVERY: Duration = Duration::from_secs(120);

/// How far apart the service's looks are.
fn every() -> Duration {
    let ms = std::env::var("GATHR_WAIT_EVERY_MS").ok();
    let ms = ms.and_then(|ms| ms.parse().ok());
    ms.map_or(EVERY, Duration::from_millis)
}

/// A chat's tasks, waits, asks and plans, as `GET /api/chats/<id>/tasks` answers and the live
/// socket sends.
pub fn listed(conn: &rusqlite::Connection, chat: &str) -> Result<Value, gathr::Error> {
    let tasks = tasks::list(conn, chat)?;
    let waits = gathr::waits::in_chat(conn, chat, None)?;
    let asks = gathr::asks::in_chat(conn, chat)?;
    let plans = gathr::plans::in_chat(conn, chat)?;
    let caught = gathr::hold::look::caught(conn, chat)?;
    Ok(json!({ "tasks": tasks, "waits": waits, "asks": asks, "plans": plans, "caught": caught }))
}

/// What only the running service knows of what the chat waits on (0050): why its queued turns
/// wait, as their agents' queues say, the turns with an approval waiting, those that have gone
/// silent, Gathr's own update, and the PRs it lands whose looks fail.
pub fn live_now(chats: &Chats, chat: &str) -> gathr::waiting::Live {
    let said = chats.queued_in(chat);
    let behind = said
        .iter()
        .filter_map(|(turn, why)| Some((turn.clone(), why.behind.clone()?)));
    gathr::waiting::Live {
        running: true,
        behind: behind.collect(),
        queued: said
            .into_iter()
            .map(|(turn, why)| (turn, why.words))
            .collect(),
        approvals: chats.states.asking_in(chat),
        updating: chats.updating(),
        silent: chats.silent_in(chat).into_iter().collect(),
        landing: crate::landing::failing_in(chat),
    }
}

/// The person's open tasks, as `GET /api/person-tasks` answers and the live socket sends.
pub fn person_listed(
    conn: &rusqlite::Connection,
    chat: Option<&str>,
) -> Result<Value, gathr::Error> {
    let mut listed = json!({ "tasks": tasks::person::open(conn, chat)? });
    if chat.is_none() {
        // Across chats, the person's list also groups agents' stale tasks (F11).
        listed["stale"] = json!(tasks::stale::open(conn)?);
    }
    Ok(listed)
}

/// Runs after each write: every chat whose tasks or waits it changed goes out live, whole, for
/// the window's Summary (F8b), then the person's list across chats for the Tasks badge (F9). Each
/// chat the write may have moved is named on `quiet`, to be looked at for being stuck (0052).
pub fn live_tasks(
    live: broadcast::Sender<Value>,
    states: Arc<crate::chat_state::States>,
    quiet: mpsc::UnboundedSender<String>,
) -> impl FnMut(&mut rusqlite::Connection) + Send + 'static {
    move |conn| {
        let sent = tasks::touched::take(conn).and_then(|chats| {
            // A question asked or answered changes whether its chat needs the person (0040). A
            // chat whose state can't be read is recorded on its own; the rest still goes out.
            for chat in &chats {
                if let Err(e) = states.changed(conn, chat) {
                    let event = Event::point("gathr.chat.state").trace(&gathr::ids::trace_id());
                    let event = event.chat(chat).error("sqlite");
                    let _ = event.attr("why", e.to_string()).insert(conn);
                }
            }
            // A task that ended done is a line in its chat, said in no turn (0051).
            for (chat, message) in tasks::touched::take_said(conn)? {
                let _ = live.send(json!({ "type": "message", "chat": chat, "message": message }));
            }
            // What these chats wait on may have changed, and that of chats whose turns, looks or
            // background commands did: a window showing one asks again (0050).
            let mut waiting = tasks::touched::take_waiting(conn)?;
            waiting.retain(|chat| !chats.contains(chat));
            for chat in chats.iter().chain(&waiting) {
                let _ = live.send(json!({ "type": "waiting", "chat": chat }));
                let _ = quiet.send(chat.clone());
            }
            if chats.is_empty() || live.receiver_count() == 0 {
                return Ok(());
            }
            for chat in &chats {
                let mut update = listed(conn, chat)?;
                update["type"] = json!("tasks");
                update["chat"] = json!(chat);
                let _ = live.send(update);
            }
            let mut update = person_listed(conn, None)?;
            update["type"] = json!("person_tasks");
            let _ = live.send(update);
            Ok(())
        });
        if let Err(e) = sent {
            let event = Event::point("gathr.task.live").error("sqlite");
            let _ = event.attr("why", e.to_string()).insert(conn);
        }
    }
}

/// At start, before the turns left waiting are taken over: turns cut off by the stop carry on. When
/// that fails, they stay running for the next start, and the error has a reference.
pub async fn recover(chats: &Arc<Chats>) {
    let recovered = chats.writer.call(tasks::recover).await;
    let event = Event::point("gathr.task.recover");
    match recovered {
        Some(Ok(n)) => chats
            .writer
            .record(event.ok().attr("gathr.task.stopped", n)),
        Some(Err(e)) => chats.writer.record(
            event
                .trace(&gathr::ids::trace_id())
                .error("sqlite")
                .attr("why", e.to_string()),
        ),
        None => {}
    }
}

/// Looks at every open wait now, then every two minutes, for as long as the service runs.
pub async fn run(chats: Arc<Chats>) {
    let mut tick = tokio::time::interval(every());
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        looks::once(&chats).await;
    }
}

/// Runs a change on the database and queues the turns it calls for; a failure is recorded.
async fn apply(
    chats: &Arc<Chats>,
    name: &str,
    trace: &str,
    work: impl FnOnce(&mut rusqlite::Connection) -> Result<Vec<Start>, gathr::Error> + Send + 'static,
) {
    match chats.writer.call(work).await {
        Some(Ok(starts)) => chats.start(starts),
        Some(Err(e)) => {
            let event = Event::point(name).trace(trace).error("sqlite");
            chats.writer.record(event.attr("why", e.to_string()));
        }
        None => {}
    }
}
