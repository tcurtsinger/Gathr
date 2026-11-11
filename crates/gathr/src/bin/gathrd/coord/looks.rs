//! One look at everything the service waits on itself: expired waits, agents now free, CI, PRs,
//! checks, and tasks' PRs. GitHub is asked through `gh`, each call tried once more on a failure
//! (G5); a check's command runs in the agent's folder, in Git Bash on Windows.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use gathr::ids::{now_us, trace_id};
use gathr::platform::{
    self,
    env::{self, Vars},
};
use gathr::run::run;
use gathr::waits::ci::{self, Seen, Target};
use gathr::waits::{self, Wait};
use gathr::{tasks, worktree};
use serde_json::Value;

use super::apply;
use crate::chats::Chats;

const LIMIT: Duration = Duration::from_secs(60);
/// The most of what a met check printed that its agent is told.
const PRINTED_MOST: usize = 300;

/// What one look found: met, with the line its agent is woken with; not yet; or why it failed.
enum Look {
    Met(String),
    NotYet { head: Option<(String, i64)> },
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
    crate::cleanup::sweep(chats, None).await;
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
        let args = ["pr", "view", &pr.to_string(), "--json", "state,headRefOid"];
        let Ok(answer) = gh(&args, &dir, &vars).await else {
            continue;
        };
        let state = json_text(&answer, "state");
        let head = json_text(&answer, "headRefOid");
        let t = trace.clone();
        apply(chats, "gathr.task.pr", &trace, move |c| {
            tasks::pr_looked(
                c,
                task,
                &state,
                Some(head.as_str()).filter(|h| !h.is_empty()),
                &t,
            )
        })
        .await;
    }
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

