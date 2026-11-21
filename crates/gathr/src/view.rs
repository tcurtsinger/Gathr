//! A page in a chat's folders, or in Designs, shown as it looks: a view is a long random key that
//! lets the window's sealed frame read the page's folder and what is below it, and nothing else.
//! Kept in memory only. A view opened for notes on a design's page also has the page served with
//! `NOTES`, the script that lets the person point at a part of it.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use crate::{files, folders, ids};

/// The most views kept: opening one more lets the oldest go.
pub const MOST: usize = 100;

/// What a design's page is served with in a chat's design tab: it tells the window which part the
/// person clicked while Comment is on, and draws the pins the window gives it. It only ever posts to
/// the window that holds the frame; the window takes what it says as untrusted words.
pub const NOTES: &str = concat!("\n<script>", include_str!("view-notes.js"), "</script>\n");

/// One open view: its key, the folder it reads, and the page served with `NOTES`, if any.
struct View {
    key: String,
    folder: PathBuf,
    noted: Option<PathBuf>,
}

/// The open views, oldest first.
pub struct Views(Mutex<VecDeque<View>>);

impl Views {
    pub const fn new() -> Self {
        Self(Mutex::new(VecDeque::new()))
    }

    fn lock(&self) -> MutexGuard<'_, VecDeque<View>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A view of the file at `path` when it is inside one of `roots` (a chat's tree): its key and the
    /// file's name. Blocks on the disk.
    pub fn open(&self, roots: &[PathBuf], path: &str) -> Option<(String, String)> {
        self.add(roots, path, false, false)
    }

    /// As `open`, for notes: only an HTML page right in a design's folder in `designs` (a chat's
    /// part of Designs), and that page alone is served with `NOTES`. Blocks on the disk.
    pub fn note(&self, designs: &Path, path: &str) -> Option<(String, String)> {
        let page = files::inside(&[designs.to_path_buf()], path)?;
        let html = page
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("htm"));
        let in_design = page.parent().and_then(Path::parent) == Some(designs);
        (html && in_design).then_some(())?;
        self.add(&[designs.to_path_buf()], path, false, true)
    }

    /// As `open`, but a page's folder keeps its one key while it is kept, now the newest: a gallery
    /// that mounts its frames and lets them go as they scroll by holds one key per design.
    pub fn reopen(&self, roots: &[PathBuf], path: &str) -> Option<(String, String)> {
        self.add(roots, path, true, false)
    }

    fn add(
        &self,
        roots: &[PathBuf],
        path: &str,
        reuse: bool,
        noted: bool,
    ) -> Option<(String, String)> {
        let page = files::inside(roots, path).filter(|p| p.is_file())?;
        let name = page.file_name()?.to_str()?.to_string();
        let folder = page.parent()?.to_path_buf();
        let mut views = self.lock();
        let kept = match reuse {
            true => views.iter().position(|v| v.folder == folder),
            false => None,
        };
        let key = match kept.and_then(|i| views.remove(i)) {
            Some(v) => v.key,
            None => ids::token(),
        };
        if views.len() >= MOST {
            views.pop_front();
        }
        let noted = noted.then_some(page);
        views.push_back(View {
            key: key.clone(),
            folder,
            noted,
        });
        Some((key, name))
    }

    /// The file at `rest` in the folder of the view `key`, once links and `..` are followed, and
    /// whether it is the page this view serves with `NOTES`: none when the key isn't open or the file
    /// isn't in that folder or below it. Never a file or folder whose name starts with a dot (`.env`,
    /// `.git`): a page loads from the web, so what it can read it can send out. Blocks on the disk.
    pub fn file(&self, key: &str, rest: &str) -> Option<(PathBuf, bool)> {
        let (folder, noted) = {
            let views = self.lock();
            let v = views.iter().find(|v| v.key == key)?;
            (v.folder.clone(), v.noted.clone())
        };
        let found = folders::real(&std::fs::canonicalize(folder.join(rest)).ok()?);
        let below = found.strip_prefix(&folder).ok()?;
        let hidden = below
            .components()
            .any(|c| c.as_os_str().to_string_lossy().starts_with('.'));
        let noted = noted.is_some_and(|page| page == found);
        (!hidden && found.is_file()).then_some((found, noted))
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

    #[test]
    fn only_a_design_page_opened_for_notes_is_served_with_the_script() {
        let base = std::env::temp_dir().join(format!("gathr-notes-{}", ids::span_id()));
        let designs = base.join("Designs").join("Shop");
        let design = designs.join("export");
        std::fs::create_dir_all(design.join("assets")).unwrap();
        std::fs::write(design.join("a.html"), "<p>a</p>").unwrap();
        std::fs::write(design.join("b.html"), "<p>b</p>").unwrap();
        std::fs::write(design.join("notes.md"), "words").unwrap();
        std::fs::write(design.join("assets").join("c.html"), "<p>c</p>").unwrap();
        std::fs::write(designs.join("loose.html"), "<p>loose</p>").unwrap();
        let designs = folders::real(&std::fs::canonicalize(&designs).unwrap());
        let at = |p: PathBuf| p.to_string_lossy().into_owned();
        let page = at(design.join("a.html"));
        let views = Views::new();
        let noted = |v: &Views, key: &str, rest: &str| v.file(key, rest).map(|f| f.1);
        let (key, name) = views.note(&designs, &page).unwrap();
        assert_eq!(name, "a.html");
        assert_eq!(noted(&views, &key, "a.html"), Some(true));
        assert_eq!(noted(&views, &key, "b.html"), Some(false), "another option");
        let below = noted(&views, &key, "assets/c.html");
        assert_eq!(below, Some(false), "a page below it");
        let note = |p: PathBuf| views.note(&designs, &at(p));
        assert!(note(design.join("notes.md")).is_none(), "not a page");
        let deeper = note(design.join("assets").join("c.html"));
        assert!(deeper.is_none(), "not right in a design");
        assert!(
            note(designs.join("loose.html")).is_none(),
            "not in a design"
        );
        let roots = [designs.clone()];
        let (plain, _) = views.open(&roots, &page).unwrap();
        assert_eq!(noted(&views, &plain, "a.html"), Some(false), "a file's tab");
        let gallery = Views::new();
        let (card, _) = gallery.reopen(&roots, &page).unwrap();
        assert_eq!(noted(&gallery, &card, "a.html"), Some(false), "the gallery");
        let _ = std::fs::remove_dir_all(&base);
    }
}
