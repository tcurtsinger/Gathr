//! `gathr dev --window`: the UI's dev server and the desktop window, beside the service.
//! The UI is served on the service's port plus 10, and its dev server adds the token (0004).

use std::path::Path;
use std::time::Duration;

use gathr::Error;
use gathr::data::DataDir;
use tokio::process::{Child, Command};

pub struct Window {
    vite: Child,
    app: Child,
}

pub async fn start(data: &DataDir, port: u16) -> Result<Window, Error> {
    let ui = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../ui");
    let vite = ui.join("node_modules/vite/bin/vite.js");
    if !vite.is_file() {
        return Err(Error::Other(
            "the UI isn't installed: run `npm ci` in ui/".into(),
        ));
    }
    let ui_port = port
        .checked_add(10)
        .ok_or_else(|| Error::Other(format!("port {port} leaves no room for the UI's (+10)")))?;
    let vite = Command::new("node")
        .arg(vite)
        .current_dir(&ui)
        .env("GATHR_DATA", data.path())
        .env("GATHR_UI_PORT", ui_port.to_string())
        .kill_on_drop(true)
        .spawn()?;
    listening(ui_port).await?;
    let address = format!("http://127.0.0.1:{ui_port}/");
    let exe = std::env::current_exe()?
        .with_file_name(format!("gathr-app{}", std::env::consts::EXE_SUFFIX));
    let app = Command::new(exe)
        .args(["--ui", &address])
        .kill_on_drop(true)
        .spawn()?;
    println!("Gathr window on {address} (a browser can open it too).");
    Ok(Window { vite, app })
}

impl Window {
    /// Neither holds state or engines (0012), so both are simply ended.
    pub async fn stop(mut self) {
        for child in [&mut self.app, &mut self.vite] {
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
    }
}

async fn listening(port: u16) -> Result<(), Error> {
    for _ in 0..200 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return Ok(());
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(Error::Other(format!(
        "the UI's dev server didn't listen on {port} within 10 s"
    )))
}
