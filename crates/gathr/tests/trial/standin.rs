//! The trial on the replay stand-in engines (`GATHR_TRIAL_STANDIN`), to prove the harness before any
//! real token is spent. An odd block's engines play their recordings through: Claude Code's
//! `signed_in` up to five turns, Codex's `codex_signed_in` each turn. An even block's Claude Code asks
//! an approval (`approval`) and its Codex holds until stopped, so a run is spoiled and one stops at
//! its time limit.

use std::path::{Path, PathBuf};

use super::common::{held_recording, recording, replay_engine};
use super::setup::dir;

fn folder(data: &Path) -> PathBuf {
    dir(data).join("standin")
}

/// The replay engine copied into `at` with the recording it plays beside it, as an installed
/// service's engines find theirs: Claude Code's and Codex's can then play different ones.
fn engine(at: &Path, recorded: &Path) -> PathBuf {
    std::fs::create_dir_all(at).unwrap();
    let program = at.join(replay_engine().file_name().unwrap());
    if !program.exists() {
        std::fs::copy(replay_engine(), &program).unwrap();
    }
    std::fs::write(at.join("replay-recording"), recorded.display().to_string()).unwrap();
    program
}

/// The service's environment for block `n` on the stand-in.
pub fn env(data: &Path, n: u64) -> Vec<(&'static str, PathBuf)> {
    let at = folder(data).join(format!("block-{n}"));
    std::fs::create_dir_all(&at).unwrap();
    let (claude, codex) = match n % 2 {
        1 => {
            let turns = std::fs::read_to_string(recording("signed_in")).unwrap();
            let five = at.join("five.jsonl");
            std::fs::write(&five, turns.repeat(5)).unwrap();
            (five, recording("codex_signed_in"))
        }
        _ => (
            recording("approval"),
            held_recording(&at, "codex_signed_in"),
        ),
    };
    vec![
        ("GATHR_CLAUDE_PATH", engine(&at.join("claude"), &claude)),
        ("GATHR_CODEX_PATH", engine(&at.join("codex"), &codex)),
        // Never made: a held engine holds until its chat is stopped.
        ("GATHR_REPLAY_HOLD", folder(data).join("never")),
    ]
}
