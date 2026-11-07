//! Stands in for Claude Code in tests: plays back a recorded session (`tests/frames/*.jsonl`).
//! Each recorded `in` line waits for a line on stdin; each `out` or `err` line is written as it
//! was recorded; an `exit` line ends it with that code. So the service's driver meets the engine's real frames, in their real order.
//! A `sleep` row pauses that many milliseconds, for a stream at a real pace (timings, not tests).
//!
//! `GATHR_REPLAY` names the recording; `GATHR_REPLAY_ARGS`, if set, is a file each run appends
//! its arguments to, as one JSON line.
//!
//! `auth status` answers as Claude Code does where its login is a file: exit 0 when
//! `CLAUDE_CONFIG_DIR` holds `.credentials.json`, 1 when it doesn't. It isn't a run.
//!
//! `auth login` prints what Claude Code 2.1.318 prints with no terminal (task 893), waits for a
//! pasted `code#state` on stdin, and signs the folder in when the code is `made-up-code`.
//! `GATHR_REPLAY_PAGE_MS` holds its page back that long, as a slow launcher.
//!
//! `--version` answers `GATHR_REPLAY_VERSION`, else the version the recordings were made with;
//! `stall` there answers only after a minute, as a launcher that hangs.

use std::io::{BufRead, Write};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().map(String::as_str).eq(["auth", "login"]) {
        auth_login();
    }
    if args == ["--version"] {
        let version = std::env::var("GATHR_REPLAY_VERSION");
        if version.as_deref() == Ok("stall") {
            std::thread::sleep(std::time::Duration::from_secs(60));
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
            Some("in") => match stdin.next() {
                Some(Ok(_)) => {}
                _ => return,
            },
            Some("out") => {
                writeln!(stdout, "{line}").unwrap();
                stdout.flush().unwrap();
            }
            Some("err") => writeln!(stderr, "{line}").unwrap(),
            Some("exit") => std::process::exit(line.parse().unwrap_or(1)),
            Some("sleep") => {
                std::thread::sleep(std::time::Duration::from_millis(line.parse().unwrap_or(0)))
            }
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

fn auth_login() -> ! {
    if let Some(ms) = std::env::var("GATHR_REPLAY_PAGE_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    let mut stdout = std::io::stdout().lock();
    writeln!(stdout, "Opening browser to sign in…").unwrap();
    let page = "https://claude.example/oauth/authorize?code=true&state=made-up-state";
    writeln!(stdout, "If the browser didn't open, visit: {page}").unwrap();
    write!(stdout, "Paste code here if prompted > ").unwrap();
    stdout.flush().unwrap();
    let mut code = String::new();
    std::io::stdin().lock().read_line(&mut code).unwrap();
    if !code.starts_with("made-up-code#") {
        eprintln!("Login failed: That code wasn't accepted. Try signing in again.");
        std::process::exit(1);
    }
    let folder = std::path::PathBuf::from(std::env::var_os("CLAUDE_CONFIG_DIR").unwrap());
    let expires = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()
        + 3_600_000;
    let login = serde_json::json!({ "claudeAiOauth": {
        "accessToken": "made-up", "refreshToken": "made-up", "expiresAt": expires as u64 } });
    std::fs::write(folder.join(".credentials.json"), login.to_string()).unwrap();
    std::process::exit(0);
}
