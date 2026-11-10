//! Gathr's own service, kept in the data folder the way Claude Code is (0025, 0032): `gathrd` runs
//! from `<data>/app/<version>`, never from where it was installed, so an installer or an update
//! never touches the program that's running. `current.json` names the copy to start, the one
//! before it, and whether the current one is still on trial: switched to and not yet proven by a
//! finished turn or `TRIAL` of running, and gone back from if it doesn't start or dies before.

mod handover;
pub mod release;

pub use handover::{HandOver, STARTING};

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::data::DataDir;
use crate::fetch::{parse_version, sha256_file};
use crate::{VERSION, ids, platform};

/// What `current.json` says.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Current {
    pub version: String,
    #[serde(default)]
    pub previous: Option<String>,
    /// Switched to and not proven yet (a finished turn, or `TRIAL` up): a start that fails, or a
    /// service that dies before then, goes back to `previous`.
    #[serde(default)]
    pub trial: bool,
    /// On trial and started once: its switch is recorded.
    #[serde(default)]
    pub trial_started: bool,
    /// On trial and up, or died: a clean stop clears it, so the next start finds a death.
    #[serde(default)]
    pub trial_running: bool,
    /// Releases that failed to start here, never switched to again.
    #[serde(default)]
    pub refused: Vec<String>,
    /// The last rollback, until the service it went back to records it.
    #[serde(default)]
    pub rolled_back: Option<RolledBack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolledBack {
    pub version: String,
    pub why: String,
}

/// What a service learns from `current.json` at its start: the version it took over from, when it
/// was on trial, and the rollback that brought it back, if one did.
pub struct Started {
    pub switched_from: Option<String>,
    pub rolled_back: Option<RolledBack>,
    pub on_trial: bool,
}

/// How long a copy on trial must run with no finished turn before it passes (0032).
pub const TRIAL: std::time::Duration = std::time::Duration::from_secs(10 * 60);

pub struct Copies {
    root: PathBuf,
    db: PathBuf,
}

