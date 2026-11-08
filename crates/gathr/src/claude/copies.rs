//! Gathr's own copies of Claude Code, in its data folder: installed from the release feed, checked,
//! and switched to only once they pass (docs/decisions/0025-engine-copies.md).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use super::release::{self, Release};
pub use super::release::{FEED_ENV, feed, sha256};
use super::{command, frames};
use crate::data::DataDir;

/// How long a new copy has to answer `initialize`: Claude Code 2.1.318 took 0.3 s.
const PROBE_WAIT: Duration = Duration::from_secs(30);
/// How often a wait looks at `stop`.
const TICK: Duration = Duration::from_millis(100);

/// What `current.json` says: the copy chats start, and the one before it, the known good.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Current {
    pub version: String,
    pub sha256: String,
    pub previous: Option<String>,
}

/// What a check did.
#[derive(Debug, PartialEq)]
pub enum Updated {
    UpToDate(String),
    Installed {
        version: String,
        from: Option<String>,
    },
}

/// Why a check stopped: `kind` is the event's `error.type`.
#[derive(Debug)]
pub struct Failure {
    pub kind: &'static str,
    pub why: String,
}

fn failed(kind: &'static str) -> impl FnOnce(String) -> Failure {
    move |why| Failure { kind, why }
}

pub struct Copies {
    root: PathBuf,
}

impl Copies {
    pub fn new(data: &DataDir) -> Copies {
        Copies {
            root: data.path().join("engines").join("claude"),
        }
    }

    fn binary() -> &'static str {
        if cfg!(windows) {
            "claude.exe"
        } else {
            "claude"
        }
    }

    fn path(&self, version: &str) -> PathBuf {
        self.root.join(version).join(Self::binary())
    }

    fn current_file(&self) -> PathBuf {
        self.root.join("current.json")
    }

    pub fn current(&self) -> Option<Current> {
        serde_json::from_slice(&std::fs::read(self.current_file()).ok()?).ok()
    }

    /// The copy a chat starts: the current one, else the one before it if only that is there.
    pub fn program(&self) -> Option<PathBuf> {
        let current = self.current()?;
        std::iter::once(current.version)
            .chain(current.previous)
            .map(|v| self.path(&v))
            .find(|p| p.is_file())
    }

    /// Installs the feed's latest release if it's newer than the copy in use and passes every
    /// check; otherwise the copy in use stays. Blocks for the download, which `stop` ends: call
    /// it off async threads.
    pub fn update(&self, feed: &str, stop: &AtomicBool) -> Result<Updated, Failure> {
        let target = crate::platform::release_target().ok_or_else(|| {
            failed("platform")("Claude Code ships no build for this computer".into())
        })?;
        let latest = release::latest(feed, stop).map_err(failed("feed"))?;
        let current = self.current().filter(|c| self.path(&c.version).is_file());
        if let Some(c) = &current
            && release::parse_version(&c.version) >= release::parse_version(&latest)
        {
            self.prune(c);
            return Ok(Updated::UpToDate(c.version.clone()));
        }
        let found = release::release(feed, &latest, target, Self::binary(), stop)
            .map_err(failed("manifest"))?;
        self.install(&found, current.as_ref().map(|c| c.version.clone()), stop)
    }

    /// Downloads into a staging folder, moves it to `<version>`, and checks it there, where chats
    /// will run it; only then does `current.json` name it.
    fn install(
        &self,
        found: &Release,
        from: Option<String>,
        stop: &AtomicBool,
    ) -> Result<Updated, Failure> {
        let disk = |e: std::io::Error| failed("disk")(e.to_string());
        let id = crate::ids::span_id();
        let staging = self.root.join(format!(".download-{id}"));
        let aside = self.root.join(format!(".kept-{id}"));
        std::fs::create_dir_all(&staging).map_err(disk)?;
        let folder = self.root.join(&found.version);
        let file = staging.join(Self::binary());
        let mut placed = false;
        let checked = release::download(found, &file, stop)
            .map_err(|(kind, why)| Failure { kind, why })
            .and_then(|()| crate::platform::make_executable(&file).map_err(disk))
            .and_then(|()| {
                // A folder already there may be the known good (a repair after the current copy
                // went missing): it's moved aside, and back if this copy fails.
                if folder.exists() {
                    std::fs::rename(&folder, &aside).map_err(disk)?;
                }
                std::fs::rename(&staging, &folder).map_err(disk)?;
                placed = true;
                Ok(())
            })
            .and_then(|()| {
                probe(&self.path(&found.version), &found.version, &self.root, stop)
                    .map_err(failed("probe"))
            })
            .and_then(|()| self.switch(found, from.clone()));
        let _ = std::fs::remove_dir_all(&staging);
        if checked.is_err() && placed {
            let _ = std::fs::remove_dir_all(&folder);
        }
        if checked.is_err() && aside.exists() {
            let _ = std::fs::rename(&aside, &folder);
        }
        let _ = std::fs::remove_dir_all(&aside);
        checked?;
        Ok(Updated::Installed {
            version: found.version.clone(),
            from,
        })
    }

    /// Names the copy in `current.json` by a rename, so a start reads the old copy or the new
    /// one, never half of one.
    fn switch(&self, found: &Release, from: Option<String>) -> Result<(), Failure> {
        let disk = |e: std::io::Error| failed("disk")(e.to_string());
        let current = Current {
            version: found.version.clone(),
            sha256: found.sha256.clone(),
            previous: from,
        };
        let next = self.root.join("current.json.next");
        crate::platform::write_private(
            &next,
            &serde_json::to_vec_pretty(&current).expect("it serializes"),
        )
        .map_err(disk)?;
        // Replaces the old file on Windows too (MoveFileEx with MOVEFILE_REPLACE_EXISTING).
        std::fs::rename(&next, self.current_file()).map_err(disk)?;
        self.prune(&current);
        Ok(())
    }
    /// Deletes every copy but the current one and the one before it, and any download left by a
    /// check that didn't finish. A copy still running on Windows can't be deleted: the next
    /// check tries again.
    fn prune(&self, keep: &Current) {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let kept = name == keep.version || Some(&name) == keep.previous.as_ref();
            if entry.path().is_dir() && !kept {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }
}

