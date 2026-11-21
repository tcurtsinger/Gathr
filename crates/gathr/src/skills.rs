//! Skills, in the open Agent Skills format: each a folder in `<agent folder>/Skills` holding a
//! SKILL.md whose front matter gives its `name` and `description`. The folder is the list: a skill
//! put there by hand counts too. Gathr keeps no copy and no table, only the names the person
//! disabled, in one setting. Every agent reads them where they sit.

use std::io::{Cursor, Read};
use std::path::{Component, Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::{Error, folders};

pub const FOLDER: &str = "Skills";
/// The setting that holds the disabled skills' names, as a JSON list.
pub const SETTING: &str = "skills_off";
const FILE: &str = "SKILL.md";
/// The format's longest name.
const LONGEST: usize = 64;
/// A zip's unpacked size at most: a small zip can unpack to far more than it weighs.
const MOST_UNPACKED: u64 = 50 * 1024 * 1024;
/// A zip's files at most, however small each is.
const MOST_FILES: usize = 2_000;

#[derive(Debug, Serialize)]
pub struct Skill {
    /// Its folder's name.
    pub name: String,
    pub description: Option<String>,
    pub path: PathBuf,
    pub enabled: bool,
    /// Why it can't be read, in the person's words.
    pub problem: Option<String>,
}

/// The Skills folder, made or not.
pub fn folder(conn: &Connection, data: &Path) -> Result<PathBuf, Error> {
    Ok(folders::real(&folders::root(conn, data)?).join(FOLDER))
}

/// Every folder in `dir` as a skill, by name; none when `dir` isn't there.
pub fn list(dir: &Path, off: &[String]) -> Vec<Skill> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut skills: Vec<Skill> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        // A hidden folder, such as a .git kept there, is no skill.
        .filter(|name| !name.starts_with('.'))
        .map(|name| {
            let path = dir.join(&name);
            let (description, problem) = match described(&path) {
                Ok(description) => (Some(description), None),
                Err(why) => (None, Some(why)),
            };
            Skill {
                enabled: !off.contains(&name),
                name,
                description,
                path,
                problem,
            }
        })
        .collect();
    skills.sort_by_key(|s| s.name.to_lowercase());
    skills
}

/// The skill's description, or why it can't be read.
fn described(skill: &Path) -> Result<String, String> {
    let text = match std::fs::read_to_string(skill.join(FILE)) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err("It has no SKILL.md".into());
        }
        Err(e) => return Err(format!("SKILL.md can't be read: {e}")),
    };
    let (fields, _) = front(&text).ok_or("SKILL.md has no front matter")?;
    match field(&fields, "description") {
        Some(d) if !d.is_empty() => Ok(d.to_string()),
        _ => Err("SKILL.md has no description".into()),
    }
}

/// A SKILL.md's text after its front matter, or all of it with none; none without a SKILL.md.
pub fn text(skill: &Path) -> Option<String> {
    let text = std::fs::read_to_string(skill.join(FILE)).ok()?;
    Some(match front(&text) {
        Some((_, body)) => body.trim_start().to_string(),
        None => text,
    })
}

/// The front matter's `key: value` lines and the text after it. A value may be quoted, or carried
/// on the indented lines below (`description: >`); nothing else of YAML is read.
fn front(text: &str) -> Option<(Vec<(String, String)>, &str)> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut lines = text.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end() != "---" {
        return None;
    }
    let mut at = first.len();
    let mut fields: Vec<(String, String)> = Vec::new();
    for line in lines {
        at += line.len();
        let bare = line.trim_end();
        if bare == "---" {
            return Some((fields, &text[at..]));
        }
        if line.starts_with([' ', '\t']) {
            if let Some((_, value)) = fields.last_mut() {
                if !value.is_empty() {
                    value.push(' ');
                }
                value.push_str(bare.trim());
            }
            continue;
        }
        if let Some((key, value)) = bare.split_once(':') {
            let value = match value.trim() {
                ">" | "|" | ">-" | "|-" | ">+" | "|+" => String::new(),
                value => unquote(value),
            };
            fields.push((key.trim().to_string(), value));
        }
    }
    None
}

fn field<'a>(fields: &'a [(String, String)], key: &str) -> Option<&'a str> {
    fields.iter().find(|(k, _)| k == key).map(|(_, v)| v.trim())
}

fn unquote(value: &str) -> String {
    if let Some(inner) = value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        return inner.replace("''", "'");
    }
    let Some(inner) = value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) else {
        return value.to_string();
    };
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            c => out.push(c),
        }
    }
    out
}

