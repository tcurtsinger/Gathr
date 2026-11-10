//! Downloads through the system's own curl (0025), checked against the size and SHA-256 a release
//! names, for Gathr's copies of Claude Code and of itself.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

/// Whether a request may follow redirects: GitHub's release links redirect to its file host;
/// Claude's feed never does. Followed only over HTTPS.
#[derive(Clone, Copy)]
pub enum Redirects {
    Refused,
    Followed,
}

/// Why a check for a release stopped: `kind` is its event's `error.type`.
#[derive(Debug)]
pub struct Failure {
    pub kind: &'static str,
    pub why: String,
}

/// Makes a [`Failure`] of `kind` from its reason.
pub fn failed(kind: &'static str) -> impl FnOnce(String) -> Failure {
    move |why| Failure { kind, why }
}

/// A file a release names: where it is, and what it must be.
#[derive(Debug, PartialEq)]
pub struct File {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// Downloads `file` to `to` and checks its size and SHA-256. `Err((kind, why))`, where kind is
/// `download` or `checksum`. Ends early, with curl killed, once `stop` is set.
pub fn download(
    file: &File,
    to: &Path,
    redirects: Redirects,
    stop: &AtomicBool,
) -> Result<(), (&'static str, String)> {
    transfer(&file.url, to, file.size, redirects, stop).map_err(|why| ("download", why))?;
    let size = std::fs::metadata(to).map(|m| m.len()).unwrap_or(0);
    if size != file.size {
        let why = format!(
            "{} is {size} bytes; its release says {}",
            file.url, file.size
        );
        return Err(("checksum", why));
    }
    let sha = sha256_file(to).map_err(|e| ("download", format!("it wasn't read back: {e}")))?;
    if sha != file.sha256 {
        let why = format!("its SHA-256 is {sha}; its release says {}", file.sha256);
        return Err(("checksum", why));
    }
    Ok(())
}

/// Lowercase hex SHA-256.
pub fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
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

/// Whether `sha` is a SHA-256 as a release writes it: 64 hex characters.
pub fn is_sha256(sha: &str) -> bool {
    sha.len() == 64 && sha.bytes().all(|b| b.is_ascii_hexdigit())
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

/// A small text file, within 30 s or until `stop` is set.
pub fn fetch(url: &str, redirects: Redirects, stop: &AtomicBool) -> Result<String, String> {
    let mut command = curl(
        redirects,
        &["--max-time", "30", "--max-filesize", "1048576"],
    );
    command.arg(url);
    run_until(command, stop)
}

/// curl, failing on any HTTP error.
fn curl(redirects: Redirects, limits: &[&str]) -> Command {
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
    if let Redirects::Followed = redirects {
        command.args(["--location", "--max-redirs", "5", "--proto-redir", "=https"]);
    }
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

/// Shared transfer for an npm archive (SHA-512) and a release file (SHA-256).
pub(crate) fn transfer(
    url: &str,
    to: &Path,
    max_size: u64,
    redirects: Redirects,
    stop: &AtomicBool,
) -> Result<(), String> {
    let size = max_size.to_string();
    let mut command = curl(
        redirects,
        &["--max-filesize", &size, "--speed-limit", "1024"],
    );
    command
        .args(["--speed-time", "60", "--output"])
        .arg(to)
        .arg(url);
    run_until(command, stop).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::parse_version;

    #[test]
    fn versions_compare_by_number() {
        assert!(parse_version("2.1.100") > parse_version("2.1.99"));
        assert_eq!(parse_version("2.1"), None);
        assert_eq!(parse_version("2.1.x"), None);
    }
}
