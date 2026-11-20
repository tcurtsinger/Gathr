//! One look at everything the service waits on itself: expired waits, agents now free, CI, PRs,
//! checks, and tasks' PRs. GitHub is asked through `gh`, each call tried once more on a failure
//! (G5); a check's command runs in the agent's folder, in Git Bash on Windows.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use gathr::ids::{now_us, trace_id};
use gathr::platform::{self, env::Vars};
use gathr::run::{fresh, gh, run};
use gathr::waits::ci::{self, Seen, Target};
use gathr::waits::{self, Wait};
use gathr::{tasks, worktree};
use serde_json::Value;

use super::apply;
use crate::chats::Chats;

const LIMIT: Duration = Duration::from_secs(60);
/// The most of what a met check printed that its agent is told.
const PRINTED_MOST: usize = 300;

/// What one look found: met, with the line its agent is woken with; CI met with a failure, with
/// the commit it failed at too; not yet, with what it saw instead; or why it failed.
enum Look {
    Met(String),
    Red(String, String),
    NotYet {
        head: Option<(String, i64)>,
        seen: String,
    },
}

fn not_yet(seen: String) -> Look {
    Look::NotYet { head: None, seen }
}

/// A day for the stale rules (F11): `GATHR_STALE_DAY_MS` shortens it, in dev and tests only.
fn day_us() -> i64 {
    let ms = std::env::var("GATHR_STALE_DAY_MS")
        .ok()
        .and_then(|ms| ms.parse::<i64>().ok());
    ms.map_or(86_400_000_000, |ms| ms * 1000)
}

pub async fn once(chats: &Arc<Chats>) {
    let trace = trace_id();
    let now = now_us();
    let t = trace.clone();
    apply(chats, "gathr.wait.expire", &trace, move |c| {
        waits::expire(c, now, &t)
    })
    .await;
    let t = trace.clone();
    apply(chats, "gathr.wait.agent", &trace, move |c| {
        let mut starts = Vec::new();
        for (chat, agent) in waits::waited_on(c)? {
            starts.extend(waits::agent_free(c, &chat, &agent, &t)?);
        }
        Ok(starts)
    })
    .await;
    let (t, paused, day) = (trace.clone(), chats.pauses(), day_us());
    apply(chats, "gathr.task.stale", &trace, move |c| {
        Ok(tasks::stale::sweep(c, now, day, &paused, &t)?.starts)
    })
    .await;
    // At every look, whether or not a wait or a PR is open (0052).
    super::quiet::look(chats).await;
    crate::cleanup::sweep(chats, None).await;
    crate::check_runs::sweep(chats, day_us()).await;
    let found = chats.writer.call(|c| waits::to_look_at(c)).await;
    let prs = chats.writer.call(|c| tasks::with_prs(c)).await;
    let (Some(Ok(found)), Some(Ok(prs))) = (found, prs) else {
        return;
    };
    if found.is_empty() && prs.is_empty() {
        return;
    }
    let vars = fresh().await;
    for wait in found {
        // A pause holds what the service would start on its own in its scope, a check's command too.
        if gathr::pauses::holding(&chats.pauses(), &wait.chat, Some(&wait.agent)).is_some() {
            continue;
        }
        // Archive, or its task's cancel, may have ended it since the list was read.
        if !open(chats, &wait.id).await {
            continue;
        }
        let checking = (wait.kind == "check").then(|| (wait.chat.clone(), wait.agent.clone()));
        if let Some((chat, agent)) = &checking {
            chats.check_running(chat, agent, true).await;
        }
        // Ended while it's looked at: its check's command is killed as the look is dropped.
        let looked = tokio::select! {
            looked = look(&wait, chats.data.path(), &vars) => Some(looked),
            () = ended(chats, &wait.id) => None,
        };
        if let Some(looked) = looked {
            settle(chats, wait, looked).await;
        }
        if let Some((chat, agent)) = &checking {
            chats.check_running(chat, agent, false).await;
        }
    }
    for (task, chat, owner, pr) in prs {
        let dir = match worktree_of(chats, &chat, &owner).await {
            Some(folder) if folder.exists() => folder,
            _ => project_of(chats, &chat).await,
        };
        let args = [
            "pr",
            "view",
            &pr.to_string(),
            "--json",
            "state,headRefOid,headRefName,baseRefName,isCrossRepository",
        ];
        let Ok(answer) = gh(&args, &dir, &vars).await else {
            continue;
        };
        let state = json_text(&answer, "state");
        let head = json_text(&answer, "headRefOid");
        // Judged before the checks start: both use git in the same repository.
        let base = json_text(&answer, "baseRefName");
        let seen = match state.as_str() {
            "OPEN" => judged(chats, task, &head, &base, &dir).await,
            _ => None,
        };
        // A review's PR: Gathr runs the project's checks at each head it sees (0043 C3).
        if state == "OPEN" && !head.is_empty() {
            crate::check_runs::for_review(chats, &chat, task, &head).await;
        }
        let view: Value = serde_json::from_str(&answer).unwrap_or_default();
        let ended = matches!(state.as_str(), "MERGED" | "CLOSED");
        let (t, on, was, at) = (trace.clone(), chat.clone(), state.clone(), head.clone());
        apply(chats, "gathr.task.pr", &trace, move |c| {
            if ended {
                crate::cleanup::queue(c, &chat, &view, &t)?;
            }
            tasks::pr_looked(
                c,
                task,
                &state,
                Some(head.as_str()).filter(|h| !h.is_empty()),
                seen.as_ref().map(|(of, covers)| (of.as_str(), *covers)),
                &t,
            )
        })
        .await;
        if ended {
            let at = Some(at.as_str()).filter(|h| !h.is_empty());
            crate::changes::ended(chats, &on, pr, &was, at, false, &trace).await;
        }
    }
    // Every PR whose newest review is done is among the tasks' PRs above (0049).
    crate::landing::sweep(chats.clone(), None).await;
    crate::cleanup::sweep(chats, None).await;
}

