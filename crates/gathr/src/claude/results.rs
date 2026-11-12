//! What came back from a Claude Code tool call, for its step (docs/decisions/0044-what-a-step-returned.md).
//! The `tool_result` block is what the model was given; Claude Code's own `tool_use_result` beside
//! it says what kind of step it was, as recorded live (Claude Code 2.1.322, `tests/frames/results_claude.jsonl`).

use serde_json::Value;

use crate::steps::{self, StepResult, counted};

/// The step's result. `done` is Claude Code's `tool_use_result`, read only when the frame ends one
/// call: with several, it can't say whose it is, and the text answers alone.
pub(super) fn of(block: &Value, done: Option<&Value>) -> Option<StepResult> {
    let said = content(&block["content"]);
    let failed = block.get("is_error") == Some(&Value::Bool(true));
    let Some(done) = done.filter(|d| d.is_object() && !failed) else {
        return steps::first_lines(&said);
    };
    // A command: its output's last lines. Claude Code gives no exit code (not seen live).
    if let Some(out) = done["stdout"].as_str() {
        let err = done["stderr"].as_str().unwrap_or_default();
        let all = [out, err]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>();
        return steps::last_lines(&all.join("\n"), None);
    }
    // A read: how many lines, never what they hold.
    if let Some(n) = done["file"]["numLines"].as_i64() {
        return Some(steps::said(counted(n, "line", "lines")));
    }
    // An edit or a new file: lines added and removed.
    if let Some(patch) = done["structuredPatch"].as_array() {
        let (added, removed) = match (patch.is_empty(), done["content"].as_str()) {
            (true, Some(made)) => (made.lines().count(), 0),
            _ => patch_counts(patch),
        };
        return Some(steps::said(format!("+{added} −{removed}")));
    }
    // A search: how many it found, then its first hits.
    if let Some(files) = done["numFiles"].as_i64() {
        if done["mode"] == "content" {
            let n = done["numLines"].as_i64().unwrap_or_default();
            let hits = done["content"].as_str().unwrap_or(&said);
            return Some(steps::found(
                counted(n, "matching line", "matching lines"),
                hits,
            ));
        }
        let names: Vec<&str> = done["filenames"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        return Some(steps::found(
            counted(files, "file", "files"),
            &names.join("\n"),
        ));
    }
    steps::first_lines(&said)
}

/// The text the model was given: a string, or text blocks joined; an image reads "[image]".
fn content(content: &Value) -> String {
    match content {
        Value::String(text) => text.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|b| match b["type"].as_str() {
                Some("text") => b["text"].as_str().map(String::from),
                Some("image") => Some("[image]".to_string()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn patch_counts(patch: &[Value]) -> (usize, usize) {
    let lines = patch
        .iter()
        .flat_map(|hunk| hunk["lines"].as_array().into_iter().flatten())
        .filter_map(Value::as_str);
    lines.fold((0, 0), |(added, removed), line| match line.chars().next() {
        Some('+') => (added + 1, removed),
        Some('-') => (added, removed + 1),
        _ => (added, removed),
    })
}
