//! What differs between Windows and macOS (docs/decisions/0016-macos.md).

use std::io;
use std::path::{Path, PathBuf};

pub mod env;
pub mod login;

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
    #[cfg(target_os = "macos")]
    return in_path(std::ffi::OsStr::new(&env::login_path().ok()?));
    #[cfg(not(target_os = "macos"))]
    None
}

/// Starts a child with no console window on Windows, never detached (0001); on Unix in its
/// own process group (0016).
pub fn no_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(not(windows))]
    std::os::unix::process::CommandExt::process_group(command, 0);
}

/// Ends `program`, started with `no_window` as `pid`, with everything it started, and returns once
/// they're gone. On Unix that's its process group. On Windows it's the tree of processes below it,
/// read before it's killed; but an MSYS program's fork and exec leave children whose Windows
/// parent is gone, so for one (Git Bash) its own `kill` ends its process group first.
pub fn end_tree(pid: u32, program: &Path) {
    #[cfg(windows)]
    {
        let msys = program.with_file_name("msys-2.0.dll").is_file();
        let bash = program.with_file_name("bash.exe");
        if msys && bash.is_file() {
            // A process started from outside MSYS leads its own group, under its MSYS pid. Only
            // built-ins: the service's PATH may not have Git's usr\bin.
            let group = r#"for p in /proc/[0-9]*; do
                w=; read -r w 2>/dev/null < "$p/winpid"
                [ "$w" = "$1" ] && { kill -s KILL -- "-${p#/proc/}"; exit; }
            done; exit 1"#;
            let mut command = std::process::Command::new(bash);
            command.args(["-c", group, "end-tree", &pid.to_string()]);
            quietly(command);
        }
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let mut command =
            std::process::Command::new(PathBuf::from(root).join(r"System32\taskkill.exe"));
        command.args(["/T", "/F", "/PID", &pid.to_string()]);
        quietly(command);
    }
    #[cfg(not(windows))]
    {
        let _ = program;
        let mut command = std::process::Command::new("/bin/kill");
        command.args(["-KILL", "--", &format!("-{pid}")]);
        quietly(command);
    }
}

/// Runs a command to its end with no input, output or window.
fn quietly(mut command: std::process::Command) {
    no_window(&mut command);
    let _ = command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

/// This computer in Claude Code's release feed, such as `win32-x64`; none where it ships nothing.
pub fn release_target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Some("win32-x64"),
        ("windows", "aarch64") => Some("win32-arm64"),
        ("macos", "aarch64") => Some("darwin-arm64"),
        ("macos", "x86_64") => Some("darwin-x64"),
        _ => None,
    }
}

/// The system's own curl, by its full path so the PATH can't swap it: Windows 10 and later and
/// macOS both ship it (0025).
pub fn curl() -> PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        return PathBuf::from(root).join(r"System32\curl.exe");
    }
    PathBuf::from("/usr/bin/curl")
}

/// Lets a downloaded program run: mode 0755 on Unix. Windows needs nothing.
pub fn make_executable(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))?;
    }
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
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
