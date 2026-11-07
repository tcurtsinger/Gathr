//! Gathr's window (0012): it shows the web UI at the address it's given and holds no state.
//! Engines live in `gathrd`, so closing or reloading the window never touches a turn (0004).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::webview::NewWindowResponse;
use tauri::{WebviewUrl, WebviewWindowBuilder};

fn main() {
    let ui = std::env::args()
        .skip_while(|a| a != "--ui")
        .nth(1)
        .unwrap_or_else(|| exit("Usage: gathr-app --ui <address of the UI>"));
    let url = ui
        .parse()
        .unwrap_or_else(|e| exit(&format!("Start failed: {ui} isn't an address ({e})")));
    tauri::Builder::default()
        .setup(move |app| {
            WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
                .title("Gathr")
                .inner_size(1440.0, 900.0)
                .min_inner_size(480.0, 480.0)
                // With no handler, the webview drops a link's new window: nothing opens.
                .on_new_window(|url, _| {
                    open_in_browser(&url);
                    NewWindowResponse::Deny
                })
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
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