/// The names the person disabled.
pub fn off(conn: &Connection) -> Result<Vec<String>, Error> {
    let saved: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING],
            |r| r.get(0),
        )
        .optional()?;
    Ok(saved
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default())
}

/// Enables or disables `name` for every agent; a removed skill is forgotten the same way.
pub fn set_enabled(conn: &Connection, name: &str, on: bool) -> Result<(), Error> {
    plain(name)?;
    let mut names = off(conn)?;
    names.retain(|n| n != name);
    if !on {
        names.push(name.to_string());
    }
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![SETTING, serde_json::to_string(&names)?],
    )?;
    Ok(())
}

/// Refused unless `name` is one plain folder's name: no separator, no `..`, no drive.
fn plain(name: &str) -> Result<(), Error> {
    let mut parts = Path::new(name).components();
    let one = matches!(
        (parts.next(), parts.next()),
        (Some(Component::Normal(_)), None)
    );
    match one && !name.contains(['/', '\\', ':']) && !name.starts_with('.') {
        true => Ok(()),
        false => Err(Error::Refused(format!("\"{name}\" isn't a skill's name"))),
    }
}

/// Refused unless `name` follows the format: lowercase letters, digits and single hyphens inside.
fn new_name(name: &str) -> Result<(), Error> {
    let allowed = name
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    let fits = !name.is_empty() && name.len() <= LONGEST;
    if allowed && fits && !name.starts_with('-') && !name.ends_with('-') && !name.contains("--") {
        return Ok(());
    }
    Err(Error::Refused(format!(
        "\"{name}\" can't be a skill's name: use lowercase letters, digits and hyphens, {LONGEST} at most"
    )))
}

/// The new skill's folder, made, or refused when the name is wrong or taken.
fn make(dir: &Path, name: &str) -> Result<PathBuf, Error> {
    new_name(name)?;
    let path = dir.join(name);
    if path.exists() {
        return Err(Error::Refused(format!(
            "There's already a skill named {name}"
        )));
    }
    std::fs::create_dir_all(&path)?;
    Ok(path)
}

/// Writes a new skill: its folder and SKILL.md.
pub fn create(dir: &Path, name: &str, description: &str, text: &str) -> Result<(), Error> {
    let path = make(dir, name)?;
    let description = description.split_whitespace().collect::<Vec<_>>().join(" ");
    let quoted = description.replace('\\', "\\\\").replace('"', "\\\"");
    let file = format!(
        "---\nname: {name}\ndescription: \"{quoted}\"\n---\n\n{}\n",
        text.trim()
    );
    std::fs::write(path.join(FILE), file)?;
    Ok(())
}

/// Writes one file of a skill sent a file at a time, at `inner`, its path inside the skill. The
/// first (`new`) makes the skill's folder; the rest go into it.
pub fn add_file(
    dir: &Path,
    skill: &str,
    inner: &str,
    bytes: &[u8],
    new: bool,
) -> Result<(), Error> {
    plain(skill)?;
    let mut path = match new {
        true => make(dir, skill)?,
        false => dir.join(skill),
    };
    if !path.is_dir() {
        return Err(Error::Refused(format!("There's no skill {skill}")));
    }
    let parts: Vec<&str> = inner.split(['/', '\\']).collect();
    for part in &parts {
        if plain(part).is_err() {
            return Err(Error::Refused(format!(
                "\"{inner}\" isn't a path inside a skill"
            )));
        }
        path.push(part);
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, bytes)?;
    Ok(())
}

/// A single Markdown file as a skill of its own, its SKILL.md: named by its front matter, else by
/// the file's name. The name it took.
pub fn add_single(dir: &Path, file: &str, bytes: &[u8]) -> Result<String, Error> {
    let name = named(file, bytes);
    let path = make(dir, &name)?;
    std::fs::write(path.join(FILE), bytes)?;
    Ok(name)
}

/// A SKILL.md's name: its front matter's, else the file's name without its extension.
fn named(file: &str, skill_md: &[u8]) -> String {
    let text = String::from_utf8_lossy(skill_md);
    let named = front(&text).and_then(|(fields, _)| field(&fields, "name").map(String::from));
    match named.filter(|n| !n.is_empty()) {
        Some(name) => name,
        None => {
            let last = file.rsplit(['/', '\\']).next().unwrap_or(file);
            let stem = Path::new(last).file_stem().unwrap_or_default();
            let stem = stem.to_string_lossy().to_lowercase();
            stem.replace([' ', '_'], "-")
        }
    }
}

