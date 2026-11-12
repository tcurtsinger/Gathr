//! What a Codex tool item ran, for its step in the chat (docs/decisions/0041-what-a-step-ran.md).

use serde_json::Value;

use crate::steps::{self, StepInput};

/// What the item's step keeps, or the name of the field it's read from when the item lacks it.
/// MCP calls, image views and anything not listed keep nothing.
pub(super) fn step(item: &Value) -> Result<Option<StepInput>, &'static str> {
    match item["type"].as_str() {
        Some("commandExecution") => {
            let command = item["command"].as_str().ok_or("command")?;
            Ok(steps::input(&unwrapped(command), None))
        }
        Some("fileChange") => {
            let changes = item["changes"].as_array().ok_or("changes")?;
            let first = changes.first().and_then(|c| c["path"].as_str());
            let first = first.ok_or("changes[].path")?;
            Ok(match changes.len() - 1 {
                0 => steps::input(first, None),
                more => steps::input(&format!("{first} +{more} more"), None),
            })
        }
        // Empty when the search starts; its words come with the item's end (Codex 0.167.0).
        Some("webSearch") => Ok(steps::input(item["query"].as_str().ok_or("query")?, None)),
        _ => Ok(None),
    }
}

const SHELLS: [&str; 5] = ["pwsh", "powershell", "bash", "zsh", "sh"];

/// The command as the agent wrote it. Codex hands over the command line of the shell it runs it
/// in, its words quoted as a POSIX shell quotes them, on Windows too:
/// `"C:\\…\\pwsh.exe" -Command 'git status'`. A line that isn't a shell, flags ending in a command
/// flag, and one script stays as it is.
fn unwrapped(command: &str) -> String {
    let script = || {
        let mut words = words(command)?;
        let script = words.pop()?;
        let (shell, flags) = words.split_first()?;
        let name = shell.rsplit(['\\', '/']).next()?.to_ascii_lowercase();
        let name = name.strip_suffix(".exe").unwrap_or(&name);
        // Only what follows a command flag is a script: PowerShell's `-Command` or `-c`, a POSIX
        // shell's `-c` or short flags run together that end in `c` (`-lc`). After anything else
        // it's a file the shell runs.
        let (last, before) = flags.split_last()?;
        let short = last.strip_prefix('-').unwrap_or_default();
        let run_together = short.len() <= 3 && short.bytes().all(|b| b.is_ascii_lowercase());
        let command = match name {
            // There `-e` and `-ec` are -EncodedCommand: its base64 is no script to show.
            "pwsh" | "powershell" => ["-command", "-c"]
                .iter()
                .any(|c| last.eq_ignore_ascii_case(c)),
            _ => short.ends_with('c') && run_together,
        };
        let wraps = SHELLS.contains(&name) && command && before.iter().all(|f| f.starts_with('-'));
        wraps.then_some(script)
    };
    script().unwrap_or_else(|| command.to_string())
}

