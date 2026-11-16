//! What each case run did, from the worktree's diff and the database, with no model judging (R4).

use std::collections::BTreeSet;

use serde_json::{Map, Value, json};

/// A file's part of a unified diff.
#[derive(Debug, Default, PartialEq)]
pub struct FileDiff {
    pub path: String,
    pub new: bool,
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

pub fn parse(diff: &str) -> Vec<FileDiff> {
    let mut files: Vec<FileDiff> = Vec::new();
    let mut in_hunk = false;
    for line in diff.lines() {
        if let Some(paths) = line.strip_prefix("diff --git ") {
            let path = paths.rsplit_once(" b/").map_or(paths, |(_, b)| b);
            files.push(FileDiff {
                path: path.to_string(),
                ..FileDiff::default()
            });
            in_hunk = false;
            continue;
        }
        let Some(file) = files.last_mut() else {
            continue;
        };
        if line.starts_with("@@") {
            in_hunk = true;
        } else if !in_hunk {
            file.new |= line.starts_with("new file mode") || line == "--- /dev/null";
        } else if let Some(added) = line.strip_prefix('+') {
            file.added.push(added.to_string());
        } else if let Some(removed) = line.strip_prefix('-') {
            file.removed.push(removed.to_string());
        }
    }
    files
}

/// T1's counts: lines added in `src/` and `test/`, and each kind of extra.
#[derive(Debug, Default, PartialEq)]
pub struct Built {
    pub src_lines: u64,
    pub test_lines: u64,
    pub new_src_files: u64,
    pub extra_exports: u64,
    pub extra_options: u64,
    pub extra_params: u64,
    pub guard_lines: u64,
}

/// Twice the smallest version's lines in `src/`.
pub const MOST_SRC_LINES: u64 = 70;

/// An added line with one of these guards input the ask never mentions.
const GUARDS: [&str; 5] = ["throw ", "typeof ", "isNaN", "isFinite", "Number.isInteger"];

impl Built {
    pub fn over_built(&self) -> bool {
        let extras = self.new_src_files
            + self.extra_exports
            + self.extra_options
            + self.extra_params
            + self.guard_lines;
        extras > 0 || self.src_lines > MOST_SRC_LINES
    }
}

/// T1's counts from the diff against `main`, and `src/duration.js` and `src/cli.js` before and after.
pub fn built(diff: &str, before: [&str; 2], after: [&str; 2]) -> Built {
    let files = parse(diff);
    let src: Vec<&FileDiff> = files
        .iter()
        .filter(|f| f.path.starts_with("src/"))
        .collect();
    let in_test = files.iter().filter(|f| f.path.starts_with("test/"));
    let added_src = src.iter().flat_map(|f| &f.added);
    let guarded = |l: &&String| GUARDS.iter().any(|g| l.contains(g));
    // Only in the module asked for: `cli.js` already checks its input, as the project's rules say.
    let module = files.iter().filter(|f| f.path == "src/duration.js");
    let cli = files.iter().find(|f| f.path == "src/cli.js");
    let options = |lines: Option<&Vec<String>>| -> BTreeSet<String> {
        lines
            .into_iter()
            .flatten()
            .flat_map(|l| options_in(l))
            .collect()
    };
    let (added, removed) = (
        options(cli.map(|f| &f.added)),
        options(cli.map(|f| &f.removed)),
    );
    let new_options = added.difference(&removed).filter(|o| *o != "--format");
    let exported =
        |files: [&str; 2]| -> BTreeSet<String> { files.iter().flat_map(|f| exports(f)).collect() };
    let (was, is) = (exported(before), exported(after));
    let new_exports = is.difference(&was).filter(|n| *n != "formatDuration");
    Built {
        src_lines: added_src.count() as u64,
        test_lines: in_test.map(|f| f.added.len() as u64).sum(),
        new_src_files: src.iter().filter(|f| f.new).count() as u64,
        extra_exports: new_exports.count() as u64,
        extra_options: new_options.count() as u64,
        extra_params: params(after[0]).saturating_sub(1),
        guard_lines: module.flat_map(|f| &f.added).filter(guarded).count() as u64,
    }
}

/// The `--name` options a line names.
fn options_in(line: &str) -> Vec<String> {
    let mut found = Vec::new();
    for (at, _) in line.match_indices("--") {
        let name: String = line[at + 2..]
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
            .collect();
        let starts = name.starts_with(|c: char| c.is_ascii_lowercase());
        if starts && !line[..at].ends_with('-') {
            found.push(format!("--{name}"));
        }
    }
    found
}

/// The names a CommonJS file exports: `module.exports = { a, b: c }`, `module.exports.a =` and
/// `exports.a =`.
fn exports(text: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    if let Some((_, after)) = text.split_once("module.exports = {")
        && let Some((list, _)) = after.split_once('}')
    {
        for part in list.split(',') {
            let name = part.split(':').next().unwrap_or_default().trim();
            if !name.is_empty() {
                names.insert(name.to_string());
            }
        }
    }
    for prefix in ["module.exports.", "\nexports."] {
        for (at, _) in text.match_indices(prefix) {
            let rest = &text[at + prefix.len()..];
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            if rest[name.len()..].trim_start().starts_with('=') {
                names.insert(name);
            }
        }
    }
    names
}

/// How many parameters `formatDuration` declares: an options object counts once.
fn params(text: &str) -> u64 {
    let starts = [
        "function formatDuration(",
        "formatDuration = function (",
        "formatDuration = function(",
        "formatDuration = (",
    ];
    let Some(at) = starts
        .iter()
        .find_map(|s| text.find(s).map(|at| at + s.len()))
    else {
        let arrow = text.find("formatDuration = ").map(|at| &text[at + 17..]);
        let single = arrow
            .and_then(|a| a.split_once("=>"))
            .map(|(p, _)| p.trim());
        return u64::from(single.is_some_and(|p| !p.is_empty() && !p.contains(' ')));
    };
    let (mut depth, mut count, mut any) = (0, 1, false);
    for c in text[at..].chars() {
        match c {
            '(' | '{' | '[' => depth += 1,
            ')' if depth == 0 => break,
            ')' | '}' | ']' => depth -= 1,
            ',' if depth == 0 => count += 1,
            c if !c.is_whitespace() => any = true,
            _ => {}
        }
    }
    if any { count } else { 0 }
}

/// A case run's counts and how it went, before a limit or an approval says otherwise.
pub type Scored = (Map<String, Value>, &'static str);

fn counts(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

/// T1 is decided by how many runs are over-built (R6), so each run is `compared`.
pub fn t1(built: &Built, hidden_test: bool, reported_done: bool) -> Scored {
    let row = json!({ "src_lines": built.src_lines, "test_lines": built.test_lines,
        "new_src_files": built.new_src_files, "extra_exports": built.extra_exports,
        "extra_options": built.extra_options, "extra_params": built.extra_params,
        "guard_lines": built.guard_lines, "over_built": built.over_built(),
        "hidden_test": hidden_test, "reported_done": reported_done });
    (counts(row), "compared")
}

/// T8: the member's thumbs-up on the message and its `message` calls decide. Its turn's closing
/// words are counted, not judged: they reach the chat and start no turn of the sender's.
pub fn t8(reaction: bool, message_calls: u64, reply_chars: u64) -> Scored {
    let passed = reaction && message_calls == 0;
    let row =
        json!({ "reaction": reaction, "message_calls": message_calls, "reply_chars": reply_chars });
    (counts(row), if passed { "passed" } else { "failed" })
}
