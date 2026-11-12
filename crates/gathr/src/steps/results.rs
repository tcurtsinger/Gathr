//! What came back from a step, as the chat keeps it (docs/decisions/0044-what-a-step-returned.md):
//! up to five lines, each hidden and cut as a step's input is (0041). Each driver picks what came
//! back; the cut and the cleaning are here, once.

use serde::Serialize;

use crate::redact::redact;
use crate::text;

/// How many lines a result keeps (the owner's call, room task 140).
const LINES: usize = 5;
const LINE: usize = 200;
/// A command, whole, as its Request: up to 8 lines and 2,000 characters (room task 141).
const REQUEST_LINES: usize = 8;
const REQUEST: usize = 2_000;

/// What a step returned: its lines, how many there were, and whether more followed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StepResult {
    pub text: String,
    pub lines: usize,
    pub more: bool,
}

/// The first lines of what came back: a tool's answer, an error.
pub fn first_lines(out: &str) -> Option<StepResult> {
    kept(out, false, None)
}

/// The last lines, where a command ends with its summary or its error, and its exit code when that
/// isn't 0.
pub fn last_lines(out: &str, exit: Option<i64>) -> Option<StepResult> {
    kept(out, true, exit.filter(|code| *code != 0))
}

/// One line Gathr writes about what came back, never what came back itself: "4 lines", "+1 −1".
pub fn said(line: String) -> StepResult {
    StepResult {
        text: line,
        lines: 1,
        more: false,
    }
}

/// A search: how many it found, then its first hits, in the five lines.
pub fn found(count: String, hits: &str) -> StepResult {
    let shown = kept(hits, false, None);
    let mut text = count;
    let (mut lines, mut more) = (1, false);
    if let Some(shown) = shown {
        let first: Vec<&str> = shown.text.lines().take(LINES - 1).collect();
        more = shown.lines > first.len();
        lines += shown.lines;
        text = format!("{text}\n{}", first.join("\n"));
    }
    StepResult { text, lines, more }
}

/// "1 line", "4 lines".
pub fn counted(n: i64, one: &str, many: &str) -> String {
    match n {
        1 => format!("1 {one}"),
        n => format!("{n} {many}"),
    }
}

/// A command as the agent wrote it, for the step's Request: hidden first, then cut.
pub fn request(command: &str) -> Option<String> {
    let hidden = redact(command.trim());
    let lines: Vec<&str> = hidden.lines().collect();
    if lines.is_empty() {
        return None;
    }
    let kept = lines[..lines.len().min(REQUEST_LINES)].join("\n");
    Some(text::cut(&kept, REQUEST, lines.len() > REQUEST_LINES))
}

fn kept(out: &str, from_end: bool, exit: Option<i64>) -> Option<StepResult> {
    // Hidden whole before it's split: a secret spread over lines is still seen as one.
    let hidden = redact(out);
    let all: Vec<&str> = hidden.lines().map(str::trim_end).collect();
    let start = all.iter().position(|l| !l.trim().is_empty()).unwrap_or(all.len());
    let end = all.iter().rposition(|l| !l.trim().is_empty()).map_or(start, |e| e + 1);
    let all = &all[start..end];
    let mut shown: Vec<String> = match from_end {
        true => &all[all.len().saturating_sub(LINES)..],
        false => &all[..all.len().min(LINES)],
    }
    .iter()
    .map(|line| text::cut(line, LINE, false))
    .collect();
    if let Some(code) = exit {
        shown.push(format!("exit {code}"));
    }
    if shown.is_empty() {
        return None;
    }
    Some(StepResult {
        text: shown.join("\n"),
        lines: all.len(),
        more: all.len() > LINES,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(n: usize) -> String {
        (1..=n).map(|i| format!("line {i}\n")).collect()
    }

    #[test]
    fn a_result_keeps_five_lines_from_its_start_or_its_end() {
        let first = first_lines(&numbered(8)).unwrap();
        assert_eq!(first.text, "line 1\nline 2\nline 3\nline 4\nline 5");
        assert_eq!((first.lines, first.more), (8, true));
        let last = last_lines(&numbered(8), Some(0)).unwrap();
        assert_eq!(last.text, "line 4\nline 5\nline 6\nline 7\nline 8");
        let short = first_lines("\n  ok\n\n").unwrap();
        assert_eq!((short.text.as_str(), short.lines, short.more), ("  ok", 1, false));
        assert_eq!(first_lines(" \n"), None);
    }

    #[test]
    fn a_failing_command_says_its_exit_code() {
        let failed = last_lines("fatal: not a git repository\n", Some(128)).unwrap();
        assert_eq!(failed.text, "fatal: not a git repository\nexit 128");
        assert_eq!(last_lines("", Some(1)).unwrap().text, "exit 1");
        assert_eq!(last_lines("", Some(0)), None);
    }

    #[test]
    fn each_line_is_hidden_then_cut_to_200_characters() {
        let out = format!("{}\ntoken: Bearer abc123secret", "é".repeat(300));
        let kept = first_lines(&out).unwrap();
        let lines: Vec<&str> = kept.text.lines().collect();
        assert_eq!(lines[0], format!("{}…", "é".repeat(199)));
        assert_eq!(lines[1], "token: Bearer [redacted:bearer]");
    }

    #[test]
    fn a_search_says_how_many_then_its_first_hits() {
        let hits = found(counted(7, "file", "files"), &numbered(7));
        assert_eq!(hits.text, "7 files\nline 1\nline 2\nline 3\nline 4");
        assert!(hits.more);
        assert_eq!(found(counted(0, "file", "files"), "").text, "0 files");
    }

    #[test]
    fn a_request_is_the_whole_command_up_to_8_lines_hidden_first() {
        let script = format!("{}API_TOKEN=made-up-value npm test", numbered(3));
        assert_eq!(
            request(&script).unwrap(),
            "line 1\nline 2\nline 3\nAPI_TOKEN=[redacted:env] npm test"
        );
        let long = request(&numbered(10)).unwrap();
        assert_eq!(long.lines().count(), 8);
        assert!(long.ends_with("line 8…"), "{long}");
        assert_eq!(request("  "), None);
    }
}
