//! One running Claude Code: its stdin, its output lines, and a close that never kills while
//! the engine might be renewing its login (0001, 0002).

use std::ffi::OsString;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin};
use tokio::sync::mpsc;

pub enum Line {
    Out(String),
    Err(String),
}

pub struct Engine {
    pub pid: u32,
    child: Child,
    stdin: Option<ChildStdin>,
}

/// How a close ended.
pub enum Closed {
    Exited(Option<i32>),
    /// It hadn't exited within the grace period, so it was killed.
    Killed,
}

impl Engine {
    /// Starts the engine. Its output arrives on the receiver, one line at a time, and the
    /// receiver ends when the engine has closed both stdout and stderr.
    pub fn start(
        program: &Path,
        args: &[String],
        env: Vec<(OsString, OsString)>,
        cwd: &Path,
    ) -> std::io::Result<(Engine, mpsc::UnboundedReceiver<Line>)> {
        let mut command = tokio::process::Command::new(program);
        command
            .args(args)
            .env_clear()
            .envs(env)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::platform::no_window(command.as_std_mut());
        let mut child = command.spawn()?;
        let (tx, rx) = mpsc::unbounded_channel();
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");
        tokio::spawn(forward(stdout, tx.clone(), Line::Out));
        tokio::spawn(forward(stderr, tx, Line::Err));
        let engine = Engine {
            pid: child.id().unwrap_or(0),
            stdin: child.stdin.take(),
            child,
        };
        Ok((engine, rx))
    }

    /// Writes one frame as a line.
    pub async fn write(&mut self, frame: &Value) -> std::io::Result<()> {
        self.write_line(&frame.to_string()).await
    }

    pub async fn write_line(&mut self, text: &str) -> std::io::Result<()> {
        let stdin = self
            .stdin
            .as_mut()
            .ok_or_else(|| std::io::Error::other("stdin is closed"))?;
        stdin.write_all(format!("{text}\n").as_bytes()).await?;
        stdin.flush().await
    }

    /// The gentle close: stdin closed, then the engine's own exit. Killed only after `grace`.
    pub async fn close(mut self, grace: Duration) -> Closed {
        drop(self.stdin.take());
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(status) => Closed::Exited(status.ok().and_then(|s| s.code())),
            Err(_) => {
                let _ = self.child.kill().await;
                Closed::Killed
            }
        }
    }

    /// Whether it has exited, with its code.
    pub fn exited(&mut self) -> Option<Option<i32>> {
        self.child.try_wait().ok().flatten().map(|s| s.code())
    }
}

async fn forward<R: tokio::io::AsyncRead + Unpin>(
    stream: R,
    tx: mpsc::UnboundedSender<Line>,
    wrap: fn(String) -> Line,
) {
    // Split on bytes, not `lines()`: one invalid UTF-8 byte would end `lines()` and leave the
    // engine blocked on a full pipe.
    let mut lines = BufReader::new(stream).split(b'\n');
    while let Ok(Some(bytes)) = lines.next_segment().await {
        let line = String::from_utf8_lossy(&bytes)
            .trim_end_matches('\r')
            .to_string();
        if tx.send(wrap(line)).is_err() {
            return;
        }
    }
}
