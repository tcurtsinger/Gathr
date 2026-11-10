//! Keeps Gathr's copy of Claude Code current: at start, every 24 hours, and when asked
//! (docs/decisions/0025-engine-copies.md).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gathr::claude::copies::{self, Copies, Updated};
use gathr::data::DataDir;
use gathr::event::Event;
use gathr::ids;
use gathr::writer::Writer;
use serde_json::{Value, json};
use tokio::sync::Notify;

const DAY: Duration = Duration::from_secs(24 * 60 * 60);

pub struct Updates {
    engine: &'static str,
    data: DataDir,
    writer: Writer,
    shown: Mutex<Shown>,
    wake: Notify,
    stop: AtomicBool,
}

#[derive(Default)]
struct Shown {
    checking: bool,
    checked_us: Option<i64>,
    /// The last check's failure: why, and its reference.
    failed: Option<(String, String)>,
}

impl Updates {
    pub fn new(data: DataDir, writer: Writer) -> Updates {
        Self::for_engine(data, writer, "claude")
    }

    pub fn for_engine(data: DataDir, writer: Writer, engine: &'static str) -> Updates {
        Updates {
            engine,
            data,
            writer,
            shown: Mutex::default(),
            wake: Notify::new(),
            stop: AtomicBool::new(false),
        }
    }

    fn feed(&self) -> Option<String> {
        if self.engine == "codex" {
            gathr::codex::copies::feed()
        } else {
            copies::feed()
        }
    }

    fn copies(&self) -> Copies {
        if self.engine == "codex" {
            gathr::codex::copies::storage(&self.data)
        } else {
            Copies::new(&self.data)
        }
    }

    fn shown(&self) -> std::sync::MutexGuard<'_, Shown> {
        self.shown
            .lock()
            .expect("the updates lock is never poisoned")
    }

    /// The copy in use and the last check, for Settings.
    pub fn view(&self) -> Value {
        let current = self.copies().current();
        let shown = self.shown();
        let state = match (self.feed(), shown.checking) {
            (None, _) => "off",
            (_, true) => "checking",
            _ => "idle",
        };
        json!({
            "version": current.as_ref().map(|c| &c.version),
            "previous": current.as_ref().and_then(|c| c.previous.as_ref()),
            "state": state,
            "checked": shown.checked_us.map(ids::utc),
            "failed": shown.failed.as_ref().map(|(why, r)| json!({ "why": why, "ref": r })),
        })
    }

    /// Starts a check now; it shows as checking from this moment.
    pub fn check_now(&self) -> bool {
        if self.feed().is_none() {
            return false;
        }
        self.shown().checking = true;
        self.wake.notify_one();
        true
    }

    /// Ends a download under way, at shutdown.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Checks at start, then every 24 hours or when asked, until the service stops.
pub async fn run(updates: std::sync::Arc<Updates>, chats: std::sync::Arc<crate::chats::Chats>) {
    let Some(feed) = updates.feed() else {
        return;
    };
    loop {
        updates.shown().checking = true;
        let check = updates.clone();
        let feed = feed.clone();
        let _ = tokio::task::spawn_blocking(move || check.check(&feed)).await;
        chats.retry_waiting();
        tokio::select! {
            _ = tokio::time::sleep(DAY) => {}
            _ = updates.wake.notified() => {}
        }
    }
}

impl Updates {
    /// One check, recorded as one `gathr.engine.update` event.
    fn check(&self, feed: &str) {
        let (trace, began) = (ids::trace_id(), Instant::now());
        let result = if self.engine == "codex" {
            gathr::codex::copies::update(&self.data, feed, &self.stop)
        } else {
            self.copies().update(feed, &self.stop)
        };
        let mut event = Event::point("gathr.engine.update")
            .trace(&trace)
            .attr("engine", self.engine);
        event.duration_us = Some(began.elapsed().as_micros() as i64);
        let failed = match result {
            Ok(Updated::UpToDate(version)) => {
                event = event.ok().attr("version", version).attr("installed", false);
                None
            }
            Ok(Updated::Installed { version, from }) => {
                event = event
                    .ok()
                    .attr("version", version)
                    .attr("from", from)
                    .attr("installed", true);
                None
            }
            Err(failure) => {
                event = event.error(failure.kind).attr("why", failure.why.clone());
                Some((failure.why, ids::reference(&trace).to_string()))
            }
        };
        self.writer.record(event);
        let mut shown = self.shown();
        shown.checking = false;
        shown.checked_us = Some(ids::now_us());
        shown.failed = failed;
    }
}
