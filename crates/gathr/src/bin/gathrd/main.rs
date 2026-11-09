//! `gathrd`: Gathr's service (docs/decisions/0004-service-process.md).
#![windows_subsystem = "windows"]

mod accounts;
mod agents;
mod api;
mod chats;
mod coord;
mod logs;
mod models;
mod projects;
mod routes;
mod sign_in;
mod startup;
mod team;
mod tools;
mod turns;
mod ui;
mod updates;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use gathr::crash_log::CrashLog;
use gathr::data::{DataDir, ServiceInfo};
use gathr::event::Event;
use gathr::writer::Writer;
use gathr::{Error, VERSION, db, ids, platform};
use serde_json::json;
use tokio::net::TcpListener;

#[derive(Parser)]
#[command(version, about = "Gathr's service")]
struct Args {
    /// The data folder. Default: the system's app data folder.
    #[arg(long)]
    data: Option<PathBuf>,
    /// The port on 127.0.0.1. 0 picks a free one. Default: the port of the last start.
    #[arg(long)]
    port: Option<u16>,
    /// The built UI (`ui/dist`). Default: the `ui` folder beside this program.
    #[arg(long)]
    ui: Option<PathBuf>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let data = DataDir::new(args.data.unwrap_or_else(platform::default_data_dir));
    let crash_log = CrashLog::new(data.crash_log());
    let ui = args.ui.unwrap_or_else(|| beside_me("ui"));
    match start(&data, &crash_log, args.port, ui) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            crash_log.append(&format!(
                "gathrd {VERSION} pid {} on {}",
                std::process::id(),
                data.path().display()
            ));
            crash_log.append(&format!("start failed: {e}"));
            ExitCode::FAILURE
        }
    }
}

fn beside_me(name: &str) -> PathBuf {
    let me = std::env::current_exe().unwrap_or_default();
    me.with_file_name(name)
}

/// Everything up to serving: the lock, the database, the writer and the port.
fn start(
    data: &DataDir,
    crash_log: &CrashLog,
    port: Option<u16>,
    ui: PathBuf,
) -> Result<(), Error> {
    std::fs::create_dir_all(data.path())?;
    let _lock = data.take_lock()?;
    let conn =
        db::open(&data.db()).map_err(|e| Error::Other(format!("the database didn't open: {e}")))?;
    let early_lines = crash_log.take();
    gathr::tasks::touched::watch(&conn)
        .map_err(|e| Error::Other(format!("the database didn't open: {e}")))?;
    let live = tokio::sync::broadcast::channel(4096).0;
    let writer = Writer::start(conn, crash_log.clone(), coord::live_tasks(live.clone()));
    logs::install(writer.clone(), crash_log.clone());
    if !early_lines.is_empty() {
        writer.record(Event::point("gathr.service.early_log").attr("lines", early_lines.clone()));
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(data, (writer, live), port, ui, early_lines.len()))
}

