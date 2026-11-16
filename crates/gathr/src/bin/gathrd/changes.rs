//! A PR's change, measured when Gathr would land or merge it (0054), and when its building task
//! reports it done (0064): the lines it adds that someone wrote, against the line its project's own
//! recent changes set, or its task's when that is lower. Every git command runs through
//! `merging::git`, and nothing holds the database across one.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};
use std::time::{Duration, Instant};

use gathr::changes::count::{self, Count, File};
use gathr::changes::report::{self, Counted, ToCount};
use gathr::changes::{self, Measure};
use gathr::event::Event;
use gathr::platform::env::Vars;
use rusqlite::Connection;

use crate::chats::Chats;
use crate::landing::db;
use crate::merging::{Project, git};

/// Each project's line for each base, at the base's commit it was read from: read again when the
/// base moves.
static LINES: LazyLock<Mutex<Lines>> = LazyLock::new(Default::default);

/// (project, base) to (the base's commit, the line).
type Lines = HashMap<(String, String), (String, i64)>;

/// `gathr::run` gives a command no stdin, so paths go to `git check-attr` as arguments, this many
/// characters of them at a time.
const BATCH: usize = 20_000;

/// How long a done report waits for its PR's count, the lock's wait included (0064).
const REPORT_LIMIT: Duration = Duration::from_secs(20);

