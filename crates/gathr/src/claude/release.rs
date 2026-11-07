//! Claude Code's release feed, as its own installer reads it, through the system's curl (0025).

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

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
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// The feed's latest version, `x.y.z`.
pub fn latest(feed: &str, stop: &AtomicBool) -> Result<String, String> {
    let text = fetch(&format!("{feed}/latest"), stop)?;
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
    let text = fetch(&format!("{feed}/{version}/manifest.json"), stop)?;
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
    if sha256.len() != 64 || !sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
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
        url: format!("{feed}/{version}/{target}/{binary}"),
        sha256: sha256.to_ascii_lowercase(),
        size,
    })
}

/// Downloads the release to `to` and checks its size and SHA-256. `Err((kind, why))`, where
/// kind is `download` or `checksum`.
/// Ends early, with curl killed, once `stop` is set.
pub fn download(
    release: &Release,
    to: &Path,
    stop: &AtomicBool,
) -> Result<(), (&'static str, String)> {
    // No total time limit: a slow link may take a while. A stalled one stops within a minute.
    let mut command = curl(&[
        "--max-filesize",
        &release.size.to_string(),
        "--speed-limit",
        "1024",
    ]);
    command
        .args(["--speed-time", "60", "--output"])
        .arg(to)
        .arg(&release.url);
    run_until(command, stop).map_err(|why| ("download", why))?;
    let size = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    if size != release.size {
        let why = format!(
            "{} is {size} bytes; its manifest says {}",
            release.url, release.size
        );
        return Err(("checksum", why));
    }
    let sha = sha256_file(to).map_err(|e| ("download", format!("it wasn't read back: {e}")))?;
    if sha != release.sha256 {
        let why = format!("its SHA-256 is {sha}; its manifest says {}", release.sha256);
        return Err(("checksum", why));
    }
    Ok(())
}

/// Lowercase hex SHA-256.
pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut file = std::fs::File::open(path)?;
    let mut buffer = vec![0; 1 << 20];
    loop {
        match std::io::Read::read(&mut file, &mut buffer)? {
            0 => return Ok(hex(&hasher.finalize())),
            n => hasher.update(&buffer[..n]),
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// `x.y.z` as numbers, so 2.1.99 sorts before 2.1.100.
pub fn parse_version(version: &str) -> Option<[u32; 3]> {
    let parts: Vec<u32> = version
        .split('.')
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    parts.try_into().ok()
}

/// A small text file from the feed, within 30 s or until `stop` is set.
fn fetch(url: &str, stop: &AtomicBool) -> Result<String, String> {
    let mut command = curl(&["--max-time", "30", "--max-filesize", "1048576"]);
    command.arg(url);
    run_until(command, stop)
}

/// curl, failing on any HTTP error and never following a redirect away from the feed.
fn curl(limits: &[&str]) -> Command {
    let mut command = Command::new(crate::platform::curl());
    command
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--connect-timeout",
            "15",
        ])
        .args(limits)
        .stdin(Stdio::null());
    crate::platform::no_window(&mut command);
    command
}

/// curl's output to its end, or curl killed once `stop` is set; its error line if it fails.
fn run_until(mut command: Command, stop: &AtomicBool) -> Result<String, String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("curl didn't start: {e}"))?;
    // Read on threads so a full pipe never holds curl up while this loop waits for it.
    let read = |pipe: Option<Box<dyn std::io::Read + Send>>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_string(&mut text);
            }
            text
        })
    };
    let stdout = read(child.stdout.take().map(|p| Box::new(p) as _));
    let stderr = read(child.stderr.take().map(|p| Box::new(p) as _));
    let status = loop {
        if stop.load(Ordering::Relaxed) {
            let _ = child.kill();
            let _ = child.wait();
            return Err("the service stopped".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("curl wasn't waited for: {e}")),
        }
    };
    let (out, error) = (stdout.join(), stderr.join());
    if status.success() {
        return Ok(out.unwrap_or_default());
    }
    Err(format!("{} ({status})", error.unwrap_or_default().trim()))
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
        assert_eq!(good.url, "https://f/2.1.319/win32-x64/claude.exe");
        assert_eq!(good.sha256, "ab".repeat(32));
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

    #[test]
    fn versions_compare_by_number() {
        assert!(parse_version("2.1.100") > parse_version("2.1.99"));
        assert_eq!(parse_version("2.1"), None);
        assert_eq!(parse_version("2.1.x"), None);
    }
}