/// A zipped skill (a Claude `.skill` file is one) unpacked as a new skill: its one top folder is the
/// skill and names it, else a SKILL.md at the top is, named as `add_single` names one. Every entry is
/// checked before anything is written, and a refusal or a failed write leaves no folder. The name it took.
pub fn add_zip(dir: &Path, file: &str, bytes: &[u8]) -> Result<String, Error> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(unreadable)?;
    // Each file's index and its path's parts.
    let mut kept: Vec<(usize, Vec<String>)> = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(unreadable)?;
        let name = entry.name().map_err(unreadable)?.into_owned();
        if entry.encrypted() {
            return Err(Error::Refused("This zip is encrypted".into()));
        }
        if entry.is_symlink() {
            return Err(Error::Refused(format!("This zip holds a link: {name}")));
        }
        let parts: Vec<&str> = name
            .trim_end_matches(['/', '\\'])
            .split(['/', '\\'])
            .collect();
        // Left out, as a folder upload leaves them: a Mac's __MACOSX and hidden files.
        let hidden =
            |p: &&str| (p.starts_with('.') && !matches!(*p, "." | "..")) || *p == "__MACOSX";
        let outside = entry.enclosed_name().is_none()
            || parts.iter().any(|p| !hidden(p) && plain(p).is_err());
        if outside {
            return Err(Error::Refused(format!(
                "This zip can't be unpacked: {name} would land outside the skill"
            )));
        }
        if !entry.is_dir() && !parts.iter().any(hidden) {
            kept.push((i, parts.into_iter().map(String::from).collect()));
        }
    }
    if kept.len() > MOST_FILES {
        return Err(Error::Refused(format!(
            "This zip has more than {MOST_FILES} files"
        )));
    }
    let top = match kept.first() {
        Some((_, parts)) if parts.len() > 1 => Some(parts[0].clone()),
        _ => None,
    };
    let top = top.filter(|t| kept.iter().all(|(_, p)| p.len() > 1 && p[0] == *t));
    if top.is_some() {
        for (_, parts) in &mut kept {
            parts.remove(0);
        }
    }
    let Some(&(at, _)) = kept.iter().find(|(_, p)| p.len() == 1 && p[0] == FILE) else {
        return Err(Error::Refused(
            "This zip isn't a skill: it has no SKILL.md".into(),
        ));
    };
    let name = match top {
        Some(top) => top,
        None => {
            let mut skill_md = Vec::new();
            let entry = zip.by_index(at).map_err(unreadable)?;
            entry.take(MOST_UNPACKED).read_to_end(&mut skill_md)?;
            named(file, &skill_md)
        }
    };
    let path = make(dir, &name)?;
    let unpacked = unpack(&mut zip, &kept, &path);
    if unpacked.is_err() {
        let _ = std::fs::remove_dir_all(&path);
    }
    unpacked.map(|()| name)
}

/// Writes the kept files into the new skill's folder, counting what's written against the cap.
fn unpack(
    zip: &mut ZipArchive<Cursor<&[u8]>>,
    kept: &[(usize, Vec<String>)],
    skill: &Path,
) -> Result<(), Error> {
    let mut left = MOST_UNPACKED;
    for (i, parts) in kept {
        let path = parts
            .iter()
            .fold(skill.to_path_buf(), |p, part| p.join(part));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let entry = zip.by_index(*i).map_err(unreadable)?;
        let wrote = std::io::copy(
            &mut entry.take(left + 1),
            &mut std::fs::File::create(&path)?,
        )?;
        if wrote > left {
            return Err(Error::Refused(format!(
                "This zip unpacks to more than {} MB",
                MOST_UNPACKED >> 20
            )));
        }
        left -= wrote;
    }
    Ok(())
}

fn unreadable(e: ZipError) -> Error {
    Error::Refused(format!("This zip can't be read: {e}"))
}

/// Deletes the skill's folder and all it holds.
pub fn remove(dir: &Path, name: &str) -> Result<(), Error> {
    std::fs::remove_dir_all(path(dir, name)?)?;
    Ok(())
}

/// The skill's folder: refused when there's none.
pub fn path(dir: &Path, name: &str) -> Result<PathBuf, Error> {
    plain(name)?;
    let path = dir.join(name);
    match path.is_dir() {
        true => Ok(path),
        false => Err(Error::Refused(format!("There's no skill {name}"))),
    }
}