/// Measures the chat's PR at `head` and keeps it: the PR's files, which its sign-off reads, and
/// what the PR rests on, if anything (the person's answer, or a new head). `by` is `landing` or
/// `merge`. The line is the lower of the project's and its work task's (0064).
pub async fn landing(
    chats: &Arc<Chats>,
    project: &Project,
    (chat, pr, by): (&str, i64, &'static str),
    (base, head, b): (&str, &str, &str),
    trace: &str,
    vars: &Vars,
) -> Result<(Vec<File>, Option<String>), String> {
    let repo = &project.folder;
    let projects = line(repo, &project.id, base, b, vars).await?;
    let (id, on) = (project.id.clone(), chat.to_string());
    let read = move |c: &mut Connection| {
        Ok((
            changes::stored(c, &id, pr)?,
            report::landing_line(c, &on, pr)?,
        ))
    };
    let (stored, task) = db(chats, read).await?;
    let (line, line_of) = match task {
        Some((line, of)) if line < projects => (line, of),
        _ => (projects, "project"),
    };
    let kept = stored.filter(|(h, l, _)| h == head && *l == line);
    let (files, count) = counted(repo, b, head, kept.map(|(.., c)| c), vars).await?;
    let m = Measure {
        project: project.id.clone(),
        chat: chat.to_string(),
        pr,
        head: head.to_string(),
        count,
        line,
        line_of,
        by,
    };
    let trace = trace.to_string();
    let (rests, starts) = db(chats, move |c| changes::measured(c, &m, &trace)).await?;
    chats.start(starts);
    Ok((files, rests))
}

/// The PR's files from the base's commit `b` to `head`, as landing and a report count them, and
/// their count: `kept` when it was counted at that head and line already.
async fn counted(
    repo: &Path,
    b: &str,
    head: &str,
    kept: Option<Count>,
    vars: &Vars,
) -> Result<(Vec<File>, Count), String> {
    let since = format!("{b}...{head}");
    let diff = ["diff", "--numstat", "--no-renames", "-z", &since];
    let files = count::numstat(&git(repo, &diff, vars).await?);
    let count = match kept {
        Some(count) => count,
        None => {
            let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
            count::counted(&files, &marked(repo, b, &paths, vars).await?)
        }
    };
    Ok((files, count))
}

/// Counts the PR a building task's done report names, before the report is written (0064): `find`
/// says whether to count it, then landing's own count runs in the project's folder under its lock,
/// within `REPORT_LIMIT`, with nothing holding the database. A count that fails or takes longer is
/// recorded, and the report goes on as it would uncounted.
pub async fn report(
    chats: &Arc<Chats>,
    chat: &str,
    find: impl FnOnce(&mut Connection) -> Result<Option<ToCount>, gathr::Error> + Send + 'static,
    trace: &str,
) -> Counted {
    let to = match chats.writer.call(find).await {
        Some(Ok(Some(to))) => to,
        // A report the handler refuses says why itself.
        Some(Ok(None) | Err(gathr::Error::Refused(_))) => return Counted::NotCounted,
        Some(Err(e)) => return failed(chats, (chat, None), ("sqlite", e.to_string()), trace),
        None => {
            let why = "the database didn't answer".to_string();
            return failed(chats, (chat, None), ("sqlite", why), trace);
        }
    };
    let started = Instant::now();
    let limit = std::env::var("GATHR_REPORT_COUNT_MS")
        .ok()
        .and_then(|ms| ms.parse().ok());
    let limit = limit.map_or(REPORT_LIMIT, Duration::from_millis);
    let error = match tokio::time::timeout(limit, count_of(chats, &to)).await {
        Ok(Ok(m)) => return Counted::Measured(m, started.elapsed().as_millis() as i64),
        Ok(Err(error)) => error,
        Err(_) => (
            "timeout",
            format!("the count took over {} ms", limit.as_millis()),
        ),
    };
    let event = Event::point("gathr.change.report")
        .attr("line", to.line)
        .attr("line_of", to.line_of)
        .attr("duration_ms", started.elapsed().as_millis() as i64);
    failed(chats, (chat, Some((&to, event))), error, trace)
}

/// Records why a report's count failed: the report goes on uncounted.
fn failed(
    chats: &Arc<Chats>,
    (chat, to): (&str, Option<(&ToCount, Event)>),
    (kind, why): (&'static str, String),
    trace: &str,
) -> Counted {
    let event = match to {
        Some((to, event)) => event
            .task(to.task)
            .attr("gathr.task.id", to.task)
            .attr("pr", to.pr),
        None => Event::point("gathr.change.report"),
    };
    let event = event.trace(trace).chat(chat).error(kind).attr("why", why);
    chats.writer.record(event);
    Counted::Failed
}

/// The count of the PR `to` names, at its head on origin, against its task's line.
async fn count_of(chats: &Arc<Chats>, to: &ToCount) -> Result<Measure, (&'static str, String)> {
    let Some(project) = crate::merging::project(chats, &to.project).await else {
        return Err(("sqlite", "the project wasn't read".to_string()));
    };
    let lock = crate::merging::lock_of(&project.folder);
    let _one = lock.lock().await;
    let vars = crate::coord::fresh().await;
    let repo = &project.folder;
    let (_, head, base) = crate::merging::open_pr(to.pr, repo)
        .await
        .map_err(|why| ("github", why))?;
    let b = crate::merging::fetched(repo, &base, &vars)
        .await
        .map_err(|why| ("git", why))?;
    if !crate::merging::known(repo, &head, &vars).await {
        return Err(("not_on_origin", format!("{head} isn't fetched from origin")));
    }
    let (_, count) = counted(repo, &b, &head, None, &vars)
        .await
        .map_err(|why| ("git", why))?;
    Ok(Measure {
        project: to.project.clone(),
        chat: to.chat.clone(),
        pr: to.pr,
        head,
        count,
        line: to.line,
        line_of: to.line_of,
        by: "report",
    })
}

/// The chat's PR merged or was closed (`state` as GitHub says it), at `head` when known; `ours`
/// when Gathr merged it. Its question ends, and a merge is recorded.
pub async fn ended(
    chats: &Arc<Chats>,
    chat: &str,
    pr: i64,
    state: &str,
    head: Option<&str>,
    ours: bool,
    trace: &str,
) {
    let (c, s, h, t) = (
        chat.to_string(),
        state.to_string(),
        head.map(String::from),
        trace.to_string(),
    );
    let done = db(chats, move |conn| {
        changes::ended(conn, &c, pr, &s, h.as_deref(), ours, &t)
    });
    match done.await {
        Ok(starts) => chats.start(starts),
        Err(why) => {
            let event = Event::point("gathr.change.ended").trace(trace).chat(chat);
            chats
                .writer
                .record(event.error("sqlite").attr("pr", pr).attr("why", why));
        }
    }
}

/// The project's line on `base` at its commit `b`: the most any of its last 50 changes added.
async fn line(repo: &Path, project: &str, base: &str, b: &str, vars: &Vars) -> Result<i64, String> {
    let key = (project.to_string(), base.to_string());
    let kept = LINES.lock().expect("never poisoned").get(&key).cloned();
    if let Some((at, line)) = kept
        && at == b
    {
        return Ok(line);
    }
    let log = [
        "log",
        "--first-parent",
        "--diff-merges=first-parent",
        "-n",
        "50",
        "--numstat",
        "--no-renames",
        "-z",
        "--format=%x01%H",
        b,
    ];
    let commits = count::history(&git(repo, &log, vars).await?);
    let paths: BTreeSet<&str> = commits.iter().flatten().map(|f| f.path.as_str()).collect();
    let paths: Vec<&str> = paths.into_iter().collect();
    let marked = marked(repo, b, &paths, vars).await?;
    let counts: Vec<count::Count> = commits.iter().map(|c| count::counted(c, &marked)).collect();
    let line = count::line(&counts);
    let kept = (b.to_string(), line);
    LINES.lock().expect("never poisoned").insert(key, kept);
    Ok(line)
}

/// The paths `b`'s own attributes mark generated or vendored.
async fn marked(repo: &Path, b: &str, paths: &[&str], vars: &Vars) -> Result<Vec<String>, String> {
    let source = format!("--source={b}");
    let mut batches: Vec<Vec<&str>> = Vec::new();
    let mut size = BATCH;
    for path in paths {
        if size + path.len() > BATCH {
            batches.push(Vec::new());
            size = 0;
        }
        batches.last_mut().expect("just pushed").push(path);
        size += path.len() + 1;
    }
    let mut marked = Vec::new();
    for batch in batches {
        let mut args = vec![
            "check-attr",
            "-z",
            &source,
            "linguist-generated",
            "linguist-vendored",
            "--",
        ];
        args.extend(batch);
        let out = match git(repo, &args, vars).await {
            Ok(out) => out,
            // `--source` needs git 2.40 (Apple's git with Xcode 15 and 16 is 2.39): read the
            // checkout's own attributes then, which may lag the base.
            Err(_) => {
                args.remove(2);
                git(repo, &args, vars).await?
            }
        };
        marked.extend(count::marked(&out));
    }
    Ok(marked)
}
