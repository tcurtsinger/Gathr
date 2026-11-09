//! How the window and `gathr open` reach the service (0004): the one already running on the data
//! folder, else a `gathrd` they start, which outlives them.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::app::Copies;
use crate::crash_log::CrashLog;
use crate::data::{DataDir, ServiceInfo};
use crate::{Error, http, platform};

/// The service, and whether this call started it.
pub struct Found {
    pub info: ServiceInfo,
    pub started: bool,
}

/// How long a started `gathrd` has to write `service.json`: a first start after an update may
/// migrate a large database.
const START_WAIT: Duration = Duration::from_secs(60);

/// Where a service started here gets its environment from.
#[derive(Clone, Copy)]
enum Env {
    /// The caller's, without dev variables: the window's and `gathr open`'s.
    Clean,
    /// The caller's as it is: a service handing over to its update runs as it ran.
    Kept,
}

/// The service on `data`, else the copy of `gathrd` its folder names, started (`installed` is
/// copied in first when the folder has none, or an older one: 0032).
pub fn find_or_start(data: &DataDir, installed: &Path) -> Result<Found, Error> {
    std::fs::create_dir_all(data.path())?;
    match data.take_lock() {
        Ok(lock) => drop(lock),
        Err(Error::AlreadyRunning) => {
            let info = running(data)?;
            return Ok(Found {
                info,
                started: false,
            });
        }
        Err(e) => return Err(e),
    }
    Copies::new(data).adopt(installed).map_err(|e| {
        let into = Copies::new(data).root().display().to_string();
        Error::Other(format!(
            "copying {} into {into} failed: {e}",
            installed.display()
        ))
    })?;
    start_current(data, Env::Clean)
}

/// After an update, a stopped service starts the copy now current, which goes back to the one
/// before if it fails to start.
pub fn hand_over(data: &DataDir) -> Result<Found, Error> {
    start_current(data, Env::Kept)
}

fn start_current(data: &DataDir, env: Env) -> Result<Found, Error> {
    let copies = Copies::new(data);
    let Some(program) = copies.program() else {
        let why = format!("{} names no copy of gathrd", copies.root().display());
        return Err(Error::Other(why));
    };
    let failed = match start(data, &program, env) {
        Ok(found) => return Ok(found),
        Err(e) => e,
    };
    match copies.roll_back(&failed.to_string())? {
        Some(previous) => start(data, &previous, env),
        None => Err(failed),
    }
}

