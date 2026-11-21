//! The person's files in a chat's messages, and the tree, words and pictures the window shows from
//! a chat's folders.
//! A file is saved in `<Project>/Shared/files/<chat part>`, the one folder every agent of the chat
//! reads on both engines, and named in the message by one line with its path: each engine opens it
//! from there. The folder goes with the chat, as its agents' folders do.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::folders::{self, Made};

/// What starts the line naming a file in a message's words: `Attached: <full path>`.
pub const ATTACHED: &str = "Attached: ";
/// The largest file the person may add to a message.
pub const MOST: usize = 10 * 1024 * 1024;

/// Where the person's files of `chat` go, recorded first so that deleting the chat removes it.
pub fn folder(conn: &Connection, data: &Path, chat: &str) -> Result<PathBuf, Error> {
    let dir = folders::shared_of(conn, data, chat)?
        .join("files")
        .join(folders::chat_part(chat));
    let made = Made {
        path: &dir,
        kind: "folder",
        chat,
        agent: None,
        project: None,
    };
    folders::made(conn, &made)?;
    Ok(dir)
}

/// Saves `bytes` in `dir` as the person's file `name`, made safe and unique there: its saved name.
pub fn save(dir: &Path, name: &str, bytes: &[u8]) -> Result<String, Error> {
    std::fs::create_dir_all(dir)?;
    // The extension is kept whole: a long name is cut in its stem.
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= 8 => (stem, ext),
        _ => (name, ""),
    };
    let ext = match folders::safe_name(ext, "") {
        ext if ext.is_empty() => ext,
        ext => format!(".{ext}"),
    };
    let stem = folders::safe_name(stem, "file");
    for n in 1.. {
        let saved = match n {
            1 => format!("{stem}{ext}"),
            n => format!("{stem} {n}{ext}"),
        };
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(dir.join(&saved));
        match file {
            Ok(mut file) => {
                std::io::Write::write_all(&mut file, bytes)?;
                return Ok(saved);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!("a number is free")
}

/// The lines naming `names`, files saved in the chat's folder `dir`; refused when one isn't there.
pub fn lines(dir: &Path, names: &[String]) -> Result<String, Error> {
    let mut lines = Vec::new();
    for name in names {
        let one = Path::new(name).file_name().and_then(|n| n.to_str()) == Some(name);
        let plain = one && !name.contains([':', '\\', '/']);
        let file = dir.join(name);
        if !plain || !file.is_file() {
            return Err(Error::Refused(format!("{name} isn't a file of this chat")));
        }
        lines.push(format!("{ATTACHED}{}", file.display()));
    }
    Ok(lines.join("\n"))
}

/// The lines naming `paths`, full paths of files in `roots` (a chat's tree); refused when one isn't.
/// Blocks on the disk.
pub fn attached(roots: &[PathBuf], paths: &[String]) -> Result<String, Error> {
    let mut lines = Vec::new();
    for path in paths {
        let Some(found) = inside(roots, path).filter(|f| f.is_file()) else {
            let why = format!("{path} isn't a file in this chat's folders");
            return Err(Error::Refused(why));
        };
        lines.push(format!("{ATTACHED}{}", found.display()));
    }
    Ok(lines.join("\n"))
}

/// `words` with the lines naming the files after them.
pub fn with_lines(words: &str, lines: &str) -> String {
    match (words.trim().is_empty(), lines.is_empty()) {
        (_, true) => words.to_string(),
        (true, false) => lines.to_string(),
        (false, false) => format!("{}\n\n{lines}", words.trim_end()),
    }
}

/// The folders a picture of `chat` may be shown from, anywhere in its project: the project's folder
/// in the agent folder, which holds Shared and every agent's folders, and the project's own folder.
pub fn roots(conn: &Connection, data: &Path, chat: &str) -> Result<Vec<PathBuf>, Error> {
    let shared = folders::shared_of(conn, data, chat)?;
    let mut roots = vec![shared.parent().unwrap_or(&shared).to_path_buf()];
    let own: Option<String> = conn
        .query_row(
            "SELECT p.folder FROM chats c JOIN projects p ON p.id = c.project_id WHERE c.id = ?1",
            [chat],
            |r| r.get(0),
        )
        .optional()?;
    roots.extend(own.map(PathBuf::from));
    Ok(roots.iter().map(|r| folders::real(r)).collect())
}

/// `path` as the disk names it, when it is inside one of `roots` once links and `..` are followed.
pub(crate) fn inside(roots: &[PathBuf], path: &str) -> Option<PathBuf> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return None;
    }
    let found = folders::real(&std::fs::canonicalize(path).ok()?);
    roots
        .iter()
        .any(|root| found.starts_with(root))
        .then_some(found)
}

