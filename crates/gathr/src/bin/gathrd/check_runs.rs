//! Gathr runs a project's checks at a commit (0043 C3). Each project has one checkout in its
//! agent folder, `<Project>/checks`, reset to the commit before each run (a forced checkout, then
//! `git clean -ffdx`), so nothing an agent left uncommitted, and nothing a run left, reaches the
//! next. Caches live beside it in `<Project>/caches`, so a run after the first starts warm. One
//! run at a time per project; each command's exit, duration and log are kept.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use axum::extract::{Extension, Path as Id, State as Extract};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use gathr::event::Event;
use gathr::platform::{self, env::Vars};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::Mutex;

use crate::api::{State, Trace};
use crate::chats::Chats;
use crate::routes::{Answer, db, fail};

/// How long one command may take: a full build and test run, cold.
const COMMAND_LIMIT: Duration = Duration::from_secs(60 * 60);
const GIT_LIMIT: Duration = Duration::from_secs(120);
/// Caches nothing used for this many days go.
const UNUSED_DAYS: i64 = 14;

/// One run at a time per project: the checkout and caches are the project's.
static RUNNING: LazyLock<std::sync::Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(Default::default);

fn lock_of(project: &str) -> Arc<Mutex<()>> {
    let mut locks = RUNNING.lock().expect("never poisoned");
    locks.entry(project.to_string()).or_default().clone()
}

pub fn routes() -> Router<Arc<State>> {
    Router::new()
        .route(
            "/api/projects/{project}/checks/runs",
            get(runs).post(run_now),
        )
        .route(
            "/api/projects/{project}/caches",
            get(cache_size).delete(remove_caches),
        )
}

#[derive(serde::Deserialize)]
struct At {
    /// One commit's runs, however old, rather than the project's newest.
    head: Option<String>,
}

async fn runs(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(project): Id<String>,
    axum::extract::Query(at): axum::extract::Query<At>,
) -> Answer {
    crate::projects::find(&s, &t, &project, "Checks failed").await?;
    db(&s, &t, "Checks failed", move |c| {
        let listed = gathr::check_runs::list(c, &project, at.head.as_deref())?;
        Ok(json!({ "runs": listed }))
    })
    .await
}

#[derive(Deserialize)]
struct RunAt {
    head: String,
}

async fn run_now(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(project): Id<String>,
    Json(body): Json<RunAt>,
) -> Answer {
    const ERROR: &str = "Checks failed";
    let found = crate::projects::find(&s, &t, &project, ERROR).await?;
    if found.checks.is_empty() {
        return Err(fail(
            StatusCode::CONFLICT,
            ERROR,
            "the project has no checks: its Supervisor sets them",
            &t.0,
        ));
    }
    let head = body.head.trim().to_string();
    if head.is_empty() || !head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(fail(
            StatusCode::BAD_REQUEST,
            ERROR,
            "head is a commit's id",
            &t.0,
        ));
    }
    let run = start(&s.chats, &project, &head, &t.0)
        .await
        .map_err(|why| fail(StatusCode::INTERNAL_SERVER_ERROR, ERROR, &why, &t.0))?;
    Ok(Json(json!({ "run": run, "state": "running" })))
}

/// Starts a run of the project's checks at `head`, in the background: its id.
pub async fn start(
    chats: &Arc<Chats>,
    project: &str,
    head: &str,
    trace: &str,
) -> Result<String, String> {
    let (p, h, tr) = (project.to_string(), head.to_string(), trace.to_string());
    let run = chats
        .writer
        .call(move |c| gathr::check_runs::start(c, &p, &h, &tr))
        .await;
    let run = match run {
        Some(Ok(run)) => run,
        Some(Err(e)) => return Err(e.to_string()),
        None => return Err("the database didn't answer".into()),
    };
    let event = Event::point("gathr.checks.start").trace(trace).ok();
    chats.writer.record(
        event
            .attr("gathr.project.id", project.to_string())
            .attr("head", head.to_string()),
    );
    publish(chats, project, head, &run, "running");
    let (chats, project, head, trace, id) = (
        chats.clone(),
        project.to_string(),
        head.to_string(),
        trace.to_string(),
        run.clone(),
    );
    tokio::spawn(async move {
        let lock = lock_of(&project);
        let _one = lock.lock().await;
        let mut why = run_steps(&chats, &project, &head, &id).await.err();
        let (r, w) = (id.clone(), why.clone());
        let ended = chats
            .writer
            .call(move |c| gathr::check_runs::end(c, &r, w.as_deref()))
            .await;
        // An end not saved is never shown as a pass: the run stays "running" in the database
        // until the next start ends it failed and interrupted (`recover`).
        match ended {
            Some(Ok(())) => {}
            Some(Err(e)) => why = Some(format!("its result wasn't saved: {e}")),
            None => why = Some("its result wasn't saved: the database didn't answer".into()),
        }
        let event = Event::point("gathr.checks.end")
            .trace(&trace)
            .attr("gathr.project.id", project.clone())
            .attr("head", head.clone());
        let event = match &why {
            Some(why) => event.error("checks_failed").attr("why", why.clone()),
            None => event.ok(),
        };
        chats.writer.record(event);
        publish(
            &chats,
            &project,
            &head,
            &id,
            if why.is_some() { "failed" } else { "passed" },
        );
    });
    Ok(run)
}

