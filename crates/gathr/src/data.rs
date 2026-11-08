//! The data folder's layout and `service.json`, the file that says where the service listens.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{Error, platform};

/// A Gathr data folder.
#[derive(Debug, Clone)]
pub struct DataDir(PathBuf);

impl DataDir {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        DataDir(path.into())
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn db(&self) -> PathBuf {
        self.0.join("gathr.db")
    }

    pub fn lock(&self) -> PathBuf {
        self.0.join("gathrd.lock")
    }

    pub fn service_file(&self) -> PathBuf {
        self.0.join("service.json")
    }

    pub fn crash_log(&self) -> PathBuf {
        self.0.join("crash.log")
    }

    /// Takes the data folder's lock: one service, or one seed, at a time.
    /// The lock lasts as long as the returned file is open.
    pub fn take_lock(&self) -> Result<std::fs::File, Error> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.lock())?;
        match file.try_lock() {
            Ok(()) => Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => Err(Error::AlreadyRunning),
            Err(std::fs::TryLockError::Error(e)) => Err(e.into()),
        }
    }
}

/// Where the running service listens, with the token every request needs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

impl ServiceInfo {
    pub fn read(data: &DataDir) -> Result<ServiceInfo, Error> {
        Ok(serde_json::from_slice(&std::fs::read(
            data.service_file(),
        )?)?)
    }

    pub fn write(&self, data: &DataDir) -> Result<(), Error> {
        Ok(platform::write_private(
            &data.service_file(),
            &serde_json::to_vec_pretty(self)?,
        )?)
    }
}
