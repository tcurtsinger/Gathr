//! Stands in for Claude Code in tests: plays back a recorded session (`tests/frames/*.jsonl`).
//! Each recorded `in` line waits for a line on stdin; each `out` or `err` line is written as it
//! was recorded; an `exit` line ends it with that code. So the service's driver meets the engine's real frames, in their real order.
//!
//! `GATHR_REPLAY` names the recording; `GATHR_REPLAY_ARGS`, if set, is a file each run appends
//! its arguments to, as one JSON line.
//!
//! `auth status` answers as Claude Code does where its login is a file: exit 0 when
//! `CLAUDE_CONFIG_DIR` holds `.credentials.json`, 1 when it doesn't. It isn't a run.
//!
//! `--version` answers `GATHR_REPLAY_VERSION`, else the version the recordings were made with;
//! `stall` there answers only after a minute, as a launcher that hangs; `folder` answers the name
//! of the folder it runs from, as Gathr lays out its copies (0025).
//!
//! An `initialize` request is answered `success`, as Claude Code does, and isn't one of the
//! recording's `in` lines.

use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        let version = std::env::var("GATHR_REPLAY_VERSION");
        if version.as_deref() == Ok("stall") {
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        if version.as_deref() == Ok("folder") {
            let exe = std::env::current_exe().unwrap();
            let folder = exe.parent().and_then(|p| p.file_name()).unwrap_or_default();
            println!("{} (Claude Code)", folder.to_string_lossy());
            return;
        }
        println!("{}", version.as_deref().unwrap_or("2.1.318 (Claude Code)"));
        return;
    }
    if args.iter().map(String::as_str).eq(["auth", "status"]) {
        // `GATHR_REPLAY_AUTH=0` answers signed in with no file, as the macOS Keychain does.
        if let Ok(code) = std::env::var("GATHR_REPLAY_AUTH") {
            std::process::exit(code.parse().unwrap_or(1));
        }
        let folder = std::env::var_os("CLAUDE_CONFIG_DIR").unwrap_or_default();
        let signed_in = std::path::Path::new(&folder)
            .join(".credentials.json")
            .is_file();
        std::process::exit(if signed_in { 0 } else { 1 });
    }
    let recording = std::env::var("GATHR_REPLAY").expect("GATHR_REPLAY names a recording");
    if let Ok(file) = std::env::var("GATHR_REPLAY_ARGS") {
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
            Some("in") => loop {
                match stdin.next() {
                    Some(Ok(line)) if answered_initialize(&line, &mut stdout) => {}
                    Some(Ok(_)) => break,
                    _ => return,
                }
            },
            Some("out") => {
                writeln!(stdout, "{line}").unwrap();
                stdout.flush().unwrap();
            }
            Some("err") => writeln!(stderr, "{line}").unwrap(),
            Some("exit") => std::process::exit(line.parse().unwrap_or(1)),
            _ => {}
        }
    }
    // Like Claude Code: done when its input closes.
    for line in stdin {
        match line {
            Ok(line) => _ = answered_initialize(&line, &mut stdout),
            Err(_) => return,
        }
    }
}

/// Answers an `initialize` request with an empty `success`; false for any other line.
fn answered_initialize(line: &str, stdout: &mut impl Write) -> bool {
    let frame: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    if frame["request"]["subtype"] != "initialize" {
        return false;
    }
    let id = &frame["request_id"];
    let answer = serde_json::json!({ "type": "control_response",
        "response": { "subtype": "success", "request_id": id, "response": {} } });
    writeln!(stdout, "{answer}").unwrap();
    stdout.flush().unwrap();
    true
}