/// The look's verdict on the task's review, for the commit reviewed now and the PR at `head`:
/// that commit, and whether a review of it still covers the head. Asked of git once for each such
/// pair and kept (`judged`), so a review recorded again or a head that moves is judged again (0047).
async fn judged(
    chats: &Arc<Chats>,
    task: i64,
    head: &str,
    base: &str,
    dir: &Path,
) -> Option<(String, bool)> {
    let read = chats.writer.call(move |c| tasks::get(c, task)).await;
    let t = read?.ok()??;
    let reviewed = t.reviewed.filter(|_| !head.is_empty())?;
    if t.judged.as_deref() == Some(format!("{reviewed}:{head}").as_str()) {
        return Some((reviewed, !t.outdated));
    }
    let vars = fresh().await;
    // No verdict when git couldn't say: nothing is kept, and the next look asks again.
    let covers = crate::merging::still_covers(dir, &reviewed, head, base, &vars).await?;
    Some((reviewed, covers))
}

/// Whether the wait is still open: a wait that can't be read isn't.
async fn open(chats: &Arc<Chats>, id: &str) -> bool {
    let id = id.to_string();
    let read = chats.writer.call(move |c| waits::read(c, &id)).await;
    matches!(read, Some(Ok(w)) if w.state == "open")
}