async fn serve(
    data: &DataDir,
    (writer, live): (Writer, tokio::sync::broadcast::Sender<serde_json::Value>),
    port: Option<u16>,
    ui: PathBuf,
    early_lines: usize,
) -> Result<(), Error> {
    let listener = bind(data, port).await?;
    let port = listener.local_addr()?.port();
    let token = ids::token();
    ServiceInfo {
        port,
        token: token.clone(),
        pid: std::process::id(),
    }
    .write(data)?;
    // Briefings live as long as their engines; any left are from a run that didn't stop cleanly.
    let _ = std::fs::remove_dir_all(data.path().join("briefings"));
    let chats = chats::Chats::new(writer.clone(), data.clone(), port, live);
    let updates = Arc::new(updates::Updates::new(data.clone(), writer.clone()));
    let sign_ins = Arc::new(sign_in::SignIns::default());
    let state = Arc::new(api::State {
        chats: chats.clone(),
        updates: updates.clone(),
        sign_ins: sign_ins.clone(),
        models: Default::default(),
        stopping: Default::default(),
        writer: writer.clone(),
        token,
        port,
        pid: std::process::id(),
        started_us: ids::now_us(),
        data: data.clone(),
        stop: tokio::sync::Notify::new(),
        ui,
        session: ids::token(),
        once_codes: Default::default(),
    });
    writer.record(
        Event::point("gathr.service.start")
            .ok()
            .attr("version", VERSION)
            .attr("pid", std::process::id()),
    );
    writer.record(health_event(data, port, early_lines));
    chats.pick_account().await;
    coord::recover(&chats).await;
    chats.take_over().await;
    tokio::spawn(coord::run(chats.clone()));
    tokio::spawn(updates::run(updates.clone()));

    let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
    let stop_state = state.clone();
    let stopping = async move {
        let reason = tokio::select! {
            _ = stop_state.stop.notified() => "requested over the API",
            _ = os_stop_signal() => "stop signal from the system",
        };
        // Before the server waits on its requests, so a model ask or sign-out doesn't hold it.
        stop_state.stopping.send_replace(true);
        let _ = stopped_tx.send(reason);
    };
    axum::serve(listener, api::router(state.clone()))
        .with_graceful_shutdown(stopping)
        .await?;
    let reason = stopped_rx.await.unwrap_or("the server ended");
    updates.stop();
    sign_ins.cancel_all().await;
    tokio::join!(chats.close_all(), state.models.closed());
    writer
        .record_now(
            Event::point("gathr.service.stop")
                .ok()
                .attr("reason", reason),
        )
        .await;
    // Move the WAL into gathr.db, so the stopped data folder is whole in that one file.
    let checkpoint = "PRAGMA wal_checkpoint(TRUNCATE)";
    let checkpointed = writer
        .call(|conn| conn.execute_batch(checkpoint).map_err(|e| e.to_string()))
        .await;
    if let Some(Err(why)) = checkpointed {
        let event = Event::point("gathr.service.checkpoint")
            .error("sqlite")
            .attr("why", why);
        writer.record_now(event).await;
    }
    Ok(())
}

/// The room's dev services and their UIs (AGENTS.md): a service with no port asked never keeps one
/// of these, so the installed Gathr can't take a member's.
const DEV_PORTS: std::ops::RangeInclusive<u16> = 47100..=47114;

/// Binds the asked port; with none asked, the last start's port if it's still free, else any free one.
async fn bind(data: &DataDir, port: Option<u16>) -> Result<TcpListener, Error> {
    let at = |port| SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    if let Some(port) = port {
        return Ok(TcpListener::bind(at(port)).await?);
    }
    if let Ok(last) = ServiceInfo::read(data)
        && !DEV_PORTS.contains(&last.port)
        && let Ok(listener) = TcpListener::bind(at(last.port)).await
    {
        return Ok(listener);
    }
    Ok(TcpListener::bind(at(0)).await?)
}

fn health_event(data: &DataDir, port: u16, early_lines: usize) -> Event {
    let size = std::fs::metadata(data.db()).map(|m| m.len()).unwrap_or(0);
    Event::point("gathr.health").ok().attr(
        "found",
        json!({
            "version": VERSION,
            "data_folder": data.path(),
            "database_bytes": size,
            "schema_version": db::SCHEMA_VERSION,
            "port": port,
            "crash_log_lines_copied": early_lines,
        }),
    )
}

/// Ctrl+C, or SIGTERM on Unix. Without a console (`windows_subsystem`) Ctrl+C can't arrive,
/// so a failed handler waits forever instead of stopping the service at once.
async fn os_stop_signal() {
    let ctrl_c = async {
        if tokio::signal::ctrl_c().await.is_err() {
            std::future::pending::<()>().await;
        }
    };
    #[cfg(unix)]
    let term = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => drop(signal.recv().await),
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {}
        _ = term => {}
    }
}
