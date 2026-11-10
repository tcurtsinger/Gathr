//! The agent folder (0034): one folder, set in Settings > General, that holds every project's work:
//! `<root>/<Project>/Shared` for the team's kept files and `<root>/<Project>/<Agent>` for each
//! agent's worktrees and its engines' temporary files. Agents write there without asking; Gathr's
//! data folder (logins, the database, engine copies) is never one of them. Gathr records each
//! folder it makes before making it, and only ever removes what it recorded.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// The setting's key, and its name in `/api/settings`.
pub const SETTING: &str = "agent_folder";
/// Stands in for the default in dev and tests only, so a check never writes in the person's own.
pub const DEFAULT_VAR: &str = "GATHR_AGENT_FOLDER";
/// Where a chat with no project keeps its work, as a project would.
const NO_PROJECT: &str = "Chats";
const SHARED: &str = "Shared";
/// Longer names make deep worktree paths, which Windows tools still trip on past 260 characters.
const LONGEST: usize = 40;

/// The agent folder: as set, else the home folder's `Gathr`.
pub fn root(conn: &Connection) -> Result<PathBuf, Error> {
    let saved: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [SETTING],
            |r| r.get(0),
        )
        .optional()?;
    Ok(saved.map_or_else(default_root, PathBuf::from))
}

fn default_root() -> PathBuf {
    match std::env::var_os(DEFAULT_VAR).filter(|v| !v.is_empty()) {
        Some(set) => PathBuf::from(set),
        None => std::env::home_dir().unwrap_or_default().join("Gathr"),
    }
}

/// The default for a data folder used for a check (`gathr dev`): beside it, as `<name>-agents`.
pub fn beside(data: &Path) -> PathBuf {
    let name = data.file_name().unwrap_or_default().to_string_lossy();
    data.with_file_name(format!("{name}-agents"))
}

/// Why `root` can't be the agent folder, or `Ok` once it's made and written to. Blocks on the disk.
pub fn usable(root: &Path, data: &Path) -> Result<(), String> {
    if root.as_os_str().is_empty() {
        return Err("Enter a full path".into());
    }
    if !root.is_absolute() {
        return Err(format!("{} isn't a full path", root.display()));
    }
    if crate::approvals::overlaps(root, data) {
        let why = "Gathr's data folder can't hold it or be in it. Choose another folder";
        return Err(format!("{}: {why}", root.display()));
    }
    std::fs::create_dir_all(root).map_err(|e| format!("{} can't be made: {e}", root.display()))?;
    let probe = root.join(format!(".gathr-{}", crate::ids::span_id()));
    std::fs::write(&probe, b"").map_err(|e| format!("{} can't be written: {e}", root.display()))?;
    let _ = std::fs::remove_file(&probe);
    Ok(())
}

pub fn set_root(conn: &Connection, root: &Path) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![SETTING, root.to_string_lossy()],
    )?;
    Ok(())
}

/// `name` as one folder's name on Windows and macOS: no character either refuses or reads as a
/// path, no device name Windows keeps (`CON`, `COM1`), no leading dot (hidden on macOS) or trailing
/// dot or space (Windows drops them), at most 40 characters; `fallback` when nothing is left.
pub fn safe_name(name: &str, fallback: &str) -> String {
    let swapped = name
        .chars()
        .map(|c| match c.is_control() || r#"<>:"/\|?*"#.contains(c) {
            true => '-',
            false => c,
        });
    let cut: String = swapped.take(LONGEST).collect();
    let name = cut.trim_matches(|c: char| c == '.' || c.is_whitespace());
    if name.is_empty() {
        return fallback.to_string();
    }
    let stem = name.split('.').next().unwrap_or_default();
    let device = matches!(
        stem.to_uppercase().as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || ["COM", "LPT"].iter().any(|kind| {
        let rest = stem.to_uppercase();
        let digit = rest.strip_prefix(kind).unwrap_or_default();
        digit.chars().count() == 1 && "0123456789¹²³".contains(digit)
    });
    match device {
        true => format!("{stem}_{}", &name[stem.len()..]),
        false => name.to_string(),
    }
}

/// `base`, else `base 2`, `base 3`… the first that isn't `taken` (which compares ignoring case).
fn unique(base: &str, taken: impl Fn(&str) -> bool) -> String {
    let named = |n: u32| match n {
        1 => base.to_string(),
        n => format!("{base} {n}"),
    };
    (1..)
        .map(named)
        .find(|c| !taken(c))
        .expect("a number is free")
}

/// Whether `name` is one of `names`, ignoring case: both systems' disks ignore it by default.
fn among(names: &[String], name: &str) -> bool {
    names
        .iter()
        .any(|n| n.to_lowercase() == name.to_lowercase())
}

/// An agent's folders for a chat: its own, and its project's Shared.
#[derive(Debug, Clone)]
pub struct Place {
    pub agent: PathBuf,
    pub shared: PathBuf,
}

