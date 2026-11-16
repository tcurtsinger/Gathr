//! The trial's made-up project (R4): its files in `fixture/`, the repository made from them, a
//! worktree's diff against `main`, and T1's hidden test.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use super::common::git;

pub fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/trial/fixture")
}

/// Copies the folder `from` into `to`, over what is there.
pub fn copy(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap().flatten() {
        let target = to.join(entry.file_name());
        match entry.file_type().unwrap().is_dir() {
            true => copy(&entry.path(), &target),
            false => _ = std::fs::copy(entry.path(), target).unwrap(),
        }
    }
}

/// The project's repository in `dir`: the project on `main`, and the planted change on
/// `trial/planted`, a branch beside it. No remote: nothing can be pushed.
pub fn make(dir: &Path) {
    copy(&fixture().join("project"), dir);
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "Durations as short text"]);
    git(dir, &["checkout", "-q", "-b", "trial/planted"]);
    copy(&fixture().join("planted"), dir);
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", "Accept upper-case units"]);
    git(dir, &["checkout", "-q", "main"]);
}

/// Leaves the repository in `dir` with only `main` and `trial/planted`, so no agent finds an earlier
/// run's work with `git log --all`. Archiving a chat removes its worktrees but keeps their branches,
/// and keeps a worktree with changes not committed (0034): those go here, with the service stopped.
pub fn forget(dir: &Path) {
    let listed = git(dir, &["worktree", "list", "--porcelain"]);
    let worktrees = listed.lines().filter_map(|l| l.strip_prefix("worktree "));
    // The first is the repository's own.
    for worktree in worktrees.skip(1) {
        git(dir, &["worktree", "remove", "--force", worktree]);
    }
    git(dir, &["worktree", "prune"]);
    let branches = git(dir, &["branch", "--format=%(refname:short)"]);
    for branch in branches.lines() {
        if !["main", "trial/planted"].contains(&branch) {
            git(dir, &["branch", "-D", branch]);
        }
    }
}

/// What a worktree changed against `main`, committed and uncommitted, new files included (they are
/// added to its index as intended, which changes nothing else).
pub fn diff(worktree: &Path) -> String {
    git(worktree, &["add", "--intent-to-add", "."]);
    git(worktree, &["diff", "--no-color", "--no-ext-diff", "main"])
}

/// `src/duration.js` and `src/cli.js` as they are in `dir`, empty when gone.
pub fn sources(dir: &Path) -> [String; 2] {
    ["src/duration.js", "src/cli.js"]
        .map(|f| std::fs::read_to_string(dir.join(f)).unwrap_or_default())
}

const HIDDEN: &str = "format.hidden.test.mjs";

/// Whether T1's hidden test passes in `dir`: written there last, run with node for at most 30 s,
/// and deleted.
pub fn hidden_test(dir: &Path) -> bool {
    let file = dir.join(HIDDEN);
    std::fs::copy(fixture().join("hidden").join(HIDDEN), &file).unwrap();
    let mut node = Command::new("node")
        .arg(HIDDEN)
        .current_dir(dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("node runs the hidden test: is Node on PATH?");
    let deadline = Instant::now() + Duration::from_secs(30);
    let passed = loop {
        if let Some(status) = node.try_wait().unwrap() {
            break status.success();
        }
        if Instant::now() > deadline {
            let _ = node.kill();
            let _ = node.wait();
            break false;
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    std::fs::remove_file(&file).unwrap();
    passed
}
