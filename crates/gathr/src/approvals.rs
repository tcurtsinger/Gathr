//! Gathr's own asks, on top of the agent's mode (0029): before a push, and before a file tool
//! writes outside the agent's folders: the one its engine runs in, its own folder and its
//! project's Shared (0034), or into Gathr's data folder whatever those say. Claude Code asks Gathr
//! through a hook before each such tool, whatever its mode or a project's rules would allow.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// Why Gathr asks the person before this tool call, or `None` to leave it to the agent's mode.
/// `folders` are where it writes without asking, the engine's own first: a relative path is in it.
/// `data` is Gathr's data folder, never written without asking, though a link in a folder leads
/// there: only the engine's own folder when it's one `from_before` the agent folder (0034).
pub fn ask(
    tool: &str,
    input: &Value,
    folders: &[PathBuf],
    data: &Path,
    from_before: bool,
) -> Option<&'static str> {
    let text = |key: &str| input.get(key).and_then(Value::as_str);
    match tool {
        "Write" | "Edit" | "NotebookEdit" => {
            let path = text("file_path").or_else(|| text("notebook_path"))?;
            let own = folders.first()?;
            let path = own.join(path);
            let old_own = from_before && inside(&path, own);
            if reaches(&path, data) && !old_own {
                return Some("it writes in Gathr's data folder");
            }
            let inside_one = folders.iter().any(|folder| inside(&path, folder));
            (!inside_one).then_some("it writes outside the agent's folders")
        }
        "Bash" | "PowerShell" => runs_push(tool, input).then_some("it pushes"),
        // Gathr runs a wait's check itself, outside Claude Code's own checks and auto mode's
        // classifier: asked here, the call waits for the person as long as they take.
        "mcp__gathr__wait" => text("check")
            .is_some_and(|c| !c.trim().is_empty())
            .then_some("Gathr runs this check until it exits 0"),
        _ => None,
    }
}

/// Whether the call is a shell command that reads as a push. The worktree's `pre-push` hook is
/// what catches every push; this asks earlier, before the command runs.
pub fn runs_push(tool: &str, input: &Value) -> bool {
    let command = input.get("command").and_then(Value::as_str);
    matches!(tool, "Bash" | "PowerShell") && command.is_some_and(pushes)
}

/// Whether either folder holds the other, either way the disk may resolve them.
pub fn overlaps(a: &Path, b: &Path) -> bool {
    reaches(a, b) || reaches(b, a)
}

/// Whether `path`, relative to `boundary` when it isn't absolute, is in `boundary` as the disk
/// resolves both: links (symlinks, junctions) followed. Windows works out `..` by the text
/// before following a link and macOS after, so the path must be inside both ways.
fn inside(path: &Path, boundary: &Path) -> bool {
    resolved(path, boundary).iter().all(|&inside| inside)
}

/// Whether `folder` must never be given to an agent: it holds the data folder or lands in it,
/// links followed. An engine's own folder `from_before` the agent folder is in the data folder by
/// design (0034): it's shut only when it holds the data folder.
pub fn shut(folder: &Path, data: &Path, from_before: bool) -> bool {
    match from_before {
        true => reaches(data, folder),
        false => overlaps(folder, data),
    }
}

/// Whether `path` may land in `boundary`, either way `..` is worked out.
fn reaches(path: &Path, boundary: &Path) -> bool {
    resolved(path, boundary).iter().any(|&inside| inside)
}

/// Whether `path` is in `boundary` with `..` worked out after following links and before. Case is
/// folded: both systems' disks ignore it by default.
fn resolved(path: &Path, boundary: &Path) -> [bool; 2] {
    let Some(base) = real(boundary).map(|b| fold(&b)) else {
        return [false; 2];
    };
    let path = boundary.join(path);
    [real(&path), real(&lexical(&path))].map(|p| p.is_some_and(|p| fold(&p).starts_with(&base)))
}

fn fold(path: &Path) -> PathBuf {
    PathBuf::from(path.to_string_lossy().to_lowercase())
}