/// Where `agent` works in `chat`, naming the project and agent in the agent folder the first time.
/// A chat with no project keeps its work in `Chats`, as a project would.
pub fn place(conn: &Connection, chat: &str, agent: &str) -> Result<Place, Error> {
    let root = root(conn)?;
    let project: Option<String> = conn
        .query_row("SELECT project_id FROM chats WHERE id = ?1", [chat], |r| {
            r.get(0)
        })
        .optional()?
        .flatten();
    let project = match project {
        Some(id) => named(conn, "projects", &id, &root, &[NO_PROJECT])?,
        None => NO_PROJECT.to_string(),
    };
    let project = root.join(project);
    let agent = named(conn, "agents", agent, &project, &[SHARED])?;
    Ok(Place {
        agent: project.join(agent),
        shared: project.join(SHARED),
    })
}

/// The folder name of the project or agent `id` in `table`, chosen once from its name: unique
/// among the table's, never one of `kept`, and not a folder already in `under`.
fn named(
    conn: &Connection,
    table: &str,
    id: &str,
    under: &Path,
    kept: &[&str],
) -> Result<String, Error> {
    let sql = format!("SELECT name, folder_name FROM {table} WHERE id = ?1");
    let found: Option<(String, Option<String>)> = conn
        .query_row(&sql, [id], |r| Ok((r.get(0)?, r.get(1)?)))
        .optional()?;
    let (name, folder) =
        found.ok_or_else(|| Error::Other(format!("there's no {id} in {table}")))?;
    if let Some(folder) = folder {
        return Ok(folder);
    }
    let sql = format!("SELECT folder_name FROM {table} WHERE folder_name IS NOT NULL");
    let mut stmt = conn.prepare(&sql)?;
    let mut names: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    names.extend(kept.iter().map(|k| k.to_string()));
    let fallback = if table == "projects" {
        "Project"
    } else {
        "Agent"
    };
    let base = safe_name(&name, fallback);
    let chosen = unique(&base, |c| among(&names, c) || under.join(c).exists());
    let sql = format!("UPDATE {table} SET folder_name = ?2 WHERE id = ?1");
    conn.execute(&sql, params![id, chosen])?;
    Ok(chosen)
}

/// The short part of a chat's id that names its folders (and its branches, 0028): `x4k2pq`.
pub fn chat_part(chat: &str) -> String {
    chat[chat.len().saturating_sub(6)..].to_lowercase()
}

/// A new folder for `agent`'s work in a chat, in its own folder: named by the chat, unique there.
pub fn new_workspace(place: &Place, chat: &str) -> PathBuf {
    let name = unique(&chat_part(chat), |c| place.agent.join(c).exists());
    place.agent.join(name)
}

/// Where the agent's engine in a chat keeps its temporary files (TEMP, TMP, TMPDIR), from its
/// start: they go when it closes.
pub fn scratch(place: &Place, chat: &str) -> PathBuf {
    place
        .agent
        .join("temp")
        .join(format!("chat-{}", chat_part(chat)))
}

/// A folder Gathr is about to make for an agent's work.
pub struct Made<'a> {
    pub path: &'a Path,
    pub kind: &'a str,
    pub chat: &'a str,
    pub agent: Option<&'a str>,
    pub project: Option<&'a str>,
}

/// Records a folder before it's made: one made again (its chat restored, its engine started
/// again) is open again, its last removal forgotten.
pub fn made(conn: &Connection, m: &Made) -> Result<(), Error> {
    conn.execute(
        "INSERT INTO agent_folders (path, kind, chat_id, agent_id, project, made_ts)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (path) DO UPDATE SET made_ts = excluded.made_ts, due_ts = NULL, trace = NULL,
           skipped = NULL, removed_ts = NULL",
        params![
            m.path.to_string_lossy(),
            m.kind,
            m.chat,
            m.agent,
            m.project,
            crate::ids::now_us()
        ],
    )?;
    Ok(())
}

/// The folder `agent` works in for `chat`, as recorded when it was made, removed since or not.
pub fn workspace(conn: &Connection, chat: &str, agent: &str) -> Result<Option<PathBuf>, Error> {
    let path: Option<String> = conn
        .query_row(
            "SELECT path FROM agent_folders WHERE chat_id = ?1 AND agent_id = ?2
             AND kind IN ('worktree', 'folder') ORDER BY made_ts DESC LIMIT 1",
            params![chat, agent],
            |r| r.get(0),
        )
        .optional()?;
    Ok(path.map(PathBuf::from))
}

/// The temporary files of `agent`'s engine in `chat` go: the engine has closed. How many.
pub fn engine_closed(conn: &Connection, chat: &str, agent: &str) -> Result<usize, Error> {
    Ok(conn.execute(
        "UPDATE agent_folders SET due_ts = ?3
         WHERE chat_id = ?1 AND agent_id = ?2 AND kind = 'scratch' AND removed_ts IS NULL
           AND due_ts IS NULL",
        params![chat, agent, crate::ids::now_us()],
    )?)
}