/// Records how the look went: a met wait fires; a failure counts, and shows from the third in a
/// row, with a reference (0010, 0019).
async fn settle(chats: &Arc<Chats>, wait: Wait, looked: Result<Look, String>) {
    let trace = trace_id();
    let id = wait.id.clone();
    match looked {
        Ok(Look::Met(line)) => {
            let t = trace.clone();
            apply(chats, "gathr.wait.fire", &trace, move |c| {
                waits::met(c, &id, &line, &t)
            })
            .await;
        }
        Ok(Look::NotYet { head }) => {
            let seen = move |c: &mut rusqlite::Connection| {
                let head = head.as_ref().map(|(h, ts)| (h.as_str(), *ts));
                waits::looked(c, &id, head, None)
            };
            let _ = chats.writer.call(seen).await;
        }
        Err(why) => {
            let error = why.clone();
            let failures = chats
                .writer
                .call(move |c| waits::looked(c, &id, None, Some(&error)))
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
                _ => Look::NotYet { head: None },
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

/// An up-to-date wait (0036 C12): met once the PR has every commit its base has, or when it's
/// merged or closed (nothing left to keep up to date); else not yet.
fn up_to_date(pr: &str, state: &str, base: &str, behind: Option<u64>) -> Look {
    match (state, behind) {
        ("MERGED", _) => Look::Met(format!("#{pr} was merged")),
        ("CLOSED", _) => Look::Met(format!("#{pr} was closed without merging")),
        (_, Some(0)) => Look::Met(format!("#{pr} is up to date with {base}")),
        _ => Look::NotYet { head: None },
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
    Ok(match ci::verdict(&seen) {
        (true, line) => Look::Met(line),
        (false, _) => Look::NotYet {
            head: Some((head, since)),
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
        // Let through unasked (C11): run as the very words that were checked, with no shell to
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
    if ran.code != Some(0) {
        return Ok(Look::NotYet { head: None });
    }
    let printed: String = ran.out.chars().take(PRINTED_MOST).collect();
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

/// The person's environment now (G23), as engines and git get it.
pub async fn fresh() -> Vars {
    tokio::task::spawn_blocking(|| env::fresh(std::env::vars_os().collect()).vars)
        .await
        .unwrap_or_else(|_| std::env::vars_os().collect())
}

/// Runs `gh`, once more if it fails (G5): what it printed, or why it failed.
pub(super) async fn gh(args: &[&str], dir: &Path, vars: &Vars) -> Result<String, String> {
    let mut why = String::new();
    for _ in 0..2 {
        match run(Path::new("gh"), args, dir, vars, LIMIT).await {
            Ok(ran) if ran.code == Some(0) => return Ok(ran.out),
            Ok(ran) => why = format!("gh {} failed: {}", args[0], ran.reason()),
            Err(e) => why = e,
        }
    }
    Err(why)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 0036 C12: met when the base's commits are all in the PR, not before; a merged or closed PR
    /// has nothing left to keep up to date.
    #[test]
    fn an_up_to_date_wait_is_met_once_the_base_is_in_the_pr() {
        let met = |look: Look| matches!(look, Look::Met(_));
        assert!(
            !met(up_to_date("7", "OPEN", "main", Some(2))),
            "behind by 2"
        );
        assert!(
            met(up_to_date("7", "OPEN", "main", Some(0))),
            "the base moved into it"
        );
        assert!(met(up_to_date("7", "MERGED", "main", None)));
        assert!(met(up_to_date("7", "CLOSED", "main", None)));
        let Look::Met(said) = up_to_date("7", "OPEN", "main", Some(0)) else {
            unreachable!()
        };
        assert_eq!(said, "#7 is up to date with main");
    }

    /// Vale, #116: git takes a filter from places outside the tree (here `.git/info/attributes`)
    /// and runs it on `git status`, so `git status` asks; a read-only check runs as its checked
    /// words, not through `sh`, and runs nothing the repository names.
    #[tokio::test]
    async fn a_read_only_check_runs_nothing_the_repository_names() {
        let root = std::env::temp_dir().join(format!("gathr-c11-run-{}", gathr::ids::span_id()));
        std::fs::create_dir_all(&root).unwrap();
        let vars: Vars = env::fresh(std::env::vars_os().collect()).vars;
        let git = |args: &[&str]| {
            let mut command = std::process::Command::new("git");
            command
                .args(args)
                .current_dir(&root)
                .env_clear()
                .envs(vars.iter().cloned());
            assert!(command.output().unwrap().status.success(), "git {args:?}");
        };
        git(&["init", "-q"]);
        for (key, value) in [
            ("user.name", "t"),
            ("user.email", "t@example.invalid"),
            ("core.autocrlf", "false"),
            ("core.fsmonitor", "echo > fsmonitor; exit 1"),
            ("filter.x.clean", "echo > filter; cat"),
        ] {
            git(&["config", key, value]);
        }
        std::fs::write(root.join("a.txt"), "a\n").unwrap();
        git(&["add", "."]);
        git(&["commit", "-qm", "a"]);
        std::fs::write(root.join(".git/info/attributes"), "*.txt filter=x\n").unwrap();
        let marks = || ["fsmonitor", "filter"].map(|m| root.join(m).exists());
        let change = |text: &str| {
            for mark in ["fsmonitor", "filter"] {
                let _ = std::fs::remove_file(root.join(mark));
            }
            // The same size, a second later: only reading the file tells git it changed.
            std::thread::sleep(Duration::from_millis(1100));
            std::fs::write(root.join("a.txt"), text).unwrap();
        };
        change("b\n");
        let looked = check("git log -1 --oneline", &root, &vars).await;
        let read_only = marks();
        change("c\n");
        git(&["status", "--porcelain"]);
        let status = marks();
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(
            status,
            [true, true],
            "git status didn't run them: the test proves nothing"
        );
        assert_eq!(
            gathr::approvals::read_only_check("git status --porcelain"),
            None
        );
        assert!(matches!(looked, Ok(Look::Met(_))), "{:?}", looked.err());
        assert_eq!(
            read_only,
            [false, false],
            "a read-only check ran a program the repository names"
        );
    }
}
