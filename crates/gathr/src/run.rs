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

    fn trimmed(mut self) -> Self {
        self.out = self.out.trim().to_string();
        self
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
    started(program, args, dir, env, limit, false)
        .await
        .map(Ran::trimmed)
}

/// `run`, with what the program printed kept as it printed it: a diff's last line can be blank.
pub async fn printed(
    program: &Path,
    args: &[&str],
    dir: &Path,
    env: &Vars,
    limit: Duration,
) -> Result<Ran, String> {
    started(program, args, dir, env, limit, false).await
}

/// The person's environment now (G23), as engines and git get it.
pub async fn fresh() -> Vars {
    tokio::task::spawn_blocking(|| crate::platform::env::fresh(std::env::vars_os().collect()).vars)
        .await
        .unwrap_or_else(|_| std::env::vars_os().collect())
}

/// Asks GitHub through `gh` in `dir`, with the person's environment now, but never its debug output:
/// `GH_DEBUG` prints the HTTP traffic, headers and all, into what an error can show an agent (0046).
pub async fn github(args: &[&str], dir: &Path) -> Result<String, String> {
    let mut vars = fresh().await;
    vars.retain(|(name, _)| !name.eq_ignore_ascii_case("GH_DEBUG"));
    gh(args, dir, &vars).await
}

/// Runs `gh`, once more if it fails (G5): what it printed, or why it failed.
pub async fn gh(args: &[&str], dir: &Path, vars: &Vars) -> Result<String, String> {
    // GitHub's `gh`, or what `GATHR_GH_PATH` names in its place, in dev and tests only (the
    // replay harness's `fake_gh`).
    let program = std::env::var_os("GATHR_GH_PATH").unwrap_or_else(|| "gh".into());
    let (program, limit) = (Path::new(&program), Duration::from_secs(60));
    let mut why = String::new();
    for _ in 0..2 {
        match run(program, args, dir, vars, limit).await {
            Ok(ran) if ran.code == Some(0) => return Ok(ran.out),
            Ok(ran) => why = format!("gh {} failed: {}", args[0], ran.reason()),
            Err(e) => why = e,
        }
    }
    Err(why)
}

/// Runs `script` in `shell` as `run` does, with the shell kept until what it printed is read. A
/// script dropped before then (Archive, cancel, its limit) takes everything it started with it,
/// even what it left running after its own shell ended (1216).
pub async fn script(
    shell: &Path,
    script: &str,
    dir: &Path,
    env: &Vars,
    limit: Duration,
) -> Result<Ran, String> {
    let name = shell.file_stem().unwrap_or_default().to_string_lossy();
    let args = ["-c", HOLD, &name, script];
    started(shell, &args, dir, env, limit, true)
        .await
        .map(Ran::trimmed)
}

/// The script runs in a subshell, so its own `exit` can't end the shell, which then waits for the
/// end of its input: the service closes it once the script's output is read.
const HOLD: &str = r#"(eval "$1") </dev/null; s=$?; exec >&- 2>&-; read -r _; exit $s"#;

async fn started(
    program: &Path,
    args: &[&str],
    dir: &Path,
    env: &Vars,
    limit: Duration,
    held: bool,
) -> Result<Ran, String> {
    let name = program.file_stem().unwrap_or_default().to_string_lossy();
    let what = format!("{name} {}", args.first().unwrap_or(&""));
    let pipes = std::io::pipe().and_then(|out| Ok((out, std::io::pipe()?)));
    let ((stdout, out_end), (stderr, err_end)) =
        pipes.map_err(|e| format!("{name} didn't start: {e}"))?;
    let hold = held.then(std::io::pipe).transpose();
    let (input, hold) = match hold.map_err(|e| format!("{name} didn't start: {e}"))? {
        Some((input, hold)) => (Stdio::from(input), Some(hold)),
        None => (Stdio::null(), None),
    };
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .current_dir(dir)
        .env_clear()
        .envs(env.iter().cloned())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("GH_PROMPT_DISABLED", "1")
        .stdin(input)
        .stdout(out_end)
        .stderr(err_end);
    // The command goes with this statement, and with it the service's ends of the pipes.
    let child = crate::platform::ending_together(command.into()).spawn();
    let child = child.map_err(|e| format!("{name} didn't start: {e}"))?;
    let (stdout, stderr) = (read_all(stdout), read_all(stderr));
    let mut running = Running {
        child,
        hold,
        done: false,
    };
    let (child, hold) = (&mut running.child, &mut running.hold);
    // Waited for only once its output ends: on Unix a program not yet waited for keeps its pid,
    // and with it its group's id, from any other process.
    let waited = async {
        let (out, err) = tokio::join!(stdout, stderr);
        hold.take();
        let status = child.wait().await?;
        let read = |got: Result<std::io::Result<Vec<u8>>, oneshot::error::RecvError>| {
            got.map_err(std::io::Error::other).and_then(|read| read)
        };
        std::io::Result::Ok((status, read(out)?, read(err)?))
    };
    let (status, out, err) = tokio::time::timeout(limit, waited)
        .await
        .map_err(|_| format!("{what} didn't finish in {} s", limit.as_secs()))?
        .map_err(|e| format!("{name} wasn't waited for: {e}"))?;
    running.done = true;
    Ok(Ran {
        code: status.code(),
        out: String::from_utf8_lossy(&out).into_owned(),
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

/// A program dropped while it runs (past its limit, or its wait ended) is ended with everything it
/// started (`platform::ending_together`): a child left behind would keep the output pipe open.
/// `hold` (a script's shell) goes only after them.
struct Running {
    child: Box<dyn process_wrap::tokio::ChildWrapper>,
    hold: Option<std::io::PipeWriter>,
    done: bool,
}

impl Drop for Running {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.start_kill();
        }
    }
}
