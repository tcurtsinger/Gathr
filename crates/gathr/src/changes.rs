//! What each agent changed in a project's chat, for the window's Changes tab: everything in the
//! agent's own worktree that isn't on the project's main line yet, committed or not. That is its
//! working tree against the merge-base of its HEAD with the base `worktree::ensure` starts from,
//! and its new files git doesn't ignore.
//!
//! Read-only: `git diff` refreshes the index as it reads (an index lock an agent's own git can trip
//! on), so the plumbing `diff-index` and `ls-files` are used, which never write.

use std::io::Read;
use std::path::{Component, Path};
use std::time::Duration;

use serde::Serialize;

use crate::Error;
use crate::worktree::{git, git_printed};

/// The most files a chat's change lists.
pub const MOST_FILES: usize = 300;
/// The most lines one file's change gives.
pub const MOST_LINES: usize = 2000;
const LIMIT: Duration = Duration::from_secs(30);
/// How far git looks for a NUL byte to call a file binary.
const SNIFF: usize = 8000;
/// git's own options that must hold whatever the person's config says.
const DIFF: [&str; 4] = ["diff-index", "--no-ext-diff", "--no-textconv", "-M"];

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
/// `origin/HEAD` when it has an origin, else with the project's own HEAD.
async fn base(project: &Path, worktree: &Path) -> Result<String, String> {
    let env = crate::files::git_env();
    let remotes = git(&env, worktree, &["remote"], LIMIT).await?;
    let remote = match remotes.lines().any(|r| r == "origin") {
        true => {
            let head = ["rev-parse", "--verify", "--quiet", "origin/HEAD^{commit}"];
            git(&env, worktree, &head, LIMIT).await.ok()
        }
        false => None,
    };
    let start = match remote {
        Some(start) => start,
        None => {
            git(
                &env,
                project,
                &["rev-parse", "--verify", "HEAD^{commit}"],
                LIMIT,
            )
            .await?
        }
    };
    git(&env, worktree, &["merge-base", "HEAD", &start], LIMIT).await
}

/// The change in `worktree`, a worktree of `project`: at most `room` files. Blocks on the disk for
/// new files.
pub async fn list(project: &Path, worktree: &Path, room: usize) -> Result<Change, Error> {
    let env = crate::files::git_env();
    let base = base(project, worktree).await.map_err(Error::Other)?;
    let branch = git(
        &env,
        worktree,
        &["rev-parse", "--abbrev-ref", "HEAD"],
        LIMIT,
    )
    .await;
    let run = |extra: &'static str| {
        let args: Vec<&str> = DIFF.iter().copied().chain([extra, "-z", &base]).collect();
        let env = env.clone();
        async move { git(&env, worktree, &args, LIMIT).await }
    };
    let (counts, kinds) = tokio::join!(run("--numstat"), run("--name-status"));
    let (counts, kinds) = (counts.map_err(Error::Other)?, kinds.map_err(Error::Other)?);
    let mut files = tracked(&counts, &kinds);
    let others = ["ls-files", "-z", "--others", "--exclude-standard"];
    let others = git(&env, worktree, &others, LIMIT)
        .await
        .map_err(Error::Other)?;
    let others: Vec<String> = others
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(String::from)
        .collect();
    let more = files.len() + others.len() > room;
    files.truncate(room);
    let left = room - files.len();
    let dir = worktree.to_path_buf();
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
    let branch = branch.unwrap_or_default();
    Ok(Change {
        branch,
        files,
        more,
    })
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
    let path = plain(path)?;
    let from = from.map(plain).transpose()?;
    let env = crate::files::git_env();
    let base = base(project, worktree).await.map_err(Error::Other)?;
    let mut args: Vec<&str> = DIFF.to_vec();
    args.extend(["-p", "--no-color", "-U3", &base, "--", path]);
    args.extend(from);
    // As git printed it: trimmed, a last line that is a blank context line would be lost.
    let diff = git_printed(&env, worktree, &args, LIMIT)
        .await
        .map_err(Error::Other)?;
    if !diff.is_empty() {
        return Ok(parse(&diff));
    }
    let others = [
        "ls-files",
        "-z",
        "--others",
        "--exclude-standard",
        "--",
        path,
    ];
    let others = git(&env, worktree, &others, LIMIT)
        .await
        .map_err(Error::Other)?;
    if !others.split('\0').any(|p| p == path) {
        return Ok(Lines::default());
    }
    let roots = [crate::folders::real(worktree)];
    let at = worktree.join(path).to_string_lossy().into_owned();
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
