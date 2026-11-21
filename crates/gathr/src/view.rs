//! A page in a chat's folders shown as it looks: a view is a long random key that lets the window's
//! sealed frame read the page's folder and what is below it, and nothing else. Kept in memory only.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use crate::{files, folders, ids};

/// The most views kept: opening one more lets the oldest go.
pub const MOST: usize = 100;

/// The open views, oldest first: each key with the folder it reads.
pub struct Views(Mutex<VecDeque<(String, PathBuf)>>);

impl Views {
    pub const fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<(String, PathBuf)>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A view of the file at `path` when it is inside one of `roots` (a chat's tree): its key and the
    /// file's name. Blocks on the disk.
    pub fn open(&self, roots: &[PathBuf], path: &str) -> Option<(String, String)> {
        let page = files::inside(roots, path).filter(|p| p.is_file())?;
        let name = page.file_name()?.to_str()?.to_string();
        let key = ids::token();
        let mut views = self.lock();
        if views.len() >= MOST {
            views.pop_front();
        }
        views.push_back((key.clone(), page.parent()?.to_path_buf()));
        Some((key, name))
    }

    /// The file at `rest` in the folder of the view `key`, once links and `..` are followed: none when
    /// the key isn't open or the file isn't in that folder or below it. Never a file or folder whose
    /// name starts with a dot (`.env`, `.git`): a page loads from the web, so what it can read it can
    /// send out. Blocks on the disk.
    pub fn file(&self, key: &str, rest: &str) -> Option<PathBuf> {
        let folder = self.lock().iter().find(|(k, _)| k == key)?.1.clone();
        let found = folders::real(&std::fs::canonicalize(folder.join(rest)).ok()?);
        let below = found.strip_prefix(&folder).ok()?;
        let hidden = below
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
        (!hidden && found.is_file()).then_some(found)
    }
}

impl Default for Views {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_view_reads_only_its_pages_folder_and_below() {
        let base = std::env::temp_dir().join(format!("gathr-view-{}", ids::span_id()));
        let (shared, elsewhere) = (base.join("Shared"), base.join("Elsewhere"));
        let page = shared.join("site");
        std::fs::create_dir_all(page.join("assets")).unwrap();
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(page.join("index.html"), "<p>hi</p>").unwrap();
        std::fs::write(page.join("assets").join("site.css"), "p{}").unwrap();
        std::fs::write(shared.join("notes.md"), "above the page").unwrap();
        std::fs::create_dir_all(page.join(".git")).unwrap();
        std::fs::write(page.join(".env"), "KEY=made-up").unwrap();
        std::fs::write(page.join(".git").join("config.json"), "{}").unwrap();
        std::fs::write(elsewhere.join("secret.md"), "another chat's").unwrap();
        #[cfg(windows)]
        let made = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(page.join("out"))
            .arg(&elsewhere)
            .output()
            .unwrap();
        #[cfg(windows)]
        assert!(made.status.success(), "mklink: {made:?}");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&elsewhere, page.join("out")).unwrap();
        let views = Views::new();
        let roots = [folders::real(&shared)];
        let at = |p: PathBuf| p.to_string_lossy().into_owned();
        let (key, name) = views.open(&roots, &at(page.join("index.html"))).unwrap();
        assert_eq!(name, "index.html");
        assert!(views.file(&key, "index.html").is_some());
        assert!(
            views.file(&key, "assets/site.css").is_some(),
            "a file beside it"
        );
        assert!(views.file("0f0f", "index.html").is_none(), "an unknown key");
        assert!(views.file(&key, ".env").is_none(), "a dot-file");
        assert!(
            views.file(&key, ".git/config.json").is_none(),
            "inside a dot-folder"
        );
        assert!(views.file(&key, "../notes.md").is_none(), "'..'");
        assert!(
            views.file(&key, "..\\notes.md").is_none(),
            "'..' the Windows way"
        );
        assert!(views.file(&key, "out/secret.md").is_none(), "a link out");
        assert!(
            views.file(&key, &at(shared.join("notes.md"))).is_none(),
            "a full path"
        );
        assert!(views.file(&key, "assets").is_none(), "a folder");
        assert!(
            views
                .open(&roots, &at(elsewhere.join("secret.md")))
                .is_none(),
            "a page outside the chat's folders"
        );
        let out = page
            .join("..")
            .join("..")
            .join("Elsewhere")
            .join("secret.md");
        assert!(
            views.open(&roots, &at(out)).is_none(),
            "a page through '..'"
        );
        for _ in 0..MOST {
            views.open(&roots, &at(page.join("index.html"))).unwrap();
        }
        assert!(
            views.file(&key, "index.html").is_none(),
            "the oldest let go"
        );
        let _ = std::fs::remove_dir_all(&base);
    }
}
