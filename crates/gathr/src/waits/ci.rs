//! A wait on CI, looked at by the service (ported from SerenityOS's `ci-wait.ts`, the owner's).
//!
//! Agents wrote their own checks, and some came true before GitHub had any checks for a new commit.
//! Here CI on a PR's newest commit, or on one commit, is finished once it has checks and none is
//! still running; none 15 minutes on says so. A PR is followed to its newest commit, and the line
//! says which (G49); a commit a PR has moved past ends the wait (G69). Each check is named as it
//! stands: a cancelled job isn't a failed test, and a queued one isn't running (G144, G181).

use serde_json::Value;

/// How long a commit may have no checks at all before that's the answer.
pub const NO_CHECKS_US: i64 = 15 * 60 * 1_000_000;

/// What a CI wait waits on: a PR by number, or a commit by its id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Pr(u32),
    Commit(String),
}

impl Target {
    /// A PR's number (`#242` or `242`) or a commit's id. A branch isn't taken: the one here may be
    /// behind GitHub's, and its CI long finished.
    pub fn parse(raw: &str) -> Result<Target, String> {
        let word = raw.trim();
        let digits = word.strip_prefix('#').unwrap_or(word);
        if !digits.is_empty() && digits.len() <= 7 && digits.chars().all(|c| c.is_ascii_digit()) {
            return digits.parse().map(Target::Pr).map_err(|e| e.to_string());
        }
        let hex = word.chars().all(|c| c.is_ascii_hexdigit());
        if hex && (7..=40).contains(&word.len()) {
            return Ok(Target::Commit(word.to_ascii_lowercase()));
        }
        Err(format!(
            "a CI wait takes a PR's number or a commit's id, not \"{word}\""
        ))
    }

    pub fn name(&self) -> String {
        match self {
            Target::Pr(n) => format!("#{n}"),
            Target::Commit(c) => c.get(..7).unwrap_or(c).to_string(),
        }
    }
}

/// How one check stands. `Unknown` is a conclusion Gathr has no word for: finished, and never
/// counted as passed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stand {
    Queued,
    Running,
    Passed,
    Failed,
    Cancelled,
    Skipped,
    Unknown,
}

impl Stand {
    fn finished(self) -> bool {
        !matches!(self, Stand::Queued | Stand::Running)
    }
}

/// One check, as either of GitHub's shapes says it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub name: String,
    pub stand: Stand,
}

/// A check run: in progress is running, completed is as it concluded, and anything else (queued,
/// waiting, requested, pending) hasn't started.
fn stand_of(status: &str, conclusion: &str) -> Stand {
    match status {
        "completed" => match conclusion.to_ascii_lowercase().as_str() {
            "success" => Stand::Passed,
            "failure" | "timed_out" | "action_required" | "startup_failure" | "stale" | "error" => {
                Stand::Failed
            }
            "cancelled" => Stand::Cancelled,
            "skipped" | "neutral" => Stand::Skipped,
            _ => Stand::Unknown,
        },
        "in_progress" => Stand::Running,
        _ => Stand::Queued,
    }
}

/// A commit's checks from `gh api --paginate … --jq`: one JSON object a line, its check runs
/// (`name`, `status`, `conclusion`) and its statuses (`context`, `state`).
pub fn checks_of(runs: &str, statuses: &str) -> Result<Vec<Check>, String> {
    let each = |text: &str| -> Result<Vec<Value>, String> {
        let lines = text.lines().map(str::trim).filter(|l| !l.is_empty());
        lines
            .map(|l| {
                serde_json::from_str(l).map_err(|e| format!("GitHub's answer wasn't read: {e}"))
            })
            .collect()
    };
    let text = |v: &Value, key: &str| v[key].as_str().unwrap_or_default().to_string();
    let mut checks = Vec::new();
    for run in each(runs)? {
        checks.push(Check {
            name: text(&run, "name"),
            stand: stand_of(&text(&run, "status"), &text(&run, "conclusion")),
        });
    }
    for status in each(statuses)? {
        let state = text(&status, "state");
        // A commit status has no word for queued: pending is all it says.
        let stand = match state.as_str() {
            "pending" => Stand::Running,
            "success" => Stand::Passed,
            _ => Stand::Failed,
        };
        checks.push(Check {
            name: text(&status, "context"),
            stand,
        });
    }
    Ok(checks)
}

