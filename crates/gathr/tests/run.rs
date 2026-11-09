//! `gathr::run`, the one way the service runs a short program.

use std::path::Path;
use std::time::{Duration, Instant};

use gathr::platform::{env::Vars, shell};
use gathr::run::{run, script};

/// Git Bash ended while it forks leaves the fork's child waiting for it for 300 s (MSYS2's
/// `FORK_WAIT_TIMEOUT`), holding the program's output pipe, where no kill reaches it. The service
/// read that pipe on the runtime's blocking threads, and a runtime doesn't end until they do: 300 s.
#[test]
fn a_program_ended_while_it_forks_never_holds_up_the_end_of_the_runtime() {
    let vars: Vars = std::env::vars_os().collect();
    let (shell, vars) = shell(&vars).expect("Git Bash beside git, or sh");
    let strays = Strays(gathr::ids::span_id());
    let forever = format!("while :; do /usr/bin/true; done # {}", strays.0);
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
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
    // A kill lands mid-fork about one time in five: forty make it all but certain.
    runtime.block_on(async {
        for _ in 0..10 {
            tokio::join!(forking(), forking(), forking(), forking());
        }
    });
    let ending = Instant::now();
    drop(runtime);
    let took = ending.elapsed();
    assert!(
        took < Duration::from_secs(10),
        "the runtime took {took:?} to end"
    );
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
/// bash's). Each holds this test's own output, which cargo reads to the end, so they're ended here.
struct Strays(String);

impl Drop for Strays {
    fn drop(&mut self) {
        if cfg!(windows) {
            let end = format!(
                "Get-CimInstance Win32_Process -Filter \"Name='bash.exe'\" | \
                 Where-Object CommandLine -like '*{}*' | ForEach-Object {{ Stop-Process -Id $_.ProcessId -Force }}",
                self.0
            );
            let _ = std::process::Command::new("powershell")
                .args(["-NoProfile", "-Command", &end])
                .status();
        }
    }
}
