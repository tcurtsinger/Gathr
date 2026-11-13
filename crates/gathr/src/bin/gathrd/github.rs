//! Gathr's GitHub calls for agents (0046, G185): a PR and its checks, a failed CI run's details, and
//! bringing a PR up to date with its base, made host-side with the service's own `gh` login, so an
//! engine's sandbox needs none. Each runs fixed arguments in the chat's project's own folder (its
//! `origin` is the repository), never one an agent names, and nothing passes a command through.
//! Merging stays `merge` (0043 C4).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use gathr::event::Event;
use rusqlite::OptionalExtension;
use serde_json::Value;

use crate::chats::Chats;
use crate::merging::By;
use crate::tools::Caller;

/// What a run's failed jobs show at most: their logs' last lines, and their first annotations.
const LOG_LINES: usize = 40;
const ANNOTATIONS: usize = 10;
const LINE: usize = 300;
/// How long an update waits for GitHub to show the PR's new head.
const UPDATE_WAIT: Duration = Duration::from_secs(20);

/// The PR as GitHub has it, with each check's state.
pub async fn pr(chats: &Arc<Chats>, caller: &Caller, pr: i64) -> Result<String, String> {
    let repo = project_folder(chats, caller).await?;
    let view = view(pr, &repo).await?;
    let text = |key: &str| view[key].as_str().unwrap_or_default().to_string();
    let mut lines = vec![format!(
        "#{pr} {} · {} · head {} ({}) into {} · mergeable {} · review {}",
        text("title"),
        text("state").to_lowercase(),
        short(&text("headRefOid")),
        text("headRefName"),
        text("baseRefName"),
        text("mergeable").to_lowercase(),
        text("reviewDecision").to_lowercase(),
    )];
    let checks = view["statusCheckRollup"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    lines.push(match checks.len() {
        0 => "No checks on its head.".to_string(),
        n => format!("Checks on its head ({n}):"),
    });
    for check in &checks {
        let name = check["name"]
            .as_str()
            .or(check["context"].as_str())
            .unwrap_or("?");
        let state = check["conclusion"]
            .as_str()
            .filter(|c| !c.is_empty())
            .or(check["state"].as_str())
            .or(check["status"].as_str())
            .unwrap_or("?");
        lines.push(format!("- {name}: {}", state.to_lowercase()));
    }
    record(chats, caller, "pr", pr, &text("headRefOid"));
    Ok(lines.join("\n"))
}

/// Each failed check on the PR's head that ran on GitHub Actions: its log's last lines and its
/// annotations, hidden as Diagnostics hides secrets.
pub async fn failures(chats: &Arc<Chats>, caller: &Caller, pr: i64) -> Result<String, String> {
    let repo = project_folder(chats, caller).await?;
    let view = view(pr, &repo).await?;
    let head = view["headRefOid"].as_str().unwrap_or_default().to_string();
    let failed: Vec<&Value> = view["statusCheckRollup"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|c| outcome(c).is_some())
        .collect();
    if failed.is_empty() {
        record(chats, caller, "failures", pr, &head);
        return Ok(format!(
            "No failed checks on #{pr}'s head {}.",
            short(&head)
        ));
    }
    let mut said = vec![format!("Failed checks on #{pr}'s head {}:", short(&head))];
    for check in failed {
        // A check run names itself and its page; a commit status (`StatusContext`) its context and
        // its target (#162's review).
        let name = check["name"]
            .as_str()
            .or(check["context"].as_str())
            .unwrap_or("?");
        let url = check["detailsUrl"]
            .as_str()
            .or(check["targetUrl"].as_str())
            .unwrap_or_default();
        said.push(String::new());
        said.push(format!(
            "## {name} ({})",
            outcome(check).unwrap_or_default()
        ));
        let Some(job) = job_of(url) else {
            said.push(format!("Not a GitHub Actions job: see {url}"));
            continue;
        };
        let log = gh(&["run", "view", "--job", &job, "--log-failed"], &repo).await;
        said.push(match log {
            Ok(log) => last_lines(&log),
            Err(why) => format!("Its log wasn't read: {why}"),
        });
        let path = format!("repos/{{owner}}/{{repo}}/check-runs/{job}/annotations");
        if let Ok(found) = gh(&["api", &path], &repo).await {
            let found: Value = serde_json::from_str(&found).unwrap_or_default();
            for note in found.as_array().into_iter().flatten().take(ANNOTATIONS) {
                let at = format!(
                    "{}:{}",
                    note["path"].as_str().unwrap_or("?"),
                    note["start_line"].as_i64().unwrap_or(0)
                );
                let message = note["message"].as_str().unwrap_or_default();
                said.push(cut(&format!("annotation {at}: {message}")));
            }
        }
    }
    record(chats, caller, "failures", pr, &head);
    Ok(said.join("\n"))
}

/// Brings the PR up to date with its base, as GitHub's "Update branch" does, for its reviewer: only
/// while its head is still `expected` and it's open into the default branch, so it never updates a
/// head that moved, a closed PR or one retargeted since. Says the head before and after.
pub async fn update(
    chats: &Arc<Chats>,
    caller: &Caller,
    pr: i64,
    expected: &str,
) -> Result<String, String> {
    let repo = project_folder(chats, caller).await?;
    reviewer(chats, caller, pr).await?;
    let by = By {
        chat: caller.chat.clone(),
        agent: caller.agent.clone(),
        trace: caller.trace().unwrap_or_else(gathr::ids::trace_id),
        landed: false,
    };
    let (head, now, base) =
        bring_up_to_date(chats, &repo, &by, pr, expected, update_wait()).await?;
    Ok(match now == head {
        true => format!(
            "Asked GitHub to update #{pr} from {} with {base}: its new head isn't shown yet. Read it again with `pr`.",
            short(&head)
        ),
        false => format!(
            "Updated #{pr} with {base}: head {} is now {}.",
            short(&head),
            short(&now)
        ),
    })
}

/// The update itself, in whoever's place it's made: the reviewer's with `update_pr`, or Gathr's
/// own, landing the PR (0049). Waits up to `wait` for GitHub to show the new head. The head
/// before, the head after (the same one when it isn't shown yet) and the base.
pub async fn bring_up_to_date(
    chats: &Arc<Chats>,
    repo: &Path,
    by: &By,
    pr: i64,
    expected: &str,
    wait: Duration,
) -> Result<(String, String, String), String> {
    let was = view(pr, repo).await?;
    let text = |key: &str| was[key].as_str().unwrap_or_default().to_string();
    let (state, head, base) = (text("state"), text("headRefOid"), text("baseRefName"));
    if state != "OPEN" {
        return Err(format!("#{pr} is {}", state.to_lowercase()));
    }
    let expected = expected.trim().to_ascii_lowercase();
    if expected.len() < 7 || !head.starts_with(&expected) {
        return Err(format!(
            "#{pr}'s head is {}, not {expected}: read it again with `pr` before updating",
            short(&head)
        ));
    }
    let default = gh(&["repo", "view", "--json", "defaultBranchRef"], repo).await?;
    let default: Value = serde_json::from_str(&default).unwrap_or_default();
    let default = default["defaultBranchRef"]["name"]
        .as_str()
        .unwrap_or_default();
    if base != default {
        return Err(format!(
            "#{pr} is into {base}, not the default branch {default}"
        ));
    }
    // GitHub pins the update to the head (`expected_head_sha`), not to the PR's state or base: read
    // again just before the write, and check what came back after (#162's review). A retarget in the
    // instant between this read and the write can't be refused; it's caught in what came back.
    let again = view(pr, repo).await?;
    let now_text = |key: &str| again[key].as_str().unwrap_or_default().to_string();
    if now_text("state") != "OPEN"
        || now_text("headRefOid") != head
        || now_text("baseRefName") != default
    {
        return Err(changed(pr, &again, default));
    }
    let path = format!("repos/{{owner}}/{{repo}}/pulls/{pr}/update-branch");
    let pinned = format!("expected_head_sha={head}");
    gh(&["api", "-X", "PUT", &path, "-f", &pinned], repo).await?;
    let until = std::time::Instant::now() + wait;
    let mut now = head.clone();
    let mut last = again;
    while now == head && std::time::Instant::now() < until {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(seen) = view(pr, repo).await {
            now = seen["headRefOid"].as_str().unwrap_or_default().to_string();
            last = seen;
        }
    }
    let still = last["state"] == "OPEN" && last["baseRefName"].as_str() == Some(default);
    if !still {
        return Err(changed(pr, &last, default));
    }
    let event = Event::point("gathr.github.update")
        .trace(&by.trace)
        .chat(&by.chat)
        .ok()
        .attr("pr", pr)
        .attr("requested_head", head.clone())
        .attr("returned_head", now.clone())
        .attr("landed", by.landed)
        .attr("gen_ai.agent.id", by.agent.clone());
    chats.writer.record(event);
    Ok((head, now, base))
}

/// The PR isn't as the update needs it: open, at the head given, into the default branch.
fn changed(pr: i64, seen: &Value, default: &str) -> String {
    let text = |key: &str| seen[key].as_str().unwrap_or_default().to_string();
    format!(
        "#{pr} changed while it was updated: it's {} at {} into {}, not open into {default}. Read it again with `pr`",
        text("state").to_lowercase(),
        short(&text("headRefOid")),
        text("baseRefName")
    )
}

/// `GATHR_GITHUB_UPDATE_WAIT_MS` shortens the wait for a new head, in dev and tests only.
fn update_wait() -> Duration {
    let shorter = cfg!(debug_assertions)
        .then(|| {
            std::env::var("GATHR_GITHUB_UPDATE_WAIT_MS")
                .ok()?
                .parse()
                .ok()
        })
        .flatten();
    shorter.map_or(UPDATE_WAIT, Duration::from_millis)
}

async fn view(pr: i64, repo: &std::path::Path) -> Result<Value, String> {
    let fields = "number,title,state,headRefOid,headRefName,baseRefName,mergeable,reviewDecision,statusCheckRollup";
    let view = gh(&["pr", "view", &pr.to_string(), "--json", fields], repo).await?;
    serde_json::from_str(&view).map_err(|_| format!("GitHub's answer for #{pr} wasn't read"))
}

/// `gh` with fixed arguments in the project's folder, its failure said in words an agent can act on.
async fn gh(args: &[&str], repo: &std::path::Path) -> Result<String, String> {
    crate::coord::github(args, repo)
        .await
        .map_err(|why| said(&why))
}

/// Why a call failed, told apart: Gathr's own GitHub sign-in, or this account's reach.
fn said(why: &str) -> String {
    let low = why.to_ascii_lowercase();
    let signed_out = [
        "401",
        "bad credentials",
        "gh auth login",
        "not logged",
        "authentication",
        "gh_token",
    ];
    let no_reach = [
        "404",
        "403",
        "could not resolve to a repository",
        "not accessible",
        "not found",
    ];
    if signed_out.iter().any(|w| low.contains(w)) {
        format!(
            "GitHub sign-in isn't available to Gathr: the person signs in with `gh auth login` on this computer ({why})"
        )
    } else if no_reach.iter().any(|w| low.contains(w)) {
        format!("this GitHub account can't reach the project's repository ({why})")
    } else {
        format!("GitHub didn't answer: {why}")
    }
}

/// The chat's project's folder: the repository Gathr asks GitHub about.
async fn project_folder(chats: &Arc<Chats>, caller: &Caller) -> Result<PathBuf, String> {
    let chat = caller.chat.clone();
    let found = chats
        .writer
        .call(move |c| -> Result<Option<String>, gathr::Error> {
            let project: Option<String> = c
                .query_row("SELECT project_id FROM chats WHERE id = ?1", [&chat], |r| {
                    r.get(0)
                })
                .optional()?
                .flatten();
            let Some(project) = project else {
                return Ok(None);
            };
            Ok(gathr::projects::get(c, &project)?.map(|p| p.folder))
        })
        .await;
    match found {
        Some(Ok(Some(folder))) => Ok(PathBuf::from(folder)),
        Some(Ok(None)) => Err("this chat isn't in a project, so it has no repository".into()),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

/// The caller owns a review of the PR in the chat: only its reviewer updates it.
async fn reviewer(chats: &Arc<Chats>, caller: &Caller, pr: i64) -> Result<(), String> {
    let (chat, agent) = (
        caller.chat.clone(),
        caller.agent.clone().unwrap_or_default(),
    );
    let found = chats
        .writer
        .call(move |c| {
            c.query_row(
                "SELECT EXISTS (SELECT 1 FROM tasks WHERE chat_id = ?1 AND kind = 'review'
                     AND pr = ?2 AND owner = ?3)",
                rusqlite::params![chat, pr, agent],
                |r| r.get::<_, bool>(0),
            )
        })
        .await;
    match found {
        Some(Ok(true)) => Ok(()),
        Some(Ok(false)) => Err(format!(
            "only #{pr}'s reviewer updates it, and you have no review of it here"
        )),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}

/// How a check failed, if it did: a check run by its conclusion, a commit status by its state.
fn outcome(check: &Value) -> Option<String> {
    let failed = |word: &str| {
        matches!(
            word,
            "FAILURE" | "ERROR" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE"
        )
    };
    [&check["conclusion"], &check["state"]]
        .iter()
        .filter_map(|v| v.as_str())
        .find(|w| failed(w))
        .map(str::to_lowercase)
}

/// The job id in an Actions check's link (`…/actions/runs/<run>/job/<job>`).
fn job_of(url: &str) -> Option<String> {
    let (_, job) = url.split_once("/job/")?;
    let job: String = job.chars().take_while(char::is_ascii_digit).collect();
    (!job.is_empty()).then_some(job)
}

/// A failed job's log, as `gh run view --log-failed` gives it, up to its last error: the steps after
/// it (artifact upload, cleanup) say nothing of why it failed. Each line loses its job, step and
/// time (`<job>\t<step>\t<time> `), seen live (#131's Rust jobs).
fn last_lines(log: &str) -> String {
    let hidden = gathr::redact::redact(log);
    let mut lines: Vec<&str> = hidden
        .lines()
        .map(|l| l.rsplit('\t').next().unwrap_or(l))
        .map(|l| match l.split_once(' ') {
            Some((time, rest)) if time.len() > 20 && time.ends_with('Z') && time.contains('T') => {
                rest
            }
            _ => l,
        })
        .filter(|l| !l.trim().is_empty())
        .collect();
    if let Some(last) = lines.iter().rposition(|l| l.contains("##[error]")) {
        lines.truncate(last + 1);
    }
    let kept = &lines[lines.len().saturating_sub(LOG_LINES)..];
    let mut said: Vec<String> = kept.iter().map(|l| cut(l)).collect();
    if lines.len() > kept.len() {
        said.insert(
            0,
            format!("(the last {} of {} lines)", kept.len(), lines.len()),
        );
    }
    said.join("\n")
}

fn cut(line: &str) -> String {
    gathr::text::cut(&gathr::redact::redact(line), LINE, false)
}

fn short(commit: &str) -> String {
    commit.get(..7).unwrap_or(commit).to_string()
}

/// Each call, without what GitHub said: the operation, the PR and its head (0011).
fn record(chats: &Arc<Chats>, caller: &Caller, op: &str, pr: i64, head: &str) {
    let event = Event::point("gathr.github.read")
        .trace(&caller.trace().unwrap_or_else(gathr::ids::trace_id))
        .chat(&caller.chat)
        .ok()
        .attr("op", op)
        .attr("pr", pr)
        .attr("head", head.to_string())
        .attr("gen_ai.agent.id", caller.agent.clone());
    chats.writer.record(event);
}
