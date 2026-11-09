//! Gathr's window (0012): it shows the web UI and holds no state. Installed, it finds the service
//! on the data folder or starts `gathrd` beside it, and signs in through a one-time link (0004).
//! Closing it hides it to the tray (the menu bar on macOS); Quit there stops the service and the
//! window. Opening Gathr again shows the same window: one per data folder.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod update;

use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::time::Instant;

use gathr::data::{DataDir, ServiceInfo};
use gathr::{http, launch, platform};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::webview::NewWindowResponse;
use tauri::{
    AppHandle, Context, Manager, RunEvent, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent, Wry,
};

fn main() {
    let opened = Instant::now();
    let context = tauri::generate_context!();
    // `gathr dev --window`: the UI's dev server, which adds the token itself. Closing it quits.
    if let Some(ui) = arg("--ui") {
        let title = arg("--title").unwrap_or_else(|| "Gathr".into());
        return dev(context, parse(&ui), title);
    }
    // `GATHR_DATA` for a Mac app bundle on a test folder: a bundle can't pass arguments, only its
    // `LSEnvironment`, and macOS ignores `HOME` there. Debug builds only, so a stray variable
    // never points the person's installed Gathr at another folder.
    let test_data = cfg!(debug_assertions).then(|| std::env::var_os("GATHR_DATA"));
    let data = arg("--data")
        .map(PathBuf::from)
        .or_else(|| test_data.flatten().map(PathBuf::from));
    let data = DataDir::new(data.unwrap_or_else(platform::default_data_dir));
    installed(context, data, opened);
}

fn dev(context: Context<Wry>, url: tauri::Url, title: String) {
    tauri::Builder::default()
        .setup(move |app| {
            window(app.handle(), url, &title, None)?.show()?;
            Ok(())
        })
        .run(context)
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
}

/// The data folder, and the service on it once reached: Quit stops that one.
struct Service {
    data: DataDir,
    info: Mutex<Option<ServiceInfo>>,
}

fn installed(mut context: Context<Wry>, data: DataDir, opened: Instant) {
    // Made now, so the first launch keys the folder as every later one does. A folder that can't
    // be made fails in `connect`, with the reason shown.
    let _ = std::fs::create_dir_all(data.path());
    let key = launch::folder_key(data.path(), &platform::default_data_dir());
    if let Some(key) = &key {
        context.config_mut().identifier.push_str(key);
    }
    // A test folder keeps its webview's data with it, so deleting the folder leaves nothing behind.
    let webview_data = key.map(|_| data.path().join("webview"));
    let app = tauri::Builder::default()
        // A second launch on this folder hands over to this one and exits before its own setup runs.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            reopen(app, "second launch")
        }))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(Service {
            data,
            info: Mutex::new(None),
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let shown = window.hide();
                let attrs = serde_json::json!({});
                report(
                    window.app_handle(),
                    "gathr.ui.window.hide",
                    Instant::now(),
                    &attrs,
                    shown.err().map(|e| e.to_string()),
                );
                update::hidden(window.app_handle());
            }
        })
        .setup(move |app| {
            let (url, started) = connect(app.handle());
            let window = window(app.handle(), url, "Gathr", webview_data)?;
            tray(app.handle())?;
            // Started at login (`gathr::platform::login`), or after an update made while hidden:
            // in the tray, its window ready to show.
            let hidden = std::env::args().any(|a| a == "--hidden")
                | update::take_restart_hidden(app.handle());
            if !hidden {
                window.show()?;
            }
            if let Some(started) = started {
                let attrs = serde_json::json!({ "started_service": started, "hidden": hidden });
                report(app.handle(), "gathr.ui.window.open", opened, &attrs, None);
            }
            update::run(app.handle());
            Ok(())
        })
        .build(context)
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => reopen(app, "dock"),
        // Quit in the tray, Cmd+Q, or the system ending the session: the service goes too, gently.
        // Not for an update: only the window restarts.
        RunEvent::Exit => {
            let updates = app.try_state::<update::Updates>();
            if updates.is_some_and(|u| u.installing.load(Ordering::SeqCst)) {
                return;
            }
            if let Some(info) = app.state::<Service>().info.lock().unwrap().take() {
                let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
            }
        }
        _ => {}
    });
}