/// The picture at `path` with its media type, when it is a png, jpeg, gif or webp inside one of
/// `roots` once links and `..` are followed. Blocks on the disk.
pub fn picture(roots: &[PathBuf], path: &str) -> Option<(PathBuf, &'static str)> {
    let found = inside(roots, path)?;
    let ext = found.extension()?.to_str()?.to_lowercase();
    let kind = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    found.is_file().then_some((found, kind))
}

/// The largest file the window shows as text.
pub const SHOWN: u64 = 2 * 1024 * 1024;

/// A folder at the top of a chat's tree in the window.
#[derive(Debug, Serialize, Deserialize)]
pub struct Part {
    pub name: String,
    pub path: PathBuf,
    /// The agent whose folder it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent: Option<String>,
}

/// The folders of one chat the window shows: the project's own, Shared, and each of its agents'
/// folder for this chat, those that are on the disk. Never another chat's.
#[derive(Debug, Serialize, Deserialize)]
pub struct Tree {
    pub project: Option<Part>,
    pub shared: Option<Part>,
    pub agents: Vec<Part>,
}

impl Tree {
    pub fn roots(&self) -> Vec<PathBuf> {
        let tops = self.project.iter().chain(&self.shared).chain(&self.agents);
        tops.map(|p| p.path.clone()).collect()
    }
}

/// `chat`'s tree. Blocks on the disk.
pub fn tree(conn: &Connection, data: &Path, chat: &str) -> Result<Tree, Error> {
    let part = |path: PathBuf, name: Option<String>, agent: Option<String>| {
        let path = folders::real(&path);
        let name = name.unwrap_or_else(|| {
            path.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into()
        });
        path.is_dir().then_some(Part { name, path, agent })
    };
    let own: Option<String> = conn
        .query_row(
            "SELECT p.folder FROM chats c JOIN projects p ON p.id = c.project_id WHERE c.id = ?1",
            [chat],
            |r| r.get(0),
        )
        .optional()?;
    let shared = folders::shared_of(conn, data, chat)?;
    let mut agents = Vec::new();
    for m in crate::team::members(conn, chat)? {
        if let Some(path) = folders::workspace(conn, chat, &m.agent)? {
            agents.extend(part(path, Some(m.name), Some(m.agent)));
        }
    }
    Ok(Tree {
        project: own.and_then(|own| part(PathBuf::from(own), None, None)),
        shared: part(shared, Some(folders::SHARED.into()), None),
        agents,
    })
}

/// One thing in a folder of the tree.
#[derive(Debug, Serialize)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub folder: bool,
}

/// The folder at `path` inside one of `roots`, folders first and then files, each by name, `.git`
/// left out; none when it isn't a folder in them. Blocks on the disk.
pub fn listing(roots: &[PathBuf], path: &str) -> Option<Vec<Entry>> {
    let found = inside(roots, path)?;
    let mut entries: Vec<Entry> = std::fs::read_dir(&found)
        .ok()?
        .flatten()
        .map(|e| Entry {
            name: e.file_name().to_string_lossy().into(),
            folder: e.path().is_dir(),
            path: e.path(),
        })
        .filter(|e| e.name != ".git")
        .collect();
    entries.sort_by_cached_key(|e| (!e.folder, e.name.to_lowercase()));
    Some(entries)
}

/// The most files a search answers.
pub const FOUND: usize = 50;

/// A file whose name holds the words searched for: its path, and its place in the tree from the top
/// folder's name down to its own name.
#[derive(Debug, Serialize)]
pub struct Found {
    pub path: PathBuf,
    pub crumbs: Vec<String>,
}

