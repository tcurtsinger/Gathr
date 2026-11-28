//! Keep awake: while agents work, this computer doesn't go to sleep by itself. The screen may still
//! turn off and lock, and the person can still put it to sleep. A small helper the system ships holds
//! the request and lets go when it ends, or when this service does: Gathr's own code stays safe Rust.

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

const SETTING: &str = "keep_awake";

/// Whether Keep awake is on: on unless the person turned it off.
pub fn keep_awake(conn: &Connection) -> Result<bool, Error> {
    let saved: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING],
            |r| r.get(0),
        )
        .optional()?;
    Ok(saved.as_deref() != Some("off"))
}

pub fn set_keep_awake(conn: &Connection, on: bool) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2) ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![SETTING, if on { "on" } else { "off" }],
    )?;
    Ok(())
}

/// The system kept awake for as long as this lives.
pub struct Hold(Child);

impl Hold {
    pub fn new() -> Result<Hold, String> {
        let mut command =
            helper().ok_or("this system has no way to stay awake that Gathr knows")?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        crate::platform::no_window(&mut command);
        let child = command
            .spawn()
            .map_err(|e| format!("the keep-awake helper didn't start: {e}"))?;
        Ok(Hold(child))
    }

    /// Why the helper stopped by itself, if it did: then the system isn't kept awake.
    pub fn stopped(&mut self) -> Option<String> {
        match self.0.try_wait() {
            Ok(Some(status)) => Some(format!("the keep-awake helper stopped: {status}")),
            Ok(None) => None,
            Err(e) => Some(format!("the keep-awake helper wasn't checked: {e}")),
        }
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The helper, which also ends when this service does, however it ends. Windows: PowerShell asks
/// for the system to stay on (`ES_CONTINUOUS | ES_SYSTEM_REQUIRED`) and waits for this process.
/// macOS: `caffeinate -i`.
fn helper() -> Option<Command> {
    let me = std::process::id();
    if cfg!(windows) {
        let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
        let shell = PathBuf::from(root).join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let script = format!(
            "$k = Add-Type -Name Awake -Namespace Gathr -PassThru -MemberDefinition \
             '[DllImport(\"kernel32.dll\")] public static extern uint SetThreadExecutionState(uint f);'; \
             if ($k::SetThreadExecutionState([uint32]2147483649) -eq 0) {{ exit 1 }}; \
             Wait-Process -Id {me}"
        );
        let mut c = Command::new(shell);
        c.args(["-NoProfile", "-NonInteractive", "-Command", &script]);
        return Some(c);
    }
    if cfg!(target_os = "macos") {
        let mut c = Command::new("/usr/bin/caffeinate");
        c.args(["-i", "-w", &me.to_string()]);
        return Some(c);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Temporary: shows on each CI system that the helper really holds the system awake.
    #[test]
    fn the_helper_holds_the_system_awake() {
        if !cfg!(any(windows, target_os = "macos")) {
            return;
        }
        let mut hold = Hold::new().unwrap();
        std::thread::sleep(std::time::Duration::from_secs(8));
        assert_eq!(hold.stopped(), None);
        let (program, args): (&str, &[&str]) = if cfg!(windows) {
            ("powercfg", &["/requests"])
        } else if cfg!(target_os = "macos") {
            ("pmset", &["-g", "assertions"])
        } else {
            return;
        };
        let out = Command::new(program).args(args).output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        println!("KEEP-AWAKE CHECK\n{text}");
        drop(hold);
        let out = Command::new(program).args(args).output().unwrap();
        println!("AFTER RELEASE\n{}", String::from_utf8_lossy(&out.stdout));
        panic!("temporary: show the output");
    }
}
