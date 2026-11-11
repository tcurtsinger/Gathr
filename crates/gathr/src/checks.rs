//! A project's checks (0036, owner's call on task 1455): the Supervisor sets them with Gathr's
//! tool, starting from what Gathr finds in the project; the person never types one. A command the
//! project's own files name runs freely; any other is the person's to allow, once per project.

use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::Error;

/// The most checks a project keeps: a test run, not a script.
pub const MOST: usize = 20;

/// What the project's files say its own checks are, in order, each once: its package.json's
/// `test`, `lint` and `build` scripts (run as its lockfile's tool does), Cargo's test and clippy,
/// pytest when pyproject.toml names it, then each `run:` step of its GitHub workflows, whether
/// Actions runs them or not. A start for the Supervisor, not a list to keep as it is.
pub fn found(dir: &Path) -> Vec<String> {
    let mut found = Vec::new();
    let read = |name: &str| std::fs::read_to_string(dir.join(name)).ok();
    if let Some(package) = read("package.json") {
        found.extend(scripts(dir, &package));
    }
    if read("Cargo.toml").is_some() {
        found.extend(["cargo test".to_string(), "cargo clippy".to_string()]);
    }
    if read("pyproject.toml").is_some_and(|p| p.contains("pytest")) {
        found.push("pytest".into());
    }
    let workflows = std::fs::read_dir(dir.join(".github").join("workflows"));
    let mut files: Vec<_> = workflows
        .into_iter()
        .flatten()
        .flatten()
        .map(|f| f.path())
        .collect();
    files.sort();
    for file in files {
        let yaml = file.extension().is_some_and(|e| e == "yml" || e == "yaml");
        if let Some(text) = yaml.then(|| std::fs::read_to_string(&file).ok()).flatten() {
            found.extend(run_steps(&text));
        }
    }
    let mut seen = std::collections::HashSet::new();
    found.retain(|c| seen.insert(c.clone()));
    found
}

fn scripts(dir: &Path, package: &str) -> Vec<String> {
    let Ok(package) = serde_json::from_str::<serde_json::Value>(package) else {
        return Vec::new();
    };
    let tool = if dir.join("pnpm-lock.yaml").is_file() {
        "pnpm"
    } else if dir.join("yarn.lock").is_file() {
        "yarn"
    } else {
        "npm"
    };
    let has = |name: &str| package["scripts"][name].is_string();
    let mut found = Vec::new();
    if has("test") {
        found.push(format!("{tool} test"));
    }
    for name in ["lint", "build"].into_iter().filter(|n| has(n)) {
        found.push(format!("{tool} run {name}"));
    }
    found
}

/// The `run:` steps of a workflow, read line by line (no YAML library for this one use): a
/// one-line `run:`, or a `run: |` block as one script, after `cd` to the step's
/// `working-directory`. Anything else in the file is left alone.
fn run_steps(text: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut found = Vec::new();
    for (at, line) in lines.iter().enumerate() {
        let Some(value) = key(line, "run") else {
            continue;
        };
        // A step's keys line up under its `- `; the run line may be that first line.
        let keys = indent(line)
            + if line.trim_start().starts_with("- ") {
                2
            } else {
                0
            };
        let start = (0..=at)
            .rev()
            .find(|&i| lines[i].trim_start().starts_with("- ") && indent(lines[i]) + 2 == keys)
            .unwrap_or(at);
        let end = (at + 1..lines.len())
            .find(|&i| {
                !lines[i].trim().is_empty()
                    && (indent(lines[i]) < keys
                        || lines[i].trim_start().starts_with("- ") && indent(lines[i]) + 2 == keys)
            })
            .unwrap_or(lines.len());
        let script: Vec<&str> = match value.starts_with('|') || value.starts_with('>') {
            true => (at + 1..end)
                .take_while(|&i| lines[i].trim().is_empty() || indent(lines[i]) > keys)
                .map(|i| lines[i].trim())
                .filter(|l| !l.is_empty())
                .collect(),
            false => vec![value.trim_matches(['"', '\''])],
        };
        let folder = lines[start..end]
            .iter()
            .filter(|l| indent(l) == keys || l.trim_start().starts_with("- "))
            .find_map(|l| key(l, "working-directory"));
        let command = script.join("\n");
        if !command.is_empty() {
            found.push(match folder {
                Some(folder) => format!("cd {folder} && {command}"),
                None => command,
            });
        }
    }
    found
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// The value of `name:` on the line, a list item's first key too.
fn key<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let trimmed = line.trim_start().trim_start_matches("- ");
    trimmed.strip_prefix(name)?.strip_prefix(':').map(str::trim)
}

/// The commands as saved, each trimmed, or why they can't be.
pub fn commands(list: Vec<String>) -> Result<Vec<String>, String> {
    let list: Vec<String> = list.iter().map(|c| c.trim().to_string()).collect();
    if list.iter().any(String::is_empty) {
        return Err("A check needs a command".into());
    }
    if list.len() > MOST {
        return Err(format!("A project keeps at most {MOST} checks"));
    }
    Ok(list)
}

/// The commands the person hasn't allowed for this project yet.
pub fn not_allowed(
    conn: &Connection,
    project: &str,
    commands: &[String],
) -> Result<Vec<String>, Error> {
    let mut stmt = conn.prepare_cached(
        "SELECT 1 FROM project_check_allowed WHERE project_id = ?1 AND command = ?2",
    )?;
    let mut left = Vec::new();
    for command in commands {
        if stmt
            .query_row(params![project, command], |_| Ok(()))
            .optional()?
            .is_none()
        {
            left.push(command.clone());
        }
    }
    Ok(left)
}

/// Replaces the project's checks with `commands`, each set by `source` now, and records the ones
/// the person just `allowed`, in one step.
pub fn set(
    conn: &mut Connection,
    project: &str,
    commands: &[String],
    source: &str,
    allowed: &[String],
) -> Result<(), Error> {
    let now = crate::ids::now_us();
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM project_checks WHERE project_id = ?1",
        [project],
    )?;
    for (position, command) in commands.iter().enumerate() {
        tx.execute(
            "INSERT INTO project_checks (project_id, position, command, source, set_ts) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![project, position as i64, command, source, now],
        )?;
    }
    for command in allowed {
        tx.execute(
            "INSERT OR IGNORE INTO project_check_allowed (project_id, command, allowed_ts) VALUES (?1, ?2, ?3)",
            params![project, command, now],
        )?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_workflow_gives_each_run_step_in_its_folder_and_nothing_else() {
        let workflow = "\
name: CI
jobs:
  ui:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: lint
        run: npx biome ci
        working-directory: ui
      - name: test
        working-directory: ui
        run: |
          npx tsc --noEmit
          npx vitest run
      - run: cargo test --workspace --locked
        env:
          KEEP: \"1\"
";
        assert_eq!(
            run_steps(workflow),
            [
                "cd ui && npx biome ci",
                "cd ui && npx tsc --noEmit\nnpx vitest run",
                "cargo test --workspace --locked",
            ]
        );
    }
}