/// The files of `tree` whose name holds `words`, case ignored: a name that starts with them first,
/// then by name, at most `FOUND`. In a git checkout git lists them, so what the project ignores (its
/// builds, its packages) is never read; any other folder is walked. `.git` and links that lead out of
/// the tree are left out. The top folders are read at once.
pub async fn find(tree: &Tree, words: &str) -> Vec<Found> {
    let words = words.trim().to_lowercase();
    if words.is_empty() {
        return Vec::new();
    }
    let parts: Vec<&Part> = tree
        .project
        .iter()
        .chain(&tree.shared)
        .chain(&tree.agents)
        .collect();
    let mut reads = tokio::task::JoinSet::new();
    for (i, part) in parts.iter().enumerate() {
        let (root, words) = (part.path.clone(), words.clone());
        reads.spawn(async move {
            let listed = match git_files(&root).await {
                Some(listed) => listed,
                None => tokio::task::spawn_blocking(move || walk(&root))
                    .await
                    .unwrap_or_default(),
            };
            let hit = |rel: String| {
                let name = rel.rsplit('/').next().unwrap_or_default().to_lowercase();
                let at = name.find(&words)?;
                Some((at != 0, name, i, rel))
            };
            listed.into_iter().filter_map(hit).collect::<Vec<_>>()
        });
    }
    let mut hits: Vec<_> = reads.join_all().await.into_iter().flatten().collect();
    hits.sort();
    let roots = tree.roots();
    hits.into_iter()
        .filter_map(|(_, _, i, rel)| {
            let part = parts[i];
            let path = rel.split('/').fold(part.path.clone(), |p, s| p.join(s));
            inside(&roots, &path.to_string_lossy())?.is_file().then(|| {
                let steps = rel.split('/').map(String::from);
                let crumbs = std::iter::once(part.name.clone()).chain(steps).collect();
                Found { path, crumbs }
            })
        })
        .take(FOUND)
        .collect()
}

/// The files git knows in the checkout at `root` (tracked, and new ones it doesn't ignore), each as
/// its path from `root` with `/` between steps; none when `root` isn't a checkout or git fails.
async fn git_files(root: &Path) -> Option<Vec<String>> {
    if !root.join(".git").exists() {
        return None;
    }
    let env: crate::platform::env::Vars = std::env::vars_os()
        .filter(|(k, _)| {
            let k = k.to_string_lossy();
            !crate::platform::env::LOCAL_GIT_VARS
                .iter()
                .any(|v| v.eq_ignore_ascii_case(&k))
        })
        .collect();
    let args = [
        "ls-files",
        "-z",
        "--cached",
        "--others",
        "--exclude-standard",
    ];
    let limit = std::time::Duration::from_secs(30);
    let ran = crate::run::run(Path::new("git"), &args, root, &env, limit).await;
    let ran = ran.ok().filter(|r| r.code == Some(0))?;
    let mut listed: Vec<String> = ran
        .out
        .split('\0')
        .filter(|l| !l.is_empty())
        .map(String::from)
        .collect();
    // A file in a merge is listed once per side.
    listed.dedup();
    Some(listed)
}

/// The files under `root`, each as its path from it with `/` between steps, never inside `.git` or
/// through a link.
fn walk(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut folders = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, at)) = folders.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            let (Ok(kind), name) = (entry.file_type(), entry.file_name()) else {
                continue;
            };
            let name = name.to_string_lossy();
            let here = match at.is_empty() {
                true => name.to_string(),
                false => format!("{at}/{name}"),
            };
            if kind.is_dir() && name != ".git" {
                folders.push((entry.path(), here));
            } else if kind.is_file() {
                files.push(here);
            }
        }
    }
    files
}

/// Why a file isn't shown as text.
#[derive(Debug, PartialEq)]
pub enum Unshown {
    /// Not a file in the roots: moved, deleted, or never one of them.
    Gone,
    /// Over `SHOWN`.
    Large,
    /// Not UTF-8 text, or a PDF or workbook that can't be read (`documents`).
    NotText,
}

