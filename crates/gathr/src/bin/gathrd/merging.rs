//! Agents don't merge; Gathr does (0043 points 3-4, C4). A PR's reviewer asks with `merge`, and
//! Gathr refuses, saying why, unless a done review covers the PR's head H, the checks the project
//! asks for passed on H, and the default branch's commit B is in H. A PR that changes tests or check
//! settings, or that brings a project its first passing checks, also needs the reviewer's sign-off.
//! One merge per repository at a time, by the project's method; a push git rejects (B moved) is
//! refused too. Once merged, the PR's tasks settle at once. Gathr lands a PR with the same code,
//! in its reviewer's place (0049, `landing`).

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

mod review;

use crate::chats::Chats;
use crate::tools::Caller;

const GIT_LIMIT: Duration = Duration::from_secs(120);

/// One merge at a time per repository: two at once would each check against the same B.
static MERGING: LazyLock<std::sync::Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>> =
    LazyLock::new(Default::default);

/// The project a PR is merged in, as its Checks & merge settings stand.
pub struct Project {
    pub id: String,
    pub folder: PathBuf,
    pub ci: String,
    pub method: String,
    pub has_checks: bool,
}

/// What Gathr knows of the chat's project and the caller's review of the PR.
struct Asked {
    project: Project,
    reviewed: String,
    /// The reviewer signed off with the review itself (0049).
    signed_off: bool,
}

/// In whose place a PR is merged: its reviewer's, who asked with `merge`, or Gathr's own, landing
/// it on the reviewer's done review (0049). `agent` is who knows of the merge already: it gets no
/// turn to hear of it (G159).
pub struct By {
    pub chat: String,
    pub agent: Option<String>,
    pub trace: String,
    /// Gathr landed it: nobody called `merge`.
    pub landed: bool,
}

/// A merge that was pushed: what to say of it, and whether GitHub showed the PR merged.
pub struct Merged {
    pub said: String,
    pub confirmed: bool,
}

/// The repository's lock: one merge at a time, the tool's and Gathr's own.
pub fn lock_of(folder: &Path) -> Arc<Mutex<()>> {
    let mut locks = MERGING.lock().expect("never poisoned");
    locks.entry(folder.to_path_buf()).or_default().clone()
}

/// Merges PR `pr` for the caller: what was done, or why not.
pub async fn merge(
    chats: &Arc<Chats>,
    caller: &Caller,
    pr: i64,
    signed_off: bool,
) -> Result<String, String> {
    let asked = asked(chats, caller, pr).await?;
    let project = &asked.project;
    let lock = lock_of(&project.folder);
    let one = lock.lock().await;
    let vars = crate::coord::fresh().await;
    let repo = &project.folder;
    let (view, head, base) = open_pr(pr, repo).await?;
    let b = fetched(repo, &base, &vars).await?;
    if !known(repo, &head, &vars).await {
        return Err(format!(
            "{head} isn't fetched from origin: a PR from a fork isn't merged here"
        ));
    }
    reviewed_is_head(repo, &asked.reviewed, &head, &b, pr, &vars).await?;
    if !ancestor(repo, &b, &head, &vars).await {
        return Err(format!(
            "#{pr} isn't up to date with {base}: update it, then its checks run again"
        ));
    }
    checks_passed(chats, project, &head, &view).await?;
    let needs_sign_off = sign_off_for(chats, project, &b, &head, &vars).await?;
    if !needs_sign_off.is_empty() && !(signed_off || asked.signed_off) {
        return Err(format!(
            "#{pr} needs your sign-off, as {}. Check they still test the code, then merge with signed_off: true",
            needs_sign_off.join("; and ")
        ));
    }
    let by = By {
        chat: caller.chat.clone(),
        agent: caller.agent.clone(),
        trace: caller.trace().unwrap_or_else(gathr::ids::trace_id),
        landed: false,
    };
    let merged = push_merge(chats, &by, project, pr, (&head, &base, &b), &view, &vars).await?;
    drop(one);
    // The others reviewed clean are behind now: the first in line is brought up to date (0049).
    crate::landing::soon(chats, Some(project.id.clone()));
    Ok(merged.said)
}

