//! Claude Code's release feed, as its own installer reads it, through the system's curl (0025).

use std::path::Path;
use std::sync::atomic::AtomicBool;

use crate::fetch::{self, Redirects};
pub use crate::fetch::{parse_version, sha256};

/// Claude Code's own release feed.
const CLAUDE_RELEASES: &str = "https://downloads.claude.ai/claude-code-releases";
/// Names another feed: a stand-in in tests, or Claude's in a dev build, which has none by default.
pub const FEED_ENV: &str = "GATHR_CLAUDE_RELEASES";
/// The largest file accepted: Claude Code for Windows is 345 MB.
const MAX_SIZE: u64 = 512 * 1024 * 1024;

/// The feed to check; none in a debug build unless `GATHR_CLAUDE_RELEASES` names one, so a dev
/// service or a test never downloads 345 MB unasked.
pub fn feed() -> Option<String> {
    match std::env::var(FEED_ENV) {
        Ok(feed) if !feed.trim().is_empty() => Some(feed.trim().trim_end_matches('/').into()),
        _ => (!cfg!(debug_assertions)).then(|| CLAUDE_RELEASES.into()),
    }
}

/// One release for this computer, as its manifest names it.
#[derive(Debug, PartialEq)]
pub struct Release {
    pub version: String,
    pub file: fetch::File,
}

/// The feed's latest version, `x.y.z`.
pub fn latest(feed: &str, stop: &AtomicBool) -> Result<String, String> {
    let text = fetch::fetch(&format!("{feed}/latest"), Redirects::Refused, stop)?;
    let version = text.trim();
    match parse_version(version) {
        Some(_) => Ok(version.to_string()),
        None => Err(format!("the feed's latest version {version:?} isn't x.y.z")),
    }
}

/// The release's file for `target` (such as `win32-x64`), named `binary`.
pub fn release(
    feed: &str,
    version: &str,
    target: &str,
    binary: &str,
    stop: &AtomicBool,
) -> Result<Release, String> {
    let url = format!("{feed}/{version}/manifest.json");
    let text = fetch::fetch(&url, Redirects::Refused, stop)?;
    let manifest: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("its manifest isn't JSON: {e}"))?;
    from_manifest(&manifest, feed, version, target, binary)
}

fn from_manifest(
    manifest: &serde_json::Value,
    feed: &str,
    version: &str,
    target: &str,
    binary: &str,
) -> Result<Release, String> {
    if manifest["version"] != version {
        return Err(format!(
            "its manifest is for {}, not {version}",
            manifest["version"]
        ));
    }
    let entry = &manifest["platforms"][target];
    let sha256 = entry["checksum"].as_str().unwrap_or_default();
    let size = entry["size"].as_u64().unwrap_or(0);
    if entry["binary"] != binary {
        return Err(format!("its manifest has no {binary} for {target}"));
    }
    if !fetch::is_sha256(sha256) {
        return Err(format!(
            "its manifest's SHA-256 for {target} isn't 64 hex characters"
        ));
    }
    if size == 0 || size > MAX_SIZE {
        return Err(format!(
            "its manifest's size for {target}, {size} bytes, isn't 1 to {MAX_SIZE}"
        ));
    }
    Ok(Release {
        version: version.to_string(),
        file: fetch::File {
            url: format!("{feed}/{version}/{target}/{binary}"),
            sha256: sha256.to_ascii_lowercase(),
            size,
        },
    })
}

/// Downloads the release to `to` and checks its size and SHA-256 (`fetch::download`).
pub fn download(
    release: &Release,
    to: &Path,
    stop: &AtomicBool,
) -> Result<(), (&'static str, String)> {
    fetch::download(&release.file, to, Redirects::Refused, stop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_manifest_gives_this_platforms_file_only_when_every_field_holds() {
        let sha = "AB".repeat(32);
        let manifest = |version: &str, binary: &str, sha: &str, size: u64| {
            json!({ "version": version, "platforms": {
                "win32-x64": { "binary": binary, "checksum": sha, "size": size } } })
        };
        let read = |m: &serde_json::Value| {
            from_manifest(m, "https://f", "2.1.319", "win32-x64", "claude.exe")
        };
        let good = read(&manifest("2.1.319", "claude.exe", &sha, 344_807_072)).unwrap();
        assert_eq!(good.file.url, "https://f/2.1.319/win32-x64/claude.exe");
        assert_eq!(good.file.sha256, "ab".repeat(32));
        let bad = [
            manifest("2.1.318", "claude.exe", &sha, 1),
            manifest("2.1.319", "claude", &sha, 1),
            manifest("2.1.319", "claude.exe", "abc", 1),
            manifest("2.1.319", "claude.exe", &"zz".repeat(32), 1),
            manifest("2.1.319", "claude.exe", &sha, 0),
            manifest("2.1.319", "claude.exe", &sha, MAX_SIZE + 1),
        ];
        for m in bad {
            assert!(read(&m).is_err(), "accepted {m}");
        }
    }
}
