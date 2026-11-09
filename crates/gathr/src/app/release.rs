//! Gathr's own releases (0032): each GitHub Release carries `gathrd.json`, which names each
//! platform's archive (`gathrd` and its built UI) with its SHA-256 and size.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use super::Copies;
use crate::fetch::{self, Failure, Redirects, failed, parse_version};
use crate::{ids, platform};

/// The newest release's files: GitHub redirects `latest` to it.
const GATHR_RELEASES: &str = "https://github.com/tcurtsinger/Gathr/releases/latest/download";
/// Names another feed: a stand-in in tests. A dev build has none by default.
pub const FEED_ENV: &str = "GATHR_APP_RELEASES";
/// A feed built in instead of GitHub's (`GATHR_APP_FEED` when building): a test release, so its
/// installed copies update from a feed on this computer exactly as real ones do from GitHub.
const BUILT_FEED: Option<&str> = option_env!("GATHR_APP_FEED");
/// The largest archive accepted.
const MAX_SIZE: u64 = 512 * 1024 * 1024;
/// How long a new copy has to say its version.
const PROBE_WAIT: Duration = Duration::from_secs(10);

/// The feed to check: `GATHR_APP_RELEASES`, else the one built in, else GitHub's in a release
/// build; none in a debug build.
pub fn feed() -> Option<String> {
    let named = std::env::var(FEED_ENV)
        .ok()
        .filter(|f| !f.trim().is_empty());
    let built = BUILT_FEED
        .filter(|f| !f.trim().is_empty())
        .map(String::from);
    let release = (!cfg!(debug_assertions)).then(|| GATHR_RELEASES.to_string());
    let feed = named.or(built).or(release)?;
    Some(feed.trim().trim_end_matches('/').to_string())
}

/// The newest release, with this computer's archive.
#[derive(Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub file: fetch::File,
}

/// The feed's newest release for `target` (such as `win32-x64`).
pub fn latest(feed: &str, target: &str, stop: &AtomicBool) -> Result<Release, Failure> {
    let url = format!("{feed}/gathrd.json");
    let text = fetch::fetch(&url, Redirects::Followed, stop).map_err(failed("feed"))?;
    let manifest: serde_json::Value = serde_json::from_str(&text)
        .map_err(|e| failed("manifest")(format!("gathrd.json isn't JSON: {e}")))?;
    from_manifest(&manifest, feed, target).map_err(failed("manifest"))
}

fn from_manifest(
    manifest: &serde_json::Value,
    feed: &str,
    target: &str,
) -> Result<Release, String> {
    let version = manifest["version"].as_str().unwrap_or_default();
    if parse_version(version).is_none() {
        return Err(format!("its version {version:?} isn't x.y.z"));
    }
    let entry = &manifest["platforms"][target];
    let file = entry["file"].as_str().unwrap_or_default();
    let sha256 = entry["sha256"].as_str().unwrap_or_default();
    let size = entry["size"].as_u64().unwrap_or(0);
    let plain = |c: char| c.is_ascii_alphanumeric() || "._-".contains(c);
    if file.is_empty() || !file.chars().all(plain) || file.starts_with('.') {
        return Err(format!("it names no plain file for {target}: {file:?}"));
    }
    if !fetch::is_sha256(sha256) {
        return Err(format!("its SHA-256 for {target} isn't 64 hex characters"));
    }
    if size == 0 || size > MAX_SIZE {
        return Err(format!(
            "its size for {target}, {size} bytes, isn't 1 to {MAX_SIZE}"
        ));
    }
    Ok(Release {
        version: version.to_string(),
        file: fetch::File {
            url: format!("{feed}/{file}"),
            sha256: sha256.to_ascii_lowercase(),
            size,
        },
    })
}