/// Returns once the wait is no longer open.
async fn ended(chats: &Arc<Chats>, id: &str) {
    while open(chats, id).await {
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn met(chats: &Arc<Chats>, trace: &str, id: String, line: String, red: Option<String>) {
    let t = trace.to_string();
    apply(chats, "gathr.wait.fire", trace, move |c| {
        waits::met(c, &id, &line, red.as_deref(), &t)
    })
    .await;
}

/// Records how the look went: a met wait fires; a failure counts, and shows from the third in a
/// row, with a reference (0010, 0019).
async fn settle(chats: &Arc<Chats>, wait: Wait, looked: Result<Look, String>) {
    let trace = trace_id();
    let id = wait.id.clone();
    match looked {
        Ok(Look::Met(line)) => met(chats, &trace, id, line, None).await,
        Ok(Look::Red(line, head)) => met(chats, &trace, id, line, Some(head)).await,
        Ok(Look::NotYet { head, seen }) => {
            let save = move |c: &mut rusqlite::Connection| {
                let head = head.as_ref().map(|(h, ts)| (h.as_str(), *ts));
                waits::looked(c, &id, head, Some(&seen), None)
            };
            let _ = chats.writer.call(save).await;
        }
        Err(why) => {
            let error = why.clone();
            let failures = chats
                .writer
                .call(move |c| waits::looked(c, &id, None, None, Some(&error)))
                .await;
            if let Some(Ok(failures)) = failures
                && failures == waits::SHOWN_AFTER
            {
                let event = Event::point("gathr.wait.failing")
                    .trace(&trace)
                    .chat(&wait.chat);
                let event = event.error("wait_look").attr("why", why);
                let event = event
                    .attr("gathr.wait.id", wait.id)
                    .attr("gathr.wait.failures", failures);
                chats.writer.record(match wait.task {
                    Some(task) => event.task(task),
                    None => event,
                });
            }
        }
    }
}

async fn look(wait: &Wait, data: &Path, vars: &Vars) -> Result<Look, String> {
    let dir = wait
        .cwd
        .as_deref()
        .map_or(data.to_path_buf(), PathBuf::from);
    let target = wait.target.as_deref().unwrap_or_default();
    let repo = wait.repo.as_deref();
    match wait.kind.as_str() {
        // In the agent's own folder only, never the service's.
        "check" => match &wait.cwd {
            Some(cwd) => check(target, Path::new(cwd), vars).await,
            None => Err("a check runs in its agent's folder, and this wait has none".into()),
        },
        "pr" => {
            let answer = gh(&pr_view(target, "state", repo), &dir, vars).await?;
            Ok(match json_text(&answer, "state").as_str() {
                "MERGED" => Look::Met(format!("#{target} was merged")),
                "CLOSED" => Look::Met(format!("#{target} was closed without merging")),
                _ => not_yet(format!("#{target} is open")),
            })
        }
        "up_to_date" => {
            let view = gh(
                &pr_view(target, "state,headRefOid,baseRefName", repo),
                &dir,
                vars,
            )
            .await?;
            let (state, head, base) = (
                json_text(&view, "state"),
                json_text(&view, "headRefOid"),
                json_text(&view, "baseRefName"),
            );
            if state != "OPEN" {
                return Ok(up_to_date(target, &state, &base, None));
            }
            if head.is_empty() || base.is_empty() {
                return Err(format!("GitHub gave no head or base for #{target}"));
            }
            // How far the head is behind its base, as GitHub compares them.
            let path = format!(
                "repos/{}/compare/{base}...{head}",
                repo.unwrap_or("{owner}/{repo}")
            );
            let compared = gh(&["api", &path, "--jq", ".behind_by"], &dir, vars).await?;
            let behind = compared
                .trim()
                .parse()
                .map_err(|_| format!("GitHub didn't say how far #{target} is behind"))?;
            Ok(up_to_date(target, &state, &base, Some(behind)))
        }
        _ => look_ci(wait, &Target::parse(target)?, &dir, vars).await,
    }
}

/// An up-to-date wait (0043 C12): met once the PR has every commit its base has, or when it's
/// merged or closed (nothing left to keep up to date); else not yet.
fn up_to_date(pr: &str, state: &str, base: &str, behind: Option<u64>) -> Look {
    match (state, behind) {
        ("MERGED", _) => Look::Met(format!("#{pr} was merged")),
        ("CLOSED", _) => Look::Met(format!("#{pr} was closed without merging")),
        (_, Some(0)) => Look::Met(format!("#{pr} is up to date with {base}")),
        (_, Some(1)) => not_yet(format!("#{pr} is 1 commit behind {base}")),
        (_, Some(n)) => not_yet(format!("#{pr} is {n} commits behind {base}")),
        (_, None) => not_yet(format!("#{pr} is {}", state.to_lowercase())),
    }
}

async fn look_ci(wait: &Wait, target: &Target, dir: &Path, vars: &Vars) -> Result<Look, String> {
    let repo = wait.repo.as_deref();
    let (head, state) = match target {
        Target::Pr(n) => {
            let answer = gh(
                &pr_view(&n.to_string(), "headRefOid,state", repo),
                dir,
                vars,
            )
            .await?;
            let head = json_text(&answer, "headRefOid");
            if head.is_empty() {
                return Err(format!("GitHub gave no commit for #{n}"));
            }
            (head, Some(json_text(&answer, "state")))
        }
        Target::Commit(commit) => {
            if let Some(line) = moved_past(commit, repo, dir, vars).await? {
                return Ok(Look::Met(line));
            }
            (commit.clone(), None)
        }
    };
    let now = now_us();
    // The no-checks grace starts again when the head moves: a push restarts CI.
    let since = match &wait.head {
        Some(seen) if *seen == head => wait.head_ts.unwrap_or(wait.created_ts),
        Some(_) => now,
        None => wait.created_ts,
    };
    let repo = repo.unwrap_or("{owner}/{repo}");
    let runs = format!("repos/{repo}/commits/{head}/check-runs?per_page=100");
    let statuses = format!("repos/{repo}/commits/{head}/status?per_page=100");
    let runs = [
        "api",
        "--paginate",
        &runs,
        "--jq",
        ".check_runs[] | {name, status, conclusion}",
    ];
    let statuses = [
        "api",
        "--paginate",
        &statuses,
        "--jq",
        ".statuses[] | {context, state}",
    ];
    let (runs, statuses) = tokio::join!(gh(&runs, dir, vars), gh(&statuses, dir, vars));
    let checks = ci::checks_of(&runs?, &statuses?)?;
    let what = target.name();
    let seen = Seen {
        what: &what,
        head: &head,
        state: state.as_deref(),
        checks: &checks,
        since_us: since,
        now_us: now,
    };
    let red = checks.iter().any(|c| c.stand == ci::Stand::Failed);
    Ok(match ci::verdict(&seen) {
        (true, line) if red => Look::Red(line, head),
        (true, line) => Look::Met(line),
        (false, seen) => Look::NotYet {
            head: Some((head, since)),
            seen,
        },
    })
}

/// A commit an open PR has moved past: that wait would wake on an old head (G69).
async fn moved_past(
    commit: &str,
    repo: Option<&str>,
    dir: &Path,
    vars: &Vars,
) -> Result<Option<String>, String> {
    let repo = repo.unwrap_or("{owner}/{repo}");
    let path = format!("repos/{repo}/commits/{commit}/pulls");
    let jq = r#".[] | select(.state == "open") | {number, head: .head.sha}"#;
    let answer = gh(&["api", &path, "--jq", jq], dir, vars).await?;
    for line in answer.lines().filter(|l| !l.trim().is_empty()) {
        let pr: Value = serde_json::from_str(line).map_err(|e| e.to_string())?;
        let head = pr["head"].as_str().unwrap_or_default();
        if !head.is_empty() && !head.starts_with(commit) {
            let short = |c: &str| c.get(..7).unwrap_or(c).to_string();
            return Ok(Some(format!(
                "PR #{} moved from commit {} to {}. Wait on #{}'s CI instead.",
                pr["number"],
                short(commit),
                short(head),
                pr["number"]
            )));
        }
    }
    Ok(None)
}

/// A check's command: exit 0 is met; any other exit is "not yet" (G69); one that can't run or
/// finish is a failed look.
async fn check(command: &str, dir: &Path, vars: &Vars) -> Result<Look, String> {
    let ran = match gathr::approvals::read_only_check(command) {
        // Let through unasked (0043 C11): run as the very words that were checked, with no shell to
        // expand them, and with what the repository names switched off.
        Some(_) => {
            let words: Vec<&str> = command.split_whitespace().collect();
            let mut vars = vars.clone();
            let guard = gathr::approvals::read_only_env().into_iter();
            vars.extend(guard.map(|(k, v)| (k.into(), v.into())));
            run(Path::new(words[0]), &words[1..], dir, &vars, LIMIT).await?
        }
        None => {
            let (shell, vars) = platform::shell(vars).ok_or("Git Bash wasn't found beside git")?;
            gathr::run::script(&shell, command, dir, &vars, LIMIT).await?
        }
    };
    // Kept on the wait and said in its wake: never with a secret the command printed. Hidden
    // before the cut: a key the cut shortens is no longer known for one.
    let printed = gathr::redact::redact(&ran.out);
    let printed: String = printed.chars().take(PRINTED_MOST).collect();
    if ran.code != Some(0) {
        let exit = ran
            .code
            .map_or("with no exit code".to_string(), |c| c.to_string());
        return Ok(not_yet(match printed.trim().is_empty() {
            true => format!("the check exited {exit}"),
            false => format!("the check exited {exit}: \"{}\"", printed.trim()),
        }));
    }
    Ok(Look::Met(match printed.is_empty() {
        true => "Check passed".to_string(),
        false => format!("Check passed: \"{printed}\""),
    }))
}

fn pr_view<'a>(number: &'a str, fields: &'a str, repo: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec!["pr", "view", number, "--json", fields];
    if let Some(repo) = repo {
        args.extend(["--repo", repo]);
    }
    args
}

fn json_text(answer: &str, key: &str) -> String {
    let value: Value = serde_json::from_str(answer).unwrap_or_default();
    value[key].as_str().unwrap_or_default().to_string()
}

/// The agent's worktree in the chat: as recorded (0034), else where worktrees were made before.
async fn worktree_of(chats: &Arc<Chats>, chat: &str, agent: &str) -> Option<PathBuf> {
    let (c, a) = (chat.to_string(), agent.to_string());
    let read = chats
        .writer
        .call(move |conn| gathr::folders::workspace(conn, &c, &a));
    match read.await {
        Some(Ok(Some(folder))) => Some(folder),
        Some(Ok(None)) => Some(worktree::old_folder(chats.data.path(), chat, agent)),
        _ => None,
    }
}

/// The chat's project folder, or the data folder when it has none.
async fn project_of(chats: &Arc<Chats>, chat: &str) -> PathBuf {
    let chat = chat.to_string();
    let found = chats
        .writer
        .call(move |c| {
            c.query_row("SELECT project FROM chats WHERE id = ?1", [chat], |r| {
                r.get::<_, Option<String>>(0)
            })
        })
        .await;
    match found {
        Some(Ok(Some(project))) => PathBuf::from(project),
        _ => chats.data.path().to_path_buf(),
    }
}
