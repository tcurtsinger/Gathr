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
            // Read again at each write: a link made since the engine started counts too.
            let old_own = from_before && !linked(own, data) && inside(&path, own);
            if reaches(&path, data) && !old_own {
                return Some("it writes in Gathr's data folder");
            }
            let inside_one = folders.iter().any(|folder| inside(&path, folder));
            (!inside_one).then_some("it writes outside the agent's folders")
        }
        "Bash" | "PowerShell" => {
            let command = text("command").filter(|_| runs_push(tool, input))?;
            pushes_risky(command).then_some("it may force, delete or skip Gathr's push hook")
        }
        // Gathr runs a wait's check itself, outside Claude Code's own checks and auto mode's
        // classifier: asked here, the call waits for the person as long as they take. One known
        // read-only command alone doesn't ask (0036 C11).
        "mcp__gathr__wait" => text("check")
            .is_some_and(|c| !c.trim().is_empty() && read_only_check(c).is_none())
            .then_some("Gathr runs this check in sh (Git Bash on Windows) until it exits 0"),
        _ => None,
    }
}

/// The rule that lets a wait's check run without asking (owner, task 1450): one of a few commands
/// that only read, alone, by its plain name, with nothing the shell would chain, redirect, expand or
/// substitute, and none of their options that write a file, open a browser, reach another repo or
/// run a program the repository's config names (a diff driver, textconv, gpg). It runs with
/// `READ_ONLY_CONFIG` over the repository's own. Anything else is `None` and asks as before.
pub fn read_only_check(command: &str) -> Option<&'static str> {
    const SHELL: [char; 14] = [
        ';', '&', '|', '<', '>', '`', '$', '(', ')', '{', '}', '\n', '\r', '\\',
    ];
    if command.contains(SHELL) {
        return None;
    }
    let words: Vec<&str> = command.split_whitespace().collect();
    let (rule, rest) = match words.as_slice() {
        ["gh", "pr", "view", rest @ ..] => ("gh pr view", rest),
        ["gh", "pr", "checks", rest @ ..] => ("gh pr checks", rest),
        ["git", "status", rest @ ..] => ("git status", rest),
        ["git", "log", rest @ ..] => ("git log", rest),
        ["git", "rev-parse", rest @ ..] => ("git rev-parse", rest),
        _ => return None,
    };
    let fine = |word: &&str| match rule {
        // No diff, stat or signature: those run what the repository's config names.
        "git log" => {
            let name = word.split('=').next().unwrap_or(word);
            let count = word.len() > 1 && word[1..].chars().all(|c| c.is_ascii_digit());
            !word.contains("%G") && (!word.starts_with('-') || count || LOG_OPTIONS.contains(&name))
        }
        "gh pr view" | "gh pr checks" => {
            let lower = word.to_lowercase();
            !["--web", "--repo", "-w", "-r"]
                .iter()
                .any(|o| lower.starts_with(o))
        }
        _ => true,
    };
    rest.iter().all(fine).then_some(rule)
}

/// `git log`'s options a read-only check may use: which commits, and how each is named.
const LOG_OPTIONS: [&str; 26] = [
    "--oneline",
    "--format",
    "--pretty",
    "-n",
    "--max-count",
    "--skip",
    "--since",
    "--after",
    "--until",
    "--before",
    "--author",
    "--committer",
    "--grep",
    "--first-parent",
    "--no-merges",
    "--merges",
    "--decorate",
    "--no-decorate",
    "--date",
    "--reverse",
    "--all",
    "--branches",
    "--tags",
    "--remotes",
    "--abbrev-commit",
    "--",
];