/// The base's commit on origin, fetched now, or why there's nothing to merge into.
pub async fn fetched(repo: &Path, base: &str, vars: &Vars) -> Result<String, String> {
    let remotes = git(repo, &["remote"], vars).await?;
    if !remotes.lines().any(|r| r == "origin") {
        return Err("the project has no origin to merge into".into());
    }
    git(repo, &["fetch", "--quiet", "origin"], vars).await?;
    let origin = format!("refs/remotes/origin/{base}");
    git(repo, &["rev-parse", &origin], vars).await
}

/// Why the PR needs its reviewer's sign-off, if it does: it brings the project its first passing
/// checks, or it changes tests or check settings.
pub async fn sign_off_for(
    chats: &Arc<Chats>,
    project: &Project,
    b: &str,
    head: &str,
    vars: &Vars,
) -> Result<Vec<String>, String> {
    let repo = &project.folder;
    let mut needs = Vec::new();
    let gathr_checks = matches!(project.ci.as_str(), "gathr" | "both");
    if gathr_checks && first_baseline(chats, &project.id, repo, b, vars).await {
        needs.push("it brings the project's first passing checks on its main".to_string());
    }
    // What the PR itself changes, from where it left its base: a PR behind its base reads the same.
    let since = format!("{b}...{head}");
    let changed = git(repo, &["diff", "--name-only", &since], vars).await?;
    let tests: Vec<&str> = changed.lines().filter(|f| touches_checks(f)).collect();
    if !tests.is_empty() {
        needs.push(format!(
            "it changes tests or check settings: {}",
            tests.join(", ")
        ));
    }
    Ok(needs)
}

/// The merge itself, for whoever's place it's made in, once everything it needs holds at `head`:
/// the PR is read again, the project's method pushes it, and its tasks settle once GitHub shows
/// it merged. An error is a merge that wasn't made.
pub async fn push_merge(
    chats: &Arc<Chats>,
    by: &By,
    project: &Project,
    pr: i64,
    (head, base, b): (&str, &str, &str),
    view: &Value,
    vars: &Vars,
) -> Result<Merged, String> {
    let repo = &project.folder;
    // The head as GitHub has it right before the push: a push made after it moved would merge a
    // commit the PR no longer is (Vale, #120).
    let (_, now, now_base) = open_pr(pr, repo).await?;
    if now != head {
        return Err(format!(
            "#{pr}'s head moved to {} while it was being merged: review the new head",
            short(&now)
        ));
    }
    if now_base != base {
        return Err(format!(
            "#{pr} was retargeted from {base} to {now_base} while it was being merged: merge it again"
        ));
    }
    let merged = match project.method.as_str() {
        "squash" => squash(pr, head, repo).await?,
        "merge" => {
            let tree = format!("{head}^{{tree}}");
            let message = format!("Merge pull request #{pr}");
            let args = ["commit-tree", &tree, "-p", b, "-p", head, "-m", &message];
            let m = git(repo, &args, vars).await?;
            push(repo, &m, base, pr, vars).await?;
            m
        }
        _ => {
            push(repo, head, base, pr, vars).await?;
            head.to_string()
        }
    };
    let trace = &by.trace;
    let event = Event::point("gathr.merge.done")
        .trace(trace)
        .chat(&by.chat)
        .attr("gathr.project.id", project.id.clone())
        .attr("pr", pr)
        .attr("head", head.to_string())
        .attr("method", project.method.clone())
        .attr("merged", merged.clone())
        .attr("landed", by.landed);
    let merged_line = format!(
        "Merged #{pr}'s {} into {base} ({})",
        short(head),
        project.method
    );
    let reference = gathr::ids::reference(trace).to_string();
    // Settled only once GitHub says the PR merged, at the head and base checked: one that moved
    // or was retargeted after the push stays open, and its tasks aren't this merge's (Vale, #120).
    if let Err(seen) = confirmed(pr, repo, head, base).await {
        let why = format!(
            "GitHub doesn't show #{pr} merged at {} into {base}: {seen}",
            short(head)
        );
        chats
            .writer
            .record_now(event.error("unconfirmed").attr("why", why))
            .await;
        let said = format!(
            "{merged_line}, but GitHub doesn't show #{pr} merged ({seen}), so its tasks aren't marked merged: \
             if it moved or was retargeted, its new head needs a review and its checks (ref {reference})."
        );
        return Ok(Merged {
            said,
            confirmed: false,
        });
    }
    let said = match settle(chats, by, pr, head, view).await {
        Ok(()) => {
            chats.writer.record(event.ok());
            format!("{merged_line}: its tasks are settled.")
        }
        Err(why) => {
            chats
                .writer
                .record_now(event.error("settle").attr("why", why.clone()))
                .await;
            format!(
                "{merged_line}, but its tasks weren't settled ({why}): the PR looks settle them once GitHub \
                 reports it merged (ref {reference})."
            )
        }
    };
    Ok(Merged {
        said,
        confirmed: true,
    })
}

