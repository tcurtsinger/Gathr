//! Memory: short notes agents keep across chats, in plain files in `<agent folder>/Memory`. The folder
//! is the list, as with Skills: `You.md` about the person, `Projects/<project>.md` for whoever works in
//! a project, `Agents/<agent>.md` an agent's own notes about doing its work. Each file is named by the
//! folder name Gathr chose once for its project or agent, so a rename leaves it where it is.
//!
//! A note is a line that starts with `- `, maybe ending in who added it and when, as an HTML comment:
//! `- Prefers one pull request per ask <!-- Ann · 2026-11-21T14:02:00Z -->`. Every other line is the
//! person's and is kept byte for byte: Gathr only adds a line at the top of the notes, replaces one
//! or removes one, and writes the file whole in one rename.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::{Error, folders};

pub const FOLDER: &str = "Memory";
pub const PROJECTS: &str = "Projects";
pub const AGENTS: &str = "Agents";
const YOU: &str = "You.md";
/// The longest note, in characters.
pub const LONGEST: usize = 200;
/// Who a note the person adds in the window is by.
pub const PERSON: &str = "You";

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Note {
    pub text: String,
    /// Who added it: an agent's name, `You`, or nobody for a line written in the file by hand.
    pub by: Option<String>,
    pub when: Option<String>,
}

/// Where a note is kept: about the person, a project's, or an agent's own.
#[derive(Debug, Clone, PartialEq)]
pub enum Place {
    You,
    Project(String),
    Agent(String),
}

impl Place {
    /// `you`, `project:<id>` or `agent:<id>`.
    pub fn parse(id: &str) -> Result<Place, Error> {
        match id.split_once(':') {
            None if id == "you" => Ok(Place::You),
            Some(("project", p)) if !p.is_empty() => Ok(Place::Project(p.to_string())),
            Some(("agent", a)) if !a.is_empty() => Ok(Place::Agent(a.to_string())),
            _ => Err(Error::Refused(format!("\"{id}\" isn't a place in Memory"))),
        }
    }

    pub fn id(&self) -> String {
        match self {
            Place::You => "you".to_string(),
            Place::Project(id) => format!("project:{id}"),
            Place::Agent(id) => format!("agent:{id}"),
        }
    }
}

/// The agent folder, as the disk names it: Memory is its `Memory`.
pub fn root(conn: &Connection, data: &Path) -> Result<PathBuf, Error> {
    Ok(folders::real(&folders::root(conn, data)?))
}

/// The place's file under the agent folder `root`; refused for a project or agent that isn't there.
pub fn file(conn: &Connection, root: &Path, place: &Place) -> Result<PathBuf, Error> {
    let dir = root.join(FOLDER);
    let there = |table: &str, id: &str| -> Result<(), Error> {
        let sql = format!("SELECT 1 FROM {table} WHERE id = ?1");
        let found: Option<i64> = conn.query_row(&sql, [id], |r| r.get(0)).optional()?;
        found
            .map(|_| ())
            .ok_or_else(|| Error::Refused(format!("There's no {id}")))
    };
    Ok(match place {
        Place::You => dir.join(YOU),
        Place::Project(id) => {
            there("projects", id)?;
            let name = folders::project_name(conn, root, id)?;
            dir.join(PROJECTS).join(format!("{name}.md"))
        }
        Place::Agent(id) => {
            there("agents", id)?;
            let name = folders::agent_name(conn, root, id)?;
            dir.join(AGENTS).join(format!("{name}.md"))
        }
    })
}

/// What an agent starting in `chat` knows: about the person, the chat's project and its own work,
/// each place that has notes with its file and its notes.
pub fn known(
    conn: &Connection,
    root: &Path,
    chat: &str,
    agent: Option<&str>,
) -> Result<Vec<(Place, PathBuf, Vec<Note>)>, Error> {
    let project: Option<String> = conn
        .query_row("SELECT project_id FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let places = [
        Some(Place::You),
        project.map(Place::Project),
        agent.map(|a| Place::Agent(a.to_string())),
    ];
    let mut known = Vec::new();
    for place in places.into_iter().flatten() {
        let path = file(conn, root, &place)?;
        let notes = notes(&path)?;
        if !notes.is_empty() {
            known.push((place, path, notes));
        }
    }
    Ok(known)
}