/// The words of the file at `path` inside one of `roots`. Blocks on the disk.
pub fn text(roots: &[PathBuf], path: &str) -> Result<String, Unshown> {
    let found = inside(roots, path)
        .filter(|f| f.is_file())
        .ok_or(Unshown::Gone)?;
    let size = std::fs::metadata(&found).map_err(|_| Unshown::Gone)?.len();
    if size > SHOWN {
        return Err(Unshown::Large);
    }
    let bytes = std::fs::read(&found).map_err(|_| Unshown::Gone)?;
    match String::from_utf8(bytes) {
        Ok(words) if !words.contains('\0') => Ok(words),
        _ => Err(Unshown::NotText),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_outside_the_chats_folders_is_refused() {
        let base = std::env::temp_dir().join(format!("gathr-files-{}", crate::ids::span_id()));
        let shared = base.join("Shared");
        let elsewhere = base.join("Elsewhere");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(shared.join("in.png"), b"png").unwrap();
        std::fs::write(elsewhere.join("out.png"), b"png").unwrap();
        let roots = [folders::real(&shared)];
        let shown = |p: PathBuf| picture(&roots, &p.to_string_lossy()).is_some();
        assert!(shown(shared.join("in.png")));
        assert!(
            !shown(shared.join("..").join("Elsewhere").join("out.png")),
            "a '..' path"
        );
        assert!(
            !shown(elsewhere.join("out.png")),
            "an absolute path elsewhere"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn a_folder_or_file_outside_the_chats_folders_is_refused() {
        let base = std::env::temp_dir().join(format!("gathr-files-{}", crate::ids::span_id()));
        let shared = base.join("Shared");
        let elsewhere = base.join("Elsewhere");
        std::fs::create_dir_all(shared.join("docs")).unwrap();
        std::fs::create_dir_all(elsewhere.join("docs")).unwrap();
        std::fs::write(shared.join("in.md"), "in").unwrap();
        std::fs::write(elsewhere.join("out.md"), "out").unwrap();
        let roots = [folders::real(&shared)];
        let at = |p: PathBuf| p.to_string_lossy().into_owned();
        let listed = listing(&roots, &at(shared.clone())).unwrap();
        let names: Vec<&str> = listed.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, ["docs", "in.md"], "folders first");
        assert!(
            listing(&roots, &at(shared.join("..").join("Elsewhere"))).is_none(),
            "'..'"
        );
        assert!(
            listing(&roots, &at(elsewhere.clone())).is_none(),
            "a folder elsewhere"
        );
        assert!(listing(&roots, "Elsewhere").is_none(), "a relative path");
        assert_eq!(text(&roots, &at(shared.join("in.md"))), Ok("in".into()));
        let out = shared.join("..").join("Elsewhere").join("out.md");
        assert_eq!(text(&roots, &at(out)), Err(Unshown::Gone), "'..'");
        let out = elsewhere.join("out.md");
        assert_eq!(
            text(&roots, &at(out)),
            Err(Unshown::Gone),
            "a file elsewhere"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn a_search_finds_files_by_name_only_in_the_chats_folders() {
        let base = std::env::temp_dir().join(format!("gathr-find-{}", crate::ids::span_id()));
        let (shared, project) = (base.join("Shared"), base.join("shop"));
        let elsewhere = base.join("Elsewhere");
        for dir in [
            shared.join(".git"),
            project.join("src"),
            project.join("build"),
            elsewhere.clone(),
        ] {
            std::fs::create_dir_all(dir).unwrap();
        }
        // Shared isn't a checkout: it is walked, and its `.git` and a link out of it are passed by.
        std::fs::write(shared.join("export-plan.md"), "").unwrap();
        std::fs::write(shared.join(".git").join("export-head"), "").unwrap();
        std::fs::write(elsewhere.join("export-secret.md"), "").unwrap();
        #[cfg(windows)]
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(shared.join("out"))
            .arg(&elsewhere)
            .output()
            .unwrap();
        #[cfg(windows)]
        assert!(made.status.success(), "mklink: {made:?}");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&elsewhere, shared.join("out")).unwrap();
        // The project is: what it ignores isn't listed.
        let git = |args: &[&str]| {
            let ran = std::process::Command::new("git")
                .args(args)
                .current_dir(&project)
                .output()
                .unwrap();
            assert!(ran.status.success(), "git {args:?}: {ran:?}");
        };
        git(&["init", "--quiet"]);
        std::fs::write(project.join(".gitignore"), "build/\n").unwrap();
        std::fs::write(project.join("src").join("ORDER-EXPORT.ts"), "").unwrap();
        std::fs::write(project.join("build").join("export.js"), "").unwrap();
        let part = |path: &Path, name: &str| Part {
            name: name.into(),
            path: folders::real(path),
            agent: None,
        };
        let tree = Tree {
            project: Some(part(&project, "shop")),
            shared: Some(part(&shared, "Shared")),
            agents: Vec::new(),
        };
        let found = find(&tree, " Export").await;
        let crumbs: Vec<Vec<&str>> = found
            .iter()
            .map(|f| f.crumbs.iter().map(String::as_str).collect())
            .collect();
        assert_eq!(
            crumbs,
            [
                vec!["Shared", "export-plan.md"],
                vec!["shop", "src", "ORDER-EXPORT.ts"]
            ],
            "starts with the words first; never .git, a link out, or what git ignores"
        );
        assert!(found[0].path.is_file());
        assert!(find(&tree, "  ").await.is_empty());
        let _ = std::fs::remove_dir_all(&base);
    }
}
