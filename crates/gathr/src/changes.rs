//! What each agent changed in a chat, for the window's Changes tab: everything in the agent's own
//! worktree that isn't on the project's main line yet, committed or not (its working tree against
//! the merge-base of its HEAD with the base `worktree::ensure` starts from, and its new files git
//! doesn't ignore); and, in any other git checkout, what is not committed of the files the agent
//! wrote there with its file tools.
//!
//! Read-only: `git diff` refreshes the index as it reads (an index lock an agent's own git can trip
//! on), so the plumbing `diff-index` and `ls-files` are used, which never write. Each git call costs
//! tens of milliseconds to start on Windows, so a worktree's change is two rounds of calls run
//! together, and every worktree and checkout is read at once.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::worktree::{git, git_printed};

/// The most files a chat's change lists.
pub const MOST_FILES: usize = 300;
/// The most lines one file's change gives.
pub const MOST_LINES: usize = 2000;
const LIMIT: Duration = Duration::from_secs(30);
/// How far git looks for a NUL byte to call a file binary.
const SNIFF: usize = 8000;
/// git's own options that must hold whatever the person's config says. A path is never a pattern.
const DIFF: [&str; 6] = [
    "--literal-pathspecs",
    "--no-optional-locks",
    "diff-index",
    "--no-ext-diff",
    "--no-textconv",
    "-M",
];
const OTHERS: [&str; 6] = [
    "--literal-pathspecs",
    "--no-optional-locks",
    "ls-files",
    "-z",
    "--others",
    "--exclude-standard",
];
/// The most characters of paths one git call is given: Windows takes a command line of 32,767.
const MOST_NAMED: usize = 20_000;

/// One changed file: its path in the worktree with `/` between steps, and what it was before a rename.
#[derive(Debug, Serialize)]
pub struct File {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    pub added: u64,
    pub removed: u64,
    pub kind: Kind,
    pub binary: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Changed,
    New,
    Deleted,
    Renamed,
}

/// An agent's change: its branch, its files by path (at most what was asked), and whether there are more.
#[derive(Debug, Serialize)]
pub struct Change {
    pub branch: String,
    pub files: Vec<File>,
    pub more: bool,
}

/// One file's change as lines to draw.
#[derive(Debug, Default, Serialize)]
pub struct Lines {
    pub binary: bool,
    /// Over `MOST_LINES`: only the first are given.
    pub cut: bool,
    pub hunks: Vec<Hunk>,
}

/// A hunk: its old and new ranges as `[start, count]`, and its lines.
#[derive(Debug, Serialize)]
pub struct Hunk {
    pub old: [u64; 2],
    pub new: [u64; 2],
    pub lines: Vec<Line>,
}

