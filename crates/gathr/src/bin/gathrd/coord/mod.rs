//! Coordination in the service (0010): what a turn's start and end do to its task and to the waits
//! on its agent, and the loop that looks at waits on CI, PRs and checks, and at tasks' PRs.

use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use gathr::tasks::{self, Outcome, Start};
use gathr::waits;

use crate::chats::Chats;

mod api;
mod looks;

pub use api::routes;

/// How often waits are looked at, unless `GATHR_WAIT_EVERY_MS` says (dev and tests only).
const EVERY: Duration = Duration::from_secs(120);

/// A turn whose status changed.
pub struct Moved<'a> {
    pub chat: &'a str,
    pub turn: &'a str,
    pub agent: Option<&'a str>,
    pub task: Option<i64>,
    pub trace: &'a str,
}

/// A turn started or ended: its task moves, waits on its agent may fire, and what follows is
/// queued.
pub async fn turn_moved(chats: &Arc<Chats>, m: Moved<'_>, status: &str, why: Option<&str>) {
    let ended = matches!(status, "done" | "failed" | "stopped");
    if status != "running" && !ended {
        return;
    }
    let (chat, turn, agent, task, trace) = (
        m.chat.to_string(),
        m.turn.to_string(),
        m.agent.map(String::from),
        m.task,
        m.trace.to_string(),
    );
    let (status, why) = (status.to_string(), why.unwrap_or(status).to_string());
    let work = move |conn: &mut rusqlite::Connection| {
        let mut starts = Vec::new();
        if let Some(task) = task {
            match status.as_str() {
                "running" => tasks::started(conn, task, &trace)?,
                "done" => {
                    let tools: i64 = conn.query_row(
                        "SELECT count(*) FROM messages WHERE turn_id = ?1 AND kind = 'tool'",
                        [&turn],
                        |r| r.get(0),
                    )?;
                    starts.extend(tasks::ended(conn, task, Outcome::Ok { tools }, &trace)?);
                }
                "failed" => starts.extend(tasks::ended(conn, task, Outcome::Failed(why), &trace)?),
                _ => starts.extend(tasks::ended(conn, task, Outcome::Stopped(why), &trace)?),
            }
        }
        if let (true, Some(agent)) = (status != "running", agent) {
            starts.extend(waits::agent_free(conn, &chat, &agent, &trace)?);
        }
        Ok(starts)
    };
    apply(chats, "gathr.task.turn", m.trace, work).await;
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
