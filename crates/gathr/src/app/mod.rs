//! Gathr's own service, kept in the data folder the way Claude Code is (0025, 0032): `gathrd` runs
//! from `<data>/app/<version>`, never from where it was installed, so an installer or an update
//! never touches the program that's running. `current.json` names the copy to start, the one
//! before it, and whether the current one is still on trial: started once it's switched to, and
//! gone back from if it doesn't start.

pub mod release;

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
    /// Switched to and not started yet: a start that fails goes back to `previous`.
    #[serde(default)]
    pub trial: bool,
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
}

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

    /// The database as it was before `version` first started, kept until that start passes.
    fn kept_db(&self, version: &str) -> PathBuf {
        self.root.join(format!("gathr.db.before-{version}"))
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
    /// version built again (a check on a fresh build). A newer one starts on trial.
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
            let mut next = self.take_files(installed)?;
            next.previous = Some(usable.version.clone());
            next.trial = true;
            next.refused = usable.refused.clone();
            return self.write(&next);
        }
        if usable.version == VERSION && !same_file(installed, &self.path(VERSION))? {
            return self.take(installed, usable.previous.clone(), usable.refused.clone());
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
                std::fs::rename(&staging, folder)
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
        std::fs::copy(&self.db, self.kept_db(version))?;
        self.write(&Current {
            version: version.to_string(),
            previous: Some(running),
            trial: true,
            refused: current.refused,
            rolled_back: None,
        })
    }

    /// After the copy on trial failed to start: back to the one before it, with the database as
    /// it was before the failed start. The copy to start then, or `None` when nothing was on trial.
    pub fn roll_back(&self, why: &str) -> io::Result<Option<PathBuf>> {
        let Some(current) = self.current().filter(|c| c.trial) else {
            return Ok(None);
        };
        let Some(previous) = current.previous.clone() else {
            return Ok(None);
        };
        let kept = self.kept_db(&current.version);
        if kept.is_file() {
            for end in ["-wal", "-shm"] {
                let file = PathBuf::from(format!("{}{end}", self.db.display()));
                if file.exists() {
                    std::fs::remove_file(file)?;
                }
            }
            std::fs::rename(&kept, &self.db)?;
        }
        let mut refused = current.refused;
        refused.push(current.version.clone());
        self.write(&Current {
            version: previous.clone(),
            previous: None,
            trial: false,
            refused,
            rolled_back: Some(RolledBack {
                version: current.version,
                why: why.to_string(),
            }),
        })?;
        Ok(Some(self.path(&previous)))
    }

    /// The service started from its copy: a trial passes, and a rollback is taken to be recorded.
    pub fn started(&self) -> io::Result<Started> {
        let none = Started {
            switched_from: None,
            rolled_back: None,
        };
        let Some(mut current) = self.current() else {
            return Ok(none);
        };
        if current.version != self.running_version() {
            return Ok(none);
        }
        let trial = std::mem::take(&mut current.trial);
        let rolled_back = current.rolled_back.take();
        if !trial && rolled_back.is_none() {
            return Ok(none);
        }
        self.write(&current)?;
        let _ = std::fs::remove_file(self.kept_db(&current.version));
        Ok(Started {
            switched_from: current.previous.filter(|_| trial),
            rolled_back,
        })
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
