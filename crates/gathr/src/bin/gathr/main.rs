//! `gathr`: Gathr's command line. `diag` reads the event record; `seed` makes test data;
//! `dev` runs the service for development.

mod dev;
mod text;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gathr::data::DataDir;
use gathr::{Error, db, diag, ids, platform, seed};
use serde_json::{Value, json};

#[derive(Parser)]
#[command(version, about = "Gathr's command line")]
struct Cli {
    /// The data folder. Default: the system's app data folder.
    #[arg(long, global = true)]
    data: Option<PathBuf>,
    /// Readable text instead of JSON.
    #[arg(long, global = true)]
    text: bool,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Diagnostics from the event record, read-only.
    #[command(subcommand)]
    Diag(Diag),
    /// Adds made-up chats: 500 messages, 50 tool calls in a turn, a 20,000-word answer.
    Seed,
    /// Starts the service on a data folder and port; Ctrl+C stops it and everything it started.
    Dev {
        #[arg(long, default_value_t = 0)]
        port: u16,
        /// Seed the data folder first.
        #[arg(long)]
        seed: bool,
    },
}

#[derive(Subcommand)]
enum Diag {
    /// The service now, the database, the last start-up check and the crash log.
    Health,
    /// Everything in one trace, from an error's reference.
    Trace { reference: String },
    /// Recent failures grouped by cause, with references.
    Failures {
        /// How far back: 30m, 24h, 7d.
        #[arg(long, default_value = "24h")]
        since: String,
    },
    /// Read-only SQL over the database, up to 1,000 rows. The `event_log` view shows events readably.
    Query { sql: String },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let data = DataDir::new(cli.data.unwrap_or_else(platform::default_data_dir));
    // A person reads `dev`; agents read the rest as JSON unless they ask for text.
    let text = cli.text || matches!(cli.command, Command::Dev { .. });
    let show = |answer: &Value| match text {
        true => text::render(answer),
        false => answer.to_string(),
    };
    let (name, result) = match cli.command {
        Command::Diag(diag) => ("diag", run_diag(&data, diag)),
        Command::Seed => ("seed", run_seed(&data)),
        Command::Dev { port, seed } => (
            "dev",
            dev::run(&data, port, seed, show).map(|()| Value::Null),
        ),
    };
    match result {
        Ok(Value::Null) => ExitCode::SUCCESS,
        Ok(answer) => {
            println!("{}", show(&answer));
            ExitCode::SUCCESS
        }
        Err(e) => {
            let answer = json!({ "error": format!("gathr {name} failed"), "why": e.to_string() });
            eprintln!("{}", show(&answer));
            ExitCode::FAILURE
        }
    }
}

fn run_diag(data: &DataDir, diag: Diag) -> Result<Value, Error> {
    if let Diag::Health = diag {
        return Ok(diag::health(data));
    }
    let (conn, _) = db::open_read_only(&data.db())?;
    match diag {
        Diag::Health => unreachable!("answered above"),
        Diag::Trace { reference } => diag::trace(&conn, &reference),
        Diag::Failures { since } => diag::failures(&conn, ids::now_us() - parse_since(&since)?),
        Diag::Query { sql } => diag::query(&conn, &sql),
    }
}

/// Seeds with the data folder's lock held, so the service can't start mid-seed.
pub fn run_seed(data: &DataDir) -> Result<Value, Error> {
    std::fs::create_dir_all(data.path())?;
    let _lock = data.take_lock().map_err(|e| match e {
        Error::AlreadyRunning => Error::Other(
            "the service is running on this data folder: stop it, or use `dev --seed`".into(),
        ),
        other => other,
    })?;
    let started = std::time::Instant::now();
    let made = seed::seed(&mut db::open(&data.db())?)?;
    Ok(json!({ "seeded": made, "ms": started.elapsed().as_millis() as u64 }))
}

/// `30m`, `24h` or `7d`, in microseconds.
fn parse_since(since: &str) -> Result<i64, Error> {
    let bad = || {
        Error::Other(format!(
            "--since {since:?}: use minutes, hours or days, like 30m, 24h or 7d"
        ))
    };
    let (number, unit) = since.split_at(since.len().saturating_sub(1));
    let number: i64 = number.parse().map_err(|_| bad())?;
    let unit_us = match unit {
        "m" => 60_000_000,
        "h" => 3_600_000_000,
        "d" => 86_400_000_000,
        _ => return Err(bad()),
    };
    Ok(number * unit_us)
}
