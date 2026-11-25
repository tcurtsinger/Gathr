//! Computer use: cua-driver, a program that uses this PC's apps in the background, served as the tool
//! `computer` in Plugins, whose switch turns it on. Gathr fetches one pinned copy into the data folder the
//! first time it's needed, checked against its release's SHA-256, and runs it with its telemetry and update
//! checks off and its home in that folder: never the installer, which would start it with Windows and share
//! `~/.cua-driver` with any other copy on the PC.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use crate::fetch::{self, File, Redirects};
use crate::pc_tools::Runs;

/// The tool's id in Plugins (migration 0085).
pub const ID: &str = "computer";
const VERSION: &str = "0.62.1";
const SHA256: &str = "8acaf2517f73ab5f759343fea34096aa3d6b2d8047ff05025073f59636418828";
const SIZE: u64 = 32_770_143;

/// Where the copy lives: `<data>/computer/<version>`, the program right in it.
pub fn folder(data: &Path) -> PathBuf {
    data.join("computer").join(VERSION)
}

pub fn program(data: &Path) -> PathBuf {
    folder(data).join("cua-driver.exe")
}

/// What the tool runs: `cua-driver mcp` with no `--socket`, which on Windows owns its runtime instead of
/// joining a daemon another copy may hold. Its settings and state stay in the data folder.
pub fn runs(data: &Path) -> Runs {
    let home = data.join("computer").join("home");
    let variables = BTreeMap::from([
        ("CUA_DRIVER_RS_TELEMETRY_ENABLED".into(), "0".into()),
        ("DO_NOT_TRACK".into(), "1".into()),
        ("CUA_DRIVER_RS_UPDATE_CHECK".into(), "false".into()),
        ("HOME".into(), home.display().to_string()),
        (
            "CUA_DRIVER_TELEMETRY_HOME".into(),
            home.join(".cua-driver").display().to_string(),
        ),
    ]);
    Runs {
        program: program(data).display().to_string(),
        args: vec!["mcp".into()],
        folder: Some(home.display().to_string()),
        variables,
    }
}

/// The copy, fetched and unpacked when it isn't there yet. Blocking: about 31 MB. `Err` says why in plain
/// words.
pub fn fetched(data: &Path, stop: &AtomicBool) -> Result<PathBuf, String> {
    let root = data.join("computer");
    // Its home, where it runs: made even for a copy that's there, should the folder have gone.
    std::fs::create_dir_all(root.join("home")).map_err(|e| e.to_string())?;
    let program = program(data);
    if program.is_file() {
        return Ok(program);
    }
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err("Computer use runs on Windows (x64) only for now.".into());
    }
    let zip = root.join(format!("cua-driver-{VERSION}.zip"));
    let file = File {
        url: format!(
            "https://github.com/trycua/cua/releases/download/cua-driver-rs-v{VERSION}/cua-driver-rs-{VERSION}-windows-x86_64.zip"
        ),
        sha256: SHA256.into(),
        size: SIZE,
    };
    let downloaded = fetch::download(&file, &zip, Redirects::Followed, stop);
    let unpacked = downloaded
        .map_err(|(_, why)| format!("cua-driver didn't download: {why}"))
        .and_then(|()| std::fs::read(&zip).map_err(|e| e.to_string()))
        .and_then(|bytes| unpack(&bytes, &folder(data)));
    let _ = std::fs::remove_file(&zip);
    unpacked.map(|()| program)
}

/// Unpacks the release's zip into `to`, each file without the zip's one top folder. Written beside it first,
/// so a copy cut short is never taken for a whole one.
fn unpack(bytes: &[u8], to: &Path) -> Result<(), String> {
    let unreadable = |e: zip::result::ZipError| format!("cua-driver's zip wasn't read: {e}");
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(unreadable)?;
    let part = to.with_extension("part");
    let _ = std::fs::remove_dir_all(&part);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(unreadable)?;
        let Some(name) = entry.enclosed_name() else {
            return Err("cua-driver's zip names a file outside its folder.".into());
        };
        let inside: PathBuf = name.components().skip(1).collect();
        if entry.is_dir() || inside.as_os_str().is_empty() {
            continue;
        }
        let path = part.join(inside);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }
    std::fs::rename(&part, to).map_err(|e| e.to_string())
}
