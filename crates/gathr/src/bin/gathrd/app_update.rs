//! Gathr updates its own service between turns (0032). It looks for a newer release at start,
//! every 10 minutes and when asked; installs it beside the copy that runs and checks it; holds
//! the turns it queued (never the person's own message) until nothing runs; then stops and hands over to it
//! (`gathr::launch::hand_over`), which goes back to the copy before if the new one doesn't start.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::app::{Copies, release};
use gathr::event::Event;
use gathr::fetch::{Failure, failed, parse_version};
use gathr::pauses::{self, Pause};
use gathr::{ids, platform};
use serde_json::{Value, json};
use tokio::sync::Notify;

use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, fail};

const EVERY: Duration = Duration::from_secs(10 * 60);
/// How long turns are held for an update before it waits for the next look instead.
const HOLD: Duration = Duration::from_secs(30 * 60);
/// How often a hold looks at what still runs.
const LOOK: Duration = Duration::from_millis(500);

pub struct AppUpdates {
    copies: Copies,
    chats: Arc<Chats>,
    /// The version this service runs as.
    pub running: String,
    shown: Mutex<Shown>,
    wake: Notify,
    stop: AtomicBool,
    /// The version to hand over to, once nothing runs.
    handing_over: Mutex<Option<String>>,
    ready: Notify,
}

#[derive(Default)]
struct Shown {
    checking: bool,
    checked_us: Option<i64>,
    /// The last failure: why, and its reference.
    failed: Option<(String, String)>,
    /// The update holding turns now, and since when.
    holding: Option<(String, i64)>,
}

enum Found {
    UpToDate,
    /// Newer, but it failed to start here before.
    Refused(String),
    Ready {
        version: String,
        installed: bool,
    },
}

impl AppUpdates {
    pub fn new(chats: Arc<Chats>) -> AppUpdates {
        let copies = Copies::new(&chats.data);
        AppUpdates {
            running: copies.running_version(),
            copies,
            chats,
            shown: Mutex::default(),
            wake: Notify::new(),
            stop: AtomicBool::new(false),
            handing_over: Mutex::default(),
            ready: Notify::new(),
        }
    }

