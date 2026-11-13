//! Gathr lands a pull request (0049). A clean review is the word to land: once the newest review of
//! a PR in a project's chat is done on a commit that covers its head, the PR is into the default
//! branch and up to date with it, what the project asks for passed there and no CI run is still
//! going, Gathr merges it with the merge tool's own code, in its reviewer's place. It looks when a
//! review is recorded done, when a checks run ends, after each merge, and at each of the service's
//! looks. A PR behind its base is brought up to date, the first in line only; whatever else stops
//! one wakes its reviewer on no task, once for that head.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::Duration;

use gathr::event::Event;
use gathr::ids::{now_us, trace_id};
use gathr::landings::{self, Act, Review, Stand};
use gathr::platform::env::Vars;
use gathr::waits::ci;
use serde_json::Value;

use crate::chats::Chats;
use crate::merging::{self, By, Project, short};
use crate::tools::Caller;

/// How long CI gets to start on a head Gathr's own update made, before no checks reads as none.
const CI_STARTS_US: i64 = 120 * 1_000_000;
/// How long a reviewer's report waits for another look at its project's PRs to end.
const TURN: Duration = Duration::from_secs(15);

/// Why the last look at each PR failed, so a look that keeps failing is recorded once.
static FAILED: LazyLock<Mutex<HashMap<(String, i64), String>>> = LazyLock::new(Default::default);

/// What one look at a PR did or saw, in words, and whether the PR keeps its place in line: it
/// lands by itself, so one reviewed after it isn't brought up to date yet.
struct Looked {
    said: String,
    in_line: bool,
    /// It merged, so the project's other PRs are behind now.
    merged: bool,
}

impl Looked {
    fn of(said: String, in_line: bool) -> Looked {
        Looked {
            said,
            in_line,
            merged: false,
        }
    }
}

/// The PR as a look found it.
struct Seen {
    stand: Stand,
    head: String,
    base: String,
    /// The base's commit on origin.
    b: String,
    view: Value,
}

/// A look at the project's PRs, or every project's, in the background.
pub fn soon(chats: &Arc<Chats>, project: Option<String>) {
    tokio::spawn(sweep(chats.clone(), project));
}

/// Looks at every PR whose newest review is done: a repository at a time, the PR whose review came
/// clean first first.
pub async fn sweep(chats: Arc<Chats>, project: Option<String>) {
    let found = chats
        .writer
        .call(move |c| landings::reviews(c, project.as_deref()))
        .await;
    let mut projects: Vec<String> = Vec::new();
    for review in found.and_then(Result::ok).unwrap_or_default() {
        if review.reviewed.is_some() && !projects.contains(&review.project) {
            projects.push(review.project);
        }
    }
    for id in projects {
        let Some(project) = merging::project(&chats, &id).await else {
            continue;
        };
        let lock = merging::lock_of(&project.folder);
        let _one = lock.lock().await;
        // Read again now it's this look's turn: the one before may have landed or moved them.
        let reviews = chats
            .writer
            .call(move |c| landings::reviews(c, Some(&id)))
            .await;
        let mut first = true;
        for review in reviews.and_then(Result::ok).unwrap_or_default() {
            // A pause holds what Gathr would do by itself in its chat, a merge too.
            let paused = gathr::pauses::holding(&chats.pauses(), &review.chat, None).is_some();
            if review.reviewed.is_none() || paused {
                continue;
            }
            if let Ok(looked) = look(&chats, &project, &review, first, None).await {
                first &= !looked.in_line;
            }
        }
    }
}

