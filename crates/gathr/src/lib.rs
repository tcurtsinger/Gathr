//! What the service (`gathrd`), the command line (`gathr`) and the window share: the data folder,
//! the database, the event record, error references and the diagnostics queries.

pub mod accounts;
pub mod agents;
pub mod claude;
pub mod crash_log;
pub mod data;
pub mod db;
pub mod diag;
pub mod error;
pub mod event;
pub mod http;
pub mod ids;
pub mod launch;
pub mod platform;
pub mod redact;
pub mod seed;
pub mod store;
pub mod team;
pub mod timeline;
pub mod worktree;
pub mod writer;

pub use error::Error;

/// The version of this build, from Cargo.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
