//! `gathrd`: Gathr's service (docs/decisions/0004-service-process.md).
#![windows_subsystem = "windows"]

mod api;
mod chats;
mod logs;
mod routes;
mod sign_in;
mod turns;
mod updates;

use std::net::{Ipv4Addr, SocketAddr};
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
    data: Option<std::path::PathBuf>,
    /// The port on 127.0.0.1. 0 picks a free one. Default: the port of the last start.
    #[arg(long)]
    port: Option<u16>,
}

fn main() -> ExitCode {
    let args = Args::parse();
    let data = DataDir::new(args.data.unwrap_or_else(platform::default_data_dir));
    let crash_log = CrashLog::new(data.crash_log());
    match start(&data, &crash_log, args.port) {
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

/// Everything up to serving: the lock, the database, the writer and the port.
fn start(data: &DataDir, crash_log: &CrashLog, port: Option<u16>) -> Result<(), Error> {
    std::fs::create_dir_all(data.path())?;
    let _lock = data.take_lock()?;
    let conn =
        db::open(&data.db()).map_err(|e| Error::Other(format!("the database didn't open: {e}")))?;
    let early_lines = crash_log.take();
    let writer = Writer::start(conn, crash_log.clone());
    logs::install(writer.clone(), crash_log.clone());
    if !early_lines.is_empty() {
        writer.record(Event::point("gathr.service.early_log").attr("lines", early_lines.clone()));
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(serve(data, writer, port, early_lines.len()))
}

async fn serve(
    data: &DataDir,
    writer: Writer,
    port: Option<u16>,
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
    let chats = chats::Chats::new(writer.clone(), data.clone());
    let updates = Arc::new(updates::Updates::new(data.clone(), writer.clone()));
    let sign_in = Arc::new(sign_in::SignIn::default());
    let state = Arc::new(api::State {
        chats: chats.clone(),
        updates: updates.clone(),
        sign_in: sign_in.clone(),
        writer: writer.clone(),
        token,
        port,
        pid: std::process::id(),
        started_us: ids::now_us(),
        data: data.clone(),
        stop: tokio::sync::Notify::new(),
    });
    writer.record(
        Event::point("gathr.service.start")
            .ok()
            .attr("version", VERSION)
            .attr("pid", std::process::id()),
    );
    writer.record(health_event(data, port, early_lines));
    chats.take_over().await;
    tokio::spawn(updates::run(updates.clone()));

    let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
    let stop_state = state.clone();
    let stopping = async move {
        let reason = tokio::select! {
            _ = stop_state.stop.notified() => "requested over the API",
            _ = os_stop_signal() => "stop signal from the system",
        };
        let _ = stopped_tx.send(reason);
    };
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(stopping)
        .await?;
    let reason = stopped_rx.await.unwrap_or("the server ended");
    updates.stop();
    sign_in.cancel().await;
    chats.close_all().await;
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

/// Binds the asked port; with none asked, the last start's port if it's still free, else any free one.
async fn bind(data: &DataDir, port: Option<u16>) -> Result<TcpListener, Error> {
    let at = |port| SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    if let Some(port) = port {
        return Ok(TcpListener::bind(at(port)).await?);
    }
    if let Ok(last) = ServiceInfo::read(data)
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