fn publish(chats: &Chats, project: &str, head: &str, run: &str, state: &str) {
    chats.publish(
        json!({ "type": "checks", "project": project, "head": head, "run": run, "state": state }),
    );
}

/// The checkout and caches folders of the project, and its checks and git folder.
async fn places(
    chats: &Arc<Chats>,
    project: &str,
) -> Result<(PathBuf, PathBuf, PathBuf, Vec<String>), String> {
    let (id, data) = (project.to_string(), chats.data.path().to_path_buf());
    let found = chats
        .writer
        .call(move |c| {
            let folder = gathr::folders::project_folder(c, &data, &id)?;
            let found = gathr::projects::get(c, &id)?
                .ok_or(gathr::Error::Other("the project is gone".into()))?;
            Ok::<_, gathr::Error>((folder, found))
        })
        .await;
    let (folder, found) = match found {
        Some(Ok(found)) => found,
        Some(Err(e)) => return Err(e.to_string()),
        None => return Err("the database didn't answer".into()),
    };
    let commands = found.checks.into_iter().map(|c| c.command).collect();
    Ok((
        folder.join("checks"),
        folder.join("caches"),
        PathBuf::from(found.folder),
        commands,
    ))
}

/// Resets the checkout to `head` and runs each command in order, stopping at the first failure.
async fn run_steps(chats: &Arc<Chats>, project: &str, head: &str, run: &str) -> Result<(), String> {
    let (checkout, caches, repo, commands) = places(chats, project).await?;
    let id = project.to_string();
    let _ = chats
        .writer
        .call(move |c| gathr::check_runs::used(c, &id))
        .await;
    let vars = crate::coord::fresh().await;
    reset(&repo, &checkout, head, &vars).await?;
    let mut vars = vars;
    for (name, sub) in [
        ("CARGO_TARGET_DIR", "cargo-target"),
        ("npm_config_cache", "npm"),
        ("PIP_CACHE_DIR", "pip"),
    ] {
        vars.push((name.into(), caches.join(sub).into_os_string()));
    }
    std::fs::create_dir_all(&caches)
        .map_err(|e| format!("{} wasn't made: {e}", caches.display()))?;
    let (shell, vars) = platform::shell(&vars).ok_or("Git Bash wasn't found beside git")?;
    for (position, command) in commands.iter().enumerate() {
        let started = Instant::now();
        let ran = gathr::run::script(&shell, command, &checkout, &vars, COMMAND_LIMIT).await;
        let ms = started.elapsed().as_millis() as i64;
        let (exit, log) = match &ran {
            Ok(ran) => (
                ran.code,
                format!("{}\n{}", ran.out, ran.err).trim().to_string(),
            ),
            Err(why) => (None, why.clone()),
        };
        let (r, c, l) = (run.to_string(), command.clone(), log.clone());
        let saved = chats
            .writer
            .call(move |conn| gathr::check_runs::step(conn, &r, (position, &c), exit, ms, &l))
            .await;
        // Never a pass without its evidence (Vale, #119).
        match saved {
            Some(Ok(())) => {}
            Some(Err(e)) => return Err(format!("{command}: its result wasn't saved: {e}")),
            None => {
                return Err(format!(
                    "{command}: its result wasn't saved: the database didn't answer"
                ));
            }
        }
        if exit != Some(0) {
            let how = exit.map_or("didn't finish".to_string(), |code| format!("exit {code}"));
            return Err(format!("{command}: {how}"));
        }
    }
    Ok(())
}

/// The checkout at exactly `head`: fetched from the project's origin, then forced and cleaned.
async fn reset(repo: &Path, checkout: &Path, head: &str, vars: &Vars) -> Result<(), String> {
    let git = |dir: &Path, args: Vec<&str>| {
        let (dir, args) = (
            dir.to_path_buf(),
            args.iter().map(|a| a.to_string()).collect::<Vec<_>>(),
        );
        let vars = vars.clone();
        async move {
            let args: Vec<&str> = args.iter().map(String::as_str).collect();
            let ran = gathr::run::run(Path::new("git"), &args, &dir, &vars, GIT_LIMIT).await?;
            match ran.code {
                Some(0) => Ok(ran.out),
                _ => Err(format!("git {} failed: {}", args[0], ran.reason())),
            }
        }
    };
    let remotes = git(repo, vec!["remote"]).await?;
    if remotes.lines().any(|r| r == "origin") {
        git(repo, vec!["fetch", "--quiet", "origin"]).await?;
    }
    let commit = format!("{head}^{{commit}}");
    git(repo, vec!["rev-parse", "--verify", "--quiet", &commit])
        .await
        .map_err(|_| format!("{head} isn't a commit the project has"))?;
    if !checkout.join(".git").exists() {
        git(repo, vec!["worktree", "prune"]).await?;
        let path = checkout.to_string_lossy().to_string();
        git(repo, vec!["worktree", "add", "--detach", &path, head]).await?;
    }
    git(
        checkout,
        vec!["checkout", "--quiet", "--force", "--detach", head],
    )
    .await?;
    git(checkout, vec!["clean", "-ffdxq"]).await?;
    Ok(())
}