/// Downloads, checks and unpacks `release` into `<app>/<version>`: its archive's size and
/// SHA-256, then its `gathrd` saying it's that version. Nothing is placed unless all pass.
pub fn install(copies: &Copies, release: &Release, stop: &AtomicBool) -> Result<(), Failure> {
    let disk = |e: std::io::Error| failed("disk")(e.to_string());
    let staging = copies.root().join(format!(".download-{}", ids::span_id()));
    let unpacked = staging.join("unpacked");
    std::fs::create_dir_all(&unpacked).map_err(disk)?;
    let archive = staging.join("release.tar.gz");
    let program = unpacked.join(Copies::program_name());
    let done = fetch::download(&release.file, &archive, Redirects::Followed, stop)
        .map_err(|(kind, why)| Failure { kind, why })
        .and_then(|()| unpack(&archive, &unpacked).map_err(failed("unpack")))
        .and_then(|()| platform::make_executable(&program).map_err(disk))
        .and_then(|()| probe(&program, &release.version).map_err(failed("probe")))
        .and_then(|()| {
            let folder = copies.root().join(&release.version);
            if folder.exists() {
                std::fs::remove_dir_all(&folder).map_err(disk)?;
            }
            std::fs::rename(&unpacked, folder).map_err(disk)
        });
    let _ = std::fs::remove_dir_all(&staging);
    done
}

/// The system's own tar unpacks it: Windows 10 and later and macOS both ship bsdtar.
fn unpack(archive: &Path, into: &Path) -> Result<(), String> {
    let mut command = Command::new(platform::tar());
    command
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(into)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    platform::no_window(&mut command);
    let out = command
        .output()
        .map_err(|e| format!("tar didn't start: {e}"))?;
    if !out.status.success() {
        let error = String::from_utf8_lossy(&out.stderr);
        return Err(format!("tar failed ({}): {}", out.status, error.trim()));
    }
    if !into.join(Copies::program_name()).is_file() {
        return Err(format!("the archive has no {}", Copies::program_name()));
    }
    Ok(())
}

/// The copy runs here and names the release's version as its own (`gathrd x.y.z`).
fn probe(program: &Path, version: &str) -> Result<(), String> {
    let mut command = Command::new(program);
    command
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    platform::no_window(&mut command);
    let mut child = command
        .spawn()
        .map_err(|e| format!("it didn't start: {e}"))?;
    let deadline = Instant::now() + PROBE_WAIT;
    while matches!(child.try_wait(), Ok(None)) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(format!(
            "--version didn't end in {} s",
            PROBE_WAIT.as_secs()
        ));
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    let said = String::from_utf8_lossy(&out.stdout);
    let named = said.split_whitespace().last().unwrap_or_default();
    if !out.status.success() || named != version {
        return Err(format!(
            "--version said {:?} ({}), not {version}",
            said.trim(),
            out.status
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_manifest_gives_this_platforms_archive_only_when_every_field_holds() {
        let sha = "AB".repeat(32);
        let manifest = |version: &str, file: &str, sha: &str, size: u64| {
            json!({ "version": version, "platforms": {
                "win32-x64": { "file": file, "sha256": sha, "size": size } } })
        };
        let read = |m: &serde_json::Value| from_manifest(m, "https://f", "win32-x64");
        let good = read(&manifest("0.2.7", "gathrd-0.2.7-win32-x64.tar.gz", &sha, 9)).unwrap();
        assert_eq!(good.file.url, "https://f/gathrd-0.2.7-win32-x64.tar.gz");
        assert_eq!(
            (good.version.as_str(), good.file.sha256.as_str()),
            ("0.2.7", "ab".repeat(32).as_str())
        );
        let bad = [
            manifest("0.2", "a.tar.gz", &sha, 9),
            manifest("0.2.7", "", &sha, 9),
            manifest("0.2.7", "../a.tar.gz", &sha, 9),
            manifest("0.2.7", "x/a.tar.gz", &sha, 9),
            manifest("0.2.7", "a.tar.gz", "abc", 9),
            manifest("0.2.7", "a.tar.gz", &sha, 0),
            manifest("0.2.7", "a.tar.gz", &sha, MAX_SIZE + 1),
        ];
        for m in bad {
            assert!(read(&m).is_err(), "accepted {m}");
        }
    }
}