/// A command line's words, split and unquoted as a POSIX shell does; `None` when a quote or an
/// escape is left open.
fn words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word: Option<String> = None;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                let word = word.get_or_insert_default();
                loop {
                    match chars.next()? {
                        '\'' => break,
                        c => word.push(c),
                    }
                }
            }
            '"' => {
                let word = word.get_or_insert_default();
                loop {
                    match (chars.next()?, chars.peek().copied()) {
                        ('"', _) => break,
                        ('\\', Some('"' | '\\' | '$' | '`')) => word.push(chars.next()?),
                        ('\\', Some('\n')) => _ = chars.next(),
                        (c, _) => word.push(c),
                    }
                }
            }
            '\\' => word.get_or_insert_default().push(chars.next()?),
            c if c.is_whitespace() => words.extend(word.take()),
            c => word.get_or_insert_default().push(c),
        }
    }
    words.extend(word);
    Some(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claude::frames::Frame;
    use crate::codex::rpc::{Opening, Session};

    /// The quoting seen in the owner's live frames (541 commands, 2026-11-11): one quoted chunk,
    /// or several side by side where the script holds the other quote.
    #[test]
    fn a_command_comes_out_of_the_shell_codex_wraps_it_in() {
        let pwsh = r#""C:\\Program Files\\PowerShell\\7\\pwsh.exe" -Command"#;
        let cases = [
            (format!("{pwsh} 'git status --short'"), "git status --short"),
            (
                format!(r#"{pwsh} "Get-Content 'src\\lib.rs'""#),
                r"Get-Content 'src\lib.rs'",
            ),
            (
                format!(r#"{pwsh} 'echo '"'made up'"' | Out-Null'"#),
                "echo 'made up' | Out-Null",
            ),
            (
                format!(r#"{pwsh} "Write-Output \"made up\" `$x""#),
                r#"Write-Output "made up" `$x"#,
            ),
            ("/bin/zsh -lc 'cargo test'".to_string(), "cargo test"),
            ("bash -l -c \"npm test\"".to_string(), "npm test"),
            ("sh -ec 'make all'".to_string(), "make all"),
            ("pwsh -C 'git fetch'".to_string(), "git fetch"),
            ("pwsh -NoProfile -c 'git log'".to_string(), "git log"),
            ("powershell.exe -NoLogo -COMMAND 'dir'".to_string(), "dir"),
        ];
        for (command, script) in cases {
            assert_eq!(unwrapped(&command), script, "command: {command}");
        }
    }

    #[test]
    fn a_line_that_is_not_a_wrapped_script_stays_as_it_is() {
        for command in [
            "git status --short",
            "bash build.sh",
            "pwsh -File build.ps1 -Fast",
            // PowerShell's `-e`, `-ec` and `-enc` are -EncodedCommand: base64, no script to show.
            "pwsh -ec aGVsbG8=",
            "pwsh -e aGVsbG8=",
            "powershell -NoProfile -enc aGVsbG8=",
            // A script file run by a shell: without the shell it would read as a bare name.
            "bash -l build.sh",
            "sh -e deploy.sh",
            "pwsh -File build.ps1",
            "pwsh -NoProfile -ExecutionPolicy Bypass",
            "bash --login build.sh",
            "pwsh -Command 'left open",
            "cargo run -- 'one script'",
        ] {
            assert_eq!(unwrapped(command), command);
        }
    }

    /// A step's tool, what it kept at its start, and at its end.
    type Kept = (String, Option<String>, Option<String>);

    /// `steps_codex`'s tool items, read: each step's name, what it kept at its start and at its
    /// end, and any field found missing.
    fn played(name: &str) -> (Vec<Kept>, Vec<Frame>) {
        let path = format!("{}/tests/frames/{name}.jsonl", env!("CARGO_MANIFEST_DIR"));
        let (mut session, _) = Session::open(Opening::default());
        let mut frames = Vec::new();
        for row in std::fs::read_to_string(path).unwrap().lines() {
            let row: Value = serde_json::from_str(row).unwrap();
            if row["dir"] == "out" {
                let line = serde_json::from_str(row["line"].as_str().unwrap()).unwrap();
                frames.extend(session.read(&line).frames);
            }
        }
        let text = |input: &Option<StepInput>| input.as_ref().map(|i| i.text.clone());
        let mut steps = Vec::new();
        for frame in &frames {
            if let Frame::ToolStart { id, name, input } = frame {
                let end = frames.iter().find_map(|f| match f {
                    Frame::ToolEnd {
                        id: ended, input, ..
                    } if ended == id => Some(text(input)),
                    _ => None,
                });
                steps.push((name.clone(), text(input), end.flatten()));
            }
        }
        (steps, frames)
    }

    #[test]
    fn each_codex_tool_keeps_its_own_field() {
        let (steps, frames) = played("steps_codex");
        let kept = |name: &str, start: Option<&str>, end: Option<&str>| {
            (
                name.to_string(),
                start.map(String::from),
                end.map(String::from),
            )
        };
        let status = Some("git status --short");
        let read = Some("Get-Content 'src\\lib.rs' | Select-Object -First 5…");
        let curl = Some("curl --token=[redacted:flag] https://example.test/api");
        let files = Some("C:\\work\\demo\\src\\lib.rs +1 more");
        let search = Some("posix shell quoting rules");
        let expected = [
            kept("Shell", status, status),
            kept("Shell", read, read),
            kept("Shell", curl, curl),
            kept("Edit", files, files),
            kept("WebSearch", None, search),
            kept("mcp__gathr__list_tasks", None, None),
            kept("ViewImage", None, None),
        ];
        assert_eq!(steps, expected);
        let missing = frames.iter().any(|f| matches!(f, Frame::ToolShape { .. }));
        assert!(!missing, "a field read as missing: {frames:?}");
    }

    /// A web search and an image view end with no `status` at all (34 live items, 2026-11-11):
    /// they read as failed, and showed "Failed" in the chat.
    #[test]
    fn an_item_that_ends_with_no_status_did_not_fail() {
        let (_, frames) = played("steps_codex");
        let ended = frames.iter().filter_map(|f| match f {
            Frame::ToolEnd { ok, .. } => Some(*ok),
            _ => None,
        });
        // The third command exits 1 with `status: failed`; the search and the image say nothing.
        let expected = [true, true, false, true, true, true, true];
        assert_eq!(ended.collect::<Vec<_>>(), expected, "{frames:?}");
    }

    #[test]
    fn a_listed_item_without_its_field_reads_as_a_changed_shape() {
        let (mut session, _) = Session::open(Opening::default());
        let started = |item: Value| serde_json::json!({ "method": "item/started", "params": { "item": item } });
        let cases = [
            (
                serde_json::json!({ "type": "commandExecution", "id": "c", "argv": ["git"] }),
                "Shell",
                "command",
            ),
            (
                serde_json::json!({ "type": "fileChange", "id": "f", "changes": [] }),
                "Edit",
                "changes[].path",
            ),
            (
                serde_json::json!({ "type": "webSearch", "id": "w" }),
                "WebSearch",
                "query",
            ),
        ];
        for (item, tool, field) in cases {
            let frames = session.read(&started(item)).frames;
            let shape = Frame::ToolShape {
                tool: tool.into(),
                field,
            };
            assert!(frames.contains(&shape), "{tool}: {frames:?}");
            let kept_nothing = frames
                .iter()
                .any(|f| matches!(f, Frame::ToolStart { input: None, .. }));
            assert!(kept_nothing, "{tool}: {frames:?}");
        }
    }
}