fn start(data: &DataDir, gathrd: &Path, env: Env) -> Result<Found, Error> {
    let mut command = Command::new(gathrd);
    command.arg("--data").arg(data.path());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Dev variables never reach a service started here, so an agent's shell can't point the
    // person's own Gathr at the replay engine (as SerenityOS's server-env.mjs strips its own).
    for (name, _) in std::env::vars_os() {
        let dev = name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GATHR_");
        if dev && matches!(env, Env::Clean) {
            command.env_remove(name);
        }
    }
    platform::no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| Error::Other(format!("starting {} failed: {e}", gathrd.display())))?;
    let pid = child.id();
    let deadline = Instant::now() + START_WAIT;
    loop {
        if let Ok(info) = ServiceInfo::read(data)
            && info.pid == pid
        {
            // Reaped when it exits, so it never lingers as a zombie under a long-lived window.
            std::thread::spawn(move || child.wait());
            return Ok(Found {
                info,
                started: true,
            });
        }
        if let Some(status) = child.try_wait()? {
            // Another start may have taken the folder first: then its lock is held.
            if let Err(Error::AlreadyRunning) = data.take_lock() {
                let info = running(data)?;
                return Ok(Found {
                    info,
                    started: false,
                });
            }
            let lines = CrashLog::new(data.crash_log()).last_lines(3).join("; ");
            return Err(Error::Other(format!("gathrd exited ({status}): {lines}")));
        }
        if Instant::now() > deadline {
            // Before `service.json` it has started no engine, so ending it loses nothing.
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::Other(format!(
                "gathrd pid {pid} didn't start within {} s",
                START_WAIT.as_secs()
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The service holding the folder's lock, once it answers `/health`.
fn running(data: &DataDir) -> Result<ServiceInfo, Error> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        if let Ok(info) = ServiceInfo::read(data)
            && http::call(info.port, &info.token, "GET", "/health").is_ok_and(|r| r.status == 200)
        {
            return Ok(info);
        }
        if Instant::now() > deadline {
            return Err(Error::Other(
                "the service holds the data folder but didn't answer within 15 s".into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// A link that signs a browser or the window in once (`/?once=<code>`).
pub fn one_time_link(info: &ServiceInfo) -> Result<String, Error> {
    let answer = http::call(info.port, &info.token, "POST", "/api/once")?;
    match answer.json()["code"].as_str() {
        Some(code) => Ok(format!("http://127.0.0.1:{}/?once={code}", info.port)),
        None => Err(Error::Other(format!(
            "the service gave no sign-in code ({}): {}",
            answer.status, answer.body
        ))),
    }
}

/// `gathrd`, or another of Gathr's programs, in the folder this program runs from.
pub fn beside_me(program: &str) -> Result<std::path::PathBuf, Error> {
    let name = format!("{program}{}", std::env::consts::EXE_SUFFIX);
    Ok(std::env::current_exe()?.with_file_name(name))
}

/// The key for a data folder other than the person's own (`None` for theirs): the window's
/// one-instance key, and the name of its entry at login, so a test folder never stands in for
/// theirs. Every caller runs the same build, so the hasher's output matches between them.
pub fn folder_key(data: &Path, default: &Path) -> Option<String> {
    let data = identity(data);
    if identity(default) == data {
        return None;
    }
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    Some(format!(".data-{:016x}", hasher.finish()))
}

/// A folder as the file system knows it: links and `..` resolved, and case ignored, as Windows'
/// and macOS's file systems do by default. A folder that doesn't exist yet is only made absolute,
/// so the caller creates it first.
fn identity(path: &Path) -> String {
    let path = std::fs::canonicalize(path)
        .or_else(|_| std::path::absolute(path))
        .unwrap_or_else(|_| path.to_path_buf());
    let path: PathBuf = path.components().collect();
    path.to_string_lossy().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::folder_key;
    use std::path::{Path, PathBuf};

    #[test]
    fn each_data_folder_gets_its_own_window_and_the_persons_own_keeps_the_plain_one() {
        let own = Path::new("/home/someone/Gathr");
        let a = folder_key(Path::new("/work/data/a"), own);
        assert_eq!(folder_key(own, own), None, "the person's own folder");
        assert!(a.is_some(), "a test folder has a key");
        assert_ne!(a, folder_key(Path::new("/work/data/b"), own), "two folders");
        assert_eq!(
            a,
            folder_key(Path::new("/work/data/a/"), own),
            "the same folder, spelt with a /"
        );
    }

    // Windows' and macOS's file systems ignore case by default, so `X:\CODE` is `X:\code`.
    #[test]
    fn one_folder_spelt_another_way_gets_the_same_window() {
        let root = std::env::temp_dir().join(format!("gathr-key-{}", std::process::id()));
        let folder = root.join("Data");
        std::fs::create_dir_all(&folder).unwrap();
        let own = Path::new("/home/someone/Gathr");
        let key = folder_key(&folder, own);
        let upper = PathBuf::from(folder.to_string_lossy().to_uppercase());
        let dotted = root.join("Data").join("..").join("Data");
        let (by_upper, by_dots) = (folder_key(&upper, own), folder_key(&dotted, own));
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            key,
            by_upper,
            "{} and {}",
            folder.display(),
            upper.display()
        );
        assert_eq!(
            key,
            by_dots,
            "{} and {}",
            folder.display(),
            dotted.display()
        );
    }
}
