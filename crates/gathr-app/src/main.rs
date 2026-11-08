//! Gathr's window (0012): it shows the web UI and holds no state. Installed, it finds the service
//! on the data folder or starts `gathrd` beside it, and signs in through a one-time link (0004).
//! Closing it hides it to the tray (the menu bar on macOS); Quit there stops the service and the
//! window. Opening Gathr again shows the same window: one per data folder.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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
    let data = DataDir::new(arg("--data").map_or_else(platform::default_data_dir, PathBuf::from));
    installed(context, data, opened);
}

fn dev(context: Context<Wry>, url: tauri::Url, title: String) {
    tauri::Builder::default()
        .setup(move |app| {
            window(app.handle(), url, &title, None)?;
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
    let key = folder_key(data.path(), &platform::default_data_dir());
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
                    shown.err(),
                );
            }
        })
        .setup(move |app| {
            let (url, started) = connect(app.handle());
            window(app.handle(), url, "Gathr", webview_data)?;
            tray(app.handle())?;
            if let Some(started) = started {
                let attrs = serde_json::json!({ "started_service": started });
                report(app.handle(), "gathr.ui.window.open", opened, &attrs, None);
            }
            Ok(())
        })
        .build(context)
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
    app.run(|app, event| match event {
        #[cfg(target_os = "macos")]
        RunEvent::Reopen { .. } => reopen(app, "dock"),
        // Quit in the tray, Cmd+Q, or the system ending the session: the service goes too, gently.
        RunEvent::Exit => {
            if let Some(info) = app.state::<Service>().info.lock().unwrap().take() {
                let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
            }
        }
        _ => {}
    });
}

/// The one-instance key for a data folder other than the person's own (`None` for theirs), so a
/// test folder's window never stands in for theirs. Both launches run the same build, so the
/// hasher's output matches between them.
fn folder_key(data: &Path, default: &Path) -> Option<String> {
    let data = identity(data);
    if identity(default) == data {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    Some(format!(".data-{:016x}", hasher.finish()))
}

/// A folder as the file system knows it: links and `..` resolved, and case ignored, as Windows'
/// and macOS's file systems do by default. A folder that doesn't exist yet is only made absolute,
/// so the caller creates it first.
fn identity(path: &Path) -> String {
    let path = std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf());
    let path: PathBuf = path.components().collect();
    path.to_string_lossy().to_lowercase()
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
                    Some(e),
                );
            }
        }
        let shown = window
            .unminimize()
            .and_then(|()| window.show())
            .and_then(|()| window.set_focus());
        let attrs = serde_json::json!({ "via": via, "started_service": started });
        report(&app, "gathr.ui.window.reopen", opened, &attrs, shown.err());
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
        // Shown once built: a close while WebView2 was still being set up skipped the close
        // handler and quit Gathr.
        .visible(false)
        // With no handler, the webview drops a link's new window: nothing opens.
        .on_new_window(|url, _| {
            open_in_browser(&url);
            NewWindowResponse::Deny
        });
    if let Some(folder) = webview_data {
        builder = builder.data_directory(folder);
    }
    let window = builder.build()?;
    window.show()?;
    Ok(window)
}

/// The tray icon (the menu bar on macOS). On Windows a click opens the window and the menu is on
/// the right button; on macOS a click opens the menu, as menu bar items do.
fn tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Gathr")
        .menu(&Menu::with_items(app, &[&open, &quit])?)
        .show_menu_on_left_click(cfg!(target_os = "macos"))
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => reopen(app, "tray"),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
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

/// What the window did, recorded with the service's own events.
fn report(
    app: &AppHandle,
    name: &str,
    since: Instant,
    attrs: &serde_json::Value,
    error: Option<tauri::Error>,
) {
    let Some(info) = app.state::<Service>().info.lock().unwrap().clone() else {
        return;
    };
    let body = serde_json::json!({
        "name": name,
        "duration_ms": since.elapsed().as_millis() as u64,
        "attrs": attrs,
        "error": error.map(|e| e.to_string()),
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

#[cfg(test)]
mod tests {
    use super::folder_key;
    use std::path::{Path, PathBuf};

    #[test]
    fn each_data_folder_gets_its_own_window_and_the_persons_own_keeps_the_plain_one() {
        let own = Path::new("/home/someone/Gathr");
        let a = folder_key(Path::new("/work/data/a"), own);
        assert_eq!(folder_key(own, own), None, "the person's own folder");
        assert!(a.is_some(), "a test folder has a key");
        assert_ne!(a, folder_key(Path::new("/work/data/b"), own), "two folders");
        assert_eq!(
            a,
            folder_key(Path::new("/work/data/a/"), own),
            "the same folder, spelt with a /"
        );
    }

    // Windows' and macOS's file systems ignore case by default, so `X:\CODE` is `X:\code`.
    #[test]
    fn one_folder_spelt_another_way_gets_the_same_window() {
        let root = std::env::temp_dir().join(format!("gathr-key-{}", std::process::id()));
        let folder = root.join("Data");
        std::fs::create_dir_all(&folder).unwrap();
        let own = Path::new("/home/someone/Gathr");
        let key = folder_key(&folder, own);
        let upper = PathBuf::from(folder.to_string_lossy().to_uppercase());
        let dotted = root.join("Data").join("..").join("Data");
        let (by_upper, by_dots) = (folder_key(&upper, own), folder_key(&dotted, own));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            key,
            by_upper,
            "{} and {}",
            folder.display(),
            upper.display()
        );
        assert_eq!(
            key,
            by_dots,
            "{} and {}",
            folder.display(),
            dotted.display()
        );
    }
}
