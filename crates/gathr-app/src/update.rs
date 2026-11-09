//! The window updates itself (F7): Tauri's updater checks the release feed at start and every
//! hour, downloads an update and checks its signature with the updater's key. It installs once the
//! window is hidden in the tray, at once if it already is; until then the tray offers Restart to
//! update. Only the window restarts: the service runs from its own copy in the data folder (0031),
//! so the installer never touches it, and it keeps running through the restart.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use tauri::menu::{Menu, MenuItem};
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::{Service, report};

const EVERY: Duration = Duration::from_secs(60 * 60);

pub struct Updates {
    ready: Mutex<Option<(Update, Vec<u8>)>>,
    /// Set from the install on: the exit that follows leaves the service running.
    pub installing: AtomicBool,
    menu: Menu<Wry>,
    restart: MenuItem<Wry>,
}

pub fn manage(app: &AppHandle, menu: Menu<Wry>) -> tauri::Result<()> {
    let restart = MenuItem::with_id(app, "update", "Restart to update", true, None::<&str>)?;
    app.manage(Updates {
        ready: Mutex::new(None),
        installing: AtomicBool::new(false),
        menu,
        restart,
    });
    Ok(())
}

/// Checks now and every hour until an update is ready. A debug build never updates.
pub fn run(app: &AppHandle) {
    if cfg!(debug_assertions) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        while !fetched(&app) {
            std::thread::sleep(EVERY);
        }
    });
}

/// Whether an update was downloaded and checked; failures are recorded and tried again later.
fn fetched(app: &AppHandle) -> bool {
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
    let Ok(Some(ready)) = found else {
        return false;
    };
    let updates = app.state::<Updates>();
    *updates.ready.lock().unwrap() = Some(ready);
    if shown(app) {
        let _ = updates.menu.prepend(&updates.restart);
    } else {
        install(app, "hidden");
    }
    true
}

/// The window was hidden: a ready update installs now.
pub fn hidden(app: &AppHandle) {
    let ready = app.state::<Updates>().ready.lock().unwrap().is_some();
    if ready {
        let app = app.clone();
        std::thread::spawn(move || install(&app, "hide"));
    }
}

/// Installs the ready update and restarts the window, hidden again if it was hidden. On Windows
/// the installer exits this process and starts the new window itself.
pub fn install(app: &AppHandle, via: &str) {
    let updates = app.state::<Updates>();
    let Some((update, bytes)) = updates.ready.lock().unwrap().take() else {
        return;
    };
    let since = Instant::now();
    let hidden = !shown(app);
    let marker = restart_hidden(app);
    let marked = if hidden {
        std::fs::write(&marker, b"")
    } else {
        Ok(())
    };
    updates.installing.store(true, Ordering::SeqCst);
    let attrs = serde_json::json!({ "version": update.version, "via": via, "hidden": hidden });
    report(app, "gathr.ui.update.install", since, &attrs, None);
    match marked.map_err(|e| e.to_string()).and_then(|()| {
        // Windows doesn't return from here when the installer starts.
        update.install(&bytes).map_err(|e| e.to_string())
    }) {
        Ok(()) => app.restart(),
        Err(why) => {
            updates.installing.store(false, Ordering::SeqCst);
            let _ = std::fs::remove_file(&marker);
            let _ = updates.menu.remove(&updates.restart);
            report(app, "gathr.ui.update.install", since, &attrs, Some(why));
        }
    }
}

/// Whether this start follows an update made while hidden: then it starts hidden too.
pub fn take_restart_hidden(app: &AppHandle) -> bool {
    std::fs::remove_file(restart_hidden(app)).is_ok()
}

fn restart_hidden(app: &AppHandle) -> std::path::PathBuf {
    let service = app.state::<Service>();
    service.data.path().join("window-restart-hidden")
}

fn shown(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false)
}