/// A reviewer reported its review of a PR done: Gathr looks at once, and what it did or saw goes
/// in the answer to that report. None when the report lands nothing: the task isn't the done review
/// of an open PR in a project's chat.
pub async fn recorded(chats: &Arc<Chats>, caller: &Caller, task: i64) -> Option<String> {
    let chat = caller.chat.clone();
    let found = chats.writer.call(move |c| {
        let pr = gathr::tasks::get(c, task)?.and_then(|t| t.pr);
        match pr {
            Some(pr) => landings::review_of(c, &chat, pr),
            None => Ok(None),
        }
    });
    let review = found.await?.ok()??;
    let pr = review.pr;
    if review.task != task {
        return Some(format!(
            "#{pr} has a newer review, task #{}: Gathr lands it on that one.",
            review.task
        ));
    }
    review.reviewed.as_ref()?;
    if gathr::pauses::holding(&chats.pauses(), &review.chat, None).is_some() {
        return Some(format!("Gathr lands #{pr} once this chat isn't paused."));
    }
    let project = merging::project(chats, &review.project).await?;
    let lock = merging::lock_of(&project.folder);
    let Ok(one) = tokio::time::timeout(TURN, lock.lock()).await else {
        soon(chats, Some(review.project));
        return Some(format!(
            "Gathr is landing another PR of this project: #{pr} is next."
        ));
    };
    // Alone it's never first in line: the project's look, with the others, brings it up to date.
    let looked = look(chats, &project, &review, false, Some(caller)).await;
    drop(one);
    match looked {
        Ok(looked) => {
            if looked.merged || looked.in_line {
                soon(chats, Some(review.project));
            }
            Some(looked.said)
        }
        Err(why) => Some(format!(
            "Gathr couldn't look at #{pr} now ({why}). It looks again within 2 minutes."
        )),
    }
}

/// One look at a PR whose newest review is done: lands it, brings it up to date, tells its
/// reviewer, or leaves it. An error is a look that couldn't be made, recorded once while it lasts.
async fn look(
    chats: &Arc<Chats>,
    project: &Project,
    review: &Review,
    first: bool,
    asker: Option<&Caller>,
) -> Result<Looked, String> {
    let trace = asker.and_then(Caller::trace).unwrap_or_else(trace_id);
    let looked = acted(chats, project, review, first, asker, &trace).await;
    let key = (review.chat.clone(), review.pr);
    let mut failed = FAILED.lock().expect("never poisoned");
    match &looked {
        Ok(_) => {
            failed.remove(&key);
        }
        Err(why) if failed.get(&key) != Some(why) => {
            failed.insert(key, why.clone());
            let event = Event::point("gathr.land.look").trace(&trace).error("look");
            let event = event.chat(&review.chat).attr("pr", review.pr);
            chats.writer.record(event.attr("why", why.clone()));
        }
        Err(_) => {}
    }
    looked
}

async fn acted(
    chats: &Arc<Chats>,
    project: &Project,
    review: &Review,
    first: bool,
    asker: Option<&Caller>,
    trace: &str,
) -> Result<Looked, String> {
    let vars = crate::coord::fresh().await;
    let pr = review.pr;
    let seen = judge(chats, project, review, &vars).await?;
    let (chat, head) = (review.chat.clone(), seen.head.clone());
    let did = db(chats, move |c| landings::did(c, &chat, pr, &head)).await?;
    let by = By {
        chat: review.chat.clone(),
        agent: asker.and_then(|a| a.agent.clone()),
        trace: trace.to_string(),
        landed: true,
    };
    let base = seen.base.as_str();
    match landings::act(seen.stand, did, first, now_us()) {
        Act::Land => {
            let at = (seen.head.as_str(), base, seen.b.as_str());
            match merging::push_merge(chats, &by, project, pr, at, &seen.view, &vars).await {
                Ok(merged) => {
                    if !merged.confirmed {
                        let (chat, head) = (review.chat.clone(), seen.head.clone());
                        db(chats, move |c| landings::pushed(c, &chat, pr, &head)).await?;
                    }
                    Ok(Looked {
                        said: merged.said,
                        in_line: false,
                        merged: true,
                    })
                }
                Err(why) => {
                    let why = format!("its merge wasn't made: {why}");
                    tell(chats, review, &seen.head, &why, asker, trace).await
                }
            }
        }
        Act::Update => {
            let (chat, head) = (review.chat.clone(), seen.head.clone());
            // Marked before it's asked: a stop right after never asks twice.
            db(chats, move |c| landings::updating(c, &chat, pr, &head)).await?;
            let repo = &project.folder;
            let asked =
                crate::github::bring_up_to_date(chats, repo, &by, pr, &seen.head, Duration::ZERO);
            match asked.await {
                Ok(_) => Ok(Looked::of(
                    format!(
                        "#{pr} was behind {base}: Gathr asked GitHub to bring it up to date, and lands it once its \
                         checks pass again."
                    ),
                    true,
                )),
                Err(why) => {
                    let why =
                        format!("it's behind {base}, and it wasn't brought up to date: {why}");
                    tell(chats, review, &seen.head, &why, asker, trace).await
                }
            }
        }
        Act::Tell(why) => tell(chats, review, &seen.head, &why, asker, trace).await,
        Act::Wait(why) => Ok(Looked::of(
            format!("Gathr lands #{pr} by itself. Now: {why}."),
            true,
        )),
        Act::Rest(why) => Ok(Looked::of(
            format!("Gathr isn't landing #{pr} at {}: {why}.", short(&seen.head)),
            false,
        )),
    }
}