/// The chat's folders go: it was archived or deleted. How many.
pub fn due(conn: &Connection, chat: &str) -> Result<usize, Error> {
    Ok(conn.execute(
        "UPDATE agent_folders SET due_ts = ?2
         WHERE chat_id = ?1 AND removed_ts IS NULL AND due_ts IS NULL",
        params![chat, crate::ids::now_us()],
    )?)
}

/// The chat is back: what's still there stays.
pub fn kept(conn: &Connection, chat: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE agent_folders SET due_ts = NULL, trace = NULL, skipped = NULL
         WHERE chat_id = ?1 AND removed_ts IS NULL",
        [chat],
    )?;
    Ok(())
}

/// A folder to remove now, with the trace its skips and removal share.
#[derive(Debug, Clone)]
pub struct Clean {
    pub path: PathBuf,
    pub kind: String,
    pub chat: String,
    pub agent: Option<String>,
    pub project: Option<String>,
    pub trace: String,
    /// Why its last recorded skip was.
    pub skipped: Option<String>,
    /// The chat is archived, not deleted: a worktree with changes not committed stays.
    pub chat_kept: bool,
}

/// What goes now (of `chat`'s, or everyone's): the folders of chats archived or deleted, and the
/// temporary files of engines that have closed.
pub fn to_clean(conn: &Connection, chat: Option<&str>) -> Result<Vec<Clean>, Error> {
    let mut stmt = conn.prepare(
        "SELECT f.path, f.kind, f.chat_id, f.agent_id, f.project, f.trace, f.skipped,
           EXISTS (SELECT 1 FROM chats c WHERE c.id = f.chat_id)
         FROM agent_folders f
         WHERE f.removed_ts IS NULL AND f.due_ts IS NOT NULL AND (?1 IS NULL OR f.chat_id = ?1)
         ORDER BY f.made_ts",
    )?;
    let rows = stmt.query_map([chat], |r| {
        Ok(Clean {
            path: PathBuf::from(r.get::<_, String>(0)?),
            kind: r.get(1)?,
            chat: r.get(2)?,
            agent: r.get(3)?,
            project: r.get(4)?,
            trace: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
            skipped: r.get(6)?,
            chat_kept: r.get(7)?,
        })
    })?;
    let mut found: Vec<Clean> = rows.collect::<Result<_, _>>()?;
    for clean in found.iter_mut().filter(|c| c.trace.is_empty()) {
        clean.trace = crate::ids::trace_id();
        conn.execute(
            "UPDATE agent_folders SET trace = ?2 WHERE path = ?1",
            params![clean.path.to_string_lossy(), clean.trace],
        )?;
    }
    Ok(found)
}

/// Its removal was skipped for `why` (recorded once each), and is tried again at the next sweep.
pub fn skipped(conn: &Connection, path: &Path, why: &str) -> Result<(), Error> {
    conn.execute(
        "UPDATE agent_folders SET skipped = ?2 WHERE path = ?1",
        params![path.to_string_lossy(), why],
    )?;
    Ok(())
}

/// It's gone: kept as a record while its chat is, forgotten with a deleted chat.
pub fn removed(conn: &Connection, path: &Path, chat_kept: bool) -> Result<(), Error> {
    let path = path.to_string_lossy();
    match chat_kept {
        true => conn.execute(
            "UPDATE agent_folders SET removed_ts = ?2 WHERE path = ?1",
            params![path, crate::ids::now_us()],
        )?,
        false => conn.execute("DELETE FROM agent_folders WHERE path = ?1", [path])?,
    };
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_becomes_one_folder_that_windows_and_macos_both_take() {
        assert_eq!(safe_name("Orin", "Agent"), "Orin");
        assert_eq!(safe_name("Pia: UI/UX?", "Agent"), "Pia- UI-UX-");
        assert_eq!(safe_name(r#"a<b>c"d\e|f*g"#, "Agent"), "a-b-c-d-e-f-g");
        assert_eq!(safe_name("CON", "Agent"), "CON_", "a device name");
        assert_eq!(
            safe_name("nul.txt", "Agent"),
            "nul_.txt",
            "a device name with a dot"
        );
        assert_eq!(safe_name("com1", "Agent"), "com1_");
        assert_eq!(
            safe_name("Console", "Agent"),
            "Console",
            "only the whole stem counts"
        );
        assert_eq!(safe_name("COM10", "Agent"), "COM10");
        assert_eq!(
            safe_name(" .hidden. ", "Agent"),
            "hidden",
            "a leading dot hides it on macOS"
        );
        assert_eq!(
            safe_name("Release v2.", "Agent"),
            "Release v2",
            "Windows drops a trailing dot"
        );
        assert_eq!(safe_name("...", "Project"), "Project");
        assert_eq!(safe_name("tab\there", "Agent"), "tab-here");
        assert_eq!(safe_name(&"é".repeat(50), "Agent").chars().count(), 40);
    }

    #[test]
    fn a_taken_name_gets_the_next_free_number_ignoring_case() {
        let names = vec!["Orin".to_string(), "orin 2".to_string()];
        assert_eq!(unique("ORIN", |c| among(&names, c)), "ORIN 3");
        assert_eq!(unique("Pia", |c| among(&names, c)), "Pia");
    }
}
