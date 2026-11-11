//! Agents don't merge; Gathr does (0036 points 3-4, C4). A PR's reviewer asks with `merge`, and
//! Gathr refuses, saying why, unless the review is done on the PR's head H, the checks the project
//! asks for passed on H, and the default branch's commit B is in H. A PR that changes tests or check
//! settings, or that brings a project its first passing checks, also needs the reviewer's sign-off.
//! One merge per repository at a time, by the project's method; a push git rejects (B moved) is
//! refused too. Once merged, the PR's tasks settle at once.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use gathr::event::Event;
use gathr::platform::env::Vars;
use gathr::tasks;
use rusqlite::OptionalExtension;
use serde_json::Value;
use tokio::sync::Mutex;

use crate::chats::Chats;
use crate::tools::Caller;

const GIT_LIMIT: Duration = Duration::from_secs(120);

/// One merge at a time per repository: two at once would each check against the same B.
static MERGING: LazyLock<std::sync::Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(Default::default);

/// What Gathr knows of the chat's project and the caller's review of the PR.
struct Asked {
    project: String,
    folder: PathBuf,
    ci: String,
    method: String,
    has_checks: bool,
    reviewed: String,
}

/// Merges PR `pr` for the caller: what was done, or why not.
pub async fn merge(
    chats: &Arc<Chats>,
    caller: &Caller,
    pr: i64,
    signed_off: bool,
) -> Result<String, String> {
    let asked = asked(chats, caller, pr).await?;
    let lock = {
        let mut locks = MERGING.lock().expect("never poisoned");
        locks.entry(asked.folder.clone()).or_default().clone()
    };
    let _one = lock.lock().await;
    let vars = crate::coord::fresh().await;
    let view = crate::coord::github(
        &[
            "pr",
            "view",
            &pr.to_string(),
            "--json",
            "state,headRefOid,baseRefName,statusCheckRollup",
        ],
        &asked.folder,
    )
    .await
    .map_err(|why| format!("#{pr} wasn't read from GitHub: {why}"))?;
    let view: Value = serde_json::from_str(&view)
        .map_err(|_| format!("GitHub's answer for #{pr} wasn't read"))?;
    let text = |key: &str| view[key].as_str().unwrap_or_default().to_string();
    let (state, head, base) = (text("state"), text("headRefOid"), text("baseRefName"));
    if state != "OPEN" {
        return Err(format!("#{pr} is {}", state.to_lowercase()));
    }
    if !(head.starts_with(&asked.reviewed) || asked.reviewed.starts_with(&head)) || head.is_empty()
    {
        let short = |c: &str| c.get(..7).unwrap_or(c).to_string();
        return Err(format!(
            "you reviewed {}, and #{pr}'s head is now {}: review it again",
            short(&asked.reviewed),
            short(&head)
        ));
    }
    let repo = &asked.folder;
    let remotes = git(repo, &["remote"], &vars).await?;
    if !remotes.lines().any(|r| r == "origin") {
        return Err("the project has no origin to merge into".into());
    }
    git(repo, &["fetch", "--quiet", "origin"], &vars).await?;
    let b = git(
        repo,
        &["rev-parse", &format!("refs/remotes/origin/{base}")],
        &vars,
    )
    .await?;
    if !known(repo, &head, &vars).await {
        return Err(format!(
            "{head} isn't fetched from origin: a PR from a fork isn't merged here"
        ));
    }
    if !ancestor(repo, &b, &head, &vars).await {
        return Err(format!(
            "#{pr} isn't up to date with {base}: update it, then its checks run again"
        ));
    }
    checks_passed(chats, &asked, &head, &view).await?;
    let mut needs_sign_off = Vec::new();
    if asked.ci != "ci" && first_baseline(chats, &asked.project, repo, &b, &vars).await {
        needs_sign_off.push("it brings the project's first passing checks on its main".to_string());
    }
    let changed = git(repo, &["diff", "--name-only", &b, &head], &vars).await?;
    let tests: Vec<&str> = changed.lines().filter(|f| touches_checks(f)).collect();
    if !tests.is_empty() {
        needs_sign_off.push(format!(
            "it changes tests or check settings: {}",
            tests.join(", ")
        ));
    }
    if !needs_sign_off.is_empty() && !signed_off {
        return Err(format!(
            "#{pr} needs your sign-off, as {}. Check they still test the code, then merge with signed_off: true",
            needs_sign_off.join("; and ")
        ));
    }
    let merged = match asked.method.as_str() {
        "squash" => squash(pr, &head, repo).await?,
        "merge" => {
            let tree = format!("{head}^{{tree}}");
            let message = format!("Merge pull request #{pr}");
            let m = git(
                repo,
                &["commit-tree", &tree, "-p", &b, "-p", &head, "-m", &message],
                &vars,
            )
            .await?;
            push(repo, &m, &base, pr, &vars).await?;
            m
        }
        _ => {
            push(repo, &head, &base, pr, &vars).await?;
            head.clone()
        }
    };
    settle(chats, caller, pr, &head).await;
    let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
    let event = Event::point("gathr.merge.done")
        .trace(&trace)
        .chat(&caller.chat)
        .ok();
    chats.writer.record(
        event
            .attr("gathr.project.id", asked.project.clone())
            .attr("pr", pr)
            .attr("head", head.clone())
            .attr("method", asked.method.clone())
            .attr("merged", merged.clone()),
    );
    Ok(format!(
        "Merged #{pr} into {base} ({}): its tasks are settled.",
        asked.method
    ))
}