#[derive(Debug, Serialize)]
pub struct Line {
    pub kind: LineKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub new: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineKind {
    Context,
    Added,
    Removed,
}

/// The commit the worktree's change is counted from: the merge-base of its HEAD with the project's
/// `origin/HEAD` when it has one, else with the project's own HEAD.
async fn base(project: &Path, worktree: &Path) -> Result<String, String> {
    let env = crate::files::git_env();
    let remote = ["merge-base", "HEAD", "refs/remotes/origin/HEAD"];
    if let Ok(base) = git(&env, worktree, &remote, LIMIT).await {
        return Ok(base);
    }
    let head = ["rev-parse", "--verify", "HEAD^{commit}"];
    let start = git(&env, project, &head, LIMIT).await?;
    git(&env, worktree, &["merge-base", "HEAD", &start], LIMIT).await
}

/// The change in `worktree`, a worktree of `project`: at most `room` files. Blocks on the disk for
/// new files.
pub async fn list(project: &Path, worktree: &Path, room: usize) -> Result<Change, Error> {
    let env = crate::files::git_env();
    let branch = ["rev-parse", "--abbrev-ref", "HEAD"];
    let (base, branch) = tokio::join!(base(project, worktree), git(&env, worktree, &branch, LIMIT));
    let base = base.map_err(Error::Other)?;
    let (files, more) = listed(worktree, &base, &[], room).await?;
    Ok(Change {
        branch: branch.unwrap_or_default(),
        files,
        more,
    })
}

/// What isn't committed in the checkout at `top` of the files at `paths` there: at most `room`.
/// Blocks on the disk for new files.
pub async fn uncommitted(top: &Path, paths: &[String], room: usize) -> Result<Change, Error> {
    let (files, more) = listed(top, "HEAD", paths, room).await?;
    Ok(Change {
        branch: String::new(),
        files,
        more,
    })
}

/// The files a chat's agents wrote with their file tools (Claude Code's Write and Edit, Codex's file
/// change), as their steps keep them: by agent, each path as the step names it, oldest first.
/// Codex's step names only the first file of a change.
pub fn written(conn: &Connection, chat: &str) -> Result<Vec<(String, String)>, Error> {
    let mut statement = conn.prepare(
        "SELECT t.agent_id, m.body FROM messages m JOIN turns t ON t.id = m.turn_id
         WHERE m.chat_id = ?1 AND m.kind = 'tool' AND t.agent_id IS NOT NULL
           AND CASE WHEN json_valid(m.body) THEN json_extract(m.body, '$.tool') END IN ('Write', 'Edit')
         ORDER BY m.ts, m.rowid",
    )?;
    let rows = statement.query_map([chat], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (agent, body) = row?;
        let body: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        let input = &body["input"];
        // `path` when the step's text isn't the whole path; else the text, less Codex's "+1 more".
        let path = input["path"].as_str().or_else(|| {
            let text = input["text"].as_str()?;
            let more = |n: &str| {
                n.strip_suffix(" more")
                    .is_some_and(|n| n.parse::<u32>().is_ok())
            };
            Some(match text.rsplit_once(" +") {
                Some((first, n)) if more(n) => first,
                _ => text,
            })
        });
        out.extend(path.map(|path| (agent, path.to_string())));
    }
    Ok(out)
}

/// The checkouts `written` (by agent, a full path each) wrote in, other than the chat's worktrees
/// `own` and never inside the data folder `data`: by agent and checkout top, the paths in it from
/// its top with `/` between steps. A path in no git checkout, or not a full path, is left out.
/// Blocks on the disk.
pub fn outside(
    written: &[(String, String)],
    own: &[PathBuf],
    data: &Path,
) -> BTreeMap<(String, PathBuf), Vec<String>> {
    let data = crate::folders::real(data);
    let mut out: BTreeMap<(String, PathBuf), Vec<String>> = BTreeMap::new();
    for (agent, path) in written {
        let path = Path::new(path);
        if !path.is_absolute() {
            continue;
        }
        let Some(crate::approvals::Cover::Checkout(top)) = crate::approvals::cover(path) else {
            continue;
        };
        let file = crate::folders::real(path);
        if file.starts_with(&data) || own.contains(&top) {
            continue;
        }
        let Ok(rest) = file.strip_prefix(&top) else {
            continue;
        };
        let steps: Vec<_> = rest
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        let rest = steps.join("/");
        if plain(&rest).is_err() {
            continue;
        }
        let paths = out.entry((agent.clone(), top)).or_default();
        if !paths.contains(&rest) {
            paths.push(rest);
        }
    }
    out
}

/// A file a step changed with its lines kept, as `kept` reads it: the agent, its path as the step
/// names it and the whole path when that is another (`full`, the step's path then being from the
/// folder the engine runs in), and what the step kept of its change.
#[derive(Debug, Serialize, Deserialize)]
pub struct Step {
    pub agent: String,
    pub path: String,
    pub full: Option<String>,
    pub added: u64,
    pub removed: u64,
    pub new: bool,
    pub diff: String,
    pub cut: bool,
}

/// The files a chat's agents changed whose lines their steps kept, oldest first. Read from the
/// database only.
pub fn kept(conn: &Connection, chat: &str) -> Result<Vec<Step>, Error> {
    let mut statement = conn.prepare(
        "SELECT t.agent_id, m.body FROM messages m JOIN turns t ON t.id = m.turn_id
         WHERE m.chat_id = ?1 AND m.kind = 'tool' AND t.agent_id IS NOT NULL
           AND CASE WHEN json_valid(m.body) THEN json_extract(m.body, '$.changed') END IS NOT NULL
         ORDER BY m.ts, m.rowid",
    )?;
    let rows = statement.query_map([chat], |r| Ok((r.get(0)?, r.get::<_, String>(1)?)))?;
    let mut out = Vec::new();
    for row in rows {
        let (agent, body): (String, String) = row?;
        let body: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        for file in body["changed"].as_array().into_iter().flatten() {
            let (Some(path), Some(diff)) = (file["path"].as_str(), file["diff"].as_str()) else {
                continue;
            };
            out.push(Step {
                agent: agent.clone(),
                path: path.into(),
                full: file["full"].as_str().map(String::from),
                added: file["added"].as_u64().unwrap_or_default(),
                removed: file["removed"].as_u64().unwrap_or_default(),
                new: file["new"] == true,
                diff: diff.into(),
                cut: file["cut"] == true,
            });
        }
    }
    Ok(out)
}

/// A file in no git checkout, as its steps changed it: its counts summed, and each step's lines.
#[derive(Debug)]
pub struct Loose {
    pub file: File,
    pub diffs: Vec<String>,
    pub cut: bool,
}

/// The files of `kept` that are in no git checkout, never inside the data folder `data`: by agent
/// and folder, each file by its path, oldest step first. A file in the folder its engine runs in
/// has no folder and its path from there; any other, its own folder and its name. Nothing is read
/// from the files: only whether a folder above holds a `.git` (`approvals::cover`).
pub fn loose(kept: Vec<Step>, data: &Path) -> BTreeMap<(String, Option<PathBuf>), Vec<Loose>> {
    let data = crate::folders::real(data);
    let mut out: BTreeMap<(String, Option<PathBuf>), Vec<Loose>> = BTreeMap::new();
    for step in kept {
        let whole = Path::new(step.full.as_deref().unwrap_or(&step.path));
        if !whole.is_absolute() {
            continue;
        }
        let Some(crate::approvals::Cover::Files(folder)) = crate::approvals::cover(whole) else {
            continue;
        };
        if folder.starts_with(&data) {
            continue;
        }
        let own = step.full.is_some() && Path::new(&step.path).is_relative();
        let (folder, named) = match own {
            true => (None, Path::new(&step.path)),
            false => (
                Some(folder),
                Path::new(whole.file_name().unwrap_or_default()),
            ),
        };
        let steps: Vec<_> = named
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect();
        let path = steps.join("/");
        if plain(&path).is_err() {
            continue;
        }
        let files = out.entry((step.agent, folder)).or_default();
        let at = match files.iter().position(|f| f.file.path == path) {
            Some(at) => at,
            None => {
                let kind = if step.new { Kind::New } else { Kind::Changed };
                let file = File {
                    path,
                    from: None,
                    added: 0,
                    removed: 0,
                    kind,
                    binary: false,
                };
                files.push(Loose {
                    file,
                    diffs: Vec::new(),
                    cut: false,
                });
                files.len() - 1
            }
        };
        let loose = &mut files[at];
        loose.file.added += step.added;
        loose.file.removed += step.removed;
        loose.diffs.push(step.diff);
        loose.cut |= step.cut;
    }
    for files in out.values_mut() {
        files.sort_by(|a, b| a.file.path.cmp(&b.file.path));
    }
    out
}

/// A loose file's change as lines to draw: each step's hunks one after another, the first step
/// first, at most `MOST_LINES` in all.
pub fn loose_lines(loose: &Loose) -> Lines {
    let mut out = Lines {
        cut: loose.cut,
        ..Lines::default()
    };
    let mut room = MOST_LINES;
    for diff in &loose.diffs {
        let lines = parse(diff);
        out.cut |= lines.cut;
        for mut hunk in lines.hunks {
            if room == 0 {
                out.cut = true;
                return out;
            }
            if hunk.lines.len() > room {
                hunk.lines.truncate(room);
                out.cut = true;
            }
            room -= hunk.lines.len();
            out.hunks.push(hunk);
        }
    }
    out
}

/// The files of `dir` changed since `base`, and its new files git doesn't ignore, all or only those
/// at `paths`: at most `room`, and whether there were more. The three git calls run together.
async fn listed(
    dir: &Path,
    base: &str,
    paths: &[String],
    room: usize,
) -> Result<(Vec<File>, bool), Error> {
    let env = crate::files::git_env();
    let mut named: Vec<&str> = Vec::new();
    let mut length = 0;
    for path in paths {
        length += path.len() + 3;
        if length > MOST_NAMED {
            break;
        }
        named.push(path);
    }
    let cut = named.len() < paths.len();
    let tail: Vec<&str> = match named.is_empty() {
        true => Vec::new(),
        false => std::iter::once("--").chain(named).collect(),
    };
    let diff = |format| {
        let head = DIFF.iter().copied().chain([format, "-z", base]);
        head.chain(tail.iter().copied()).collect::<Vec<&str>>()
    };
    let (counts, kinds) = (diff("--numstat"), diff("--name-status"));
    let others: Vec<&str> = OTHERS.iter().copied().chain(tail.iter().copied()).collect();
    let (counts, kinds, others) = tokio::join!(
        git(&env, dir, &counts, LIMIT),
        git(&env, dir, &kinds, LIMIT),
        git(&env, dir, &others, LIMIT)
    );
    let (counts, kinds) = (counts.map_err(Error::Other)?, kinds.map_err(Error::Other)?);
    let mut files = tracked(&counts, &kinds);
    let others: Vec<String> = others
        .map_err(Error::Other)?
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    let more = cut || files.len() + others.len() > room;
    files.truncate(room);
    let left = room - files.len();
    let dir = dir.to_path_buf();
    let new = tokio::task::spawn_blocking(move || {
        let counted = |path: String| {
            let (added, binary) = counted(&dir.join(&path));
            File {
                path,
                from: None,
                added,
                removed: 0,
                kind: Kind::New,
                binary,
            }
        };
        others
            .into_iter()
            .take(left)
            .map(counted)
            .collect::<Vec<_>>()
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?;
    files.extend(new);
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok((files, more))
}

/// The tracked files from `diff-index -z`'s `--numstat` and `--name-status`.
fn tracked(counts: &str, kinds: &str) -> Vec<File> {
    let mut kind_of = std::collections::HashMap::new();
    let mut words = kinds.split('\0').filter(|w| !w.is_empty());
    while let Some(status) = words.next() {
        let two = status.starts_with(['R', 'C']);
        let first = words.next().unwrap_or_default();
        let path = match two {
            true => words.next().unwrap_or_default(),
            false => first,
        };
        let kind = match status.as_bytes().first() {
            Some(b'A') => Kind::New,
            Some(b'D') => Kind::Deleted,
            Some(b'R') => Kind::Renamed,
            _ => Kind::Changed,
        };
        kind_of.insert(path.to_string(), kind);
    }
    let mut files = Vec::new();
    let mut words = counts.split('\0');
    while let Some(record) = words.next() {
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        // A rename's paths follow it, old then new.
        let (from, path) = match path.is_empty() {
            true => {
                let from = words.next().unwrap_or_default().to_string();
                (Some(from), words.next().unwrap_or_default().to_string())
            }
            false => (None, path.to_string()),
        };
        let kind = kind_of.get(&path).copied().unwrap_or(Kind::Changed);
        files.push(File {
            from: from.filter(|_| kind == Kind::Renamed),
            binary: added == "-",
            added: added.parse().unwrap_or(0),
            removed: removed.parse().unwrap_or(0),
            kind,
            path,
        });
    }
    files
}

/// How many lines the new file at `path` has, and whether it is binary: a NUL in its first
/// `SNIFF` bytes, as git tells, or not a plain file. Only as much of it is read as its lines are
/// shown from, so a huge file is never read whole each time the list is.
fn counted(path: &Path) -> (u64, bool) {
    let plain = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_file());
    let Some(file) = plain.then(|| std::fs::File::open(path).ok()).flatten() else {
        return (0, true);
    };
    let mut file = file.take(crate::files::SHOWN);
    let (mut lines, mut last, mut read) = (0u64, b'\n', 0usize);
    let mut buffer = vec![0u8; 64 * 1024];
    while let Ok(n) = file.read(&mut buffer) {
        if n == 0 {
            break;
        }
        let chunk = &buffer[..n];
        if read < SNIFF && chunk[..n.min(SNIFF - read)].contains(&0) {
            return (0, true);
        }
        read += n;
        lines += chunk.iter().filter(|&&b| b == b'\n').count() as u64;
        last = chunk[n - 1];
    }
    (lines + u64::from(last != b'\n'), false)
}

/// `path` when it is a plain path inside a worktree: relative, every step a name, never an option.
fn plain(path: &str) -> Result<&str, Error> {
    let steps = Path::new(path).components();
    let named = steps.clone().next().is_some()
        && steps.into_iter().all(|c| matches!(c, Component::Normal(_)));
    match named && !path.starts_with('-') {
        true => Ok(path),
        false => Err(Error::Refused(format!(
            "{path} isn't a path in that agent's worktree"
        ))),
    }
}

/// The change of the file at `path` in `worktree` (with `from`, what a rename was), a worktree of
/// `project`: unified, 3 lines of context. A new file git doesn't ignore is all added lines.
pub async fn file(
    project: &Path,
    worktree: &Path,
    path: &str,
    from: Option<&str>,
) -> Result<Lines, Error> {
    let (path, from) = (plain(path)?, from.map(plain).transpose()?);
    let base = base(project, worktree).await.map_err(Error::Other)?;
    lines(worktree, &base, path, from).await
}

/// The change of the file at `path` in the checkout at `top` since its last commit.
pub async fn uncommitted_file(top: &Path, path: &str) -> Result<Lines, Error> {
    lines(top, "HEAD", plain(path)?, None).await
}

/// The change of the file at the plain `path` in `dir` since `base`.
async fn lines(dir: &Path, base: &str, path: &str, from: Option<&str>) -> Result<Lines, Error> {
    let env = crate::files::git_env();
    let mut args: Vec<&str> = DIFF.to_vec();
    args.extend(["-p", "--no-color", "-U3", base, "--", path]);
    args.extend(from);
    // As git printed it: trimmed, a last line that is a blank context line would be lost.
    let diff = git_printed(&env, dir, &args, LIMIT)
        .await
        .map_err(Error::Other)?;
    if !diff.is_empty() {
        return Ok(parse(&diff));
    }
    let others: Vec<&str> = OTHERS.iter().copied().chain(["--", path]).collect();
    let others = git(&env, dir, &others, LIMIT).await.map_err(Error::Other)?;
    if !others.split('\0').any(|p| p == path) {
        return Ok(Lines::default());
    }
    let roots = [crate::folders::real(dir)];
    let at = dir.join(path).to_string_lossy().into_owned();
    tokio::task::spawn_blocking(move || {
        let found = crate::files::inside(&roots, &at).filter(|f| f.is_file());
        let found = found.ok_or_else(|| Error::Refused(format!("{at} isn't in the worktree")))?;
        Ok(added(&found))
    })
    .await
    .map_err(|e| Error::Other(e.to_string()))?
}

/// A new file's lines, all added. Blocks on the disk.
fn added(path: &Path) -> Lines {
    let mut bytes = Vec::new();
    let read =
        std::fs::File::open(path).and_then(|f| f.take(crate::files::SHOWN).read_to_end(&mut bytes));
    if read.is_err() || bytes[..bytes.len().min(SNIFF)].contains(&0) {
        return Lines {
            binary: true,
            ..Lines::default()
        };
    }
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text
        .strip_suffix('\n')
        .unwrap_or(&text)
        .split('\n')
        .collect();
    let all = if text.is_empty() { Vec::new() } else { all };
    let cut = all.len() > MOST_LINES || bytes.len() as u64 == crate::files::SHOWN;
    let lines: Vec<Line> = all
        .iter()
        .take(MOST_LINES)
        .zip(1..)
        .map(|(text, n)| Line {
            kind: LineKind::Added,
            old: None,
            new: Some(n),
            text: text.trim_end_matches('\r').to_string(),
        })
        .collect();
    let hunks = match lines.is_empty() {
        true => Vec::new(),
        false => vec![Hunk {
            old: [0, 0],
            new: [1, lines.len() as u64],
            lines,
        }],
    };
    Lines {
        binary: false,
        cut,
        hunks,
    }
}

/// A hunk header's range, `12,5` or `12` (one line).
fn range(said: &str) -> Option<[u64; 2]> {
    let (start, count) = said.split_once(',').unwrap_or((said, "1"));
    Some([start.parse().ok()?, count.parse().ok()?])
}

/// One file's unified diff as hunks of lines, at most `MOST_LINES`.
fn parse(diff: &str) -> Lines {
    let mut out = Lines::default();
    let (mut old, mut new, mut given, mut inside) = (0, 0, 0, false);
    for line in diff.lines() {
        if let Some(head) = line.strip_prefix("@@ -") {
            let mut words = head.split(' ');
            let o = words.next().and_then(range);
            let n = words
                .next()
                .and_then(|w| w.strip_prefix('+'))
                .and_then(range);
            let (Some(o), Some(n)) = (o, n) else {
                inside = false;
                continue;
            };
            if given == MOST_LINES {
                out.cut = true;
                break;
            }
            (old, new, inside) = (o[0], n[0], true);
            out.hunks.push(Hunk {
                old: o,
                new: n,
                lines: Vec::new(),
            });
            continue;
        }
        if !inside {
            out.binary |= line.starts_with("Binary files ") || line == "GIT binary patch";
            continue;
        }
        let (kind, text) = match line.split_at_checked(1) {
            Some((" ", text)) | Some(("", text)) => (LineKind::Context, text),
            Some(("+", text)) => (LineKind::Added, text),
            Some(("-", text)) => (LineKind::Removed, text),
            Some(("\\", _)) => continue,
            _ => {
                inside = false;
                continue;
            }
        };
        if given == MOST_LINES {
            out.cut = true;
            break;
        }
        given += 1;
        let (o, n) = match kind {
            LineKind::Context => (Some(old), Some(new)),
            LineKind::Added => (None, Some(new)),
            LineKind::Removed => (Some(old), None),
        };
        old += u64::from(o.is_some());
        new += u64::from(n.is_some());
        let text = text.trim_end_matches('\r').to_string();
        let hunk = out
            .hunks
            .last_mut()
            .expect("a line follows its hunk's head");
        hunk.lines.push(Line {
            kind,
            old: o,
            new: n,
            text,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_worktrees_change_reads_through_git_and_refuses_paths_outside_it() {
        let base = std::env::temp_dir().join(format!("gathr-changes-{}", crate::ids::span_id()));
        let (project, tree) = (base.join("shop"), base.join("tree"));
        std::fs::create_dir_all(&project).unwrap();
        let git = |dir: &Path, args: &[&str]| {
            let ran = std::process::Command::new("git")
                .args([
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                ])
                .args(args)
                .current_dir(dir)
                .output()
                .unwrap();
            assert!(ran.status.success(), "git {args:?}: {ran:?}");
        };
        git(&project, &["init", "--quiet"]);
        std::fs::write(project.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(project.join("c.txt"), "old\n").unwrap();
        git(&project, &["add", "."]);
        git(&project, &["commit", "--quiet", "-m", "start"]);
        let at = tree.to_string_lossy().into_owned();
        git(
            &project,
            &["worktree", "add", "--quiet", "-b", "gathr/test", &at],
        );
        // Committed on the agent's branch, changed and not committed, and new.
        std::fs::write(tree.join("c.txt"), "new\n").unwrap();
        git(&tree, &["commit", "--quiet", "-am", "c"]);
        std::fs::write(tree.join("a.txt"), "one\n2\nthree\n").unwrap();
        std::fs::write(tree.join("n.txt"), "x\ny").unwrap();

        let change = list(&project, &tree, MOST_FILES).await.unwrap();
        assert_eq!(change.branch, "gathr/test");
        let files: Vec<_> = change
            .files
            .iter()
            .map(|f| (f.path.as_str(), f.added, f.removed, f.kind, f.binary))
            .collect();
        assert_eq!(
            files,
            [
                ("a.txt", 1, 1, Kind::Changed, false),
                ("c.txt", 1, 1, Kind::Changed, false),
                ("n.txt", 2, 0, Kind::New, false),
            ]
        );
        assert!(!change.more);
        assert!(
            list(&project, &tree, 1).await.unwrap().more,
            "over the most"
        );

        let lines = file(&project, &tree, "a.txt", None).await.unwrap();
        let hunk = &lines.hunks[0];
        assert_eq!((hunk.old, hunk.new), ([1, 3], [1, 3]));
        let drawn: Vec<_> = hunk
            .lines
            .iter()
            .map(|l| (l.kind, l.old, l.new, l.text.as_str()))
            .collect();
        assert_eq!(
            drawn,
            [
                (LineKind::Context, Some(1), Some(1), "one"),
                (LineKind::Removed, Some(2), None, "two"),
                (LineKind::Added, None, Some(2), "2"),
                (LineKind::Context, Some(3), Some(3), "three"),
            ]
        );
        let new = file(&project, &tree, "n.txt", None).await.unwrap();
        let texts: Vec<_> = new.hunks[0]
            .lines
            .iter()
            .map(|l| (l.kind, l.new, l.text.as_str()))
            .collect();
        assert_eq!(
            texts,
            [
                (LineKind::Added, Some(1), "x"),
                (LineKind::Added, Some(2), "y")
            ]
        );

        std::fs::write(base.join("secret.txt"), "out\n").unwrap();
        let outside = base.join("secret.txt").to_string_lossy().into_owned();
        for path in ["../secret.txt", outside.as_str(), "-p", "--output=x", ""] {
            let read = file(&project, &tree, path, None).await;
            assert!(matches!(read, Err(Error::Refused(_))), "{path}: {read:?}");
        }
        let read = file(&project, &tree, "a.txt", Some("../secret.txt")).await;
        assert!(
            matches!(read, Err(Error::Refused(_))),
            "a rename from outside"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