/// Config a read-only check runs with, over the repository's own (which an agent can change):
/// no fsmonitor hook, no pager, no signature check. As `GIT_CONFIG_*` pairs.
pub fn read_only_config() -> Vec<(String, String)> {
    let pairs = [
        ("core.fsmonitor", "false"),
        ("core.pager", "cat"),
        ("log.showSignature", "false"),
    ];
    let mut vars = vec![("GIT_CONFIG_COUNT".to_string(), pairs.len().to_string())];
    for (i, (key, value)) in pairs.iter().enumerate() {
        vars.push((format!("GIT_CONFIG_KEY_{i}"), key.to_string()));
        vars.push((format!("GIT_CONFIG_VALUE_{i}"), value.to_string()));
    }
    vars
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
/// design (0034): it's shut once a link in it below the data folder leads anywhere else (the
/// logins, the database), as a link there was never Gathr's.
pub fn shut(folder: &Path, data: &Path, from_before: bool) -> bool {
    match from_before {
        true => linked(folder, data),
        false => overlaps(folder, data),
    }
}

/// Whether `folder`, below `data`, isn't on the disk where its name says: a link in it.
fn linked(folder: &Path, data: &Path) -> bool {
    let Ok(below) = folder.strip_prefix(data) else {
        return true;
    };
    let real = crate::folders::real;
    below.as_os_str().is_empty() || fold(&real(folder)) != fold(&real(data).join(below))
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
    push_words(command).next().is_some()
}

/// The words of each `git … push` in a command, from `git` on.
fn push_words(command: &str) -> impl Iterator<Item = Vec<&str>> {
    let parts = command.split([';', '&', '|', '\n', '(', ')', '{', '}']);
    parts.filter_map(|part| {
        let words: Vec<&str> = part
            .split_whitespace()
            .map(|w| w.trim_matches(['"', '\'']))
            .collect();
        let git = words.iter().position(|w| {
            let name = w.rsplit(['/', '\\']).next().unwrap_or(w).to_lowercase();
            name == "git" || name == "git.exe"
        })?;
        let words = words[git..].to_vec();
        words[1..].contains(&"push").then_some(words)
    })
}

/// Whether a push command asks for what its worktree's `pre-push` hook can't see or stop: a force
/// (`--force`, `-f`, a `+` refspec), a deletion (`--delete`, `-d`, `:branch`), `--mirror`, or a
/// way past the hook (`--no-verify`, another `core.hooksPath`). The hook still sees the refs of
/// what goes through and asks for the rest (`crate::pushes`).
fn pushes_risky(command: &str) -> bool {
    push_words(command).any(|words| {
        words.iter().any(|w| {
            let lower = w.to_lowercase();
            let short = w.strip_prefix('-').filter(|f| !f.starts_with('-'));
            lower.starts_with("--force")
                || ["--delete", "--mirror", "--no-verify"].contains(&lower.as_str())
                || lower.contains("hookspath")
                || short.is_some_and(|f| f.contains(['f', 'd']))
                || w.starts_with('+')
                || w.starts_with(':')
        })
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
        // A folder from before, made a link to the logins.
        let logins = data.join("accounts");
        std::fs::create_dir_all(&logins).unwrap();
        std::fs::create_dir_all(data.join("chats")).unwrap();
        let moved = [data.join("chats").join("c2")];
        link(&logins, &moved[0]);
        assert!(shut(&moved[0], &data, true), "a link from before");
        let login = moved[0].join(".credentials.json");
        assert_eq!(old_write(login, &moved, true), asks, "through it");
        let login = logins.join(".credentials.json");
        assert_eq!(
            old_write(login, &moved, true),
            asks,
            "the logins themselves"
        );
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
    fn a_push_reads_as_one_however_it_is_written() {
        let push = |command: &str| runs_push("Bash", &json!({ "command": command }));
        for yes in [
            "git push",
            "cargo fmt && git push -u origin room/x",
            "git -c credential.helper= push",
            "/usr/bin/git push",
            r"& 'C:\Program Files\Git\cmd\git.exe' push",
        ] {
            assert!(push(yes), "{yes}");
        }
        for not in [
            "git status",
            "cargo test",
            "echo push; git log",
            "git log --grep=x",
        ] {
            assert!(!push(not), "{not}");
        }
    }

    #[test]
    fn only_a_push_that_may_force_delete_or_skip_the_hook_asks() {
        let tree = at(&["w"]);
        let folders = [tree];
        let run = |tool: &str, command: &str| {
            ask(
                tool,
                &json!({ "command": command }),
                &folders,
                &data(),
                false,
            )
        };
        for risky in [
            "git push --force",
            "cargo fmt && git push -fu origin room/x",
            "git -C ../other push --force-with-lease",
            "git push origin +room/x",
            "git push origin :room/x",
            "git push -d origin room/x",
            "git push --mirror",
            "/usr/bin/git push --no-verify",
            "git -c core.hooksPath=/dev/null push",
            r"& 'C:\Program Files\Git\cmd\git.exe' push --force",
        ] {
            assert!(run("Bash", risky).is_some(), "{risky}");
        }
        assert!(run("PowerShell", "git push -f").is_some());
        for not in [
            "git push",
            "git push origin main",
            "git push -u origin room/x",
            "git -C ../other push origin HEAD:refs/heads/room/x",
            "git status -f",
            "echo --force; git log",
            "cargo test -- --force",
        ] {
            assert_eq!(run("Bash", not), None, "{not}");
        }
    }

    /// 0036 C11 (owner, task 1450): a wait's check that is one known read-only command, alone,
    /// runs without asking; anything chained, redirected or substituted asks as before.
    #[test]
    fn a_known_read_only_check_alone_runs_without_asking() {
        let wait = |check: &str| {
            let input = json!({ "check": check, "then": "go on" });
            ask("mcp__gathr__wait", &input, &[at(&["w"])], &data(), false)
        };
        for (check, rule) in [
            ("gh pr view 12 --json state", "gh pr view"),
            ("gh pr checks 12", "gh pr checks"),
            ("git status --porcelain", "git status"),
            ("git log -1 --format=%H origin/main", "git log"),
            ("git log --oneline -n 5 --first-parent main", "git log"),
            ("git rev-parse HEAD", "git rev-parse"),
            ("  git rev-parse   HEAD  ", "git rev-parse"),
        ] {
            assert_eq!(read_only_check(check), Some(rule), "{check}");
            assert_eq!(wait(check), None, "{check}");
        }
        for check in [
            "git status; rm -rf x",
            "git status && rm x",
            "git status &",
            "git log | sh",
            "git status > out",
            "git log < in",
            "git rev-parse $(rm x)",
            "git rev-parse ${HOME}",
            "git log `rm x`",
            "git status\nrm x",
            "git -c core.pager=sh log",
            "git -C .. status",
            "git log --output=x",
            "git log -p --ext-diff",
            "git log -p",
            "git log --stat",
            "git log --show-signature",
            "git log --format=%G?",
            "gh pr view 1 --web",
            "gh pr checks 1 -w",
            "gh pr view 1 --repo other/repo",
            "git push",
            "gh pr merge 1",
            "git logx",
            "/usr/bin/git status",
        ] {
            assert_eq!(read_only_check(check), None, "{check}");
            assert!(wait(check).is_some(), "{check}");
        }
    }

    /// A repository's own config can name a program git runs (here `core.fsmonitor` for `git
    /// status`). An agent can set it, so a read-only check runs with `read_only_config` over it.
    #[test]
    fn a_read_only_check_never_runs_a_program_the_repositorys_config_names() {
        let root = std::env::temp_dir().join(format!("gathr-c11-{}", crate::ids::span_id()));
        std::fs::create_dir_all(&root).unwrap();
        let git = |args: &[&str], over: &[(String, String)]| {
            let mut command = std::process::Command::new("git");
            for var in crate::platform::env::LOCAL_GIT_VARS {
                command.env_remove(var);
            }
            command
                .args(args)
                .current_dir(&root)
                .envs(over.iter().cloned());
            command.output().unwrap()
        };
        git(&["init", "-q"], &[]);
        git(
            &["config", "core.fsmonitor", "echo ran > marker; exit 1"],
            &[],
        );
        git(&["status"], &read_only_config());
        let after_guarded = root.join("marker").exists();
        git(&["status"], &[]);
        let after_plain = root.join("marker").exists();
        let _ = std::fs::remove_dir_all(&root);
        assert!(
            after_plain,
            "the hook didn't run even unguarded: the test proves nothing"
        );
        assert!(
            !after_guarded,
            "a read-only check ran the repository's fsmonitor hook"
        );
    }
}
