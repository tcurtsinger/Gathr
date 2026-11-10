//! What differs between Windows and macOS (docs/decisions/0016-macos.md).

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

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

/// The shell a check runs in, with its environment: `sh`, or on Windows the Git Bash that Claude
/// Code's Bash tool uses. Git's `bin\bash.exe` only starts `usr\bin\bash.exe`, and killing it
/// leaves that one running the check: Gathr starts it itself, with what the launcher adds.
pub fn shell(vars: &env::Vars) -> Option<(PathBuf, env::Vars)> {
    if !cfg!(windows) {
        return Some((PathBuf::from("/bin/sh"), vars.clone()));
    }
    let at = vars
        .iter()
        .position(|(k, _)| k.eq_ignore_ascii_case("PATH"))?;
    let git = std::env::split_paths(&vars[at].1)
        .map(|dir| dir.join("git.exe"))
        .find(|p| p.is_file())?;
    // git.exe is in Git's cmd, bin or mingw64\bin.
    let top = git
        .ancestors()
        .skip(1)
        .take(3)
        .find(|dir| dir.join("usr").join("bin").join("bash.exe").is_file())?;
    let mut vars = vars.clone();
    let folders = [top.join("mingw64").join("bin"), top.join("usr").join("bin")];
    let path = std::env::split_paths(&vars[at].1);
    vars[at].1 = std::env::join_paths(folders.into_iter().chain(path)).ok()?;
    if !vars.iter().any(|(k, _)| k.eq_ignore_ascii_case("MSYSTEM")) {
        vars.push(("MSYSTEM".into(), "MINGW64".into()));
    }
    Some((top.join("usr").join("bin").join("bash.exe"), vars))
}

/// Starts a child with no console window on Windows, never detached (0001); on Unix in its
/// own process group (0016).
pub fn no_window(command: &mut std::process::Command) {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0000); // CREATE_NO_WINDOW
    #[cfg(not(windows))]
    std::os::unix::process::CommandExt::process_group(command, 0);
}

/// Opens a web page in the person's browser. It must outlive Gathr, so it never goes in the
/// child-ending job used for git and checks. BROWSER names a stand-in executable in tests.
pub fn open_browser(url: &str) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("Only HTTP and HTTPS links can open in the browser".into());
    }
    let mut command = match std::env::var_os("BROWSER").filter(|v| !v.is_empty()) {
        Some(stand_in) => std::process::Command::new(stand_in),
        None if cfg!(windows) => {
            // The shell's own handler for links, as Start-Process uses. Never explorer.exe: it
            // opens the Documents folder for a link instead of the browser (1281).
            let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
            let mut c = std::process::Command::new(
                PathBuf::from(root).join("System32").join("rundll32.exe"),
            );
            c.arg("url.dll,FileProtocolHandler");
            c
        }
        None => std::process::Command::new("/usr/bin/open"),
    };
    command
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("The browser didn't open: {e}"))?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

/// Starts a short program so that ending it ends everything it started (1219): in a Job Object on
/// Windows, which takes in each child as it's created, even one Git Bash was starting when it was
/// ended, before MSYS or any process tree lists it; in its own process group on Unix, whose id stays
/// the program's until it's waited for. Engines never start this way: they close the gentle way.
pub fn ending_together(command: tokio::process::Command) -> process_wrap::tokio::CommandWrap {
    let mut wrapped = process_wrap::tokio::CommandWrap::from(command);
    #[cfg(windows)]
    {
        use windows::Win32::System::Threading::CREATE_NO_WINDOW;
        wrapped
            .wrap(process_wrap::tokio::CreationFlags(CREATE_NO_WINDOW))
            .wrap(process_wrap::tokio::JobObject);
    }
    #[cfg(unix)]
    wrapped.wrap(process_wrap::tokio::ProcessGroup::leader());
    wrapped
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

/// The system's own tar (bsdtar on both), by its full path, for Gathr's release archives (0032).
pub fn tar() -> PathBuf {
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        return PathBuf::from(root).join(r"System32\tar.exe");
    }
    PathBuf::from("/usr/bin/tar")
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

/// How long a folder's move waits for its files to be let go.
const MOVE_WAIT: Duration = Duration::from_secs(10);

/// Moves a folder by a rename. Windows refuses to rename a folder while any file in it is open,
/// whatever the sharing ("Access is denied"), and a program just written or run is read for a
/// moment after (an antivirus scan, the indexer): the move waits up to `MOVE_WAIT` for that. Its
/// error names the folder and how long it waited.
pub fn move_folder(from: &Path, to: &Path) -> io::Result<()> {
    let began = Instant::now();
    loop {
        let e = match std::fs::rename(from, to) {
            Ok(()) => return Ok(()),
            Err(e) => e,
        };
        let held = cfg!(windows) && e.kind() == io::ErrorKind::PermissionDenied;
        if !held || began.elapsed() >= MOVE_WAIT {
            let (from, to, waited) = (from.display(), to.display(), began.elapsed().as_millis());
            let why = format!("{from} wasn't moved to {to} (waited {waited} ms): {e}");
            return Err(io::Error::new(e.kind(), why));
        }
        std::thread::sleep(Duration::from_millis(50));
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

#[cfg(test)]
mod tests {
    use super::*;

    /// An update failed on Windows CI with "Access is denied": something still read the new
    /// `gathrd` as its folder moved into place.
    #[test]
    fn a_folder_moves_once_a_file_held_open_in_it_is_let_go() {
        let root = std::env::temp_dir().join(format!("gathr-move-{}", crate::ids::span_id()));
        let (from, to) = (root.join("unpacked"), root.join("0.2.0"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::write(from.join("gathrd"), "new").unwrap();
        let held = std::fs::File::open(from.join("gathrd")).unwrap();
        let reader = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(held);
        });

        let moved = move_folder(&from, &to);
        reader.join().unwrap();
        let landed = std::fs::read_to_string(to.join("gathrd"));
        let _ = std::fs::remove_dir_all(&root);
        moved.unwrap();
        assert_eq!(landed.unwrap(), "new");
    }
}