async fn cache_size(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(project): Id<String>,
) -> Answer {
    crate::projects::find(&s, &t, &project, "Caches failed").await?;
    let (_, caches, _, _) = places(&s.chats, &project).await.map_err(|why| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Caches failed",
            &why,
            &t.0,
        )
    })?;
    let bytes = tokio::task::spawn_blocking(move || size(&caches))
        .await
        .unwrap_or(0);
    Ok(Json(json!({ "bytes": bytes })))
}

async fn remove_caches(
    Extract(s): Extract<Arc<State>>,
    Extension(t): Extension<Trace>,
    Id(project): Id<String>,
) -> Answer {
    crate::projects::find(&s, &t, &project, "Remove failed").await?;
    remove(&s.chats, &project, &t.0).await.map_err(|why| {
        fail(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Remove failed",
            &why,
            &t.0,
        )
    })?;
    Ok(Json(json!({ "removed": true })))
}

/// Removes the project's caches once no run uses them.
async fn remove(chats: &Arc<Chats>, project: &str, trace: &str) -> Result<(), String> {
    let lock = lock_of(project);
    let _one = lock.lock().await;
    let (_, caches, _, _) = places(chats, project).await?;
    let gone = match std::fs::remove_dir_all(&caches) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(format!("{} wasn't removed: {e}", caches.display()))
        }
        _ => Ok(()),
    };
    let id = project.to_string();
    let _ = chats
        .writer
        .call(move |c| gathr::check_runs::forget(c, &id))
        .await;
    let event = Event::point("gathr.checks.caches")
        .trace(trace)
        .attr("gathr.project.id", project.to_string());
    chats.writer.record(match &gone {
        Ok(()) => event.ok(),
        Err(why) => event.error("caches").attr("why", why.clone()),
    });
    gone
}

/// Removes the caches of projects nothing has run in for 14 days: with the service's looks.
pub async fn sweep(chats: &Arc<Chats>, day_us: i64) {
    let before = gathr::ids::now_us() - UNUSED_DAYS * day_us;
    let unused = chats
        .writer
        .call(move |c| gathr::check_runs::unused_since(c, before))
        .await;
    for project in unused.and_then(Result::ok).unwrap_or_default() {
        let _ = remove(chats, &project, &gathr::ids::trace_id()).await;
    }
}

fn size(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    entries
        .flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => size(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// What the PR looks see of a review task's PR: a run at its head, once per commit (C3).
pub async fn for_review(chats: &Arc<Chats>, chat: &str, task: i64, head: &str) {
    let (c, h) = (chat.to_string(), head.to_string());
    let wanted = chats
        .writer
        .call(move |conn| -> Result<Option<String>, gathr::Error> {
            let review = gathr::tasks::get(conn, task)?
                .is_some_and(|t| t.kind == gathr::tasks::Kind::Review);
            let project: Option<String> =
                conn.query_row("SELECT project_id FROM chats WHERE id = ?1", [&c], |r| {
                    r.get(0)
                })?;
            let Some(project) = project.filter(|_| review) else {
                return Ok(None);
            };
            let checks =
                gathr::projects::get(conn, &project)?.is_some_and(|p| !p.checks.is_empty());
            Ok((checks && !gathr::check_runs::ran_at(conn, &project, &h)?).then_some(project))
        })
        .await;
    if let Some(Ok(Some(project))) = wanted {
        let _ = start(chats, &project, head, &gathr::ids::trace_id()).await;
    }
}

/// At start, before anything runs: runs the last stop cut off end failed, each with its own
/// `gathr.checks.end` so its reference traces, and may run again.
pub async fn recover(chats: &Arc<Chats>) {
    let recovered = chats.writer.call(|c| gathr::check_runs::recover(c)).await;
    let Some(Ok(runs)) = recovered else {
        let event = Event::point("gathr.checks.recover").error("sqlite");
        chats
            .writer
            .record(event.attr("why", format!("{recovered:?}")));
        return;
    };
    for (run, trace) in runs {
        let event = Event::point("gathr.checks.end")
            .trace(&trace)
            .error("checks_failed");
        let event = event
            .attr("why", gathr::check_runs::INTERRUPTED)
            .attr("gathr.checks.run", run);
        chats.writer.record(event);
    }
}