/// What one look saw.
pub struct Seen<'a> {
    /// `#242` or a short commit id.
    pub what: &'a str,
    pub head: &'a str,
    /// The PR's state (`OPEN`, `MERGED`, `CLOSED`), for a PR.
    pub state: Option<&'a str>,
    pub checks: &'a [Check],
    /// Since when this head has been seen: a push starts the no-checks grace again.
    pub since_us: i64,
    pub now_us: i64,
}

/// Whether CI has finished, and the line: what its agent is woken with once it has, and what the
/// wait is seen to wait on until then.
pub fn verdict(s: &Seen) -> (bool, String) {
    let at = format!(" at {}", s.head.get(..7).unwrap_or(s.head));
    let state = s.state.unwrap_or_default().to_ascii_uppercase();
    if state == "MERGED" || state == "CLOSED" {
        let finished: Vec<Check> = s
            .checks
            .iter()
            .filter(|c| c.stand.finished())
            .cloned()
            .collect();
        let mut line = format!("{} was {}", s.what, state.to_lowercase());
        if !finished.is_empty() {
            line.push_str(&format!("; its checks{at}: {}", counted(&finished)));
            let running = s.checks.len() - finished.len();
            if running > 0 {
                line.push_str(&format!(", {running} still running"));
            }
        }
        return (true, line);
    }
    if s.checks.is_empty() {
        if s.now_us - s.since_us >= NO_CHECKS_US {
            let minutes = NO_CHECKS_US / 60_000_000;
            return (
                true,
                format!("no CI checks on {}{at} {minutes} minutes on", s.what),
            );
        }
        return (false, format!("no checks on {}{at} yet", s.what));
    }
    let finished: Vec<Check> = s
        .checks
        .iter()
        .filter(|c| c.stand.finished())
        .cloned()
        .collect();
    if finished.len() < s.checks.len() {
        let mut line = format!(
            "{} of {} checks on {}{at} finished",
            finished.len(),
            s.checks.len(),
            s.what
        );
        if !finished.is_empty() {
            line.push_str(&format!(": {}", counted(&finished)));
        }
        for (word, stand) in [("running", Stand::Running), ("queued", Stand::Queued)] {
            let names = names(s.checks, stand);
            if !names.is_empty() {
                line.push_str(&format!("; {word}: {}", listed(&names)));
            }
        }
        return (false, line);
    }
    (
        true,
        format!("CI on {}{at} finished: {}", s.what, counted(s.checks)),
    )
}

fn names(checks: &[Check], stand: Stand) -> Vec<&str> {
    let of = checks.iter().filter(|c| c.stand == stand);
    of.map(|c| c.name.as_str()).collect()
}

/// Up to five names, then that there are more.
fn listed(names: &[&str]) -> String {
    let more = if names.len() > 5 { ", …" } else { "" };
    format!("{}{more}", names[..names.len().min(5)].join(", "))
}

/// The finished checks by how they ended, those that didn't pass or skip by name.
fn counted(checks: &[Check]) -> String {
    let mut parts = Vec::new();
    for (word, stand, named) in [
        ("passed", Stand::Passed, false),
        ("failed", Stand::Failed, true),
        ("cancelled", Stand::Cancelled, true),
        ("skipped", Stand::Skipped, false),
        ("unknown", Stand::Unknown, true),
    ] {
        let names = names(checks, stand);
        match (names.len(), named) {
            (0, _) => {}
            (n, true) => parts.push(format!("{n} {word} ({})", listed(&names))),
            (n, false) => parts.push(format!("{n} {word}")),
        }
    }
    parts.join(", ")
}
