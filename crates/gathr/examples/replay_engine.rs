//! Stands in for Claude Code in tests: plays back a recorded session (`tests/frames/*.jsonl`).
//! Each recorded `in` line waits for a line on stdin; each `out` or `err` line is written as it
//! was recorded; an `exit` line ends it with that code. So the service's driver meets the engine's real frames, in their real order.
//! A `sleep` row pauses that many milliseconds, for a stream at a real pace (timings, not tests).
//! A `hold` row waits until the file `GATHR_REPLAY_HOLD` names exists, so a test decides when
//! the engine goes on; without that variable it's skipped.
//!
//! `GATHR_REPLAY` names the recording, else a file `replay-recording` beside it; `GATHR_REPLAY_ARGS`, if set, is a file each run appends
//! its arguments to, as one JSON line; `GATHR_REPLAY_ENV` one it appends the token it was given
//! for Gathr's tools to (`{"GATHR_AGENT_TOKEN": …}`), so a test can call them as the engine, with
//! its working folder (`cwd`), `PATH`, `GATHR_PUSH_URL` and `TEMP` (its TEMP, TMP and TMPDIR).
//!
//! `auth status` answers as Claude Code does where its login is a file: exit 0 when
//! `CLAUDE_CONFIG_DIR` holds `.credentials.json`, 1 when it doesn't. It isn't a run.
//!
//! `auth login` prints what Claude Code 2.1.318 prints with no terminal (task 893), waits for a
//! pasted `code#state` on stdin, and signs the folder in when the code is `made-up-code`.
//! `GATHR_REPLAY_PAGE_MS` holds its page back that long, as a slow launcher; `GATHR_REPLAY_LOGIN_MS`
//! holds a good code that long before the login is saved, as the exchange does. Like Claude Code
//! 2.1.319, it doesn't end when its input closes before a code (checked live: still waiting 150 s on). `auth logout`
//! removes the folder's login file, as Claude Code 2.1.318 does; `GATHR_REPLAY_LOGOUT=stall`
//! first leaves `logout-started` in the folder and waits a minute.
//!
//! `--version` answers `GATHR_REPLAY_VERSION`, else the version the recordings were made with;
//! `stall` there answers only after a minute, as a launcher that hangs; `folder` answers the name
//! of the folder it runs from, as Gathr lays out its copies (0025). While the file
//! `GATHR_REPLAY_VERSION_HOLD` names exists, `--version` waits, so a test can hold the next start.
//!
//! An `initialize` request is answered with the models Claude Code listed, and isn't one of the
//! recording's `in` lines. `GATHR_REPLAY` may name no recording for a run that only asks that.
//! `GATHR_REPLAY_INIT=silent` never answers it; `linger` stays a minute after its input closes
//! (`GATHR_REPLAY_LINGER_MS` sets how long). `GATHR_REPLAY_CHILD` names a file: the engine starts a
//! child holding its output pipes, which lives while the file does (a minute at most), as an
//! engine's own children can outlive it.

use std::io::{BufRead, Write};
#[path = "replay_engine/codex.rs"]
mod codex;

