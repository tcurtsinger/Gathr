//! `gathrd`: Gathr's service (docs/decisions/0004-service-process.md).
// Not in the test build: a test program with no console gives each console child of its tests a
// window of its own on the desktop.
#![cfg_attr(not(test), windows_subsystem = "windows")]

mod accounts;
mod agents;
mod api;
mod app_update;
mod archive;
mod background;
mod board;
mod browser;
mod chat_state;
mod chats;
mod cleanup;
mod computer;
mod copies;
mod deletes;
mod documents;
mod files;
mod github;
mod likes;
mod logs;
mod memory;
mod mentions;
mod models;
mod naming;
mod pauses;
mod pc_tools;
mod projects;
mod questions;
mod reads;
mod routes;
mod saves;
mod scheduled;
mod search;
mod serve;
mod settings;
mod sign_in;
mod skills;
mod startup;
mod team;
mod terminals;
mod tool_sign_in;
mod tools;
mod turns;
mod ui;
mod updates;
mod view;
mod web_tools;

use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::Parser;
use gathr::app::{Copies, HandOver, STARTING};
use gathr::crash_log::CrashLog;
use gathr::data::{DataDir, ServiceInfo};
use gathr::event::Event;
use gathr::writer::Writer;
use gathr::{Error, VERSION, db, ids, launch, platform};
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
    if let Some(ended) = gathr::browser::relay() {
        return ended;
    }
    let args = Args::parse();
    let data = DataDir::new(args.data.unwrap_or_else(platform::default_data_dir));
    let crash_log = CrashLog::new(data.crash_log());
    let ui = args.ui.unwrap_or_else(|| beside_me("ui"));
    match start(&data, &crash_log, args.port, ui) {
        Ok(None) => ExitCode::SUCCESS,
        // Started apart from this one, which ends here; nothing else would start it (0032).
        Ok(Some(_)) => match launch::hand_over(&data) {
            Ok(_) => ExitCode::SUCCESS,
            Err(e) => {
                crash_log.append(&format!("handing over to the update failed: {e}"));
                ExitCode::FAILURE
            }
        },
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

/// Everything up to serving: the lock, the database, the writer and the port. Once stopped, the
/// version to hand over to, if it stopped for an update: switched to while the lock is held.
fn start(
    data: &DataDir,
    crash_log: &CrashLog,
    port: Option<u16>,
    ui: PathBuf,
) -> Result<Option<String>, Error> {
    std::fs::create_dir_all(data.path())?;
    let _lock = match launch::take_or_find(data)? {
        launch::Holder::Free(lock) => lock,
        launch::Holder::Service(_) => return Err(Error::AlreadyRunning),
    };
    let conn =
        db::open(&data.db()).map_err(|e| Error::Other(format!("the database didn't open: {e}")))?;
    let early_lines = crash_log.take();
    chat_state::watch(&conn).map_err(|e| Error::Other(format!("the database didn't open: {e}")))?;
    let live = tokio::sync::broadcast::channel(4096).0;
    let states = Arc::new(chat_state::States::new(live.clone()));
    let (moved, changed) = tokio::sync::mpsc::unbounded_channel();
    let after = chat_state::after_write(moved);
    let writer = Writer::start(conn, crash_log.clone(), after);
    logs::install(writer.clone(), crash_log.clone());
    if !early_lines.is_empty() {
        writer.record(Event::point("gathr.service.early_log").attr("lines", early_lines.clone()));
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let served = runtime.block_on(serve(
        data,
        (writer, live, states),
        changed,
        port,
        ui,
        early_lines.len(),
    ));
    let copies = Copies::new(data);
    let mut hand_over = served.as_ref().ok().cloned().flatten();
    let at = |hand_over: &mut Option<HandOver>, step| {
        if let Some(hand_over) = hand_over
            && let Err(e) = copies.hand_over_at(hand_over, step)
        {
            crash_log.append(&format!("noting the hand-over at {step} failed: {e}"));
        }
    };
    at(&mut hand_over, "ending background work");
    // Dropping a runtime waits for its blocking work for good, and on Windows reading an engine's
    // output is blocking work: after a kill, the engine's own children can hold that pipe open
    // (Codex's did, and 0.1.38's hand-over to 0.1.39 hung here).
    let began = Instant::now();
    runtime.shutdown_timeout(SHUTDOWN_WAIT);
    if began.elapsed() >= SHUTDOWN_WAIT {
        let secs = SHUTDOWN_WAIT.as_secs();
        crash_log.append(&format!(
            "work still running {secs} s after the stop ends with it"
        ));
        if let Some(hand_over) = &mut hand_over {
            hand_over.left_running = true;
        }
    }
    let update = served?.map(|h| h.to);
    if let Some(version) = &update {
        at(&mut hand_over, "switching copies");
        if let Err(e) = copies.switch_to(version) {
            // The copy that ran stays current, and the hand-over starts it again.
            crash_log.append(&format!("switching to {version} failed: {e}"));
        }
        at(&mut hand_over, STARTING);
    }
    Ok(update)
}

/// How long a stopped service waits for its blocking work before leaving it to end with it.
const SHUTDOWN_WAIT: Duration = Duration::from_secs(5);

async fn serve(
    data: &DataDir,
    (writer, live, states): (
        Writer,
        tokio::sync::broadcast::Sender<serde_json::Value>,
        Arc<chat_state::States>,
    ),
    changed: tokio::sync::mpsc::UnboundedReceiver<String>,
    port: Option<u16>,
    ui: PathBuf,
    early_lines: usize,
) -> Result<Option<HandOver>, Error> {
    let listener = bind(data, port).await?;
    let port = listener.local_addr()?.port();
    // After a hand-over, the stopped service's: what the window and the page send meanwhile isn't refused.
    let (token, session) = Copies::new(data)
        .handed_over_secrets()
        .unwrap_or_else(|| (ids::token(), ids::token()));
    // A copy on trial is marked running before it says it's up: a start that finds it up then
    // never takes it for one that died, and one that dies from here on rolls back.
    let started = Copies::new(data).started();
    ServiceInfo {
        port,
        token: token.clone(),
        pid: std::process::id(),
    }
    .write(data)?;
    // Briefings live as long as their engines; any left are from a run that didn't stop cleanly.
    let _ = std::fs::remove_dir_all(data.path().join("briefings"));
    let chats = chats::Chats::new(writer.clone(), data.clone(), port, (live, states));
    let updates = Arc::new(updates::Updates::new(data.clone(), writer.clone()));
    let codex_updates = Arc::new(updates::Updates::for_engine(
        data.clone(),
        writer.clone(),
        "codex",
    ));
    let app = Arc::new(app_update::AppUpdates::new(chats.clone()));
    let sign_ins = Arc::new(sign_in::SignIns::default());
    let state = Arc::new(api::State {
        chats: chats.clone(),
        updates: updates.clone(),
        codex_updates: codex_updates.clone(),
        app: app.clone(),
        sign_ins: sign_ins.clone(),
        browser: gathr::browser::Browser::new(data.path().join("browser"), writer.clone()),
        terminals: gathr::terminal::Terminals::load(data.path().join("terminals")),
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
        session,
        once_codes: Default::default(),
    });
    writer.record(
        Event::point("gathr.service.start")
            .ok()
            .attr("version", app.running.as_str())
            .attr("build", VERSION)
            .attr("pid", std::process::id()),
    );
    writer.record(health_event(data, port, early_lines));
    chats.pick_account().await;
    recover(&chats).await;
    // Before the queues fill: what a pause holds stays held after a restart.
    chats.pauses_changed().await;
    chats.take_over().await;
    // After the turns left waiting are taken over: a run queued now is never taken over twice.
    tokio::spawn(scheduled::every(chats.clone()));
    tokio::spawn(chat_state::run(chats.clone(), changed));
    tokio::spawn(cleanup::every(chats.clone()));
    tokio::spawn(updates::run(updates.clone(), chats.clone()));
    tokio::spawn(updates::run(codex_updates.clone(), chats.clone()));
    app.started(started);
    tokio::spawn(app_update::run(app.clone()));

    let (stopped_tx, stopped_rx) = tokio::sync::oneshot::channel();
    let (stop_state, stop_app) = (state.clone(), app.clone());
    let stopping = async move {
        let reason = tokio::select! {
            _ = stop_state.stop.notified() => "requested over the API",
            _ = os_stop_signal() => "stop signal from the system",
            _ = stop_app.handover() => "Gathr is updating",
        };
        // Before the server waits on its requests, so a model ask or sign-out doesn't hold it.
        stop_state.stopping.send_replace(true);
        let _ = stopped_tx.send(reason);
    };
    let left_open = serve::serve(listener, api::router(state.clone()), stopping).await;
    let reason = stopped_rx.await.unwrap_or("the server ended");
    if left_open > 0 {
        let event = Event::point("gathr.service.connections")
            .trace(&ids::trace_id())
            .error("open")
            .attr("open", left_open as i64)
            .attr("waited_ms", serve::FINISH_WAIT.as_millis() as i64)
            .attr(
                "why",
                "requests still under way: the stop went on without them",
            );
        writer.record(event);
    }
    updates.stop();
    codex_updates.stop();
    app.stop();
    app.stopped();
    sign_ins.cancel_all().await;
    state.terminals.stop_all();
    tokio::join!(
        chats.close_all(),
        state.models.closed(),
        state.browser.stop()
    );
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
    writer.close().await;
    Ok(app.handing_over().map(|to| HandOver {
        token: Some(state.token.clone()),
        session: Some(state.session.clone()),
        ..HandOver::new(&app.running, &to)
    }))
}

/// At start, before the turns left waiting are taken over: turns cut off by the stop carry on. When
/// that fails, they stay running for the next start, and the error has a reference.
async fn recover(chats: &chats::Chats) {
    let recovered = chats.writer.call(gathr::carry::recover).await;
    let event = Event::point("gathr.turn.recover");
    match recovered {
        Some(Ok(n)) => chats.writer.record(event.ok().attr("gathr.turn.cut", n)),
        Some(Err(e)) => chats.writer.record(
            event
                .trace(&ids::trace_id())
                .error("sqlite")
                .attr("why", e.to_string()),
        ),
        None => {}
    }
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

#[cfg(all(test, windows))]
mod tests {
    use std::io::Read;

    #[test]
    fn the_test_build_is_a_console_program_so_its_children_open_no_windows() {
        let mut head = Vec::new();
        let exe = std::fs::File::open(std::env::current_exe().unwrap()).unwrap();
        exe.take(4096).read_to_end(&mut head).unwrap();
        let pe = u32::from_le_bytes(head[0x3C..0x40].try_into().unwrap()) as usize;
        let field = pe + 4 + 20 + 68;
        let subsystem = u16::from_le_bytes(head[field..field + 2].try_into().unwrap());
        assert_eq!(
            subsystem, 3,
            "expected the test build's PE subsystem to be 3 (console), got {subsystem} (2 is GUI): \
             a GUI test build has no console, so each console program its tests start opens a window"
        );
    }
}
