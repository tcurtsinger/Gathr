use std::fmt;

/// The one error type of this crate.
#[derive(Debug)]
pub enum Error {
    Db(rusqlite::Error),
    Io(std::io::Error),
    /// The database was written by a newer Gathr than this one.
    NewerDatabase {
        found: i64,
        known: i64,
    },
    /// Another `gathrd` holds the data folder's lock.
    AlreadyRunning,
    /// A change the rules turn down, and why, in words for whoever asked (S68).
    Refused(String),
    Other(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Db(e) => write!(f, "{e}"),
            Error::Io(e) => write!(f, "{e}"),
            Error::NewerDatabase { found, known } => {
                write!(f, "schema {found} requires a newer build's {known}")
            }
            Error::AlreadyRunning => write!(f, "data folder in use by gathrd"),
            Error::Refused(why) | Error::Other(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Db(e)
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Other(e.to_string())
    }
}
