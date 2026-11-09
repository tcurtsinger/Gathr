//! The window keeps to a running service (0032). After a hand-over to an update, the new service
//! has its own process and token: the window signs in to it with a fresh one-time link. A service
//! that stops answering while the window is open is started again through the same start as a
//! launch, so a new copy that died on trial rolls back at once rather than at the next open.

use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use gathr::crash_log::CrashLog;
use gathr::data::ServiceInfo;
use gathr::{Error, http, launch};
use tauri::{AppHandle, Manager};

use crate::{Service, parse, report, update};

/// Seconds without an answer before the window starts the service again: longer than a
/// hand-over's gap, which ends with a new `service.json` first.
const SILENT: u32 = 3;

pub fn run(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let mut silent = 0;
        loop {
            std::thread::sleep(Duration::from_secs(1));
            let service = app.state::<Service>();
            // None while Quit stops it; an update's install exits this window anyway.
            let Some(known) = service.info.lock().unwrap().clone() else {
                continue;
            };
            let updates = app.try_state::<update::Updates>();
            if updates.is_some_and(|u| u.installing.load(Ordering::SeqCst)) {
                continue;
            }
            match ServiceInfo::read(&service.data) {
                Ok(now) if now.pid != known.pid && answers(&now) => {
                    silent = 0;
                    let attrs = serde_json::json!({ "from_pid": known.pid, "pid": now.pid });
                    sign_in(&app, now, "gathr.ui.window.follow", attrs);
                }
                _ if answers(&known) => silent = 0,
                _ => {
                    silent += 1;
                    if silent >= SILENT {
                        silent = 0;
                        start_again(&app, known.pid);
                    }
                }
            }
        }
    });
}

fn answers(info: &ServiceInfo) -> bool {
    http::call(info.port, &info.token, "GET", "/health").is_ok_and(|r| r.status == 200)
}

fn start_again(app: &AppHandle, from_pid: u32) {
    let since = Instant::now();
    let data = app.state::<Service>().data.clone();
    let found = launch::beside_me("gathrd").and_then(|g| launch::find_or_start(&data, &g));
    match found {
        Ok(found) => {
            let attrs = serde_json::json!({
                "from_pid": from_pid, "pid": found.info.pid, "started_service": found.started
            });
            sign_in(app, found.info, "gathr.ui.window.restart", attrs);
        }
        // No service to record it: the crash log, which `gathr diag health` shows. The window
        // keeps what it shows and tries again in a few seconds.
        Err(e) => {
            let ms = since.elapsed().as_millis();
            let line = format!("the window starting gathrd again failed after {ms} ms: {e}");
            CrashLog::new(data.crash_log()).append(&line);
        }
    }
}

/// Signs the window in to `info` with a fresh one-time link and keeps it for Quit and the tray.
/// On the same port the page stays and only swaps the link for the new cookie, so unsent drafts
/// and answers survive; its live socket reconnects by itself and reloads what it shows. On
/// another port (the old one was taken) the page's origin changes, so it has to load anew.
fn sign_in(app: &AppHandle, info: ServiceInfo, event: &str, mut attrs: serde_json::Value) {
    let since = Instant::now();
    let mut in_place = false;
    let loaded = launch::one_time_link(&info).and_then(|url| {
        let window = app.get_webview_window("main");
        let window = window.ok_or_else(|| Error::Other("no window".into()))?;
        in_place = window.url().is_ok_and(|u| u.port() == Some(info.port));
        let done = match in_place {
            // A fetch that never reached the service leaves the code unused: load the link then.
            true => window.eval(format!(
                "fetch({url:?}, {{credentials: 'same-origin'}}).catch(() => location.replace({url:?}))"
            )),
            false => window.navigate(parse(&url)),
        };
        done.map_err(|e| Error::Other(e.to_string()))
    });
    *app.state::<Service>().info.lock().unwrap() = Some(info);
    attrs["in_place"] = in_place.into();
    report(
        app,
        event,
        since,
        &attrs,
        loaded.err().map(|e| e.to_string()),
    );
    // The window's update comes from the same release as the service's.
    update::check_now(app);
}