/// The service, found or started, kept for Quit, and a one-time link into it (and whether this
/// started it); or a page saying why it couldn't be reached.
fn connect(app: &AppHandle) -> (tauri::Url, Option<bool>) {
    let service = app.state::<Service>();
    let reached = launch::beside_me("gathrd")
        .and_then(|gathrd| launch::find_or_start(&service.data, &gathrd))
        .and_then(|found| Ok((launch::one_time_link(&found.info)?, found)));
    match reached {
        Ok((url, found)) => {
            *service.info.lock().unwrap() = Some(found.info);
            (parse(&url), Some(found.started))
        }
        Err(why) => (failed_page(&why), None),
    }
}

/// Shows the hidden window. The service may have stopped meanwhile: then it's found or started
/// again, as a fresh launch would, and the window loads it.
fn reopen(app: &AppHandle, via: &'static str) {
    let app = app.clone();
    // Off the event loop: starting the service can take a while.
    std::thread::spawn(move || {
        let opened = Instant::now();
        let Some(window) = app.get_webview_window("main") else {
            return;
        };
        let mut started = Some(false);
        if !still_running(&app) {
            let url;
            (url, started) = connect(&app);
            if let Err(e) = window.navigate(url) {
                report(
                    &app,
                    "gathr.ui.window.reopen",
                    opened,
                    &serde_json::json!({}),
                    Some(e.to_string()),
                );
            }
        }
        let shown = window
            .unminimize()
            .and_then(|()| window.show())
            .and_then(|()| window.set_focus());
        let attrs = serde_json::json!({ "via": via, "started_service": started });
        report(
            &app,
            "gathr.ui.window.reopen",
            opened,
            &attrs,
            shown.err().map(|e| e.to_string()),
        );
    });
}

/// Whether the service the window shows still answers on its folder.
fn still_running(app: &AppHandle) -> bool {
    let service = app.state::<Service>();
    let Some(known) = service.info.lock().unwrap().clone() else {
        return false;
    };
    ServiceInfo::read(&service.data).is_ok_and(|now| now.pid == known.pid)
        && http::call(known.port, &known.token, "GET", "/health").is_ok_and(|r| r.status == 200)
}

fn window(
    app: &AppHandle,
    url: tauri::Url,
    title: &str,
    webview_data: Option<PathBuf>,
) -> tauri::Result<WebviewWindow> {
    let mut builder = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title(title)
        .inner_size(1440.0, 900.0)
        .min_inner_size(480.0, 480.0)
        // The caller shows it once built: a close while WebView2 was still being set up skipped the
        // close handler and quit Gathr.
        .visible(false)
        // With no handler, the webview drops a link's new window: nothing opens.
        .on_new_window(|url, _| {
            open_in_browser(&url);
            NewWindowResponse::Deny
        });
    if let Some(folder) = webview_data {
        builder = builder.data_directory(folder);
    }
    builder.build()
}

