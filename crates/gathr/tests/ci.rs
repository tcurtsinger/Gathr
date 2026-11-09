//! Reading CI for a wait (ported from SerenityOS's `ci-wait.ts`): finished only once a commit has
//! checks and none still runs, and the line says which commit it looked at (G49).

use gathr::waits::ci::{self, Check, Outcome, Seen, Target};

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
            "1 of 3 checks on #54 at 0123456 still running".into()
        )
    );
    checks[1].outcome = Some(Outcome::Skipped);
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
