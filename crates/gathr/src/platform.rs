//! What differs between Windows and macOS (docs/decisions/0016-macos.md).

use std::io;
use std::path::{Path, PathBuf};

/// Where Gathr keeps its data when no `--data` is given.
pub fn default_data_dir() -> PathBuf {
    if cfg!(windows) {
        let appdata = std::env::var_os("APPDATA").unwrap_or_else(|| ".".into());
        return PathBuf::from(appdata).join("Gathr");
    }
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_else(|| ".".into()));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/Gathr")
    } else {
        home.join(".local/share/gathr")
    }
}

/// Writes a file only this user may read: mode 0600 on Unix. On Windows the data folder's
/// own permissions apply (`%APPDATA%` is the user's).
pub fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    io::Write::write_all(&mut options.open(path)?, bytes)
}