/// Tells the reviewer what stops the PR at this head, once: woken on no task, or, when its own
/// report had Gathr look, in the answer to it.
async fn tell(
    chats: &Arc<Chats>,
    review: &Review,
    head: &str,
    why: &str,
    asker: Option<&Caller>,
    trace: &str,
) -> Result<Looked, String> {
    let said = format!("Gathr can't land #{} at {}: {why}.", review.pr, short(head));
    let own = asker.is_some_and(|a| a.agent.as_deref() == Some(review.reviewer.as_str()));
    let wake = (!own).then(|| {
        format!(
            "{said} Your review stands, and this is yours to judge: give its author a fix, or merge it \
             with `merge`."
        )
    });
    let (review, head, why, trace) = (
        review.clone(),
        head.to_string(),
        why.to_string(),
        trace.to_string(),
    );
    let told = db(chats, move |c| {
        landings::tell(c, &review, &head, &why, wake.as_deref(), &trace)
    });
    chats.start(told.await?.into_iter().collect());
    Ok(Looked::of(said, false))
}

/// The PR as GitHub and git have it now, and how it stands for landing.
async fn judge(
    chats: &Arc<Chats>,
    project: &Project,
    review: &Review,
    vars: &Vars,
) -> Result<Seen, String> {
    let repo = &project.folder;
    let (view, head, base) = merging::open_pr(review.pr, repo).await?;
    let b = merging::fetched(repo, &base, vars).await?;
    let stand = stand(chats, project, review, (&view, &head, &base, &b), vars).await?;
    Ok(Seen {
        stand,
        head,
        base,
        b,
        view,
    })
}