/// The file's notes, in its order: newest first, as Gathr adds them at the top. None without a file.
pub fn notes(path: &Path) -> Result<Vec<Note>, Error> {
    Ok(read(path)?.split_inclusive('\n').filter_map(note).collect())
}

/// Adds `text` at the top of the file's notes, by `by` now. The note as kept.
pub fn add(path: &Path, text: &str, by: &str) -> Result<Note, Error> {
    let text = checked(text)?;
    let file = read(path)?;
    let mut lines: Vec<String> = file.split_inclusive('\n').map(String::from).collect();
    repeats(&lines, &text, None)?;
    let end = ending(&file);
    let made = line(&text, by);
    match lines.iter().position(|l| note(l).is_some()) {
        Some(at) => lines.insert(at, format!("{made}{end}")),
        None => {
            if let Some(last) = lines.last_mut().filter(|l| !l.ends_with('\n')) {
                last.push_str(end);
            }
            lines.push(format!("{made}{end}"));
        }
    }
    write(path, &lines.concat())?;
    Ok(note(&made).expect("a line Gathr wrote is a note"))
}

/// Puts `text`, by `by` now, in place of the note that reads `was`. The note as kept.
pub fn replace(path: &Path, was: &str, text: &str, by: &str) -> Result<Note, Error> {
    let text = checked(text)?;
    let file = read(path)?;
    let mut lines: Vec<String> = file.split_inclusive('\n').map(String::from).collect();
    let at = find(&lines, was)?;
    repeats(&lines, &text, Some(at))?;
    let made = line(&text, by);
    let end = &lines[at][body(&lines[at]).len()..];
    lines[at] = format!("{made}{end}");
    write(path, &lines.concat())?;
    Ok(note(&made).expect("a line Gathr wrote is a note"))
}

/// Takes out the note that reads `text`, its line and its line ending.
pub fn remove(path: &Path, text: &str) -> Result<(), Error> {
    let file = read(path)?;
    let mut lines: Vec<String> = file.split_inclusive('\n').map(String::from).collect();
    let at = find(&lines, text)?;
    lines.remove(at);
    write(path, &lines.concat())
}

/// The note's text as kept, or why it can't be one.
fn checked(text: &str) -> Result<String, Error> {
    let text = text.trim();
    if text.is_empty() {
        return Err(Error::Refused("A note can't be empty".into()));
    }
    if text.contains(['\n', '\r']) {
        return Err(Error::Refused("A note is one line".into()));
    }
    if text.chars().count() > LONGEST {
        return Err(Error::Refused(format!(
            "A note is {LONGEST} characters at most"
        )));
    }
    Ok(text.to_string())
}

fn repeats(lines: &[String], text: &str, but: Option<usize>) -> Result<(), Error> {
    let same = |(i, l): (usize, &String)| Some(i) != but && note(l).is_some_and(|n| n.text == text);
    match lines.iter().enumerate().any(same) {
        true => Err(Error::Refused("That note is already there".into())),
        false => Ok(()),
    }
}

fn find(lines: &[String], text: &str) -> Result<usize, Error> {
    let text = text.trim();
    let at = lines
        .iter()
        .position(|l| note(l).is_some_and(|n| n.text == text));
    at.ok_or_else(|| Error::Refused(format!("There's no note \"{text}\" here")))
}

/// A note's line, without its ending.
fn line(text: &str, by: &str) -> String {
    let now = crate::ids::utc(crate::ids::now_us());
    // To the second: `2026-11-21T14:02:00Z`.
    format!("- {text} <!-- {by} · {}Z -->", &now[..19])
}

