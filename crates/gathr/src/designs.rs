//! Every design, for the Designs screen: each folder of HTML pages in a part of Designs
//! (`<agent folder>/Designs/<Project>/<design>`), grouped by its part, with the project the part is
//! of and that project's newest chat.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use serde::Serialize;

use crate::{Error, folders};

/// A part of Designs that holds a design: a project's, or `Chats`.
#[derive(Debug, Serialize)]
pub struct Part {
    /// Its folder's name.
    pub folder: String,
    /// The project with that folder name: none for `Chats`, or for a project that's gone.
    pub project: Option<Project>,
    /// That project's newest chat that isn't archived.
    pub chat: Option<String>,
    /// Newest first.
    pub designs: Vec<Design>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Project {
    pub id: String,
    pub name: String,
}

/// A project's folder name in the agent folder, the project, and its newest chat that isn't archived.
pub type Owner = (String, Project, Option<String>);

#[derive(Debug, Serialize)]
pub struct Design {
    pub name: String,
    pub path: PathBuf,
    /// Its pages' full paths, in their names' order.
    pub pages: Vec<PathBuf>,
    /// When its newest page changed, in milliseconds since 1970.
    pub modified: u64,
}

/// The Designs folder, as the disk names it.
pub fn folder(conn: &Connection, data: &Path) -> Result<PathBuf, Error> {
    Ok(folders::real(
        &folders::root(conn, data)?.join(folders::DESIGNS),
    ))
}

/// Every project that has a folder in the agent folder.
pub fn owners(conn: &Connection) -> Result<Vec<Owner>, Error> {
    let mut stmt = conn.prepare(
        "SELECT p.folder_name, p.id, p.name,
           (SELECT c.id FROM chats c WHERE c.project_id = p.id AND c.archived_ts IS NULL
            ORDER BY c.created_ts DESC LIMIT 1)
         FROM projects p WHERE p.folder_name IS NOT NULL",
    )?;
    let rows = stmt.query_map([], |r| {
        let project = Project {
            id: r.get(1)?,
            name: r.get(2)?,
        };
        Ok((r.get(0)?, project, r.get(3)?))
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The designs in `root`, the Designs folder: a design is a folder in a part holding at least one
/// `.html` page. Never what a link or `..` leads to outside `root`, nor a name that starts with a
/// dot, which a view never serves. The part with the newest design first. Blocks on the disk.
pub fn list(root: &Path, owners: &[Owner]) -> Vec<Part> {
    let mut parts: Vec<Part> = inside(root, root)
        .into_iter()
        .filter(|(_, path)| path.is_dir())
        .filter_map(|(folder, path)| {
            let mut designs: Vec<Design> = inside(root, &path)
                .into_iter()
                .filter_map(|(name, path)| design(root, name, path))
                .collect();
            designs.sort_by_key(|d| std::cmp::Reverse(d.modified));
            let owner = owners
                .iter()
                .find(|(f, ..)| f.to_lowercase() == folder.to_lowercase());
            (!designs.is_empty()).then(|| Part {
                project: owner.map(|(_, p, _)| p.clone()),
                chat: owner.and_then(|(.., c)| c.clone()),
                folder,
                designs,
            })
        })
        .collect();
    parts.sort_by_key(|p| std::cmp::Reverse(p.designs[0].modified));
    parts
}

/// The folder at `path` as a design: its pages, when it has one.
fn design(root: &Path, name: String, path: PathBuf) -> Option<Design> {
    let mut pages: Vec<(String, PathBuf)> = inside(root, &path)
        .into_iter()
        .filter(|(n, p)| {
            let n = n.to_lowercase();
            (n.ends_with(".html") || n.ends_with(".htm")) && p.is_file()
        })
        .collect();
    pages.sort_by_cached_key(|(n, _)| n.to_lowercase());
    let modified = pages
        .iter()
        .filter_map(|(_, p)| std::fs::metadata(p).ok()?.modified().ok())
        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .max()?;
    Some(Design {
        name,
        path,
        pages: pages.into_iter().map(|(_, p)| p).collect(),
        modified,
    })
}

/// What is in the folder `dir`, by name and path, that is still in `root` once links are followed,
/// and whose name doesn't start with a dot.
fn inside(root: &Path, dir: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    read.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_str()?.to_string();
            let real = folders::real(&std::fs::canonicalize(e.path()).ok()?);
            (!name.starts_with('.') && real.starts_with(root)).then(|| (name, e.path()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::Views;

    #[test]
    fn the_gallery_lists_and_views_only_whats_inside_designs() {
        let base = std::env::temp_dir().join(format!("gathr-gallery-{}", crate::ids::span_id()));
        let elsewhere = base.join("Elsewhere");
        std::fs::create_dir_all(&base).unwrap();
        let conn = crate::db::open(&base.join("gathr.db")).unwrap();
        let sql = format!(
            "INSERT INTO settings (key, value) VALUES ('{}', '{}');
             INSERT INTO projects (id, name, name_key, folder, created_ts, folder_name) VALUES
                 ('p1', 'Shop', 'shop', '{}', 0, 'Shop');
             INSERT INTO chats (id, title, created_ts, project_id, archived_ts) VALUES
                 ('c1', 'Export', 1, 'p1', NULL), ('c2', 'Stock', 2, 'p1', 3);",
            folders::SETTING,
            base.join("agents").display(),
            base.join("shop").display(),
        );
        conn.execute_batch(&sql).unwrap();
        let root = folder(&conn, &base).unwrap();
        let write = |path: PathBuf| {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "<p>made up</p>").unwrap();
        };
        let shop = root.join("Shop");
        write(shop.join("export").join("b.html"));
        write(shop.join("export").join("A.html"));
        write(shop.join("export").join("notes.md"));
        write(shop.join("notes").join("plan.md"));
        write(shop.join(".drafts").join("a.html"));
        write(root.join("Chats").join("poster").join("a.html"));
        write(root.join("Gone").join("old").join("a.html"));
        write(elsewhere.join("secret").join("a.html"));
        let link = |at: PathBuf| {
            #[cfg(windows)]
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&at)
                .arg(&elsewhere)
                .output()
                .unwrap();
            #[cfg(windows)]
            assert!(made.status.success(), "mklink: {made:?}");
            #[cfg(unix)]
            std::os::unix::fs::symlink(&elsewhere, &at).unwrap();
        };
        link(shop.join("out"));
        link(root.join("Away"));

        let parts = list(&root, &owners(&conn).unwrap());
        let mut seen: Vec<(&str, Vec<&str>)> = parts
            .iter()
            .map(|p| {
                let names = p.designs.iter().map(|d| d.name.as_str()).collect();
                (p.folder.as_str(), names)
            })
            .collect();
        seen.sort();
        assert_eq!(
            seen,
            [
                ("Chats", vec!["poster"]),
                ("Gone", vec!["old"]),
                ("Shop", vec!["export"])
            ],
            "never a link out, a dot-folder or a folder with no page"
        );
        let shop_part = parts.iter().find(|p| p.folder == "Shop").unwrap();
        assert_eq!(shop_part.project.as_ref().unwrap().id, "p1");
        assert_eq!(shop_part.chat.as_deref(), Some("c1"), "not an archived one");
        let export = &shop_part.designs[0];
        assert_eq!(export.path, shop.join("export"));
        assert_eq!(
            export.pages,
            [
                shop.join("export").join("A.html"),
                shop.join("export").join("b.html")
            ],
            "pages only, by name"
        );
        let chats = parts.iter().find(|p| p.folder == "Chats").unwrap();
        assert!(chats.project.is_none() && chats.chat.is_none());
        let gone = parts.iter().find(|p| p.folder == "Gone").unwrap();
        assert!(gone.project.is_none(), "no project has that folder");

        let views = Views::new();
        let roots = [root.clone()];
        let at = |p: PathBuf| p.to_string_lossy().into_owned();
        let page = at(shop.join("export").join("A.html"));
        let (key, name) = views.reopen(&roots, &page).unwrap();
        assert_eq!(name, "A.html");
        let beside = at(shop.join("export").join("b.html"));
        assert_eq!(
            views.reopen(&roots, &beside).unwrap().0,
            key,
            "one key a folder"
        );
        assert!(views.file(&key, "b.html").is_some());
        let secret = elsewhere.join("secret").join("a.html");
        assert!(
            views.reopen(&roots, &at(secret)).is_none(),
            "a full path elsewhere"
        );
        let up = shop
            .join("..")
            .join("..")
            .join("..")
            .join("Elsewhere")
            .join("secret")
            .join("a.html");
        assert!(views.reopen(&roots, &at(up)).is_none(), "'..'");
        let through = shop.join("out").join("secret").join("a.html");
        assert!(views.reopen(&roots, &at(through)).is_none(), "a link out");
        drop(conn);
        let _ = std::fs::remove_dir_all(&base);
    }
}