/// The path with each part that exists resolved by the disk, links followed, and the rest as
/// written; `None` past a link that leads nowhere yet, since a write there lands wherever it
/// points.
fn real(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => _ = out.pop(),
            Component::Normal(_) => {
                out.push(part);
                match std::fs::canonicalize(&out) {
                    Ok(resolved) => out = resolved,
                    Err(_) if std::fs::symlink_metadata(&out).is_ok() => return None,
                    Err(_) => {}
                }
            }
            other => out.push(other),
        }
    }
    Some(out)
}

/// The path with `.` and `..` worked out by its text alone.
fn lexical(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => _ = out.pop(),
            other => out.push(other),
        }
    }
    out
}

/// Whether a shell command runs `git … push`: any `git` word followed by `push` in the same
/// command, so `git -C <dir> push` and `git -c k=v push` count too. A false match only asks.
fn pushes(command: &str) -> bool {
    command
        .split([';', '&', '|', '\n', '(', ')', '{', '}'])
        .any(|part| {
            let mut words = part.split_whitespace().map(|w| w.trim_matches(['"', '\'']));
            words.any(|w| {
                let name = w.rsplit(['/', '\\']).next().unwrap_or(w).to_lowercase();
                name == "git" || name == "git.exe"
            }) && words.any(|w| w == "push")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A path from its parts under this computer's kind of root.
    fn at(parts: &[&str]) -> PathBuf {
        let root = PathBuf::from(if cfg!(windows) { r"X:\" } else { "/" });
        parts.iter().fold(root, |path, part| path.join(part))
    }

    #[test]
    fn a_write_outside_the_worktree_asks_and_one_inside_does_not() {
        let tree = at(&["work", "trees", "pia-ui"]);
        let folders = [tree.clone()];
        let write = |path: PathBuf| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                &folders,
                &data(),
                false,
            )
        };
        assert_eq!(write(at(&["work", "trees", "pia-ui", "src", "a.rs"])), None);
        assert_eq!(
            write(at(&["WORK", "trees", "Pia-UI", "b.rs"])),
            None,
            "case is folded"
        );
        assert_eq!(
            write(PathBuf::from("src").join("c.rs")),
            None,
            "relative to the worktree"
        );
        let outside = Some("it writes outside the agent's folders");
        let sibling = at(&["work", "trees", "pia-ui-2", "a.rs"]);
        assert_eq!(
            write(sibling),
            outside,
            "a sibling whose name starts the same"
        );
        let climbs = at(&["work", "trees", "pia-ui", "..", "cass", "a.rs"]);
        assert_eq!(write(climbs), outside, "climbs out");
        assert_eq!(
            write(at(&["work", "gathr", "AGENTS.md"])),
            outside,
            "another checkout"
        );
        let notebook = json!({ "notebook_path": at(&["elsewhere", "n.ipynb"]) });
        assert_eq!(
            ask("NotebookEdit", &notebook, &folders, &data(), false),
            outside
        );
        let read = json!({ "file_path": at(&["elsewhere", "x"]) });
        assert_eq!(
            ask("Read", &read, &folders, &data(), false),
            None,
            "reads never ask"
        );
    }

    /// A data folder none of the other paths are in.
    fn data() -> PathBuf {
        at(&["gathr-data"])
    }

    /// A link at `link` to the folder `to`: a junction on Windows (no admin needed), else a
    /// symlink. `to` need not exist.
    fn link(to: &Path, link: &Path) {
        #[cfg(windows)]
        {
            let made = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(link)
                .arg(to)
                .output()
                .unwrap();
            assert!(made.status.success(), "mklink: {made:?}");
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink(to, link).unwrap();
    }

    #[test]
    fn links_are_followed_before_a_write_is_called_inside() {
        let root = std::env::temp_dir().join(format!("gathr-links-{}", crate::ids::span_id()));
        let (tree, outside) = (root.join("tree"), root.join("outside"));
        std::fs::create_dir_all(&tree).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        link(&outside, &tree.join("out"));
        link(&root.join("gone"), &tree.join("dangling"));
        link(&tree, &root.join("tree-link"));
        let write = |path: PathBuf, tree: &Path| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                &[tree.to_path_buf()],
                &data(),
                false,
            )
        };
        let asks = Some("it writes outside the agent's folders");
        let through = tree.join("out").join("proof.txt");
        assert_eq!(write(through, &tree), asks, "through a link out");
        let deeper = tree.join("out").join("new").join("proof.txt");
        assert_eq!(write(deeper, &tree), asks, "a new folder past a link out");
        let dangling = tree.join("dangling").join("x.txt");
        assert_eq!(write(dangling, &tree), asks, "a link to nowhere yet");
        let real = tree.join("a.txt");
        let linked = root.join("tree-link");
        assert_eq!(
            write(real, &linked),
            None,
            "the worktree named through a link"
        );
        let via = linked.join("b.txt");
        assert_eq!(
            write(via, &tree),
            None,
            "a file named through a link to the worktree"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_data_folder_asks_whatever_the_folders_say() {
        let root = std::env::temp_dir().join(format!("gathr-data-{}", crate::ids::span_id()));
        let (data, own) = (root.join("data"), root.join("agents").join("Pia"));
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&own).unwrap();
        let shared = root.join("agents").join("Shared");
        link(&data, &shared);
        let folders = [own.join("tree"), own.clone(), shared.clone()];
        let write = |path: PathBuf, folders: &[PathBuf]| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                folders,
                &data,
                false,
            )
        };
        let old_write = |path: PathBuf, folders: &[PathBuf], from_before: bool| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                folders,
                &data,
                from_before,
            )
        };
        let asks = Some("it writes in Gathr's data folder");
        assert_eq!(
            write(shared.join("gathr.db"), &folders),
            asks,
            "through Shared"
        );
        assert_eq!(write(data.join("gathr.db"), &folders), asks, "the database");
        let climbs = own.join("..").join("..").join("data").join("gathr.db");
        assert_eq!(write(climbs, &folders), asks, "climbs into it");
        assert_eq!(
            write(own.join("notes.md"), &folders),
            None,
            "its own folder"
        );
        // A chat's folder Gathr made in the data folder before the agent folder, and only that.
        let old = [data.join("chats").join("c1")];
        let mine = old[0].join("a.md");
        assert_eq!(
            old_write(mine.clone(), &old, true),
            None,
            "its own old folder"
        );
        assert_eq!(
            old_write(mine, &old, false),
            asks,
            "a folder in data made since"
        );
        let db = data.join("gathr.db");
        assert_eq!(
            old_write(db, &old, true),
            asks,
            "the rest of the data folder"
        );
        assert!(shut(&shared, &data, false), "a link into it");
        assert!(!shut(&old[0], &data, true), "a folder from before");
        assert!(shut(&data, &data, true), "one from before that holds it");
        assert!(
            super::overlaps(&shared, &data.join("accounts")),
            "a link above it"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[cfg(windows)]
    #[test]
    fn claude_codes_forward_slashes_on_windows_are_the_same_path() {
        let tree = Path::new(r"X:\work\tree");
        let write = |path: &str| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                &[tree.to_path_buf()],
                &data(),
                false,
            )
        };
        assert_eq!(write("X:/work/tree/a.rs"), None);
        assert!(write("X:/work/other/a.rs").is_some());
    }
    #[test]
    fn a_push_asks_however_it_is_written() {
        let tree = at(&["w"]);
        let folders = [tree];
        let run = |command: &str| {
            ask(
                "Bash",
                &json!({ "command": command }),
                &folders,
                &data(),
                false,
            )
        };
        for push in [
            "git push",
            "git push origin main",
            "cargo fmt && git push -u origin room/x",
            "git -C ../other push --force",
            "git -c credential.helper= push",
            "/usr/bin/git push",
            "& 'C:\\Program Files\\Git\\cmd\\git.exe' push",
        ] {
            assert_eq!(run(push), Some("it pushes"), "{push}");
        }
        for not in [
            "git status",
            "cargo test",
            "echo push; git log",
            "git log --grep=x",
        ] {
            assert_eq!(run(not), None, "{not}");
        }
        let ps = ask(
            "PowerShell",
            &json!({ "command": "git push" }),
            &folders,
            &data(),
            false,
        );
        assert_eq!(ps, Some("it pushes"));
    }
}
