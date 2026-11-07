//! An account's login state, read without starting a turn, and the renewal gate
//! (docs/decisions/0002-login-folders.md). Ported from the previous app's `claude-login.ts`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde::Deserialize;

/// A token that runs out within this long is renewed by the next start.
const RENEW_AHEAD_MS: i64 = 5 * 60_000;
/// How long the rest wait for the one renewing it.
const RENEW_WAIT: Duration = Duration::from_secs(30);
const RENEW_POLL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Login {
    SignedIn,
    /// The access token has run out but a refresh token is there: the next request renews it.
    Stale,
    SignedOut,
    /// The file is there but can't be read as it stands (being written).
    Unknown,
}

/// `.credentials.json` as Claude Code keeps it on Windows and Linux. Only whether each token
/// is there is kept: the tokens themselves are dropped as they're read.
#[derive(Deserialize)]
struct File {
    #[serde(rename = "claudeAiOauth")]
    oauth: Option<OAuth>,
}

#[derive(Deserialize)]
struct OAuth {
    #[serde(rename = "accessToken", default)]
    access: Present,
    #[serde(rename = "refreshToken", default)]
    refresh: Present,
    #[serde(rename = "expiresAt")]
    expires_at: Option<i64>,
}

#[derive(Default)]
struct Present(bool);

impl<'de> Deserialize<'de> for Present {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Present(!String::deserialize(d)?.is_empty()))
    }
}

/// The login file in an account's Claude folder.
pub fn credentials(folder: &Path) -> PathBuf {
    folder.join(".credentials.json")
}

/// The state, and when the access token runs out (ms since the epoch), from the login file.
pub fn read(file: &Path, now_ms: i64) -> (Login, Option<i64>) {
    let bytes = match std::fs::read(file) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Login::SignedOut, None),
        Err(_) => return (Login::Unknown, None),
    };
    let Ok(parsed) = serde_json::from_slice::<File>(&bytes) else {
        return (Login::Unknown, None);
    };
    let Some(oauth) = parsed.oauth else {
        return (Login::SignedOut, None);
    };
    let current = oauth.access.0 && oauth.expires_at.is_some_and(|at| at > now_ms);
    let state = match (current, oauth.refresh.0) {
        (true, _) => Login::SignedIn,
        (false, true) => Login::Stale,
        (false, false) => Login::SignedOut,
    };
    (state, oauth.expires_at)
}

/// The account's state, for sends, Settings and `gathr diag` alike: the login file, or on
/// macOS, where the login is in the Keychain (0016), `claude auth status`. Blocks while the
/// probe runs; the service calls it off its async threads.
pub fn status(
    folder: &Path,
    path_setting: Option<&str>,
    copy: Option<std::path::PathBuf>,
    now_ms: i64,
) -> (Login, Option<i64>) {
    let from_file = read(&credentials(folder), now_ms);
    if !cfg!(target_os = "macos") || from_file.0 != Login::SignedOut {
        return from_file;
    }
    match super::command::find(path_setting, copy) {
        Ok(program) => (probe(&program, folder), None),
        Err(_) => (Login::Unknown, None),
    }
}

/// `claude auth status` in the account's folder: exits 0 signed in, 1 signed out. Never
/// killed: it may be renewing.
fn probe(program: &Path, folder: &Path) -> Login {
    let mut command = std::process::Command::new(program);
    command
        .args(["auth", "status"])
        .env_clear()
        .envs(super::command::env(std::env::vars_os(), folder))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    crate::platform::no_window(&mut command);
    match command.status().map(|s| s.code()) {
        Ok(Some(0)) => Login::SignedIn,
        Ok(Some(1)) => Login::SignedOut,
        _ => Login::Unknown,
    }
}

fn needs_renewal(file: &Path) -> bool {
    let now = crate::ids::now_us() / 1000;
    match read(file, now) {
        (Login::SignedIn | Login::Stale, Some(at)) => at <= now + RENEW_AHEAD_MS,
        _ => false,
    }
}

/// One per account. With a login file, while it needs renewing, one start or turn goes ahead
/// and renews it; the rest wait until the file holds a fresh expiry, or 30 s pass. With none
/// (the macOS Keychain, 0016), the expiry can't be seen, so every start goes one at a time:
/// the next waits until the one ahead gets its first reply from the provider, fails, or 30 s pass.
#[derive(Clone)]
pub struct Gate {
    file: PathBuf,
    held: Arc<tokio::sync::Mutex<()>>,
}

