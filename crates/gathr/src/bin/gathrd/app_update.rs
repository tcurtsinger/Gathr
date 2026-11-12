//! Gathr updates its own service when the person says (0032, the room's manual-update plan). It
//! looks for a newer release at start, every 10 minutes and when asked; installs it beside the copy
//! that runs and checks it, and shows it ready. Update stops what runs as Stop now does, queues a
//! carry-on per agent stopped, and hands over (`gathr::launch::hand_over`), which goes back to the
//! copy before if the new one doesn't start. The window's own update is asked for through the data
//! folder: it writes `window-update.json` when one is ready, and Update leaves `window-install`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{Extension, State as Extract};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use gathr::app::{Copies, STARTING, Started, TRIAL, WINDOW_INSTALL, WINDOW_READY, release};
use gathr::event::Event;
use gathr::fetch::{Failure, failed, parse_version};
use gathr::pauses::{self, Pause};
use gathr::{ids, platform};
use serde_json::{Value, json};
use tokio::sync::Notify;
use tokio::sync::broadcast::error::RecvError;

use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, fail};

const EVERY: Duration = Duration::from_secs(10 * 60);
/// How long Update waits for the turns it stopped to end before it hands over anyway: nothing
/// running holds the room's update for long (G194).
const ENDING: Duration = Duration::from_secs(20);
/// How often it looks at what still runs.
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
    /// The update checked and ready, and since when.
    ready: Option<(String, i64)>,
    /// Installing now: the version, and what it still waits on to end.
    installing: Option<(String, Value)>,
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
        let window = self.window_ready();
        let shown = self.shown();
        let state = match (&shown.installing, &shown.ready, &window, shown.checking) {
            (Some(_), ..) => "updating",
            (_, Some(_), _, _) | (_, _, Some(_), _) => "ready",
            _ if release::feed().is_none() => "off",
            (.., true) => "checking",
            _ => "idle",
        };
        json!({
            "version": self.running,
            "previous": current.and_then(|c| c.previous).filter(|p| *p != self.running),
            "state": state,
            "checked": shown.checked_us.map(ids::utc),
            "failed": shown.failed.as_ref().map(|(why, r)| json!({ "why": why, "ref": r })),
            "update": shown.ready.as_ref().map(|(v, since)| json!({ "version": v, "since_us": since })),
            "window": window.map(|v| json!({ "version": v })),
            "running": shown.installing.as_ref().map(|(_, running)| running),
        })
    }

    /// The window's update it downloaded and checked, if it has one ready.
    fn window_ready(&self) -> Option<String> {
        let file = std::fs::read(self.chats.data.path().join(WINDOW_READY)).ok()?;
        let ready: Value = serde_json::from_slice(&file).ok()?;
        ready["version"].as_str().map(String::from)
    }

    /// Update: installs what's ready, the service's now and the window's when it next looks.
    pub fn install(self: &Arc<Self>, trace: &str) -> Result<Value, String> {
        let (ready, window) = (self.shown().ready.clone(), self.window_ready());
        if self.shown().installing.is_some() {
            return Err("Gathr is updating already".into());
        }
        if ready.is_none() && window.is_none() {
            return Err("no update is ready".into());
        }
        if window.is_some() {
            let asked = std::fs::write(self.chats.data.path().join(WINDOW_INSTALL), b"");
            asked.map_err(|e| format!("the window wasn't asked: {e}"))?;
        }
        let version = ready.map(|(version, _)| version);
        let event = Event::point("gathr.app.install").trace(trace).ok();
        let event = event
            .attr("version", version.clone())
            .attr("window", window.clone());
        self.chats.writer.record(event);
        if let Some(version) = &version {
            self.shown().installing = Some((version.clone(), Value::Null));
            self.publish();
            let (app, version, trace) = (self.clone(), version.clone(), trace.to_string());
            tokio::spawn(async move { app.switch(version, trace).await });
        }
        Ok(json!({ "installing": version, "window": window }))
    }

    /// Stops what runs, its agents to carry on after, and hands over once the turns stopped have
    /// ended, or after `ENDING` with what still ran recorded.
    async fn switch(&self, version: String, trace: String) {
        self.chats.set_updating(Some(version.clone())).await;
        let by = format!("Gathr's update to {version}");
        self.chats.stop_now("app", "", &by).await;
        let began = Instant::now();
        let event = Event::point("gathr.app.hold")
            .trace(&trace)
            .attr("version", version.as_str());
        let mut event = loop {
            let running = self.running_now().await;
            if running["turns"].as_array().is_none_or(Vec::is_empty) {
                break event.ok();
            }
            if began.elapsed() > ENDING {
                break event.error("busy").attr("running", running);
            }
            let shown = Some((version.clone(), running));
            if std::mem::replace(&mut self.shown().installing, shown.clone()) != shown {
                self.publish();
            }
            tokio::time::sleep(LOOK).await;
        };
        event.duration_us = Some(began.elapsed().as_micros() as i64);
        self.chats.writer.record(event);
        *self.handing_over.lock().expect("never poisoned") = Some(version);
        self.ready.notify_one();
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

    /// Where the last hand-over got to: one that stopped before starting its update is an error.
    fn record_hand_over(&self) {
        let Some(hand_over) = self.copies.take_hand_over() else {
            return;
        };
        let event = Event::point("gathr.app.handover");
        let event = match hand_over.step.as_str() {
            STARTING => event.ok(),
            _ => event.trace(&ids::trace_id()).error("stopped"),
        };
        let event = event
            .attr("from", hand_over.from)
            .attr("to", hand_over.to)
            .attr("pid", hand_over.pid)
            .attr("step", hand_over.step)
            .attr("since", ids::utc(hand_over.since_us))
            .attr("left_running", hand_over.left_running);
        self.chats.writer.record(event);
    }

    /// Records what the start found in `current.json` (`Copies::started`, read before the service
    /// said it was up): an update that started, or a rollback from one that didn't; a copy on trial
    /// is watched until it passes.
    pub fn started(self: &Arc<Self>, found: std::io::Result<Started>) {
        self.record_hand_over();
        let started = match found {
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
        if started.on_trial {
            tokio::spawn(trial(self.clone()));
        }
    }

    /// A clean stop on trial, so the next start doesn't take it for a death.
    pub fn stopped(&self) {
        if let Err(e) = self.copies.stopped() {
            let event = Event::point("gathr.app.trial").error("disk");
            self.chats.writer.record(event.attr("why", e.to_string()));
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

/// The copy on trial passes at its first finished turn or after `TRIAL`, whichever comes first.
async fn trial(app: Arc<AppUpdates>) {
    let mut live = app.chats.live.subscribe();
    let turn = async {
        loop {
            match live.recv().await {
                Ok(e) if e["type"] == "turn" && e["status"] == "done" => return,
                Err(RecvError::Closed) => std::future::pending().await,
                _ => {}
            }
        }
    };
    let by = tokio::select! {
        _ = tokio::time::sleep(TRIAL) => "time",
        () = turn => "turn",
    };
    let event = Event::point("gathr.app.trial").attr("version", app.running.as_str());
    let event = match app.copies.passed() {
        Ok(_) => event.ok().attr("by", by),
        Err(e) => event.error("disk").attr("why", e.to_string()),
    };
    app.chats.writer.record(event);
}

/// Checks at start, then every 10 minutes or when asked, until the service stops: the newest
/// release checked is shown ready until the person updates.
pub async fn run(app: Arc<AppUpdates>) {
    let Some(feed) = release::feed() else {
        return;
    };
    while !app.stop.load(Ordering::Relaxed) {
        app.shown().checking = true;
        let (check, f) = (app.clone(), feed.clone());
        if let Ok(Some(version)) = tokio::task::spawn_blocking(move || check.check(&f)).await {
            app.shown().ready = Some((version, ids::now_us()));
        }
        app.publish();
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

/// Checks for a release now; with `{"install": true}`, Update: installs what's ready.
/// `GET /api/app` shows how either went.
async fn update(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    body: axum::body::Bytes,
) -> Answer {
    let asked: Value = serde_json::from_slice(&body).unwrap_or_default();
    if asked["install"] == true {
        let installing = s.app.install(&t.0);
        let why = |why: String| fail(StatusCode::CONFLICT, "Update failed", &why, &t.0);
        return installing.map(Json).map_err(why);
    }
    if !s.app.check_now() {
        let why = "this dev build has no release feed: set GATHR_APP_RELEASES";
        return Err(fail(StatusCode::CONFLICT, "Update failed", why, &t.0));
    }
    Ok(Json(json!({ "checking": true })))
}