/// A new copy must start the way chats start it and accept Gathr's switches: `--version` names
/// the release and passes Gathr's minimum, and `initialize` answers `success`. It runs in an
/// empty folder under `root`, with no login.
fn probe(program: &Path, version: &str, root: &Path, stop: &AtomicBool) -> Result<(), String> {
    // `--version` holds itself to 5 s (command::version), so only `initialize` watches `stop`.
    let (said, usable) = command::version(program);
    usable?;
    if said.as_deref() != Some(version) {
        return Err(format!("it says it's {said:?}, not {version}"));
    }
    let scratch = root.join(format!(".probe-{}", crate::ids::span_id()));
    std::fs::create_dir_all(&scratch).map_err(|e| e.to_string())?;
    let answered = initialize(program, &scratch, stop);
    let _ = std::fs::remove_dir_all(&scratch);
    answered
}

fn initialize(program: &Path, scratch: &Path, stop: &AtomicBool) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .args(command::args(
            command::Session::New(&crate::ids::uuid()),
            Default::default(),
        ))
        .env_clear()
        .envs(command::env(std::env::vars_os(), &scratch.join("claude")))
        .current_dir(scratch)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    crate::platform::no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("it didn't start: {e}"))?;
    let stdout = child.stdout.take().expect("stdout is piped");
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let asked = writeln!(stdin, "{}", frames::initialize("gathr-probe"));
    let deadline = Instant::now() + PROBE_WAIT;
    let mut answer = Err(format!(
        "it didn't answer initialize in {} s",
        PROBE_WAIT.as_secs()
    ));
    while asked.is_ok() && Instant::now() < deadline {
        if stop.load(Ordering::Relaxed) {
            answer = Err("the service stopped".into());
            break;
        }
        let line = match rx.recv_timeout(TICK) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => continue,
            Err(RecvTimeoutError::Disconnected) => break,
        };
        let frame: serde_json::Value = serde_json::from_str(&line).unwrap_or_default();
        if frame["type"] == "control_response" && frame["response"]["request_id"] == "gathr-probe" {
            answer = match frame["response"]["subtype"].as_str() {
                Some("success") => Ok(()),
                _ => Err(format!("initialize failed: {}", frame["response"]["error"])),
            };
            break;
        }
    }
    drop(stdin);
    // Its folder has no login, so there's nothing to renew and a kill is safe.
    let closing = Instant::now() + Duration::from_secs(10);
    while matches!(child.try_wait(), Ok(None))
        && Instant::now() < closing
        && !stop.load(Ordering::Relaxed)
    {
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    answer
}