/// Holds the gate for the start that passed, until it's released or dropped (the turn failed,
/// or the engine exited). Holds nothing when the login is in a file.
#[derive(Debug, Default)]
pub struct Hold(Option<tokio::sync::oneshot::Sender<()>>);

impl Hold {
    /// The provider answered: the login works, so the next start may go.
    pub fn release(&mut self) {
        self.0.take();
    }
}

impl Gate {
    pub fn new(folder: &Path) -> Gate {
        Gate {
            file: credentials(folder),
            held: Arc::default(),
        }
    }

    /// Waits for this start's turn.
    pub async fn pass(&self) -> Hold {
        let guard = self.held.clone().lock_owned().await;
        if !self.file.exists() {
            let (hold, released) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(RENEW_WAIT, released).await;
                drop(guard);
            });
            return Hold(Some(hold));
        }
        if !needs_renewal(&self.file) {
            return Hold::default();
        }
        let file = self.file.clone();
        tokio::spawn(async move {
            let until = tokio::time::Instant::now() + RENEW_WAIT;
            while tokio::time::Instant::now() < until && needs_renewal(&file) {
                tokio::time::sleep(RENEW_POLL).await;
            }
            drop(guard);
        });
        Hold::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("gathr-login-{name}-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(dir: &Path, access: &str, refresh: &str, expires_at: i64) {
        let body = serde_json::json!({ "claudeAiOauth": {
            "accessToken": access, "refreshToken": refresh, "expiresAt": expires_at } });
        std::fs::write(credentials(dir), body.to_string()).unwrap();
    }

    #[test]
    fn each_login_file_reads_as_its_state() {
        let dir = folder("states");
        let file = credentials(&dir);
        assert_eq!(read(&file, 1_000).0, Login::SignedOut, "no file");
        write(&dir, "a", "r", 2_000);
        assert_eq!(read(&file, 1_000), (Login::SignedIn, Some(2_000)));
        assert_eq!(
            read(&file, 3_000).0,
            Login::Stale,
            "expired, refresh token there"
        );
        write(&dir, "", "", 2_000);
        assert_eq!(
            read(&file, 1_000).0,
            Login::SignedOut,
            "blanked by the engine"
        );
        std::fs::write(&file, "{\"claudeAiOauth\": {\"accessTo").unwrap();
        assert_eq!(read(&file, 1_000).0, Login::Unknown, "half written");
        std::fs::write(&file, "{\"mcpOAuth\": {}}").unwrap();
        assert_eq!(
            read(&file, 1_000).0,
            Login::SignedOut,
            "connectors' sign-ins only"
        );
    }

    #[tokio::test]
    async fn the_gate_holds_the_rest_until_the_renewal_lands() {
        let dir = folder("gate");
        let now = crate::ids::now_us() / 1000;
        write(&dir, "a", "r", now + 60_000);
        let gate = Gate::new(&dir);
        gate.pass().await;
        let second = tokio::spawn({
            let gate = gate.clone();
            async move { gate.pass().await }
        });
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(
            !second.is_finished(),
            "a second start went ahead while the first renews"
        );
        write(&dir, "a2", "r2", now + 3_600_000);
        tokio::time::timeout(Duration::from_secs(2), second)
            .await
            .expect("the second start still waits after the renewal landed")
            .unwrap();
    }

    #[tokio::test]
    async fn with_no_login_file_each_start_waits_for_the_first_reply_ahead() {
        let gate = Gate::new(&folder("keychain"));
        let next = |gate: &Gate| {
            let gate = gate.clone();
            tokio::spawn(async move { gate.pass().await })
        };
        let mut first = gate.pass().await;
        let second = next(&gate);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !second.is_finished(),
            "a second start went ahead before the first got a reply"
        );
        first.release();
        let held = tokio::time::timeout(Duration::from_secs(2), second)
            .await
            .expect("the second start still waits after the first reply")
            .unwrap();
        let third = next(&gate);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !third.is_finished(),
            "a third start didn't wait for the second"
        );
        drop(held);
        tokio::time::timeout(Duration::from_secs(2), third)
            .await
            .expect("a start that failed or exited didn't let the next one go")
            .unwrap();
    }

    #[tokio::test]
    async fn a_current_login_never_waits() {
        let dir = folder("current");
        write(&dir, "a", "r", crate::ids::now_us() / 1000 + 3_600_000);
        let gate = Gate::new(&dir);
        for _ in 0..3 {
            tokio::time::timeout(Duration::from_millis(200), gate.pass())
                .await
                .expect("a start waited on a current login");
        }
    }
}
