//! Gathr's window (0012): it shows the web UI at the address it's given and holds no state.
//! Engines live in `gathrd`, so closing or reloading the window never touches a turn (0004).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
                .build()?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| exit(&format!("Start failed: {e}")));
}

fn exit(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2)
}