/// The tray icon (the menu bar on macOS). On Windows a click opens the window and the menu is on
/// the right button; on macOS a click opens the menu, as menu bar items do.
fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let pause = MenuItem::with_id(app, "pause", "Pause all", true, None::<&str>)?;
    let resume = MenuItem::with_id(app, "resume", "Resume all", false, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &pause, &resume, &quit])?;
    app.manage(PauseItems { pause, resume });
    update::manage(app, menu.clone())?;
    show_paused(app);
    let mut builder = TrayIconBuilder::new()
        .tooltip("Gathr")
        .menu(&menu)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "update" => {
                let app = app.clone();
                std::thread::spawn(move || update::install(&app, "tray"));
            }
            "open" => reopen(app, "tray"),
            "pause" => pause_all(app, true),
            "resume" => pause_all(app, false),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // The pause may have changed elsewhere since: read before the menu opens.
            if matches!(event, TrayIconEvent::Enter { .. }) {
                show_paused(tray.app_handle());
            }
            let click = matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            );
            if cfg!(windows) && click {
                reopen(tray.app_handle(), "tray");
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// The tray's Pause all and Resume all, enabled by whether the whole app is paused.
struct PauseItems {
    pause: MenuItem<Wry>,
    resume: MenuItem<Wry>,
}

/// Pauses or resumes the whole app (0010): running turns finish, nothing new starts on its own.
fn pause_all(app: &AppHandle, pause: bool) {
    let app = app.clone();
    std::thread::spawn(move || {
        let since = Instant::now();
        let Some(info) = app.state::<Service>().info.lock().unwrap().clone() else {
            return;
        };
        let path = if pause {
            "/api/pauses"
        } else {
            "/api/pauses/resume"
        };
        let body = serde_json::json!({ "scope": "app" });
        let failed = match http::send(info.port, &info.token, "POST", path, &body) {
            Ok(r) if r.status == 200 => None,
            Ok(r) => Some(format!("{} {}", r.status, r.body)),
            Err(e) => Some(e.to_string()),
        };
        let attrs = serde_json::json!({ "pause": pause });
        report(&app, "gathr.ui.tray.pause", since, &attrs, failed);
        show_paused(&app);
    });
}

/// Enables Pause all or Resume all by the app's pause, as the service has it now.
fn show_paused(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let Some(info) = app.state::<Service>().info.lock().unwrap().clone() else {
            return;
        };
        let Ok(answer) = http::call(info.port, &info.token, "GET", "/api/pauses") else {
            return;
        };
        let pauses = answer.json()["pauses"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let paused = pauses.iter().any(|p| p["scope"] == "app");
        let items = app.state::<PauseItems>();
        let _ = items.pause.set_enabled(!paused);
        let _ = items.resume.set_enabled(paused);
    });
}

/// What the window did, recorded with the service's own events.
fn report(
    app: &AppHandle,
    name: &str,
    since: Instant,
    attrs: &serde_json::Value,
    error: Option<String>,
) {
    let Some(info) = app.state::<Service>().info.lock().unwrap().clone() else {
        return;
    };
    let body = serde_json::json!({
        "name": name,
        "duration_ms": since.elapsed().as_millis() as u64,
        "attrs": attrs,
        "error": error,
    });
    let _ = http::send(info.port, &info.token, "POST", "/api/ui/events", &body);
}

/// A page saying why the service couldn't be reached. Its crash log is in `gathr diag health`.
/// A file, since Tauri takes a `data:` address only with a feature that adds crates.
fn failed_page(why: &gathr::Error) -> tauri::Url {
    let text = format!("Start failed\n\n{why}\n\nMore: gathr diag health");
    let html = format!(
        "<!doctype html><meta charset=utf-8><title>Gathr</title>\
         <body style='font:14px system-ui;margin:2em;white-space:pre-wrap'>{}</body>",
        text.replace('&', "&amp;").replace('<', "&lt;")
    );
    let file = std::env::temp_dir().join("gathr-start-failed.html");
    std::fs::write(&file, html)
        .unwrap_or_else(|e| exit(&format!("Start failed: {why} (and its page: {e})")));
    tauri::Url::from_file_path(&file).unwrap_or_else(|()| exit(&format!("Start failed: {why}")))
}

fn arg(name: &str) -> Option<String> {
    std::env::args().skip_while(|a| a != name).nth(1)
}

fn parse(url: &str) -> tauri::Url {
    url.parse()
        .unwrap_or_else(|e| exit(&format!("Start failed: {url} isn't an address ({e})")))
}

/// A link that opens a new window goes to the person's own browser; the window shows only Gathr.
fn open_in_browser(url: &tauri::Url) {
    if !matches!(url.scheme(), "http" | "https") {
        return;
    }
    let program = if cfg!(windows) { "explorer" } else { "open" };
    let _ = std::process::Command::new(program)
        .arg(url.as_str())
        .spawn();
}

fn exit(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
