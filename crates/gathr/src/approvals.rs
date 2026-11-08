//! Gathr's own asks, on top of the agent's mode (0029): before a push, and before a file tool
//! writes outside the folder its engine runs in. Claude Code asks Gathr through a hook before
//! each such tool, whatever its mode or a project's rules would allow.

use std::path::{Component, Path, PathBuf};

use serde_json::Value;

/// Why Gathr asks the person before this tool call, or `None` to leave it to the agent's mode.
pub fn ask(tool: &str, input: &Value, boundary: &Path) -> Option<&'static str> {
    let text = |key: &str| input.get(key).and_then(Value::as_str);
    match tool {
        "Write" | "Edit" | "NotebookEdit" => {
            let path = text("file_path").or_else(|| text("notebook_path"))?;
            (!inside(Path::new(path), boundary)).then_some("it writes outside the agent's worktree")
        }
        "Bash" | "PowerShell" => pushes(text("command")?).then_some("it pushes"),
        _ => None,
    }
}

/// Whether `path`, relative to `boundary` when it isn't absolute, is in `boundary`, compared by
/// its parts with `..` worked out. Case is folded: Gathr runs on Windows and macOS, whose disks
/// ignore case by default.
fn inside(path: &Path, boundary: &Path) -> bool {
    let fold = |p: &Path| PathBuf::from(p.to_string_lossy().to_lowercase());
    let (base, path) = (lexical(&fold(boundary)), fold(path));
    lexical(&base.join(path)).starts_with(&base)
}

/// The path with `.` and `..` worked out by its text alone, so a link is never followed.
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
        let write = |path: PathBuf| ask("Write", &json!({ "file_path": path }), &tree);
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
        let outside = Some("it writes outside the agent's worktree");
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
        assert_eq!(ask("NotebookEdit", &notebook, &tree), outside);
        let read = json!({ "file_path": at(&["elsewhere", "x"]) });
        assert_eq!(ask("Read", &read, &tree), None, "reads never ask");
    }

    #[cfg(windows)]
    #[test]
    fn claude_codes_forward_slashes_on_windows_are_the_same_path() {
        let tree = Path::new(r"X:\work\tree");
        let write = |path: &str| ask("Write", &json!({ "file_path": path }), tree);
        assert_eq!(write("X:/work/tree/a.rs"), None);
        assert!(write("X:/work/other/a.rs").is_some());
    }
    #[test]
    fn a_push_asks_however_it_is_written() {
        let tree = at(&["w"]);
        let run = |command: &str| ask("Bash", &json!({ "command": command }), &tree);
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
        let ps = ask("PowerShell", &json!({ "command": "git push" }), &tree);
        assert_eq!(ps, Some("it pushes"));
    }
}