/// How the PR stands: the merge tool's conditions, in Gathr's order, and the two that are Gathr's
/// own (into the default branch, and no CI run still going whatever the project requires).
async fn stand(
    chats: &Arc<Chats>,
    project: &Project,
    review: &Review,
    (view, head, base, b): (&Value, &str, &str, &str),
    vars: &Vars,
) -> Result<Stand, String> {
    let (repo, pr, at) = (&project.folder, review.pr, short(head));
    let default = default_branch(repo, vars).await?;
    if base != default {
        return Ok(Stand::Stops(format!(
            "it's into {base}, and Gathr lands only into the default branch, {default}"
        )));
    }
    if !merging::known(repo, head, vars).await {
        return Ok(Stand::Stops(format!(
            "its head {at} isn't on origin: a PR from a fork isn't landed here"
        )));
    }
    let reviewed = review.reviewed.as_deref().unwrap_or_default();
    if let Err(why) = merging::reviewed_is_head(repo, reviewed, head, b, pr, vars).await {
        return Ok(Stand::Rests(why));
    }
    // A merge pushed that GitHub doesn't show yet: nothing is left to merge.
    if merging::ancestor(repo, head, b, vars).await {
        return Ok(Stand::Rests(format!("its head is in {base} already")));
    }
    let needs = merging::sign_off_for(chats, project, b, head, vars).await?;
    if !needs.is_empty() && !review.signed_off {
        return Ok(Stand::Stops(format!(
            "it needs your sign-off, as {}. Check they still test the code, then report this review done \
             again with signed_off: true",
            needs.join("; and ")
        )));
    }
    if !merging::ancestor(repo, b, head, vars).await {
        return Ok(Stand::Behind);
    }
    let checks = ci::rollup(&view["statusCheckRollup"]);
    let required = matches!(project.ci.as_str(), "ci" | "both");
    if let Some(going) = ci::going(&checks) {
        return Ok(Stand::Waits(format!("CI is still going on {at} ({going})")));
    }
    let passed = |c: &ci::Check| matches!(c.stand, ci::Stand::Passed | ci::Stand::Skipped);
    let failed = |c: &ci::Check| matches!(c.stand, ci::Stand::Failed | ci::Stand::Unknown);
    if checks.is_empty() {
        let chat = review.chat.clone();
        let asked = db(chats, move |c| landings::last_update(c, &chat, pr)).await?;
        if asked.is_some_and(|asked| now_us() - asked < CI_STARTS_US) {
            return Ok(Stand::Waits(format!(
                "Gathr brought it up to date, and CI hasn't started on {at} yet"
            )));
        }
        if required {
            return Ok(Stand::Stops(format!(
                "no CI checks ran on {at}, and the project requires them"
            )));
        }
    } else if required && !checks.iter().all(passed) {
        return Ok(Stand::Stops(format!(
            "CI on {at} didn't pass, and the project requires it: {}",
            ci::counted(&checks)
        )));
    } else if !required && checks.iter().any(failed) {
        // 0045: CI is information there, and a failure is read before a merge.
        return Ok(Stand::Stops(format!(
            "CI on {at} has a failure, which the project doesn't require to pass: {}. Read it with ci_failures",
            ci::counted(&checks)
        )));
    }
    if matches!(project.ci.as_str(), "gathr" | "both") {
        if !project.has_checks {
            return Ok(Stand::Stops(
                "the project requires Gathr's checks and has none: its Supervisor sets them"
                    .to_string(),
            ));
        }
        match merging::gathr_run(chats, &project.id, head)
            .await
            .as_deref()
        {
            Some("passed") => {}
            Some("running") => {
                return Ok(Stand::Waits(format!(
                    "Gathr's checks are still running on {at}"
                )));
            }
            Some("changed") => {
                return Ok(Stand::Stops(format!(
                    "the project's checks changed since they passed on {at}: they need to run again"
                )));
            }
            // None ran there, or a stop of the service cut the run off: it runs now.
            None | Some("interrupted") => {
                crate::check_runs::for_review(chats, &review.chat, review.task, head).await;
                return Ok(Stand::Waits(format!(
                    "Gathr's checks haven't finished on {at}"
                )));
            }
            Some(_) => {
                return Ok(Stand::Stops(format!(
                    "Gathr's checks failed on {at}: its run says why"
                )));
            }
        }
    }
    Ok(Stand::Ready)
}

/// The branch origin's HEAD names, asked of the remote itself: a clone's own idea of it can be old.
async fn default_branch(repo: &std::path::Path, vars: &Vars) -> Result<String, String> {
    let said = merging::git(repo, &["ls-remote", "--symref", "origin", "HEAD"], vars).await?;
    let named = said.lines().find_map(|line| {
        let branch = line.strip_prefix("ref: refs/heads/")?;
        branch.split_whitespace().next().map(String::from)
    });
    named.ok_or_else(|| "origin didn't say which branch is its default".to_string())
}

async fn db<T: Send + 'static>(
    chats: &Arc<Chats>,
    work: impl FnOnce(&mut rusqlite::Connection) -> Result<T, gathr::Error> + Send + 'static,
) -> Result<T, String> {
    match chats.writer.call(work).await {
        Some(Ok(done)) => Ok(done),
        Some(Err(e)) => Err(e.to_string()),
        None => Err("the database didn't answer".into()),
    }
}