/// The recording named in `replay-recording` beside this program: an installed service passes
/// engines no `GATHR_*` variable, so a check on one puts the replay engine in as its copy with this.
fn recording_beside_me() -> std::io::Result<String> {
    let file = std::env::current_exe()?.with_file_name("replay-recording");
    Ok(std::fs::read_to_string(file)?.trim().to_string())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a.starts_with("https://")) {
        if let Ok(folder) = std::env::var("GATHR_REPLAY_BROWSER_WAIT") {
            let folder = std::path::Path::new(&folder);
            std::fs::write(folder.join("browser-started"), "").unwrap();
            while folder.exists() && !folder.join("browser-go").exists() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            if folder.exists() {
                std::fs::write(folder.join("browser-finished"), "").unwrap();
            }
        }
        return;
    }
    if let [mode, file] = args.as_slice()
        && mode == "child-of"
    {
        let since = std::time::Instant::now();
        while std::path::Path::new(file).exists() && since.elapsed().as_secs() < 60 {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
        return;
    }
    if args.first().is_some_and(|a| a == "app-server") {
        record_start(&args);
        codex::run();
        return;
    }
    if args.iter().map(String::as_str).eq(["auth", "login"]) {
        auth_login();
    }
    if args == ["--version"] {
        let held = std::env::var_os("GATHR_REPLAY_VERSION_HOLD").unwrap_or_default();
        while !held.is_empty() && std::path::Path::new(&held).exists() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
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
    if args.iter().map(String::as_str).eq(["auth", "logout"]) {
        let folder = std::env::var_os("CLAUDE_CONFIG_DIR").unwrap_or_default();
        if std::env::var("GATHR_REPLAY_LOGOUT").as_deref() == Ok("stall") {
            let _ = std::fs::write(std::path::Path::new(&folder).join("logout-started"), "");
            std::thread::sleep(std::time::Duration::from_secs(60));
        }
        let _ = std::fs::remove_file(std::path::Path::new(&folder).join(".credentials.json"));
        println!("Successfully logged out from your Anthropic account.");
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
    let recording = std::env::var("GATHR_REPLAY")
        .or_else(|_| recording_beside_me())
        .unwrap_or_default();
    record_start(&args);
    if let Some(file) = std::env::var_os("GATHR_REPLAY_CHILD") {
        let me = std::env::current_exe().unwrap();
        // Stdio is inherited: the child holds this engine's stdout and stderr, and outlives it.
        #[allow(clippy::zombie_processes)]
        let _child = std::process::Command::new(me)
            .arg("child-of")
            .arg(file)
            .spawn()
            .unwrap();
    }
    let text = match recording.as_str() {
        "" => String::new(),
        file => std::fs::read_to_string(file).expect("the recording reads"),
    };
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
            Some("sleep") => {
                std::thread::sleep(std::time::Duration::from_millis(line.parse().unwrap_or(0)))
            }
            Some("hold") => {
                let file = std::env::var_os("GATHR_REPLAY_HOLD").unwrap_or_default();
                while !file.is_empty() && !std::path::Path::new(&file).exists() {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            _ => {}
        }
    }
    // Like Claude Code: done when its input closes.
    for line in stdin {
        match line {
            Ok(line) => _ = answered_initialize(&line, &mut stdout),
            Err(_) => break,
        }
    }
    if std::env::var("GATHR_REPLAY_INIT").as_deref() == Ok("linger") {
        let ms = std::env::var("GATHR_REPLAY_LINGER_MS").ok();
        let ms = ms.and_then(|ms| ms.parse().ok()).unwrap_or(60_000);
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
}

/// One write per line: `writeln!` on a file writes in pieces, and a test polling the file read a
/// line cut short (column 1001 of the env line, macOS CI).
fn append_line(out: &mut std::fs::File, mut line: String) {
    line.push('\n');
    out.write_all(line.as_bytes()).unwrap();
}

/// Answers an `initialize` request as Claude Code 2.1.318 did, signed in or out by the login
/// file (`tests/frames/initialize.json`); false for any other line.
fn answered_initialize(line: &str, stdout: &mut impl Write) -> bool {
    let frame: serde_json::Value = serde_json::from_str(line).unwrap_or_default();
    if frame["request"]["subtype"] != "initialize" {
        return false;
    }
    if std::env::var("GATHR_REPLAY_INIT").as_deref() == Ok("silent") {
        return true;
    }
    let recorded: serde_json::Value =
        serde_json::from_str(include_str!("../tests/frames/initialize.json")).unwrap();
    let folder = std::env::var_os("CLAUDE_CONFIG_DIR").unwrap_or_default();
    let signed_in = std::path::Path::new(&folder)
        .join(".credentials.json")
        .is_file();
    let state = if signed_in { "signed_in" } else { "signed_out" };
    let id = &frame["request_id"];
    let answer = serde_json::json!({ "type": "control_response",
        "response": { "subtype": "success", "request_id": id, "response": recorded[state] } });
    writeln!(stdout, "{answer}").unwrap();
    stdout.flush().unwrap();
    true
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
    if code.is_empty() {
        // Input closed with no code: wait, as Claude Code does, until it's stopped.
        std::thread::sleep(std::time::Duration::from_secs(600));
    }
    if !code.starts_with("made-up-code#") {
        eprintln!("Login failed: That code wasn't accepted. Try signing in again.");
        std::process::exit(1);
    }
    if let Some(ms) = std::env::var("GATHR_REPLAY_LOGIN_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
    {
        std::thread::sleep(std::time::Duration::from_millis(ms));
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

fn record_start(args: &[String]) {
    if let Ok(file) = std::env::var("GATHR_REPLAY_ARGS") {
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .expect("the args file opens");
        append_line(&mut out, serde_json::to_string(&args).unwrap());
    }
    if let Ok(file) = std::env::var("GATHR_REPLAY_ENV") {
        let token = std::env::var("GATHR_AGENT_TOKEN").ok();
        let mut out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(file)
            .expect("the env file opens");
        let cwd = std::env::current_dir()
            .ok()
            .map(|d| d.display().to_string());
        let path = std::env::var("PATH").ok();
        let push = std::env::var("GATHR_PUSH_URL").ok();
        let temp = ["TEMP", "TMP", "TMPDIR"].map(|name| std::env::var(name).ok());
        let start = serde_json::json!({ "GATHR_AGENT_TOKEN": token, "cwd": cwd, "PATH": path,
            "GATHR_PUSH_URL": push, "TEMP": temp });
        append_line(&mut out, start.to_string());
    }
}
