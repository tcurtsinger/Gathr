//! The person's files in a chat's messages, and the pictures the window shows from a chat's folders.
//! A file is saved in `<Project>/Shared/files/<chat part>`, the one folder every agent of the chat
//! reads on both engines, and named in the message by one line with its path: each engine opens it
//! from there. The folder goes with the chat, as its agents' folders do.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

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

/// The picture at `path` with its media type, when it is a png, jpeg, gif or webp inside one of
/// `roots` once links and `..` are followed. Blocks on the disk.
pub fn picture(roots: &[PathBuf], path: &str) -> Option<(PathBuf, &'static str)> {
    let path = Path::new(path);
    if !path.is_absolute() {
        return None;
    }
    let found = folders::real(&std::fs::canonicalize(path).ok()?);
    let ext = found.extension()?.to_str()?.to_lowercase();
    let kind = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    let inside = roots.iter().any(|root| found.starts_with(root));
    (inside && found.is_file()).then_some((found, kind))
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
}
