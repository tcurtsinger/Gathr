//! `gathr::run`, the one way the service runs a short program.
mod common;

use std::path::Path;
use std::time::{Duration, Instant};

use gathr::platform::{env::Vars, shell};
use gathr::run::{run, script};

/// Git Bash ended while it forks leaves the fork's child waiting for it for 300 s (MSYS2's
/// `FORK_WAIT_TIMEOUT`), holding the program's output pipe. The service read that pipe on the
/// runtime's blocking threads, and a runtime doesn't end until they do: 300 s.
#[test]
fn a_program_ended_while_it_forks_never_holds_up_the_end_of_the_runtime() {
    let strays = Strays(gathr::ids::span_id());
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(end_forty_mid_fork(&strays.0));
    let ending = Instant::now();
    drop(runtime);
    let took = ending.elapsed();
    assert!(
        took < Duration::from_secs(10),
        "the runtime took {took:?} to end"
    );
}

/// Nothing a check started outlives it, not even a child Git Bash was starting when it was ended,
/// which no process tree or MSYS group lists yet (1219).
#[tokio::test(flavor = "multi_thread")]
async fn a_program_ended_while_it_forks_leaves_no_process_behind() {
    let strays = Strays(gathr::ids::span_id());
    end_forty_mid_fork(&strays.0).await;
    tokio::time::sleep(Duration::from_secs(1)).await;
    let left = strays.left();
    assert!(left.is_empty(), "left running: {left:?}");
}

/// Runs a shell loop that forks without end forty times, each ended 300 ms in: a kill lands
/// mid-fork about one time in five, so forty make it all but certain. `mark` is in each command line.
async fn end_forty_mid_fork(mark: &str) {
    let vars: Vars = std::env::vars_os().collect();
    let (shell, vars) = shell(&vars).expect("Git Bash beside git, or sh");
    let forever = format!("while :; do /usr/bin/true; done # {mark}");
    let dir = std::env::temp_dir();
    let args = ["-c", forever.as_str()];
    let forking = || async {
        let ran = run(
            &shell,
            &args,
            Path::new(&dir),
            &vars,
            Duration::from_millis(300),
        );
        let ended = ran.await.err().expect("a loop never finishes");
        assert!(ended.ends_with("didn't finish in 0 s"), "{ended}");
    };
    for _ in 0..10 {
        tokio::join!(forking(), forking(), forking(), forking());
    }
}
/// A check dropped after its shell has exited (Archive, cancel, its time limit) still ends what it
/// left running: the group outlives its leader (1216).
#[tokio::test]
async fn a_check_dropped_after_its_shell_exits_ends_what_it_left_running() {
    let vars: Vars = std::env::vars_os().collect();
    let (shell, vars) = shell(&vars).expect("Git Bash beside git, or sh");
    let dir = std::env::temp_dir().join(gathr::ids::span_id());
    std::fs::create_dir(&dir).unwrap();
    let left = "(sleep 3; printf late > late) & exit 0";
    let ran = script(&shell, left, &dir, &vars, Duration::from_secs(30));
    let dropped = tokio::time::timeout(Duration::from_secs(1), ran).await;
    assert!(dropped.is_err(), "the left child holds the output open");
    tokio::time::sleep(Duration::from_secs(4)).await;
    let late = dir.join("late").exists();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        !late,
        "the check's background child ran on after it was dropped"
    );
}

/// The forks' children left waiting, found by the mark in their command line (a copy of their
/// shell's). Each holds this test's own output, which cargo reads to the end, so they're ended here.
struct Strays(String);

impl Strays {
    /// The process ids still running with the mark.
    fn left(&self) -> Vec<String> {
        let out = if cfg!(windows) {
            let find = format!(
                "Get-CimInstance Win32_Process | Where-Object CommandLine -like '*{}*' | \
                 Where-Object Name -ne 'powershell.exe' | ForEach-Object {{ $_.ProcessId }}",
                self.0
            );
            std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &find])
                .output()
        } else {
            std::process::Command::new("pgrep")
                .args(["-f", &self.0])
                .output()
        };
        let out = out.expect("the process list");
        String::from_utf8_lossy(&out.stdout)
            .split_whitespace()
            .map(String::from)
            .collect()
    }
}

impl Drop for Strays {
    fn drop(&mut self) {
        for pid in self.left() {
            common::kill(&pid);
        }
    }
}
