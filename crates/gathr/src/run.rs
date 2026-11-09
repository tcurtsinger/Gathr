//! The one way the service runs a short program and reads what it printed: git, gh and a wait's
//! check. Never asks for a password, has no window, and is stopped past its time limit.

use std::io::Read;
use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::platform::env::Vars;

/// How a program ended: its exit code (none when a signal ended it) and what it printed.
pub struct Ran {
    pub code: Option<i32>,
    pub out: String,
    pub err: String,
}

impl Ran {
    /// The line of what it said that gives the reason: a `fatal:` or `error:` line, else the last.
    pub fn reason(&self) -> &str {
        let lines: Vec<&str> = self
            .err
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .collect();
        let found = lines
            .iter()
            .find(|l| l.starts_with("fatal:") || l.starts_with("error:"));
        found.or(lines.last()).copied().unwrap_or("")
    }
}

/// Runs `program` in `dir` with exactly `env`: how it ended, or why it didn't start or finish.
pub async fn run(
    program: &Path,
    args: &[&str],
    dir: &Path,
    env: &Vars,
    limit: Duration,
) -> Result<Ran, String> {
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    let what = format!("{name} {}", args.first().unwrap_or(&""));
    let pipes = std::io::pipe().and_then(|out| Ok((out, std::io::pipe()?)));
    let ((stdout, out_end), (stderr, err_end)) =
        pipes.map_err(|e| format!("{name} didn't start: {e}"))?;
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(Stdio::null())
        .stdout(out_end)
        .stderr(err_end);
    crate::platform::no_window(&mut command);
    // The command goes with this statement, and with it the service's ends of the pipes.
    let child = tokio::process::Command::from(command).spawn();
    let child = child.map_err(|e| format!("{name} didn't start: {e}"))?;
    let (stdout, stderr) = (read_all(stdout), read_all(stderr));
    let group = crate::platform::group_of(child.id(), program);
    let mut running = Running {
        child,
        group: None,
        program,
        done: false,
    };
    let (child, found) = (&mut running.child, &mut running.group);
    let waited = async {
        let (out, err, status) = tokio::join!(stdout, stderr, child.wait());
        let read = |got: Result<std::io::Result<Vec<u8>>, oneshot::error::RecvError>| {
            got.map_err(std::io::Error::other).and_then(|read| read)
        };
        std::io::Result::Ok((status?, read(out)?, read(err)?))
    };
    tokio::pin!(waited, group);
    let waited = async {
        tokio::select! {
            ran = &mut waited => return ran,
            group = &mut group => *found = group,
        }
        waited.await
    };
    let (status, out, err) = tokio::time::timeout(limit, waited)
        .await
        .map_err(|_| format!("{what} didn't finish in {} s", limit.as_secs()))?
        .map_err(|e| format!("{name} wasn't waited for: {e}"))?;
    running.done = true;
    Ok(Ran {
        code: status.code(),
        out: String::from_utf8_lossy(&out).trim().to_string(),
        err: String::from_utf8_lossy(&err).to_string(),
    })
}

/// All a pipe gives until every end that writes to it is closed, read on a thread of its own: never
/// the runtime's blocking threads, which it waits for when it ends. On Windows, Git Bash ended while
/// it forks leaves the fork's child holding the pipe for 300 s, out of any kill's reach (1214).
fn read_all(mut pipe: std::io::PipeReader) -> oneshot::Receiver<std::io::Result<Vec<u8>>> {
    let (read, got) = oneshot::channel();
    std::thread::spawn(move || {
        let mut all = Vec::new();
        let _ = read.send(pipe.read_to_end(&mut all).map(|_| all));
    });
    got
}

/// A program dropped while it runs (past its limit, or its wait ended) is ended with what it
/// started: a child left behind keeps the output pipe open, and on Windows the service can't exit
/// until that child does. Once its group is known the program is killed first, so it stops at
/// once; before that, its group is found through it, so it's killed last.
struct Running<'a> {
    child: tokio::process::Child,
    group: Option<u32>,
    program: &'a Path,
    done: bool,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        if self.done {
            return;
        }
        if self.group.is_some() {
            let _ = self.child.start_kill();
        }
        if let Some(pid) = self.child.id() {
            crate::platform::end_group(self.group, pid, self.program);
        }
        let _ = self.child.start_kill();
    }
}