impl Copies {
    pub fn new(data: &DataDir) -> Copies {
        Copies {
            root: data.path().join("app"),
            db: data.db(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn program_name() -> String {
        format!("gathrd{}", std::env::consts::EXE_SUFFIX)
    }

    pub fn path(&self, version: &str) -> PathBuf {
        self.root.join(version).join(Self::program_name())
    }

    fn current_file(&self) -> PathBuf {
        self.root.join("current.json")
    }

    fn installed_file(&self) -> PathBuf {
        self.root.join("installed")
    }

    /// Records the installed `gathrd` the window (or `gathr open`) reaches the service from: the
    /// window sits beside it, not beside the copy that runs, and Open at login starts it.
    pub fn note_installed(&self, installed: &Path) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::write(
            self.installed_file(),
            installed.to_string_lossy().as_bytes(),
        )
    }

    pub fn installed(&self) -> Option<PathBuf> {
        std::fs::read_to_string(self.installed_file())
            .ok()
            .map(PathBuf::from)
    }

    /// The database as it was before `version` first started, kept until that start passes.
    fn kept_db(&self, version: &str) -> PathBuf {
        self.root.join(format!("gathr.db.before-{version}"))
    }

    /// Keeps the database as it is before `version` starts. Through SQLite, not a file copy: a
    /// service that died without its checkpoint left its last writes in the WAL beside it.
    fn keep_db(&self, version: &str) -> io::Result<()> {
        let kept = self.kept_db(version);
        let _ = std::fs::remove_file(&kept);
        let into = kept.to_string_lossy().into_owned();
        rusqlite::Connection::open(&self.db)
            .and_then(|conn| conn.execute("VACUUM INTO ?1", [into]))
            .map(drop)
            .map_err(io::Error::other)
    }

    pub fn current(&self) -> Option<Current> {
        serde_json::from_slice(&std::fs::read(self.current_file()).ok()?).ok()
    }

    /// The copy to start: the current one, else the one before it if only that is there.
    pub fn program(&self) -> Option<PathBuf> {
        let current = self.current()?;
        std::iter::once(current.version)
            .chain(current.previous)
            .map(|v| self.path(&v))
            .find(|p| p.is_file())
    }

    /// The version this program runs as: its copy's, when it runs from one, else the build's.
    pub fn running_version(&self) -> String {
        let me = std::env::current_exe().and_then(std::fs::canonicalize).ok();
        let current = self.current().unwrap_or_default();
        std::iter::once(current.version)
            .chain(current.previous)
            .find(|v| std::fs::canonicalize(self.path(v)).ok() == me && me.is_some())
            .unwrap_or_else(|| VERSION.to_string())
    }

    /// Names the copy in `current.json` by a rename, so a start reads the old one or the new one,
    /// never half of one; then deletes the copies it no longer names.
    pub fn write(&self, current: &Current) -> io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        let next = self.root.join("current.json.next");
        let bytes = serde_json::to_vec_pretty(current).expect("it serializes");
        platform::write_private(&next, &bytes)?;
        std::fs::rename(&next, self.current_file())?;
        self.prune(current);
        Ok(())
    }

    /// Copies an installed `gathrd`, with its built UI, into the folder when the folder has no
    /// usable copy, when the installed one is newer (the window updated), or when it's this same
    /// version built again (a check on a fresh build, or macOS signing the bundle's own). A newer
    /// one starts on trial; the same version again stays as it was, on trial or not, so a copy that
    /// died on trial still rolls back.
    pub fn adopt(&self, installed: &Path) -> io::Result<()> {
        let current = self.current();
        let usable = current.as_ref().filter(|c| self.path(&c.version).is_file());
        let Some(usable) = usable else {
            let previous = current.as_ref().and_then(|c| c.previous.clone());
            let previous = previous.filter(|p| self.path(p).is_file() && p != VERSION);
            return self.take(installed, previous, current.unwrap_or_default().refused);
        };
        let (installed_is, ours) = (parse_version(VERSION), parse_version(&usable.version));
        let refused = usable.refused.iter().any(|r| r == VERSION);
        if installed_is > ours && !refused {
            if self.db.is_file() {
                self.keep_db(VERSION)?;
            }
            let mut next = self.take_files(installed)?;
            next.previous = Some(usable.version.clone());
            next.trial = true;
            next.refused = usable.refused.clone();
            return self.write(&next);
        }
        if usable.version == VERSION && !same_file(installed, &self.path(VERSION))? {
            self.take_files(installed)?;
            return self.write(usable);
        }
        Ok(())
    }

    fn take(
        &self,
        installed: &Path,
        previous: Option<String>,
        refused: Vec<String>,
    ) -> io::Result<()> {
        let mut next = self.take_files(installed)?;
        next.previous = previous;
        next.refused = refused;
        self.write(&next)
    }

    /// Copies `program` and the UI beside it to `<VERSION>`, through a staging folder.
    fn take_files(&self, program: &Path) -> io::Result<Current> {
        let staging = self.root.join(format!(".adopt-{}", ids::span_id()));
        std::fs::create_dir_all(&staging)?;
        let copied = std::fs::copy(program, staging.join(Self::program_name()))
            .and_then(|_| match installed_ui(program) {
                Some(ui) => copy_dir(&ui, &staging.join("ui")),
                None => Ok(()),
            })
            .and_then(|()| {
                let folder = self.root.join(VERSION);
                if folder.exists() {
                    std::fs::remove_dir_all(&folder)?;
                }
                platform::move_folder(&staging, &folder)
            });
        if copied.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        copied?;
        Ok(Current {
            version: VERSION.to_string(),
            ..Current::default()
        })
    }

    /// Switches to `version`, installed and checked, on trial: this program's own copy is the one
    /// to go back to, and the database is kept as it is now in case the new one's start fails.
    /// Called with the service stopped and the data folder still locked.
    pub fn switch_to(&self, version: &str) -> io::Result<()> {
        let running = self.running_version();
        let current = self.current().unwrap_or_default();
        if !self.path(&running).is_file() {
            // A build that runs from elsewhere (as `VERSION`) keeps itself to go back to.
            self.take_files(&std::env::current_exe()?)?;
        }
        self.keep_db(version)?;
        self.write(&Current {
            version: version.to_string(),
            previous: Some(running),
            trial: true,
            refused: current.refused,
            ..Current::default()
        })
    }

    /// The copy on trial started before and never stopped cleanly: it died.
    pub fn died_on_trial(&self) -> bool {
        self.current().is_some_and(|c| c.trial && c.trial_running)
    }

    /// After the copy on trial failed to start or died: back to the one before it. The database
    /// goes back to how it was before the update only when the new copy moved its schema on, which
    /// the one before can't open; else what was written since stays. The copy to start then, or
    /// `None` when nothing was on trial.
    pub fn roll_back(&self, why: &str) -> io::Result<Option<PathBuf>> {
        let Some(current) = self.current().filter(|c| c.trial) else {
            return Ok(None);
        };
        let Some(previous) = current.previous.clone() else {
            return Ok(None);
        };
        let kept = self.kept_db(&current.version);
        if kept.is_file() {
            let before = schema(&kept);
            if before.is_some() && before == schema(&self.db) {
                std::fs::remove_file(&kept)?;
            } else {
                for end in ["-wal", "-shm"] {
                    let file = PathBuf::from(format!("{}{end}", self.db.display()));
                    if file.exists() {
                        std::fs::remove_file(file)?;
                    }
                }
                std::fs::rename(&kept, &self.db)?;
            }
        }
        let mut refused = current.refused;
        refused.push(current.version.clone());
        self.write(&Current {
            version: previous.clone(),
            refused,
            rolled_back: Some(RolledBack {
                version: current.version,
                why: why.to_string(),
            }),
            ..Current::default()
        })?;
        Ok(Some(self.path(&previous)))
    }

    /// The copy this program runs from, while it's on trial: `change` it and save.
    fn on_trial(&self, change: impl FnOnce(&mut Current)) -> io::Result<bool> {
        let Some(mut current) = self.current() else {
            return Ok(false);
        };
        if !current.trial || current.version != self.running_version() {
            return Ok(false);
        }
        change(&mut current);
        self.write(&current)?;
        Ok(true)
    }

    /// The copy on trial finished a turn or ran `TRIAL`: it stays, and the database copy goes.
    pub fn passed(&self) -> io::Result<bool> {
        let passed = self.on_trial(|c| {
            c.trial = false;
            c.trial_started = false;
            c.trial_running = false;
        })?;
        if passed {
            let _ = std::fs::remove_file(self.kept_db(&self.running_version()));
        }
        Ok(passed)
    }

    /// A clean stop on trial isn't a death: the next start is tried again.
    pub fn stopped(&self) -> io::Result<()> {
        self.on_trial(|c| c.trial_running = false).map(drop)
    }

    /// The service started from its copy: a trial runs on, and a rollback is taken to be recorded.
    pub fn started(&self) -> io::Result<Started> {
        let mut started = Started {
            switched_from: None,
            rolled_back: None,
            on_trial: false,
        };
        let Some(mut current) = self.current() else {
            return Ok(started);
        };
        if current.version != self.running_version() {
            return Ok(started);
        }
        started.rolled_back = current.rolled_back.take();
        if current.trial {
            if !current.trial_started {
                started.switched_from = current.previous.clone();
            }
            started.on_trial = true;
            current.trial_started = true;
            current.trial_running = true;
        } else if started.rolled_back.is_none() {
            return Ok(started);
        }
        self.write(&current)?;
        Ok(started)
    }

    /// Deletes every copy `current` doesn't name, a staging folder left by a check that didn't
    /// finish, and a database kept for a version no longer on trial. A copy still running on
    /// Windows can't be deleted: the next write tries again.
    fn prune(&self, keep: &Current) {
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return;
        };
        let kept_db = keep
            .trial
            .then(|| self.kept_db(&keep.version))
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()));
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let path = entry.path();
            if path.is_dir() && name != keep.version && Some(&name) != keep.previous.as_ref() {
                let _ = std::fs::remove_dir_all(path);
            } else if name.starts_with("gathr.db.before-") && Some(&name) != kept_db.as_ref() {
                let _ = std::fs::remove_file(path);
            }
        }
    }
}

/// The built UI an installed `gathrd` serves: beside it, or in a macOS bundle's `Resources`.
fn installed_ui(program: &Path) -> Option<PathBuf> {
    let beside = program.with_file_name("ui");
    let bundle = program.parent()?.parent()?.join("Resources").join("ui");
    [beside, bundle].into_iter().find(|p| p.is_dir())
}

/// A database's schema version, or `None` when it can't be read.
fn schema(db: &Path) -> Option<i64> {
    let flags = rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE;
    let conn = rusqlite::Connection::open_with_flags(db, flags).ok()?;
    crate::db::schema_version(&conn).ok()
}

fn same_file(a: &Path, b: &Path) -> io::Result<bool> {
    if std::fs::metadata(a)?.len() != std::fs::metadata(b)?.len() {
        return Ok(false);
    }
    Ok(sha256_file(a)? == sha256_file(b)?)
}

pub(crate) fn copy_dir(from: &Path, to: &Path) -> io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}
