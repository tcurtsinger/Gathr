//! Gathr's own asks, on top of the agent's mode (0029): before a push, and before a file tool
//! writes outside the agent's folders: the one its engine runs in, its own folder and its
//! project's Shared (0034). Claude Code asks Gathr through a hook before each such tool, whatever
//! its mode or a project's rules would allow.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// Why Gathr asks the person before this tool call, or `None` to leave it to the agent's mode.
/// `folders` are where it writes without asking, the engine's own first: a relative path is in it.
pub fn ask(tool: &str, input: &Value, folders: &[PathBuf]) -> Option<&'static str> {
    let text = |key: &str| input.get(key).and_then(Value::as_str);
    match tool {
        "Write" | "Edit" | "NotebookEdit" => {
            let path = text("file_path").or_else(|| text("notebook_path"))?;
            let path = folders.first()?.join(path);
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

/// Whether either folder holds the other, as the disk resolves them.
pub fn overlaps(a: &Path, b: &Path) -> bool {
    inside(a, b) || inside(b, a)
}

/// Whether `path`, relative to `boundary` when it isn't absolute, is in `boundary` as the disk
/// resolves both: links (symlinks, junctions) followed. Windows works out `..` by the text
/// before following a link and macOS after, so the path must be inside both ways. Case is
/// folded: both systems' disks ignore it by default.
fn inside(path: &Path, boundary: &Path) -> bool {
    let fold = |p: &Path| PathBuf::from(p.to_string_lossy().to_lowercase());
    let Some(base) = real(boundary).map(|b| fold(&b)) else {
        return false;
    };
    let path = boundary.join(path);
    [real(&path), real(&lexical(&path))]
        .into_iter()
        .all(|p| p.is_some_and(|p| fold(&p).starts_with(&base)))
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
        let write = |path: PathBuf| ask("Write", &json!({ "file_path": path }), &folders);
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
        assert_eq!(ask("NotebookEdit", &notebook, &folders), outside);
        let read = json!({ "file_path": at(&["elsewhere", "x"]) });
        assert_eq!(ask("Read", &read, &folders), None, "reads never ask");
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

    #[cfg(windows)]
    #[test]
    fn claude_codes_forward_slashes_on_windows_are_the_same_path() {
        let tree = Path::new(r"X:\work\tree");
        let write = |path: &str| {
            ask(
                "Write",
                &json!({ "file_path": path }),
                &[tree.to_path_buf()],
            )
        };
        assert_eq!(write("X:/work/tree/a.rs"), None);
        assert!(write("X:/work/other/a.rs").is_some());
    }
    #[test]
    fn a_push_asks_however_it_is_written() {
        let tree = at(&["w"]);
        let folders = [tree];
        let run = |command: &str| ask("Bash", &json!({ "command": command }), &folders);
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
        let ps = ask("PowerShell", &json!({ "command": "git push" }), &folders);
        assert_eq!(ps, Some("it pushes"));
    }
}
