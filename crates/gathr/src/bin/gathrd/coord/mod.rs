//! Coordination in the service (0010): what a turn's start and end do to its task and to the waits
//! on its agent, and the loop that looks at waits on CI, PRs and checks, and at tasks' PRs.

use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use gathr::tasks::{self, Moved, Start};
use serde_json::{Value, json};
use tokio::sync::broadcast;

use crate::chats::Chats;

mod api;
mod looks;

pub use api::{Change as SettingsChange, change_settings_as, present, routes, settings_shown};
pub use looks::fresh;

/// How often waits are looked at, unless `GATHR_WAIT_EVERY_MS` says (dev and tests only).
const EVERY: Duration = Duration::from_secs(120);

/// Saves a turn's status with what it moves (its task, its giver's hear-back, waits on its agent),
/// in one transaction, and starts the turns that calls for.
pub async fn turn_moved(chats: &Arc<Chats>, m: Moved<'_>, status: &str, why: Option<&str>) {
    let (chat, turn, agent, task, trace) = (
        m.chat.to_string(),
        m.turn.to_string(),
        m.agent.map(String::from),
        m.task,
        m.trace.to_string(),
    );
    let (status, why) = (status.to_string(), why.map(String::from));
    let work = move |conn: &mut rusqlite::Connection| {
        let m = Moved {
            chat: &chat,
            turn: &turn,
            agent: agent.as_deref(),
            task,
            trace: &trace,
        };
        tasks::turn_moved(conn, &m, &status, why.as_deref())
    };
    apply(chats, "gathr.turn.status", m.trace, work).await;
}

/// A chat's tasks and waits, as `GET /api/chats/<id>/tasks` answers and the live socket sends.
pub fn listed(conn: &rusqlite::Connection, chat: &str) -> Result<Value, gathr::Error> {
    let tasks = tasks::list(conn, chat)?;
    let waits = gathr::waits::in_chat(conn, chat, None)?;
    Ok(json!({ "tasks": tasks, "waits": waits }))
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
/// the window's Summary (F8b), then the person's list across chats for the Tasks badge (F9).
pub fn live_tasks(
    live: broadcast::Sender<Value>,
    states: Arc<crate::chat_state::States>,
) -> impl FnMut(&mut rusqlite::Connection) + Send + 'static {
    move |conn| {
        let sent = tasks::touched::take(conn).and_then(|chats| {
            // A question asked or answered changes whether its chat needs the person (0040).
            for chat in &chats {
                states.changed(conn, chat)?;
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

/// At start, before the turns left waiting are taken over: tasks cut off by the stop are stopped.
pub async fn recover(chats: &Arc<Chats>) {
    let recovered = chats.writer.call(tasks::recover).await;
    let event = Event::point("gathr.task.recover");
    match recovered {
        Some(Ok(n)) => chats
            .writer
            .record(event.ok().attr("gathr.task.stopped", n)),
        Some(Err(e)) => chats
            .writer
            .record(event.error("sqlite").attr("why", e.to_string())),
        None => {}
    }
}

/// Asks GitHub through `gh` in `dir`, with the person's environment now.
pub async fn github(args: &[&str], dir: &std::path::Path) -> Result<String, String> {
    looks::gh(args, dir, &looks::fresh().await).await
}

/// Looks at every open wait now, then every two minutes, for as long as the service runs.
pub async fn run(chats: Arc<Chats>) {
    let every = std::env::var("GATHR_WAIT_EVERY_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map_or(EVERY, Duration::from_millis);
    let mut tick = tokio::time::interval(every);
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
