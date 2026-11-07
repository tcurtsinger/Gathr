//! Stands in for Claude Code in tests: plays back a recorded session (`tests/frames/*.jsonl`).
//! Each recorded `in` line waits for a line on stdin; each `out` or `err` line is written as it
//! was recorded. So the service's driver meets the engine's real frames, in their real order.
//!
//! `GATHR_REPLAY` names the recording; `GATHR_REPLAY_ARGS`, if set, is a file each run appends
//! its arguments to, as one JSON line.

use std::io::{BufRead, Write};

fn main() {
    let recording = std::env::var("GATHR_REPLAY").expect("GATHR_REPLAY names a recording");
    if let Ok(file) = std::env::var("GATHR_REPLAY_ARGS") {
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .expect("the args file opens");
        writeln!(out, "{}", serde_json::to_string(&args).unwrap()).unwrap();
    }
    let text = std::fs::read_to_string(&recording).expect("the recording reads");
    let mut stdin = std::io::stdin().lock().lines();
    let (mut stdout, mut stderr) = (std::io::stdout().lock(), std::io::stderr().lock());
    for row in text.lines().filter(|l| !l.trim().is_empty()) {
        let row: serde_json::Value = serde_json::from_str(row).expect("a recording row is JSON");
        let line = row["line"].as_str().unwrap_or_default();
        match row["dir"].as_str() {
            Some("in") => match stdin.next() {
                Some(Ok(_)) => {}
                _ => return,
            },
            Some("out") => {
                writeln!(stdout, "{line}").unwrap();
                stdout.flush().unwrap();
            }
            Some("err") => writeln!(stderr, "{line}").unwrap(),
            _ => {}
        }
    }
    // Like Claude Code: done when its input closes.
    for line in stdin {
        if line.is_err() {
            return;
        }
    }
}