/// The line as a note, when it is one.
fn note(line: &str) -> Option<Note> {
    let rest = body(line).strip_prefix("- ")?;
    let (text, by, when) = match (rest.rfind("<!--"), rest.trim_end().strip_suffix("-->")) {
        (Some(at), Some(head)) if at + 4 <= head.len() => {
            let said = head[at + 4..].trim();
            let (by, when) = match said.rsplit_once(" · ") {
                Some((by, when)) => (by.trim(), Some(when.trim().to_string())),
                None => (said, None),
            };
            let by = Some(by.to_string()).filter(|b| !b.is_empty());
            (&rest[..at], by, when)
        }
        _ => (rest, None, None),
    };
    let text = text.trim();
    (!text.is_empty()).then(|| Note {
        text: text.to_string(),
        by,
        when,
    })
}

/// A line without its ending.
fn body(line: &str) -> &str {
    let line = line.strip_suffix('\n').unwrap_or(line);
    line.strip_suffix('\r').unwrap_or(line)
}

/// The file's line ending: Windows' when it has any, else `\n`.
fn ending(file: &str) -> &'static str {
    match file.contains("\r\n") {
        true => "\r\n",
        false => "\n",
    }
}

fn read(path: &Path) -> Result<String, Error> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e.into()),
    }
}

/// Written beside it, then renamed over it: a crash leaves the old file or the new, never half.
fn write(path: &Path, text: &str) -> Result<(), Error> {
    let dir = path
        .parent()
        .ok_or_else(|| Error::Other("no folder".into()))?;
    std::fs::create_dir_all(dir)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = dir.join(format!(".{name}.{}.tmp", crate::ids::span_id()));
    std::fs::write(&temp, text)?;
    std::fs::rename(&temp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&temp);
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_keeps_every_byte_it_does_not_own() {
        let dir = std::env::temp_dir().join(format!("gathr-memory-{}", crate::ids::span_id()));
        let path = dir.join("You.md");
        std::fs::create_dir_all(&dir).unwrap();
        let mine = "# About me\r\n- British spelling\r\n\r\nSome words of my own\r\n";
        std::fs::write(&path, mine).unwrap();
        let hand = Note {
            text: "British spelling".into(),
            by: None,
            when: None,
        };
        assert_eq!(notes(&path).unwrap(), std::slice::from_ref(&hand));

        let added = add(&path, "  Prefers one pull request per ask ", "Ann").unwrap();
        assert_eq!(added.text, "Prefers one pull request per ask");
        assert_eq!(added.by.as_deref(), Some("Ann"));
        let kept = std::fs::read_to_string(&path).unwrap();
        let line = format!(
            "- Prefers one pull request per ask <!-- Ann · {} -->\r\n",
            added.when.as_deref().unwrap()
        );
        assert_eq!(kept, mine.replace("- British", &format!("{line}- British")));
        assert_eq!(notes(&path).unwrap(), [added.clone(), hand]);

        let changed = replace(&path, "British spelling", "UK spelling", "You").unwrap();
        let kept = std::fs::read_to_string(&path).unwrap();
        let by = format!("- UK spelling <!-- You · {} -->", changed.when.unwrap());
        assert_eq!(
            kept,
            format!("# About me\r\n{line}{by}\r\n\r\nSome words of my own\r\n")
        );

        remove(&path, "Prefers one pull request per ask").unwrap();
        let kept = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            kept,
            format!("# About me\r\n{by}\r\n\r\nSome words of my own\r\n")
        );

        for refused in ["UK spelling", &"x".repeat(201), "  ", "two\nlines"] {
            let answer = add(&path, refused, "Ann");
            assert!(matches!(answer, Err(Error::Refused(_))), "{refused}");
        }
        assert!(add(&path, &"x".repeat(200), "Ann").is_ok());
        assert!(matches!(remove(&path, "Not there"), Err(Error::Refused(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
