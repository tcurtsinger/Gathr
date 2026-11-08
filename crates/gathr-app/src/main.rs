//! Gathr's window (0012): it shows the web UI and holds no state. Installed, it finds the service
//! on the data folder or starts `gathrd` beside it, and signs in through a one-time link (0004).
//! Engines live in `gathrd`, so closing the window never touches a turn; Quit in the tray stops both.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::time::Instant;

use gathr::data::{DataDir, ServiceInfo};
use gathr::{http, launch, platform};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::webview::NewWindowResponse;
use tauri::{AppHandle, WebviewUrl, WebviewWindowBuilder};

fn main() {
    let opened = Instant::now();
    // `gathr dev --window`: the UI's dev server, which adds the token itself.
    if let Some(ui) = arg("--ui") {
        let title = arg("--title").unwrap_or_else(|| "Gathr".into());
        return run(parse(&ui), title, None);
    }
    let data = DataDir::new(arg("--data").map_or_else(platform::default_data_dir, PathBuf::from));
    match reach(&data) {
        Ok((info, url, started)) => {
            report_open(&info, opened, started);
            run(parse(&url), "Gathr".into(), Some(info));
        }
        Err(why) => run(failed_page(&why), "Gathr".into(), None),
    }
}

/// The service on the data folder, found or started, and a one-time link into it.
fn reach(data: &DataDir) -> Result<(ServiceInfo, String, bool), gathr::Error> {
    let found = launch::find_or_start(data, &launch::beside_me("gathrd")?)?;
    let url = launch::one_time_link(&found.info)?;
    Ok((found.info, url, found.started))
}

fn run(url: tauri::Url, title: String, service: Option<ServiceInfo>) {
    tauri::Builder::default()
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title(title)
                .inner_size(1440.0, 900.0)
                .min_inner_size(480.0, 480.0)
                // With no handler, the webview drops a link's new window: nothing opens.
                .on_new_window(|url, _| {
                    open_in_browser(&url);
                    NewWindowResponse::Deny
                })
                .build()?;
            if let Some(info) = service {
                tray(app.handle(), info)?;
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
}

/// The tray icon (the menu bar on macOS): Quit stops the service the gentle way, then the window.
fn tray(app: &AppHandle, info: ServiceInfo) -> tauri::Result<()> {
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Gathr")
        .menu(&Menu::with_items(app, &[&quit])?)
        .on_menu_event(move |app, event| {
            if event.id() == "quit" {
                let _ = http::call(info.port, &info.token, "POST", "/api/shutdown");
                app.exit(0);
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

/// How long the window took to reach the service, recorded with the service's own events.
fn report_open(info: &ServiceInfo, opened: Instant, started: bool) {
    let body = serde_json::json!({
        "name": "gathr.ui.window.open",
        "duration_ms": opened.elapsed().as_millis() as u64,
        "attrs": { "started_service": started },
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
