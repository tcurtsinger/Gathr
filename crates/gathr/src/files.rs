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
fn inside(roots: &[PathBuf], path: &str) -> Option<PathBuf> {
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

/// Why a file isn't shown as text.
#[derive(Debug, PartialEq)]
pub enum Unshown {
    /// Not a file in the roots: moved, deleted, or never one of them.
    Gone,
    /// Over `SHOWN`.
    Large,
    /// Not UTF-8 text.
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
}
