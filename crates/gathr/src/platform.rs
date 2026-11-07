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

/// A program on the PATH: `name.exe` on Windows. On macOS, an app started from the Dock gets a
/// short PATH, so the login shell's PATH is asked too (0016).
pub fn find_on_path(name: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let in_path = |path: &std::ffi::OsStr| {
        std::env::split_paths(path)
            .map(|dir| dir.join(&file))
            .find(|p| p.is_file())
    };
    if let Some(found) = std::env::var_os("PATH").and_then(|p| in_path(&p)) {
        return Some(found);
    }
    if !cfg!(target_os = "macos") {
        return None;
    }
    let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/zsh".into());
    let output = std::process::Command::new(shell)
        .args(["-lc", "printf %s \"$PATH\""])
        .output()
        .ok()?;
    in_path(std::ffi::OsStr::new(
        &String::from_utf8_lossy(&output.stdout).into_owned(),
    ))
}

/// Starts a child with no console window on Windows, never detached (0001); on Unix in its
/// own process group (0016).
pub fn no_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(not(windows))]
    std::os::unix::process::CommandExt::process_group(command, 0);
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