/// The chat's project and the caller's review of the PR, or why there's nothing to merge.
async fn asked(chats: &Arc<Chats>, caller: &Caller, pr: i64) -> Result<Asked, String> {
    let (chat, agent) = (
        caller.chat.clone(),
        caller.agent.clone().unwrap_or_default(),
    );
    let found = chats
        .writer
        .call(move |c| -> Result<Result<Asked, String>, gathr::Error> {
            let project: Option<String> = c
                .query_row("SELECT project_id FROM chats WHERE id = ?1", [&chat], |r| {
                    r.get(0)
                })
                .optional()?
                .flatten();
            let Some(project) = project else {
                return Ok(Err("this chat isn't in a project".into()));
            };
            let Some(found) = gathr::projects::get(c, &project)? else {
                return Ok(Err("the project is gone".into()));
            };
            let review: Option<(Option<String>, String, Option<String>, bool)> = c
                .query_row(
                    "SELECT finish, state, reviewed, outdated FROM tasks
                     WHERE chat_id = ?1 AND kind = 'review' AND pr = ?2 AND owner = ?3
                     ORDER BY number DESC LIMIT 1",
                    rusqlite::params![chat, pr, agent],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
                )
                .optional()?;
            let Some((finish, state, reviewed, outdated)) = review else {
                return Ok(Err(format!(
                    "only #{pr}'s reviewer merges it, and you have no review of it here"
                )));
            };
            let done = finish.as_deref() == Some("done") || state == "done";
            let Some(reviewed) = reviewed.filter(|_| done && !outdated) else {
                return Ok(Err(format!(
                    "your review of #{pr} isn't done: finish it on the commit you reviewed"
                )));
            };
            Ok(Ok(Asked {
                project,
                folder: PathBuf::from(found.folder),
                ci: found.ci,
                method: found.merge,
                has_checks: !found.checks.is_empty(),
                reviewed,
            }))
        })
        .await;
    match found {
        Some(Ok(asked)) => asked,
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

/// The checks the project asks for, passed on `head`: GitHub's CI, Gathr's (C3), or both.
async fn checks_passed(
    chats: &Arc<Chats>,
    asked: &Asked,
    head: &str,
    view: &Value,
) -> Result<(), String> {
    let short = head.get(..7).unwrap_or(head);
    if asked.ci != "gathr" {
        let rollup = view["statusCheckRollup"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        if rollup.is_empty() {
            return Err(format!("no CI checks ran on {short}"));
        }
        let fine = |c: &Value| {
            let word = c["conclusion"]
                .as_str()
                .or(c["state"].as_str())
                .unwrap_or_default();
            matches!(word, "SUCCESS" | "NEUTRAL" | "SKIPPED")
        };
        if !rollup.iter().all(fine) {
            return Err(format!("CI checks on {short} haven't all passed"));
        }
    }
    if asked.ci == "ci" {
        return Ok(());
    }
    if !asked.has_checks {
        return Err("the project has no checks: its Supervisor sets them".into());
    }
    let (project, h) = (asked.project.clone(), head.to_string());
    let latest = chats
        .writer
        .call(move |c| -> Result<Option<String>, gathr::Error> {
            Ok(c.query_row(
                "SELECT state FROM check_runs WHERE project_id = ?1 AND head = ?2
                 ORDER BY state = 'passed' DESC, started_ts DESC LIMIT 1",
                rusqlite::params![project, h],
                |r| r.get(0),
            )
            .optional()?)
        })
        .await;
    match latest.and_then(Result::ok).flatten().as_deref() {
        Some("passed") => Ok(()),
        Some("running") => Err(format!("Gathr's checks are still running on {short}")),
        Some(_) => Err(format!(
            "Gathr's checks failed on {short}: see its run for why"
        )),
        None => Err(format!("Gathr's checks haven't run on {short} yet")),
    }
}

/// Whether no passed run of the project's checks is on its main yet (`b` or before it): the
/// first baseline, which the reviewer signs off.
async fn first_baseline(
    chats: &Arc<Chats>,
    project: &str,
    repo: &Path,
    b: &str,
    vars: &Vars,
) -> bool {
    let id = project.to_string();
    let heads = chats
        .writer
        .call(move |c| -> Result<Vec<String>, gathr::Error> {
            let mut stmt = c.prepare(
                "SELECT DISTINCT head FROM check_runs WHERE project_id = ?1 AND state = 'passed' LIMIT 200",
            )?;
            let heads = stmt.query_map([&id], |r| r.get(0))?;
            Ok(heads.collect::<Result<_, _>>()?)
        })
        .await;
    for head in heads.and_then(Result::ok).unwrap_or_default() {
        if ancestor(repo, &head, b, vars).await {
            return false;
        }
    }
    true
}

/// Whether a changed file is a test or a check setting, which a reviewer signs off.
fn touches_checks(file: &str) -> bool {
    let lower = file.to_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    const SETTINGS: [&str; 7] = [
        "package.json",
        "cargo.toml",
        "pyproject.toml",
        "makefile",
        "justfile",
        "tox.ini",
        "pytest.ini",
    ];
    lower.starts_with(".github/workflows/")
        || SETTINGS.contains(&name)
        || lower
            .split('/')
            .any(|part| part.contains("test") || part.contains("spec"))
}

/// Pushes `commit` to `base` on origin, a fast-forward or nothing: git refuses it if B moved.
async fn push(repo: &Path, commit: &str, base: &str, pr: i64, vars: &Vars) -> Result<(), String> {
    let to = format!("{commit}:refs/heads/{base}");
    git(repo, &["push", "--quiet", "origin", &to], vars)
        .await
        .map(|_| ())
        .map_err(|why| format!("{base} moved while #{pr} merged, so git refused it: update #{pr}, then merge again ({why})"))
}

/// GitHub's own squash, pinned to the head checked: `sha` makes it refuse a head that moved.
async fn squash(pr: i64, head: &str, repo: &Path) -> Result<String, String> {
    let path = format!("repos/{{owner}}/{{repo}}/pulls/{pr}/merge");
    let sha = format!("sha={head}");
    let args = [
        "api",
        "-X",
        "PUT",
        &path,
        "-f",
        "merge_method=squash",
        "-f",
        &sha,
    ];
    crate::coord::github(&args, repo)
        .await
        .map_err(|why| format!("GitHub didn't squash #{pr}: {why}"))?;
    Ok(head.to_string())
}

/// The PR's tasks in the chat, settled now as a PR look would after its merge (#100's path). Their
/// givers hear back, but never the agent that merged: it knows (G159), so the turn saved to tell
/// it goes with the same step, before anything starts it.
async fn settle(chats: &Arc<Chats>, caller: &Caller, pr: i64, head: &str) {
    let (chat, h) = (caller.chat.clone(), head.to_string());
    let merger = caller.agent.clone();
    let trace = caller.trace().unwrap_or_else(gathr::ids::trace_id);
    let starts = chats
        .writer
        .call(move |c| -> Result<Vec<gathr::tasks::Start>, gathr::Error> {
            let open = format!(
                "SELECT number FROM tasks WHERE chat_id = ?1 AND pr = ?2 AND (state NOT IN {} OR (kind = 'review' AND pr_closed = 0))",
                tasks::SETTLED
            );
            let mut stmt = c.prepare(&open)?;
            let numbers: Vec<i64> = stmt
                .query_map(rusqlite::params![chat, pr], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            drop(stmt);
            let mut starts = Vec::new();
            for number in numbers {
                starts.extend(tasks::pr_looked(c, number, "MERGED", Some(&h), &trace)?);
            }
            let (told, own): (Vec<_>, Vec<_>) =
                starts.into_iter().partition(|s| Some(&s.agent) != merger.as_ref());
            for start in own {
                c.execute("DELETE FROM messages WHERE turn_id = ?1", [&start.turn])?;
                c.execute("DELETE FROM turns WHERE id = ?1", [&start.turn])?;
            }
            Ok(told)
        })
        .await;
    if let Some(Ok(starts)) = starts {
        chats.start(starts);
    }
}

async fn git(dir: &Path, args: &[&str], vars: &Vars) -> Result<String, String> {
    let ran = gathr::run::run(Path::new("git"), args, dir, vars, GIT_LIMIT).await?;
    match ran.code {
        Some(0) => Ok(ran.out),
        _ => Err(format!("git {} failed: {}", args[0], ran.reason())),
    }
}

async fn known(repo: &Path, commit: &str, vars: &Vars) -> bool {
    git(
        repo,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        vars,
    )
    .await
    .is_ok()
}

async fn ancestor(repo: &Path, older: &str, newer: &str, vars: &Vars) -> bool {
    git(repo, &["merge-base", "--is-ancestor", older, newer], vars)
        .await
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::touches_checks;

    #[test]
    fn tests_and_check_settings_are_flagged_and_code_isnt() {
        for flagged in [
            "tests/check.sh",
            "src/a_test.rs",
            "ui/src/x.spec.ts",
            "package.json",
            ".github/workflows/ci.yml",
            "crates/gathr/Cargo.toml",
        ] {
            assert!(touches_checks(flagged), "{flagged}");
        }
        for plain in ["src/main.rs", "README.md", "ui/src/App.tsx"] {
            assert!(!touches_checks(plain), "{plain}");
        }
    }
}
