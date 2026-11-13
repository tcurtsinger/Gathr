//! Reading CI for a wait (ported from SerenityOS's `ci-wait.ts`): finished only once a commit has
//! checks and none still runs, and the line says which commit it looked at (G49).

use gathr::waits::ci::{self, Check, Seen, Stand, Target};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

fn seen<'a>(checks: &'a [Check], state: Option<&'a str>, waited_us: i64) -> Seen<'a> {
    Seen {
        what: "#54",
        head: HEAD,
        state,
        checks,
        since_us: 0,
        now_us: waited_us,
    }
}

#[test]
fn ci_is_finished_only_once_it_has_checks_and_none_runs() {
    let runs = concat!(
        r#"{"name":"rust (windows)","status":"completed","conclusion":"success"}"#,
        "\n",
        r#"{"name":"rust (macos)","status":"in_progress","conclusion":null}"#
    );
    let statuses = r#"{"context":"lint","state":"failure"}"#;
    let mut checks = ci::checks_of(runs, statuses).unwrap();
    assert_eq!(checks.len(), 3);
    assert_eq!(
        ci::verdict(&seen(&[], None, 0)),
        (false, "no checks on #54 at 0123456 yet".into())
    );
    assert_eq!(
        ci::verdict(&seen(&[], None, ci::NO_CHECKS_US)),
        (true, "no CI checks on #54 at 0123456 15 minutes on".into())
    );
    assert_eq!(
        ci::verdict(&seen(&checks, Some("OPEN"), 0)),
        (
            false,
            "2 of 3 checks on #54 at 0123456 finished: 1 passed, 1 failed (lint); running: rust (macos)"
                .into()
        )
    );
    checks[1].stand = Stand::Skipped;
    assert_eq!(
        ci::verdict(&seen(&checks, Some("OPEN"), 0)),
        (
            true,
            "CI on #54 at 0123456 finished: 1 passed, 1 failed (lint), 1 skipped".into()
        )
    );
    assert_eq!(
        ci::verdict(&seen(&[], Some("MERGED"), 0)),
        (true, "#54 was merged".into())
    );
}

/// Scenario 6g (G144, G181). On 2026-11-12 GitHub cancelled macOS jobs no runner took, and the wake
/// read "1 failed": a diagnosis task was given for a test that never ran.
#[test]
fn a_cancelled_job_is_told_from_a_failed_one_and_a_queued_one_from_a_running_one() {
    let run = |name: &str, status: &str, conclusion: &str| {
        format!(r#"{{"name":"{name}","status":"{status}","conclusion":"{conclusion}"}}"#)
    };
    let waiting = [
        run("Rust (Windows)", "completed", "success"),
        run("Rust (macOS)", "in_progress", ""),
        run("UI (macOS)", "queued", ""),
        run("UI (Windows)", "waiting", ""),
    ]
    .join("\n");
    let checks = ci::checks_of(&waiting, "").unwrap();
    let stands: Vec<Stand> = checks.iter().map(|c| c.stand).collect();
    assert_eq!(
        stands,
        [Stand::Passed, Stand::Running, Stand::Queued, Stand::Queued],
        "a job no runner has taken is queued, not running"
    );
    assert_eq!(
        ci::verdict(&seen(&checks, Some("OPEN"), 0)),
        (
            false,
            "1 of 4 checks on #54 at 0123456 finished: 1 passed; running: Rust (macOS); \
             queued: UI (macOS), UI (Windows)"
                .into()
        )
    );

    let ended = [
        run("Rust (Windows)", "completed", "success"),
        run("Rust (macOS)", "completed", "failure"),
        run("UI (macOS)", "completed", "cancelled"),
        run("Deploy", "completed", "something_new"),
        run("Docs", "completed", "neutral"),
    ]
    .join("\n");
    let checks = ci::checks_of(&ended, "").unwrap();
    let stands: Vec<Stand> = checks.iter().map(|c| c.stand).collect();
    assert_eq!(
        stands,
        [
            Stand::Passed,
            Stand::Failed,
            Stand::Cancelled,
            Stand::Unknown,
            Stand::Skipped
        ],
        "cancelled isn't failed, and a conclusion Gathr doesn't know isn't passed"
    );
    // Every check has finished, whatever each came to: the wait is met, as before.
    assert_eq!(
        ci::verdict(&seen(&checks, Some("OPEN"), 0)),
        (
            true,
            "CI on #54 at 0123456 finished: 1 passed, 1 failed (Rust (macOS)), \
             1 cancelled (UI (macOS)), 1 skipped, 1 unknown (Deploy)"
                .into()
        )
    );
}

#[test]
fn a_ci_wait_takes_a_pr_or_a_commit_never_a_branch() {
    assert_eq!(Target::parse("#54"), Ok(Target::Pr(54)));
    assert_eq!(Target::parse("54"), Ok(Target::Pr(54)));
    assert_eq!(
        Target::parse("ABC1234"),
        Ok(Target::Commit("abc1234".into()))
    );
    assert!(Target::parse("main").is_err());
    assert!(ci::checks_of("not json", "").is_err());
}
