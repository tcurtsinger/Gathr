//! How the window and `gathr open` reach the service (0004): the one already running on the data
//! folder, else a `gathrd` they start, which outlives them.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
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
    // Before finding a running service too: a window that updated itself, or moved, comes back to
    // the service it left running.
    Copies::new(data).note_installed(installed)?;
    match take_or_find(data)? {
        Holder::Free(lock) => drop(lock),
        Holder::Service(info) => {
            return Ok(Found {
                info,
                started: false,
            });
        }
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
/// before if it fails to start. Every start first goes back from a copy that died on trial.
pub fn hand_over(data: &DataDir) -> Result<Found, Error> {
    start_current(data, Env::Kept)
}

fn start_current(data: &DataDir, env: Env) -> Result<Found, Error> {
    let copies = Copies::new(data);
    let died = locked(data, || {
        if copies.died_on_trial() {
            copies.roll_back("exited on trial")
        } else {
            Ok(None)
        }
    })?;
    match died {
        Locked::Running(found) => return Ok(found),
        Locked::Done(Some(previous)) => return start(data, &previous, env),
        Locked::Done(None) => {}
    }
    let Some(program) = copies.program() else {
        let why = format!("{} names no copy of gathrd", copies.root().display());
        return Err(Error::Other(why));
    };
    let failed = match start(data, &program, env) {
        Ok(found) => return Ok(found),
        Err(e) => e,
    };
    match locked(data, || copies.roll_back(&failed.to_string()))? {
        Locked::Running(found) => Ok(found),
        Locked::Done(Some(previous)) => start(data, &previous, env),
        Locked::Done(None) => Err(failed),
    }
}

enum Locked<T> {
    Done(T),
    Running(Found),
}

/// Runs `change` holding the folder's lock, so a service another start just began (on trial and
/// running) is never taken for one that died; while one holds the lock, it's the one found.
fn locked<T>(data: &DataDir, change: impl FnOnce() -> io::Result<T>) -> Result<Locked<T>, Error> {
    let lock = match take_or_find(data)? {
        Holder::Free(lock) => lock,
        Holder::Service(info) => {
            return Ok(Locked::Running(Found {
                info,
                started: false,
            }));
        }
    };
    let done = change()?;
    drop(lock);
    Ok(Locked::Done(done))
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
            // Another start may have taken the folder first: then its service answers.
            if let Ok(Holder::Service(info)) = take_or_find(data) {
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

/// The data folder's lock, or the service holding it.
pub enum Holder {
    Free(std::fs::File),
    Service(ServiceInfo),
}

/// The folder's lock, else the service holding it once it answers `/health`. A lock held with no
/// service behind it is waited for, never taken for one: on macOS, a process another thread is
/// starting keeps a copy of the lock file open until its program runs (CI saw it, 2026-11-11).
pub fn take_or_find(data: &DataDir) -> Result<Holder, Error> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match data.take_lock() {
            Ok(lock) => return Ok(Holder::Free(lock)),
            Err(Error::AlreadyRunning) => {}
            Err(e) => return Err(e),
        }
        if let Ok(info) = ServiceInfo::read(data)
            && http::call(info.port, &info.token, "GET", "/health").is_ok_and(|r| r.status == 200)
        {
            return Ok(Holder::Service(info));
        }
        if Instant::now() > deadline {
            return Err(Error::Other(not_answering(data)));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Why the service holding the folder doesn't answer: stopped part way through an update, or not.
fn not_answering(data: &DataDir) -> String {
    let stuck = ServiceInfo::read(data)
        .ok()
        .and_then(|info| Copies::new(data).hand_over_by(info.pid));
    match stuck {
        Some(h) => format!(
            "Gathr {} stopped to update to {} but hasn't exited: process {} is stuck {} since {}. \
             End that process, then open Gathr again.",
            h.from,
            h.to,
            h.pid,
            h.step,
            crate::ids::utc(h.since_us)
        ),
        None => "the service holds the data folder but didn't answer within 15 s".into(),
    }
}

/// A link that signs a browser or the window in once (`/?once=<code>`).
pub fn one_time_link(info: &ServiceInfo) -> Result<String, Error> {
    let answer = http::call(info.port, &info.token, "POST", "/api/once")?;
    match answer.json()["code"].as_str() {
        Some(code) => Ok(format!("http://127.0.0.1:{}/?once={code}", info.port)),
        None => Err(Error::Other(format!(
            "The service returned no sign-in code ({}): {}",
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

    /// The window names a service stuck in its hand-over, and only the one holding the folder.
    #[test]
    fn a_service_stuck_mid_update_is_named_as_one() {
        use crate::app::{Copies, HandOver};
        use crate::data::{DataDir, ServiceInfo};
        let root = std::env::temp_dir().join(format!("gathr-stuck-{}", std::process::id()));
        let data = DataDir::new(&root);
        let info = |pid| ServiceInfo {
            port: 1,
            token: "made-up".into(),
            pid,
        };
        let mut hand_over = HandOver::new("0.1.38", "0.1.39");
        Copies::new(&data)
            .hand_over_at(&mut hand_over, "ending background work")
            .unwrap();
        info(hand_over.pid + 1).write(&data).unwrap();
        let other = super::not_answering(&data);
        info(hand_over.pid).write(&data).unwrap();
        let stuck = super::not_answering(&data);
        std::fs::remove_dir_all(&root).unwrap();
        assert!(other.contains("didn't answer"), "{other}");
        let named = format!("process {} is stuck ending background work", hand_over.pid);
        assert!(
            stuck.contains("0.1.38 stopped to update to 0.1.39"),
            "{stuck}"
        );
        assert!(stuck.contains(&named), "{stuck}");
    }
}
