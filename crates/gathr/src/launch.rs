//! How the window and `gathr open` reach the service (0004): the one already running on the data
//! folder, else a `gathrd` they start, which outlives them.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::crash_log::CrashLog;
use crate::data::{DataDir, ServiceInfo};
use crate::{Error, http, platform};

/// The service, and whether this call started it.
pub struct Found {
    pub info: ServiceInfo,
    pub started: bool,
}

pub fn find_or_start(data: &DataDir, gathrd: &Path) -> Result<Found, Error> {
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
    let mut command = Command::new(gathrd);
    command.arg("--data").arg(data.path());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // Dev variables never reach a service started here, so an agent's shell can't point the
    // person's own Gathr at the replay engine (as SerenityOS's server-env.mjs strips its own).
    for (name, _) in std::env::vars_os() {
        if name
            .to_string_lossy()
            .to_ascii_uppercase()
            .starts_with("GATHR_")
        {
            command.env_remove(name);
        }
    }
    platform::no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| Error::Other(format!("starting {} failed: {e}", gathrd.display())))?;
    let pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(15);
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
            return Err(Error::Other(format!(
                "gathrd pid {pid} didn't start within 15 s"
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
