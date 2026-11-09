//! `gathr::run`, the one way the service runs a short program.

use std::path::Path;
use std::time::{Duration, Instant};

use gathr::platform::{env::Vars, shell};
use gathr::run::run;

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
