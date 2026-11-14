//! A PR's change, measured when Gathr would land or merge it (0054): the lines it adds that someone
//! wrote, against the line its project's own recent changes set. Every git command runs through
//! `merging::git`, and nothing holds the database across one.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;
use std::sync::{Arc, LazyLock, Mutex};

use gathr::changes::count::{self, File};
use gathr::changes::{self, Measure};
use gathr::event::Event;
use gathr::platform::env::Vars;

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

/// Measures the chat's PR at `head` and keeps it: the PR's files, which its sign-off reads, and
/// what the PR rests on, if anything (the person's answer, or a new head). `by` is `landing` or
/// `merge`.
pub async fn landing(
    chats: &Arc<Chats>,
    project: &Project,
    (chat, pr, by): (&str, i64, &'static str),
    (base, head, b): (&str, &str, &str),
    trace: &str,
    vars: &Vars,
) -> Result<(Vec<File>, Option<String>), String> {
    let repo = &project.folder;
    let since = format!("{b}...{head}");
    let diff = ["diff", "--numstat", "--no-renames", "-z", &since];
    let files = count::numstat(&git(repo, &diff, vars).await?);
    let line = line(repo, &project.id, base, b, vars).await?;
    let id = project.id.clone();
    let stored = db(chats, move |c| changes::stored(c, &id, pr)).await?;
    let count = match stored {
        Some((h, l, count)) if h == head && l == line => count,
        _ => {
            let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
            count::counted(&files, &marked(repo, b, &paths, vars).await?)
        }
    };
    let m = Measure {
        project: project.id.clone(),
        chat: chat.to_string(),
        pr,
        head: head.to_string(),
        count,
        line,
        by,
    };
    let trace = trace.to_string();
    let rests = db(chats, move |c| changes::measured(c, &m, &trace)).await?;
    Ok((files, rests))
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
    if let Err(why) = done.await {
        let event = Event::point("gathr.change.ended").trace(trace).chat(chat);
        chats
            .writer
            .record(event.error("sqlite").attr("pr", pr).attr("why", why));
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