    fn shown(&self) -> std::sync::MutexGuard<'_, Shown> {
        self.shown.lock().expect("never poisoned")
    }

    /// The version that runs, the one before it, and the last check, for Settings.
    pub fn view(&self) -> Value {
        let current = self.copies.current();
        let shown = self.shown();
        let state = match (release::feed(), shown.checking, &shown.holding) {
            (None, ..) => "off",
            (_, _, Some(_)) => "updating",
            (_, true, _) => "checking",
            _ => "idle",
        };
        json!({
            "version": self.running,
            "previous": current.and_then(|c| c.previous).filter(|p| *p != self.running),
            "state": state,
            "checked": shown.checked_us.map(ids::utc),
            "failed": shown.failed.as_ref().map(|(why, r)| json!({ "why": why, "ref": r })),
            "update": shown.holding.as_ref().map(|(v, since)| json!({ "version": v, "since_us": since })),
        })
    }

    fn publish(&self) {
        let mut view = self.view();
        view["type"] = "app".into();
        self.chats.publish(view);
    }

    /// Starts a check now; it shows as checking from this moment.
    pub fn check_now(&self) -> bool {
        if release::feed().is_none() {
            return false;
        }
        self.shown().checking = true;
        self.wake.notify_one();
        true
    }

    /// Ends a download or a hold under way, at shutdown.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }

    /// Returns once an update may go ahead: its version.
    pub async fn handover(&self) -> String {
        loop {
            if let Some(version) = self.handing_over() {
                return version;
            }
            self.ready.notified().await;
        }
    }

    pub fn handing_over(&self) -> Option<String> {
        self.handing_over.lock().expect("never poisoned").clone()
    }

    /// Records what the start found in `current.json`: an update that passed its start, or a
    /// rollback from one that didn't.
    pub fn started(&self) {
        let started = match self.copies.started() {
            Ok(started) => started,
            Err(e) => {
                let event = Event::point("gathr.app.start").error("disk");
                self.chats.writer.record(event.attr("why", e.to_string()));
                return;
            }
        };
        if let Some(from) = started.switched_from {
            let event = Event::point("gathr.app.switch").ok();
            let event = event.attr("from", from).attr("to", self.running.as_str());
            self.chats.writer.record(event);
        }
        if let Some(back) = started.rolled_back {
            let trace = ids::trace_id();
            let event = Event::point("gathr.app.update")
                .trace(&trace)
                .error("start");
            let event = event.attr("version", back.version.as_str());
            let event = event.attr("back_to", self.running.as_str());
            self.chats
                .writer
                .record(event.attr("why", back.why.as_str()));
            let why = format!("{} start failed: {}", back.version, back.why);
            self.shown().failed = Some((why, ids::reference(&trace).to_string()));
        }
    }

    /// One check, recorded as one `gathr.app.update` event: the version ready to switch to.
    fn check(&self, feed: &str) -> Option<String> {
        let (trace, began) = (ids::trace_id(), Instant::now());
        let found = self.find(feed);
        let event = Event::point("gathr.app.update")
            .trace(&trace)
            .attr("running", self.running.as_str());
        let (mut event, ready, failed) = match found {
            Ok(Found::UpToDate) => (event.ok(), None, None),
            Ok(Found::Refused(version)) => (event.ok().attr("refused", version), None, None),
            Ok(Found::Ready { version, installed }) => {
                let event = event.ok().attr("version", version.as_str());
                (event.attr("installed", installed), Some(version), None)
            }
            Err(f) => {
                let failed = (f.why.clone(), ids::reference(&trace).to_string());
                (event.error(f.kind).attr("why", f.why), None, Some(failed))
            }
        };
        event.duration_us = Some(began.elapsed().as_micros() as i64);
        self.chats.writer.record(event);
        let mut shown = self.shown();
        shown.checking = false;
        shown.checked_us = Some(ids::now_us());
        if failed.is_some() || ready.is_some() {
            shown.failed = failed;
        }
        ready
    }

    fn find(&self, feed: &str) -> Result<Found, Failure> {
        let target = platform::release_target()
            .ok_or_else(|| failed("platform")("Gathr ships no build for this computer".into()))?;
        let latest = release::latest(feed, target, &self.stop)?;
        if parse_version(&latest.version) <= parse_version(&self.running) {
            return Ok(Found::UpToDate);
        }
        let refused = self.copies.current().unwrap_or_default().refused;
        if refused.contains(&latest.version) {
            return Ok(Found::Refused(latest.version));
        }
        // Installed by an earlier check whose hold gave up.
        let installed = !self.copies.path(&latest.version).is_file();
        if installed {
            release::install(&self.copies, &latest, &self.stop)?;
        }
        Ok(Found::Ready {
            version: latest.version,
            installed,
        })
    }

    /// Holds the turns Gathr queued until nothing runs (turns, background commands, wait checks):
    /// whether it got there. After `HOLD`, or at shutdown, it lets them go and says what still ran.
    async fn hold(&self, version: &str) -> bool {
        let (trace, began) = (ids::trace_id(), Instant::now());
        self.shown().holding = Some((version.to_string(), ids::now_us()));
        self.chats.set_updating(Some(version.to_string())).await;
        self.publish();
        let limit = hold_limit();
        let event = Event::point("gathr.app.hold")
            .trace(&trace)
            .attr("version", version);
        loop {
            let running = self.running_now().await;
            let idle = |key: &str| running[key].as_array().is_none_or(Vec::is_empty);
            if idle("turns") && idle("background") && idle("checks") {
                let mut event = event.ok();
                event.duration_us = Some(began.elapsed().as_micros() as i64);
                self.chats.writer.record(event);
                return true;
            }
            if began.elapsed() > limit || self.stop.load(Ordering::Relaxed) {
                let mut event = event.error("busy").attr("running", running);
                event.duration_us = Some(began.elapsed().as_micros() as i64);
                self.chats.writer.record(event);
                let why = format!("{version} held {limit:?}: still running");
                {
                    let mut shown = self.shown();
                    shown.holding = None;
                    shown.failed = Some((why, ids::reference(&trace).to_string()));
                }
                self.chats.set_updating(None).await;
                self.publish();
                return false;
            }
            tokio::time::sleep(LOOK).await;
        }
    }

    /// What runs anywhere now, as the app's pause sees it.
    async fn running_now(&self) -> Value {
        let checks = self.chats.checking();
        let all = Pause {
            scope: "app".into(),
            target: String::new(),
            since_us: 0,
            quiet_us: None,
            stopped: false,
        };
        let found = self
            .chats
            .writer
            .call(move |c| pauses::running(c, &all, &checks).map_err(|e| e.to_string()));
        match found.await {
            Some(Ok(running)) => running,
            // Unknown counts as running: the hold waits, never guesses.
            _ => json!({ "turns": ["unknown"] }),
        }
    }
}

/// `GATHR_APP_HOLD_MS` shortens the hold, in dev and tests only.
fn hold_limit() -> Duration {
    let shorter = cfg!(debug_assertions)
        .then(|| std::env::var("GATHR_APP_HOLD_MS").ok()?.parse().ok())
        .flatten();
    shorter.map_or(HOLD, Duration::from_millis)
}

/// Checks at start, then every 10 minutes or when asked, until an update may go ahead or the
/// service stops.
pub async fn run(app: Arc<AppUpdates>) {
    let Some(feed) = release::feed() else {
        return;
    };
    while !app.stop.load(Ordering::Relaxed) {
        app.shown().checking = true;
        let (check, f) = (app.clone(), feed.clone());
        let ready = tokio::task::spawn_blocking(move || check.check(&f)).await;
        app.publish();
        if let Ok(Some(version)) = ready
            && app.hold(&version).await
        {
            *app.handing_over.lock().expect("never poisoned") = Some(version);
            app.ready.notify_one();
            return;
        }
        tokio::select! {
            _ = tokio::time::sleep(EVERY) => {}
            _ = app.wake.notified() => {}
        }
    }
}

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route("/api/app", get(view))
        .route("/api/app/update", post(update))
}

async fn view(Extract(s): Extract<Arc<State>>) -> Json<Value> {
    Json(s.app.view())
}

/// Checks for a release now; `GET /api/app` shows how it went.
async fn update(Extract(s): Extract<Arc<State>>, Extension(t): Extension<Trace>) -> Answer {
    if !s.app.check_now() {
        let why = "this dev build has no release feed: set GATHR_APP_RELEASES";
        return Err(fail(StatusCode::CONFLICT, "Update failed", why, &t.0));
    }
    Ok(Json(json!({ "checking": true })))
}
