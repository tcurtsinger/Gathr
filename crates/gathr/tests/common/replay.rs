//! Claude Code's stand-in played turn by turn, and what each engine read: for tests that send the
//! person's words to several agents and look at what each was told.

use std::path::{Path, PathBuf};

/// Claude Code's stand-in for `n` turns of the signed-in recording, each turn in `held` waiting for
/// `go` once its message arrives. Every engine plays the file from its first turn.
pub fn turns(data: &Path, held: &[usize], n: usize) -> PathBuf {
    let frames = std::fs::read_to_string(super::recording("signed_in")).unwrap();
    let lines: Vec<&str> = frames.lines().collect();
    let mut all = Vec::new();
    for turn in 0..n {
        all.push(lines[0]);
        if held.contains(&turn) {
            all.push(r#"{"dir":"hold"}"#);
        }
        all.extend(&lines[1..]);
    }
    let file = data.join("turns.jsonl");
    std::fs::write(&file, all.join("\n") + "\n").unwrap();
    file
}

/// Lets every held engine go when the test ends, passed or failed.
pub struct LetGo(pub PathBuf);

impl Drop for LetGo {
    fn drop(&mut self) {
        let _ = std::fs::write(&self.0, "");
    }
}

/// What the engine read in `turn`, joined.
pub fn read_in(data: &Path, turn: &str) -> String {
    let sql = format!(
        "SELECT body FROM engine_raw WHERE direction = 'in' AND turn_id = '{turn}' ORDER BY id"
    );
    let bodies = super::rows(data, &sql);
    let bodies = bodies.iter().filter_map(|r| r["body"].as_str());
    bodies.collect::<Vec<_>>().join("\n")
}

/// Words as JSON escapes them in an engine's frame.
pub fn escaped(words: &str) -> String {
    let json = serde_json::to_string(words).unwrap();
    json.trim_matches('"').to_string()
}
