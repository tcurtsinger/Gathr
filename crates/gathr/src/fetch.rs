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
    retried(stop, || {
        let mut command = curl(
            redirects,
            &["--max-time", "30", "--max-filesize", "1048576"],
        );
        command.arg(url);
        command
    })
}

/// curl's exit codes for a connection that dropped or never came: worth another try.
const TRANSIENT: [i32; 8] = [
    6,  // couldn't resolve the host
    7,  // couldn't connect
    18, // the transfer ended early
    28, // timed out
    35, // the TLS handshake failed, or the connection was reset during it
    52, // the server answered nothing
    55, // sending failed
    56, // receiving failed, such as a connection reset
];
/// Tries after the first, waiting 2 s then 4 s.
const RETRIES: u32 = 2;

/// `run_until` on a fresh `command()`, tried again when the network dropped (`TRANSIENT`), so one
/// reset connection doesn't fail a check or a download.
fn retried(stop: &AtomicBool, command: impl Fn() -> Command) -> Result<String, String> {
    let mut tries = 0;
    loop {
        match run_until(command(), stop) {
            Err((Some(code), _)) if TRANSIENT.contains(&code) && tries < RETRIES => {
                tries += 1;
                let until = std::time::Instant::now() + Duration::from_secs(2 << (tries - 1));
                while std::time::Instant::now() < until {
                    if stop.load(Ordering::Relaxed) {
                        return Err("the service stopped".into());
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            done => return done.map_err(|(_, why)| why),
        }
    }
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

/// curl's output to its end, or curl killed once `stop` is set; its exit code and error line if it
/// fails.
fn run_until(mut command: Command, stop: &AtomicBool) -> Result<String, (Option<i32>, String)> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| (None, format!("curl didn't start: {e}")))?;
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
            return Err((None, "the service stopped".into()));
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err((None, format!("curl wasn't waited for: {e}"))),
        }
    };
    let (out, error) = (stdout.join(), stderr.join());
    if status.success() {
        return Ok(out.unwrap_or_default());
    }
    let why = format!("{} ({status})", error.unwrap_or_default().trim());
    Err((status.code(), why))
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
    retried(stop, || {
        let mut command = curl(
            redirects,
            &["--max-filesize", &size, "--speed-limit", "1024"],
        );
        command
            .args(["--speed-time", "60", "--output"])
            .arg(to)
            .arg(url);
        command
    })
    .map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::{Redirects, fetch, parse_version};
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn a_connection_dropped_once_is_tried_again() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/gathrd.json", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            // The first connection closes without an answer (curl: 52), the second answers.
            drop(listener.accept().unwrap());
            let (mut second, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            let _ = second.read(&mut request);
            let body = "ok";
            let answer = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            second.write_all(answer.as_bytes()).unwrap();
        });
        let got = fetch(&url, Redirects::Refused, &AtomicBool::new(false));
        assert_eq!(got.as_deref(), Ok("ok"));
        server.join().unwrap();
    }

    #[test]
    fn versions_compare_by_number() {
        assert!(parse_version("2.1.100") > parse_version("2.1.99"));
        assert_eq!(parse_version("2.1"), None);
        assert_eq!(parse_version("2.1.x"), None);
    }
}
