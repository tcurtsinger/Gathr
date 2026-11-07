//! `gathr dev`: builds and starts `gathrd` on a data folder and port, and on Ctrl+C stops it
//! and waits until it has exited, so a dev run leaves no process behind.

use std::process::ExitStatus;
use std::time::Duration;

use gathr::crash_log::CrashLog;
use gathr::data::{DataDir, ServiceInfo};
use gathr::{Error, http};

pub fn run(
    data: &DataDir,
    port: u16,
    seed: bool,
    window: bool,
    show: impl Fn(&serde_json::Value) -> String,
) -> Result<(), Error> {
    build(&["--bin", "gathrd"])?;
    if window {
        build(&["-p", "gathr-app"])?;
    }
    if seed {
        println!("{}", show(&super::run_seed(data)?));
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(supervise(data, port, window))
}

/// Under `cargo run`, Cargo builds only `gathr`; build the rest beside it, in the same profile.
fn build(what: &[&str]) -> Result<(), Error> {
    let Some(cargo) = std::env::var_os("CARGO") else {
        return Ok(());
    };
    let manifest = concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml");
    let mut build = std::process::Command::new(cargo);
    build.args(["build", "--quiet", "--manifest-path", manifest]);
    build.args(what);
    if !cfg!(debug_assertions) {
        build.arg("--release");
    }
    match build.status()? {
        status if status.success() => Ok(()),
        status => Err(Error::Other(format!("building {what:?} failed ({status})"))),
    }
}

async fn supervise(data: &DataDir, port: u16, window: bool) -> Result<(), Error> {
    let exe =
        std::env::current_exe()?.with_file_name(format!("gathrd{}", std::env::consts::EXE_SUFFIX));
    let mut child = tokio::process::Command::new(&exe)
        .arg("--data")
        .arg(data.path())
        .arg("--port")
        .arg(port.to_string())
        .spawn()?;
    let pid = child.id().unwrap_or(0);
    let info = tokio::select! {
        info = ready(data, pid) => info?,
        status = child.wait() => return Err(exited(data, status?)),
    };
    println!(
        "Gathr service on http://127.0.0.1:{}, pid {pid}, data {}. Ctrl+C stops it.",
        info.port,
        data.path().display()
    );
    let window = match window {
        true => Some(crate::window::start(data, info.port).await?),
        false => None,
    };
    tokio::select! {
        status = child.wait() => return Err(exited(data, status?)),
        () = stop_signal() => {}
    }
    if let Some(window) = window {
        window.stop().await;
    }
    println!("Stopping gathrd pid {pid}…");
    // The service may have had the Ctrl+C itself (macOS sends it to the whole group) and be gone already.
    let _ = tokio::task::spawn_blocking(move || {
        http::call(info.port, &info.token, "POST", "/api/shutdown")
    })
    .await;
    match tokio::time::timeout(Duration::from_secs(10), child.wait()).await {
        Ok(status) => println!("Stopped: gathrd pid {pid} exited ({}).", status?),
        Err(_) => {
            child.kill().await?;
            println!("gathrd pid {pid} didn't stop within 10 s, so it was killed.");
        }
    }
    Ok(())
}

/// Waits until the service with this pid has written `service.json`.
async fn ready(data: &DataDir, pid: u32) -> Result<ServiceInfo, Error> {
    for _ in 0..300 {
        if let Ok(info) = ServiceInfo::read(data)
            && info.pid == pid
        {
            return Ok(info);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(Error::Other(format!(
        "gathrd pid {pid} didn't write service.json within 15 s"
    )))
}

fn exited(data: &DataDir, status: ExitStatus) -> Error {
    let lines = CrashLog::new(data.crash_log()).last_lines(5).join("\n  ");
    Error::Other(format!("gathrd exited ({status}). Crash log:\n  {lines}"))
}

/// Ctrl+C, closing the console window on Windows, or SIGTERM on Unix.
async fn stop_signal() {
    #[cfg(windows)]
    let other = async {
        match tokio::signal::windows::ctrl_close() {
            Ok(mut close) => drop(close.recv().await),
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(unix)]
    let other = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut term) => drop(term.recv().await),
            Err(_) => std::future::pending::<()>().await,
        }
    };
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        () = other => {}
    }
}
