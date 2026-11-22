//! The window updates itself (F7): Tauri's updater checks the release feed at start and every
//! hour, downloads an update and checks its signature with the updater's key. One Update for both
//! parts (0032): once the service hands over to a release the person chose with Update, the window
//! installs its own part of it at once and restarts. Otherwise it installs once hidden in the tray.
//! Only the window restarts: the service runs from its own copy in the data folder (0031), so the
//! installer never touches it, and it keeps running through the restart.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gathr::fetch::parse_version;
use gathr::http;
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{Service, report};

const EVERY: Duration = Duration::from_secs(60 * 60);
/// Left in the data folder by an install made while hidden, or while shown: the next start comes
/// back the same way, whatever its arguments say (a start at login carries `--hidden`).
const RESTART_HIDDEN: &str = "window-restart-hidden";
const RESTART_SHOWN: &str = "window-restart-shown";

#[derive(Default)]
pub struct Updates {
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    /// The service took the person's Update to a release newer than this window.
    asked: AtomicBool,
    /// Set from the install on: the exit that follows leaves the service running.
    pub installing: AtomicBool,
    checking: AtomicBool,
}

/// What the window does with its own update.
#[derive(Debug, PartialEq)]
enum Install {
    Now,
    WhenHidden,
    Nothing,
}

/// Ready and asked for, or ready while hidden: now. Ready while shown and not asked for: once
/// hidden, as a restart with the person at work would reload the page under them.
fn when(ready: bool, asked: bool, shown: bool) -> Install {
    match (ready, asked || !shown) {
        (false, _) => Install::Nothing,
        (true, true) => Install::Now,
        (true, false) => Install::WhenHidden,
    }
}

/// Whether version `a` is newer than `b`.
fn newer(a: &str, b: &str) -> bool {
    parse_version(a) > parse_version(b)
}

/// Checks now and every hour. A debug build never updates.
pub fn run(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        loop {
            check(&app);
            std::thread::sleep(EVERY);
        }
    });
}

/// The window signed in to a service that started anew. One that runs a newer release than this
/// window took the person's Update: the window's part of that release installs at once, fetched
/// first if it isn't ready, or if what is ready is older than the service's release. The version
/// the service runs, not `current.json`'s: a hand-over that started the copy before names the new
/// one there.
pub fn signed_in(app: &AppHandle) {
    let window = app.package_info().version.to_string();
    let updates = app.state::<Updates>();
    if let Some(service) = service_version(app).filter(|s| newer(s, &window)) {
        updates.asked.store(true, Ordering::SeqCst);
        let mut ready = updates.ready.lock().unwrap();
        if ready
            .as_ref()
            .is_some_and(|(u, _)| newer(&service, &u.version))
        {
            *ready = None;
        }
    }
    let app = app.clone();
    std::thread::spawn(move || check(&app));
}

/// The version the service the window signed in to says it runs.
fn service_version(app: &AppHandle) -> Option<String> {
    let info = app.state::<Service>().info.lock().unwrap().clone()?;
    let answer = http::call(info.port, &info.token, "GET", "/api/app").ok()?;
    answer.json()["version"].as_str().map(String::from)
}

/// The window was hidden: a ready update installs now.
pub fn hidden(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || settle(&app));
}

/// Fetches an update unless one is ready or another check runs, then acts on what is ready.
fn check(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let updates = app.state::<Updates>();
    let ready = updates.ready.lock().unwrap().is_some();
    if !ready && !updates.checking.swap(true, Ordering::SeqCst) {
        fetched(app);
        updates.checking.store(false, Ordering::SeqCst);
    }
    settle(app);
}

/// Installs what is ready when it's time; the person's Update lapses with nothing to install.
fn settle(app: &AppHandle) {
    let updates = app.state::<Updates>();
    let ready = updates.ready.lock().unwrap().is_some();
    let asked = updates.asked.load(Ordering::SeqCst);
    match when(ready, asked, shown(app)) {
        Install::Now => install(app, if asked { "update" } else { "hide" }),
        Install::Nothing if !updates.checking.load(Ordering::SeqCst) => {
            updates.asked.store(false, Ordering::SeqCst);
        }
        Install::Nothing | Install::WhenHidden => {}
    }
}

