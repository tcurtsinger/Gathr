//! What the service (`gathrd`), the command line (`gathr`) and the window share: the data folder,
//! the database, the event record, error references and the diagnostics queries.

pub mod accounts;
pub mod agents;
pub mod app;
pub mod approvals;
pub mod archive;
pub mod background;
pub mod check_runs;
pub mod checks;
pub mod claude;
pub mod codex;
pub mod context;
pub mod crash_log;
pub mod data;
pub mod db;
pub mod diag;
pub mod error;
pub mod event;
pub mod fetch;
pub mod folders;
pub mod handoffs;
pub mod http;
pub mod ids;
pub mod launch;
pub mod pauses;
pub mod platform;
pub mod projects;
pub mod pushes;
pub mod redact;
pub mod run;
pub mod search;
pub mod seed;
pub mod store;
pub mod tasks;
pub mod team;
pub mod timeline;
pub mod waits;
pub mod worktree;
pub mod writer;

pub use error::Error;

/// The version of this build, from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
