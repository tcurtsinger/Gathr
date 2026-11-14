//! A change's size as git gives it (0054): the lines it adds that someone wrote, and the line a
//! project's own recent changes set.

/// A tool writes these, whatever folder they're in.
pub const LOCKS: [&str; 8] = [
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "go.sum",
    "Gemfile.lock",
    "composer.lock",
];

/// The least a project's line can be.
pub const FLOOR: i64 = 3_000;
/// Below this many changes, a project's history sets no line.
const FEWEST: usize = 10;
/// The largest counted files a count keeps.
const TOP: usize = 5;

/// One file of a diff, as `--numstat` gives it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct File {
    pub path: String,
    /// None for a binary file.
    pub added: Option<i64>,
    pub removed: i64,
}

/// What a change adds and removes, and its largest files, leaving out what nobody wrote by hand.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Count {
    pub added: i64,
    pub removed: i64,
    pub top: Vec<(String, i64)>,
}

/// `git diff --numstat --no-renames -z`: "<added>\t<removed>\t<path>" between NULs.
pub fn numstat(out: &str) -> Vec<File> {
    out.split('\0').filter_map(file).collect()
}

fn file(token: &str) -> Option<File> {
    // In a log, each commit's first file follows its header's newline.
    let mut parts = token.trim_start_matches('\n').splitn(3, '\t');
    let (added, removed, path) = (parts.next()?, parts.next()?, parts.next()?);
    Some(File {
        path: path.to_string(),
        added: added.parse().ok(),
        removed: removed.parse().unwrap_or(0),
    })
}

/// `git log --numstat --no-renames -z --format=%x01%H`: each commit's files, newest first.
pub fn history(out: &str) -> Vec<Vec<File>> {
    let mut commits: Vec<Vec<File>> = Vec::new();
    for token in out.split('\0') {
        if token.starts_with('\x01') {
            commits.push(Vec::new());
        } else if let (Some(files), Some(f)) = (commits.last_mut(), file(token)) {
            files.push(f);
        }
    }
    commits
}

/// `git check-attr -z`: "<path>\0<attribute>\0<value>\0" each. `set` or `true` marks the path.
pub fn marked(out: &str) -> Vec<String> {
    let parts: Vec<&str> = out.split('\0').collect();
    let mut paths: Vec<String> = parts
        .chunks(3)
        .filter(|c| c.len() == 3 && matches!(c[2], "set" | "true"))
        .map(|c| c[0].to_string())
        .collect();
    paths.dedup();
    paths
}

/// The files' count, leaving out binaries, lock files and the paths `marked`.
pub fn counted(files: &[File], marked: &[String]) -> Count {
    let mut kept: Vec<(&File, i64)> = files
        .iter()
        .filter(|f| {
            let name = f.path.rsplit('/').next().unwrap_or(&f.path);
            !LOCKS.contains(&name) && !marked.contains(&f.path)
        })
        .filter_map(|f| Some((f, f.added?)))
        .collect();
    kept.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.path.cmp(&b.0.path)));
    Count {
        added: kept.iter().map(|(_, n)| n).sum(),
        removed: kept.iter().map(|(f, _)| f.removed).sum(),
        top: kept
            .iter()
            .take(TOP)
            .map(|(f, n)| (f.path.clone(), *n))
            .collect(),
    }
}

/// The project's line: the most any of its recent changes added, never under the floor.
pub fn line(commits: &[Count]) -> i64 {
    if commits.len() < FEWEST {
        return FLOOR;
    }
    commits
        .iter()
        .map(|c| c.added)
        .max()
        .unwrap_or(0)
        .max(FLOOR)
}

/// Past the line, and past the line again beyond what the person let through.
pub fn over(counted: i64, line: i64, let_through: Option<i64>) -> bool {
    counted > line && let_through.is_none_or(|through| counted > through + line)
}

/// A count as people read it: 3,200.
pub fn grouped(n: i64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}
