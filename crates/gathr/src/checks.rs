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
        let command = match value.starts_with('|') || value.starts_with('>') {
            true => {
                let body = (at + 1..end)
                    .take_while(|&i| lines[i].trim().is_empty() || indent(lines[i]) > keys)
                    .map(|i| lines[i]);
                block(value, &body.collect::<Vec<_>>())
            }
            false => Some(value.trim_matches(['"', '\'']).to_string()),
        };
        let folder = lines[start..end]
            .iter()
            .filter(|l| indent(l) == keys || l.trim_start().starts_with("- "))
            .find_map(|l| key(l, "working-directory"));
        if let Some(command) = command.filter(|c| !c.is_empty()) {
            found.push(match folder {
                Some(folder) => format!("cd {folder} && {command}"),
                None => command,
            });
        }
    }
    found
}

/// A block scalar's text as YAML reads it, for the forms read here: `|` keeps each line, with the
/// indent inside the block and its blank lines; `>` joins lines with a space, a blank line making
/// a line break. `None` for what isn't read (an indent indicator such as `|2`, a more-indented
/// line in a folded block), so a command is never offered changed.
fn block(header: &str, body: &[&str]) -> Option<String> {
    let header = header.split(" #").next().unwrap_or(header).trim();
    let folded = header.starts_with('>');
    if !matches!(&header[1..], "" | "-" | "+") {
        return None;
    }
    let base = body
        .iter()
        .find(|l| !l.trim().is_empty())
        .map(|l| indent(l))?;
    // Only the block's indent is layout: every character past it, spaces too, is the script's.
    let mut lines = Vec::new();
    for line in body {
        let blank = line.trim().is_empty();
        match line.len() <= base && blank {
            true => lines.push(""),
            false if indent(line) < base && !blank || folded && indent(line) > base => return None,
            false => lines.push(&line[base..]),
        }
    }
    while lines.last() == Some(&"") {
        lines.pop();
    }
    if !folded {
        return Some(lines.join("\n"));
    }
    let mut text = String::new();
    for (i, line) in lines.iter().enumerate() {
        match (line.is_empty(), i > 0 && !lines[i - 1].is_empty()) {
            (true, _) => text.push('\n'),
            (false, true) => text.extend([" ", line]),
            (false, false) => text.push_str(line),
        }
    }
    Some(text)
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

    /// Vale, #112: a block keeps YAML's meaning. A folded one's lines are one command; a literal
    /// one keeps its blank lines and the indent inside it.
    #[test]
    fn a_block_step_keeps_its_folded_or_literal_meaning() {
        let workflow = "\
steps:
  - run: >-
      cargo test
      --workspace
  - run: |
      if true; then
        echo hi
      fi

      echo done
  - run: |2
        echo kept
";
        assert_eq!(
            run_steps(workflow),
            [
                "cargo test --workspace",
                "if true; then\n  echo hi\nfi\n\necho done",
            ],
            "and a block form it doesn't read (`|2`) isn't offered at all"
        );
    }

    /// Vale, #112: only YAML's own indent comes off a literal block. A trailing space after `\`
    /// and a line of spaces in a heredoc are the script's, not layout.
    #[test]
    fn a_literal_block_keeps_every_character_past_its_indent() {
        let workflow = [
            "steps:",
            "  - run: |",
            "      echo a \\ ",
            "      && echo b",
            "      cat <<EOF",
            "        ",
            "      EOF",
            "  - run: >",
            "      echo folded",
            "        ",
            "      echo more",
            "",
        ]
        .join("\n");
        assert_eq!(
            run_steps(&workflow),
            ["echo a \\ \n&& echo b\ncat <<EOF\n  \nEOF"],
            "and a folded block with a line of spaces past its indent isn't offered"
        );
    }
}