pub fn short(commit: &str) -> String {
    commit.get(..7).unwrap_or(commit).to_string()
}

/// The PR as GitHub has it, open: its view, head and base, or why it can't be merged.
pub async fn open_pr(pr: i64, repo: &Path) -> Result<(Value, String, String), String> {
    let fields = "state,headRefOid,baseRefName,statusCheckRollup,headRefName,isCrossRepository";
    let view = crate::coord::github(&["pr", "view", &pr.to_string(), "--json", fields], repo)
        .await
        .map_err(|why| format!("#{pr} wasn't read from GitHub: {why}"))?;
    let view: Value = serde_json::from_str(&view)
        .map_err(|_| format!("GitHub's answer for #{pr} wasn't read"))?;
    let text = |key: &str| view[key].as_str().unwrap_or_default().to_string();
    let (state, head, base) = (text("state"), text("headRefOid"), text("baseRefName"));
    if state != "OPEN" {
        return Err(format!("#{pr} is {}", state.to_lowercase()));
    }
    if head.is_empty() || base.is_empty() {
        return Err(format!("GitHub gave no head or base for #{pr}"));
    }
    Ok((view, head, base))
}

/// GitHub shows the PR merged at `head` into `base`, within a short wait: it marks a pushed
/// merge within a few seconds (Shared/evidence/merge-probe). What it showed last, if not.
async fn confirmed(pr: i64, repo: &Path, head: &str, base: &str) -> Result<(), String> {
    let wait_ms = std::env::var("GATHR_MERGE_CONFIRM_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .unwrap_or(20_000u64);
    let until = std::time::Instant::now() + Duration::from_millis(wait_ms);
    let fields = "state,headRefOid,baseRefName";
    let mut seen = "nothing read".to_string();
    loop {
        let args = ["pr", "view", &pr.to_string(), "--json", fields];
        if let Ok(view) = crate::coord::github(&args, repo).await {
            let view: Value = serde_json::from_str(&view).unwrap_or_default();
            let text = |key: &str| view[key].as_str().unwrap_or_default().to_string();
            let (state, now, now_base) = (text("state"), text("headRefOid"), text("baseRefName"));
            if state == "MERGED" && now == head && now_base == base {
                return Ok(());
            }
            seen = format!(
                "{} at {} into {now_base}",
                state.to_lowercase(),
                short(&now)
            );
        }
        if std::time::Instant::now() >= until {
            return Err(seen);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

/// Whether a review of `reviewed` still covers `head`: nothing came after it but clean merges of
/// the base branch. Gathr's look at a PR asks, by the merge tool's own rule (G165), so a review
/// reads out of date only when the merge would be refused for it. None when git couldn't say (a
/// fetch that fails, a command that errors while another git is at work in the repository): the
/// look asks again next time, and keeps no verdict. Found on macOS CI, where the fetch met the
/// checks run starting at the same head and its failure was kept as "out of date" for good.
pub async fn still_covers(
    repo: &Path,
    reviewed: &str,
    head: &str,
    base: &str,
    vars: &Vars,
) -> Option<bool> {
    git(repo, &["fetch", "--quiet", "origin"], vars)
        .await
        .ok()?;
    let origin = format!("refs/remotes/origin/{base}");
    let base = git(repo, &["rev-parse", &origin], vars).await.ok()?;
    // A head that origin doesn't have (a fork's) is covered by no review here.
    if !known(repo, head, vars).await {
        return Some(false);
    }
    let Some(resolved) = commit_of(repo, reviewed, vars).await else {
        return Some(false);
    };
    if resolved == head {
        return Some(true);
    }
    review::base_updates_only(repo, &resolved, head, &base, vars)
        .await
        .ok()
}

/// The commit a recorded review names, when the project has exactly that one: at least 7
/// characters of it.
async fn commit_of(repo: &Path, reviewed: &str, vars: &Vars) -> Option<String> {
    // Git resolves an id with more than 40 characters to its first 40: only a true prefix counts.
    let hex = (7..=40).contains(&reviewed.len()) && reviewed.chars().all(|c| c.is_ascii_hexdigit());
    let commit = format!("{reviewed}^{{commit}}");
    git(repo, &["rev-parse", "--verify", "--quiet", &commit], vars)
        .await
        .ok()
        .filter(|r| hex && r.starts_with(&reviewed.to_ascii_lowercase()))
}

/// Resolve the recorded review, then prove any later changes came only from the base.
pub async fn reviewed_is_head(
    repo: &Path,
    reviewed: &str,
    head: &str,
    base: &str,
    pr: i64,
    vars: &Vars,
) -> Result<(), String> {
    if reviewed.len() < 7 {
        return Err(format!(
            "the reviewed commit {reviewed} is too short: finish the review with at least 7 characters of it"
        ));
    }
    let resolved = commit_of(repo, reviewed, vars).await.ok_or_else(|| {
        format!(
            "the reviewed commit {reviewed} isn't a commit the project has (or names more than one)"
        )
    })?;
    if resolved != head
        && !review::base_updates_only(repo, &resolved, head, base, vars)
            .await
            .unwrap_or(false)
    {
        return Err(format!(
            "you reviewed {}, and #{pr}'s head is now {}: review it again",
            short(&resolved),
            short(head)
        ));
    }
    Ok(())
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
            let review: Option<(bool, Option<String>, bool)> = c
                .query_row(
                    "SELECT review_done, reviewed, signed_off FROM tasks
                     WHERE chat_id = ?1 AND kind = 'review' AND pr = ?2 AND owner = ?3
                     ORDER BY number DESC LIMIT 1",
                    rusqlite::params![chat, pr, agent],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((done, reviewed, signed_off)) = review else {
                return Ok(Err(format!(
                    "only #{pr}'s reviewer merges it, and you have no review of it here"
                )));
            };
            let Some(reviewed) = reviewed.filter(|_| done) else {
                return Ok(Err(format!(
                    "your review of #{pr} isn't done: finish it on the commit you reviewed"
                )));
            };
            Ok(Ok(Asked {
                project: Project::of(found),
                reviewed,
                signed_off,
            }))
        })
        .await;
    match found {
        Some(Ok(asked)) => asked,
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

impl Project {
    fn of(found: gathr::projects::Project) -> Project {
        Project {
            id: found.id,
            folder: PathBuf::from(found.folder),
            ci: found.ci,
            method: found.merge,
            has_checks: !found.checks.is_empty(),
        }
    }
}

/// The project as its settings stand now, if it's still there.
pub async fn project(chats: &Arc<Chats>, id: &str) -> Option<Project> {
    let id = id.to_string();
    let found = chats
        .writer
        .call(move |c| gathr::projects::get(c, &id))
        .await;
    found?.ok()?.map(Project::of)
}

/// The checks the project asks for, passed on `head`: GitHub's CI, Gathr's (C3), both, or none (a
/// clean review alone, 0045).
async fn checks_passed(
    chats: &Arc<Chats>,
    project: &Project,
    head: &str,
    view: &Value,
) -> Result<(), String> {
    if project.ci == "none" {
        return Ok(());
    }
    let short = head.get(..7).unwrap_or(head);
    if project.ci != "gathr" {
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
    if project.ci == "ci" {
        return Ok(());
    }
    if !project.has_checks {
        return Err("the project has no checks: its Supervisor sets them".into());
    }
    match gathr_run(chats, &project.id, head).await.as_deref() {
        Some("passed") => Ok(()),
        Some("running") => Err(format!("Gathr's checks are still running on {short}")),
        Some("changed") => Err(format!(
            "the project's checks changed since they passed on {short}: run them again"
        )),
        Some(_) => Err(format!(
            "Gathr's checks failed on {short}: see its run for why"
        )),
        None => Err(format!("Gathr's checks haven't run on {short} yet")),
    }
}

/// How the latest run of Gathr's checks at `head` stands: `passed`, `running` or `failed`;
/// `changed` when it passed before the checks were last set; `interrupted` when a stop of the
/// service cut it off, which fails it until the PR looks run that commit again. None when none ran.
pub async fn gathr_run(chats: &Arc<Chats>, project: &str, head: &str) -> Option<String> {
    let (project, h) = (project.to_string(), head.to_string());
    let latest = chats
        .writer
        .call(move |c| -> Result<Option<String>, gathr::Error> {
            Ok(c.query_row(
                "SELECT CASE WHEN r.interrupted THEN 'interrupted'
                        WHEN r.state = 'passed' AND r.started_ts <
                        (SELECT max(set_ts) FROM project_checks WHERE project_id = ?1)
                        THEN 'changed' ELSE r.state END
                 FROM check_runs r WHERE r.project_id = ?1 AND r.head = ?2
                 ORDER BY r.started_ts DESC LIMIT 1",
                rusqlite::params![project, h],
                |r| r.get(0),
            )
            .optional()?)
        })
        .await;
    latest.and_then(Result::ok).flatten()
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
    let answer = crate::coord::github(&args, repo)
        .await
        .map_err(|why| format!("GitHub didn't squash #{pr}: {why}"))?;
    let answer: Value = serde_json::from_str(&answer).unwrap_or_default();
    match (answer["merged"].as_bool(), answer["sha"].as_str()) {
        (Some(true), Some(sha)) => Ok(sha.to_string()),
        _ => Err(format!("GitHub didn't say #{pr} was squashed: {answer}")),
    }
}

/// The PR's tasks in the chat, settled now as a PR look would after its merge (#100's path). Their
/// givers hear back, but never the agent that merged: it knows (G159), so the turn saved to tell
/// it goes with the same step, before anything starts it.
async fn settle(
    chats: &Arc<Chats>,
    by: &By,
    pr: i64,
    head: &str,
    view: &Value,
) -> Result<(), String> {
    let (chat, h) = (by.chat.clone(), head.to_string());
    let merger = by.agent.clone();
    let view = view.clone();
    let trace = by.trace.clone();
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
            crate::cleanup::queue(c, &chat, &view, &trace)?;
            let mut starts = Vec::new();
            for number in numbers {
                starts.extend(tasks::pr_looked(c, number, "MERGED", Some(&h), None, &trace)?);
            }
            // Only the merger's hear-backs go, as marked where they're made: work of its own the
            // merge moved on (a task blocked on the merged one) still starts.
            let mut kept = Vec::new();
            for start in starts {
                if start.hear_back && Some(&start.agent) == merger.as_ref() {
                    c.execute("DELETE FROM messages WHERE turn_id = ?1", [&start.turn])?;
                    c.execute("DELETE FROM turns WHERE id = ?1", [&start.turn])?;
                } else {
                    kept.push(start);
                }
            }
            Ok(kept)
        })
        .await;
    match starts {
        Some(Ok(starts)) => {
            let clean = chats.clone();
            tokio::spawn(async move {
                crate::cleanup::sweep(&clean, None).await;
            });
            chats.start(starts);
            Ok(())
        }
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

pub async fn git(dir: &Path, args: &[&str], vars: &Vars) -> Result<String, String> {
    let ran = gathr::run::run(Path::new("git"), args, dir, vars, GIT_LIMIT).await?;
    match ran.code {
        Some(0) => Ok(ran.out),
        _ => Err(format!("git {} failed: {}", args[0], ran.reason())),
    }
}

pub async fn known(repo: &Path, commit: &str, vars: &Vars) -> bool {
    git(
        repo,
        &["cat-file", "-e", &format!("{commit}^{{commit}}")],
        vars,
    )
    .await
    .is_ok()
}

pub async fn ancestor(repo: &Path, older: &str, newer: &str, vars: &Vars) -> bool {
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