/// Downloads and checks an update and keeps it ready; a failure is recorded and tried again at the
/// next check.
fn fetched(app: &AppHandle) {
    let since = Instant::now();
    let mut offered = None;
    let found = tauri::async_runtime::block_on(async {
        let Some(update) = app.updater()?.check().await? else {
            return Ok(None);
        };
        offered = Some(update.version.clone());
        let bytes = update.download(|_, _| {}, || {}).await?;
        Ok::<_, tauri_plugin_updater::Error>(Some((update, bytes)))
    });
    let current = app.package_info().version.to_string();
    let (size, error) = match &found {
        Ok(Some((_, bytes))) => (bytes.len(), None),
        Ok(None) => (0, None),
        Err(e) => (0, Some(e.to_string())),
    };
    let attrs = serde_json::json!({ "current": current, "version": offered, "bytes": size });
    report(app, "gathr.ui.update.check", since, &attrs, error);
    if let Ok(Some(ready)) = found {
        *app.state::<Updates>().ready.lock().unwrap() = Some(ready);
    }
}

/// Installs the ready update and restarts the window, shown or hidden as it was. On Windows the
/// installer exits this process and starts the new window itself.
fn install(app: &AppHandle, via: &str) {
    let updates = app.state::<Updates>();
    let Some((update, bytes)) = updates.ready.lock().unwrap().take() else {
        return;
    };
    updates.asked.store(false, Ordering::SeqCst);
    let since = Instant::now();
    let hidden = !shown(app);
    let folder = app.state::<Service>().data.path().to_path_buf();
    updates.installing.store(true, Ordering::SeqCst);
    let attrs = serde_json::json!({ "version": update.version, "via": via, "hidden": hidden });
    report(app, "gathr.ui.update.install", since, &attrs, None);
    match mark_restart(&folder, hidden)
        .map_err(|e| e.to_string())
        .and_then(|()| {
            // Windows doesn't return from here when the installer starts.
            update.install(&bytes).map_err(|e| e.to_string())
        }) {
        Ok(()) => app.restart(),
        Err(why) => {
            updates.installing.store(false, Ordering::SeqCst);
            let _ = restarted(&folder);
            report(app, "gathr.ui.update.install", since, &attrs, Some(why));
        }
    }
}

fn mark_restart(folder: &Path, hidden: bool) -> std::io::Result<()> {
    let name = if hidden {
        RESTART_HIDDEN
    } else {
        RESTART_SHOWN
    };
    std::fs::write(folder.join(name), b"")
}

/// Whether this start follows an update, and then whether the window was hidden: it comes back so.
pub fn restarted(folder: &Path) -> Option<bool> {
    let taken = |name: &str| std::fs::remove_file(folder.join(name)).is_ok();
    match (taken(RESTART_HIDDEN), taken(RESTART_SHOWN)) {
        (true, _) => Some(true),
        (false, true) => Some(false),
        (false, false) => None,
    }
}

fn shown(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_update_the_person_asked_for_installs_now_while_shown() {
        assert_eq!(when(true, true, true), Install::Now);
    }

    #[test]
    fn an_update_nobody_asked_for_waits_until_the_window_is_hidden() {
        assert_eq!(when(true, false, true), Install::WhenHidden);
        assert_eq!(when(true, false, false), Install::Now);
    }

    #[test]
    fn with_nothing_ready_nothing_installs() {
        assert_eq!(when(false, true, true), Install::Nothing);
        assert_eq!(when(false, false, false), Install::Nothing);
    }

    #[test]
    fn only_a_newer_release_counts_as_newer() {
        assert!(newer("0.1.10", "0.1.9"));
        assert!(!newer("0.1.9", "0.1.9"));
        assert!(!newer("0.1.8", "0.1.9"));
    }

    #[test]
    fn a_window_updated_while_shown_comes_back_shown_once() {
        let folder = std::env::temp_dir().join(format!("gathr-app-restart-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        mark_restart(&folder, false).unwrap();
        let first = restarted(&folder);
        let second = restarted(&folder);
        mark_restart(&folder, true).unwrap();
        let hidden = restarted(&folder);
        let _ = std::fs::remove_dir_all(&folder);
        assert_eq!((first, second, hidden), (Some(false), None, Some(true)));
    }
}
