//! The trial's own checks, run as every test is: the plan and the cap, each case's scoring on
//! made-up diffs and rows, R6's rule, and the file against the shipped shape test.

use serde_json::{Value, json};

use super::cases::{self, Arm, Block, Case, Engine, Mode, SEATS};
use super::common::{TempDir, git};
use super::score::{self, Built};
use super::{decide, file, project, results, setup};

fn count(blocks: &[Block], case: Case, engine: Engine, arm: Arm) -> usize {
    let of = |b: &&Block| b.arm == arm;
    let seats = blocks.iter().filter(of).flat_map(|b| &b.seats);
    seats
        .filter(|s| s.case == case && s.engine == engine)
        .count()
}

#[test]
fn the_plan_alternates_arms_and_gives_each_case_its_runs() {
    let full = cases::plan(Mode::Full);
    let arms: Vec<Arm> = full.iter().map(|b| b.arm).collect();
    let (w, wo) = (Arm::With, Arm::Without);
    let round = |first: bool| if first { [w, wo] } else { [wo, w] };
    let expected: Vec<Arm> = (0..8).flat_map(|r| round(r % 2 == 0)).collect();
    assert_eq!(arms, expected);
    assert_eq!(
        full.iter().map(|b| b.n).collect::<Vec<_>>(),
        (1..=16).collect::<Vec<_>>()
    );
    for arm in [w, wo] {
        let runs = [
            (Case::T1, Engine::Claude, 6),
            (Case::T1, Engine::Codex, 6),
            (Case::T2, Engine::Codex, 6),
            (Case::T5, Engine::Claude, 8),
            (Case::T8, Engine::Claude, 6),
            (Case::T8, Engine::Codex, 6),
            (Case::T2, Engine::Claude, 0),
            (Case::T5, Engine::Codex, 0),
        ];
        for (case, engine, n) in runs {
            assert_eq!(
                count(&full, case, engine, arm),
                n,
                "{case:?} on {engine:?}, {arm:?}"
            );
        }
    }
    assert_eq!(
        full[15].seats.iter().map(|s| s.case).collect::<Vec<_>>(),
        [Case::T5]
    );

    let first = cases::plan(Mode::First);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].arm, Arm::With);
    let seats: Vec<(Case, Engine)> = first[0].seats.iter().map(|s| (s.case, s.engine)).collect();
    let (claude, codex) = (Engine::Claude, Engine::Codex);
    assert_eq!(
        seats,
        [
            (Case::T1, claude),
            (Case::T8, claude),
            (Case::T1, codex),
            (Case::T8, codex)
        ]
    );

    let standin = cases::plan(Mode::StandIn);
    assert_eq!(standin.iter().map(|b| b.arm).collect::<Vec<_>>(), [w, wo]);
    assert!(standin.iter().all(|b| b.seats.len() == SEATS.len()));
}

#[test]
fn no_block_starts_past_the_cap_and_the_most_is_said_from_the_limits() {
    assert!(cases::may_start(89_999_999));
    assert!(!cases::may_start(90_000_000));
    // T1 4M and T8 1M, on each engine.
    assert_eq!(cases::most(&cases::plan(Mode::First)), 10_000_000);
    // Every run to its limit is 24 × 4M + 12 × 1.5M + 16 × 1.5M + 24 × 1M = 162M; the cap and the
    // largest block (two T1, T2, T5, two T8: 13M) are less.
    assert_eq!(cases::most(&cases::plan(Mode::Full)), 103_000_000);
}

/// The project's repository in a temp folder: the folder and its repository.
fn made() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new();
    let repo = dir.0.join("project");
    project::make(&repo);
    (dir, repo)
}

#[test]
fn the_project_has_its_planted_change_on_a_branch_beside_main() {
    let (_dir, repo) = made();
    assert_eq!(git(&repo, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(git(&repo, &["status", "--porcelain"]), "");
    let planted = git(&repo, &["diff", "--name-only", "main", "trial/planted"]);
    assert_eq!(
        planted,
        "src/duration.js\nsrc/retry.js\ntest/duration.test.js"
    );
    assert!(!repo.join("src/retry.js").exists());
}

/// An earlier block's work: a run's branch with a commit, a worktree with changes not committed,
/// and a worktree whose folder is gone.
#[test]
fn a_blocks_repository_holds_no_earlier_runs_branch_or_worktree() {
    let (dir, repo) = made();
    let (kept, gone) = (dir.0.join("kept"), dir.0.join("gone"));
    git(&repo, &["branch", "cy/durations-1"]);
    let at = |p: &std::path::Path| p.to_string_lossy().to_string();
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "eli/durations-2", &at(&kept)],
    );
    std::fs::write(kept.join("src/extra.js"), "// left\n").unwrap();
    git(&kept, &["add", "."]);
    git(&kept, &["commit", "-q", "-m", "earlier work"]);
    std::fs::write(kept.join("src/cli.js"), "// not committed\n").unwrap();
    git(
        &repo,
        &["worktree", "add", "-q", "-b", "gus/durations-3", &at(&gone)],
    );
    std::fs::remove_dir_all(&gone).unwrap();

    project::forget(&repo);
    let branches = git(&repo, &["branch", "--format=%(refname:short)"]);
    assert_eq!(branches, "main\ntrial/planted");
    let worktrees = git(&repo, &["worktree", "list", "--porcelain"]);
    assert_eq!(worktrees.matches("worktree ").count(), 1, "{worktrees}");
    let commits = git(&repo, &["log", "--all", "--format=%s"]);
    assert!(!commits.contains("earlier work"), "{commits}");
    assert_eq!(commits.lines().count(), 2, "{commits}");
    assert!(!kept.exists());
}

#[test]
fn a_diff_counts_committed_uncommitted_and_new_files_against_main() {
    let (_dir, repo) = made();
    let reference = project::fixture().join("reference/src");
    git(&repo, &["checkout", "-q", "-b", "work"]);
    std::fs::copy(reference.join("duration.js"), repo.join("src/duration.js")).unwrap();
    git(&repo, &["commit", "-q", "-am", "formatDuration"]);
    std::fs::copy(reference.join("cli.js"), repo.join("src/cli.js")).unwrap();
    std::fs::write(repo.join("src/extra.js"), "// one\n// two\n").unwrap();

    let files = score::parse(&project::diff(&repo));
    let seen: Vec<(&str, bool)> = files.iter().map(|f| (f.path.as_str(), f.new)).collect();
    assert_eq!(
        seen,
        [
            ("src/cli.js", false),
            ("src/duration.js", false),
            ("src/extra.js", true)
        ]
    );
    assert_eq!(files[2].added, ["// one", "// two"]);
    assert!(
        files[1]
            .added
            .iter()
            .any(|l| l.contains("function formatDuration(ms)"))
    );
    assert!(files[0].added.iter().any(|l| l.contains("'--format'")));
}

#[test]
fn the_reference_version_is_not_over_built() {
    let (_dir, repo) = made();
    let before = project::sources(&repo);
    project::copy(&project::fixture().join("reference"), &repo);
    let after = project::sources(&repo);
    let [b0, b1] = &before;
    let [a0, a1] = &after;
    let built = score::built(&project::diff(&repo), [b0, b1], [a0, a1]);
    assert_eq!(
        (
            built.new_src_files,
            built.extra_exports,
            built.extra_options
        ),
        (0, 0, 0)
    );
    assert_eq!(
        (built.extra_params, built.guard_lines, built.test_lines),
        (0, 0, 0)
    );
    assert!((15..=35).contains(&built.src_lines), "{built:?}");
    assert!(!built.over_built(), "{built:?}");
}

#[test]
fn the_hidden_test_passes_the_reference_and_fails_the_base() {
    let dir = TempDir::new();
    project::copy(&project::fixture().join("project"), &dir.0);
    assert!(
        !project::hidden_test(&dir.0),
        "the hidden test passed the project as it is"
    );
    project::copy(&project::fixture().join("reference"), &dir.0);
    assert!(
        project::hidden_test(&dir.0),
        "the hidden test failed the reference version"
    );
    let left: Vec<_> = std::fs::read_dir(&dir.0)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert!(
        !left.iter().any(|n| n.to_string_lossy().contains("hidden")),
        "{left:?}"
    );
}

const OVER_BUILT: &str = "\
diff --git a/src/duration.js b/src/duration.js
index 1111111..2222222 100644
--- a/src/duration.js
+++ b/src/duration.js
@@ -21,4 +21,10 @@ function parseDuration(text) {
   return total;
 }

-module.exports = { parseDuration };
+function formatDuration(ms, options = {}) {
+  if (typeof ms !== 'number') throw new TypeError('not a number');
+  if (!Number.isInteger(ms)) return formatDuration(Math.round(ms), options);
+  return options.long ? `${ms} milliseconds` : `${ms}ms`;
+}
+
+module.exports = { parseDuration, formatDuration, UNITS };
diff --git a/src/cli.js b/src/cli.js
index 3333333..4444444 100644
--- a/src/cli.js
+++ b/src/cli.js
@@ -1,2 +1,6 @@
 // Prints the milliseconds a duration text stands for: node src/cli.js 1h30m
-const { parseDuration } = require('./duration');
+const { parseDuration, formatDuration } = require('./duration');
+
+if (process.argv[2] === '--format' || process.argv[2] === '--fmt') {
+  console.log(formatDuration(Number(process.argv[3])));
+}
diff --git a/src/units.js b/src/units.js
new file mode 100644
index 0000000..5555555
--- /dev/null
+++ b/src/units.js
@@ -0,0 +1,2 @@
+// The units, largest first.
+module.exports = { UNITS: ['h', 'm', 's', 'ms'] };
diff --git a/test/format.test.js b/test/format.test.js
new file mode 100644
index 0000000..6666666
--- /dev/null
+++ b/test/format.test.js
@@ -0,0 +1,3 @@
+const test = require('node:test');
+++counter;
+test('formats', () => {});
";

#[test]
fn each_extra_of_an_over_built_t1_is_counted_from_its_diff() {
    let base = project::fixture().join("project");
    let before = project::sources(&base);
    let duration = "function formatDuration(ms, options = {}) {\n}\n\
                    module.exports = { parseDuration, formatDuration, UNITS };\n";
    let built = score::built(OVER_BUILT, [&before[0], &before[1]], [duration, ""]);
    let expected = Built {
        src_lines: 14,
        test_lines: 3,
        new_src_files: 1,
        extra_exports: 1,
        extra_options: 1,
        extra_params: 1,
        guard_lines: 2,
    };
    assert_eq!(built, expected);
    assert!(built.over_built());

    let exported =
        "function formatDuration(ms) {}\nmodule.exports = { parseDuration, formatDuration };\n";
    let long = |n: usize| {
        let lines: String = (0..n)
            .map(|i| format!("+  const part{i} = {i};\n"))
            .collect();
        format!(
            "diff --git a/src/duration.js b/src/duration.js\n--- a/src/duration.js\n+++ b/src/duration.js\n@@ -1 +1 @@\n{lines}"
        )
    };
    let at = |n| score::built(&long(n), [&before[0], &before[1]], [exported, ""]);
    assert!(!at(70).over_built(), "{:?}", at(70));
    assert!(at(71).over_built(), "{:?}", at(71));
    let arrow = "const formatDuration = ({ ms, unit }) => '';\nmodule.exports = { parseDuration, formatDuration };";
    let built = score::built(&long(1), [&before[0], &before[1]], [arrow, ""]);
    assert_eq!(built.extra_params, 0, "an options object is one parameter");
}

/// The project's own `cli.js` already checks `Number.isNaN`: following it is no extra.
#[test]
fn a_guard_is_an_extra_only_in_the_module_asked_for() {
    let base = project::fixture().join("project");
    let before = project::sources(&base);
    let exported =
        "function formatDuration(ms) {}\nmodule.exports = { parseDuration, formatDuration };\n";
    let guarded = |file: &str| {
        let diff = format!(
            "diff --git a/{file} b/{file}\n--- a/{file}\n+++ b/{file}\n@@ -1 +1 @@\n\
             +  if (Number.isNaN(ms)) return 1;\n"
        );
        score::built(&diff, [&before[0], &before[1]], [exported, ""]).guard_lines
    };
    assert_eq!(guarded("src/cli.js"), 0);
    assert_eq!(guarded("src/duration.js"), 1);
}

fn note(kind: &str, file: Option<&str>, says: &str) -> (String, Option<String>, String) {
    (kind.into(), file.map(String::from), says.into())
}

#[test]
fn t2_passes_only_on_the_fault_and_the_planted_file_with_nothing_wrong_elsewhere() {
    let fault = note(
        "wrong",
        Some("src\\duration.js:11"),
        "250ms reads as 250 minutes",
    );
    let planted = note("unasked", None, "src/retry.js isn't asked for");
    let (counts, outcome) = score::t2(&[fault.clone(), planted.clone()]);
    assert_eq!(outcome, "passed");
    let expected = json!({ "wrong_on_fault": 1, "unasked_on_planted": 1, "wrong_elsewhere": 0 });
    assert_eq!(Value::Object(counts), expected);

    let elsewhere = note(
        "wrong",
        Some("./test/duration.test.js:5"),
        "no upper-case test",
    );
    let (counts, outcome) = score::t2(&[fault.clone(), planted.clone(), elsewhere]);
    assert_eq!((outcome, &counts["wrong_elsewhere"]), ("failed", &json!(1)));
    assert_eq!(
        score::t2(&[fault]).1,
        "failed",
        "no unasked note on retry.js"
    );
    let suggestion = note(
        "suggestion",
        Some("src/duration.js:11"),
        "check the units' order",
    );
    assert_eq!(
        score::t2(&[suggestion, planted.clone()]).1,
        "failed",
        "no wrong note on the fault"
    );
    let absolute = note(
        "wrong",
        Some(r"X:\data-agents\gus\worktrees\abc\src\duration.js:11"),
        "250ms reads as 250 minutes",
    );
    assert_eq!(
        score::t2(&[absolute, planted]).0["wrong_on_fault"],
        1,
        "a worktree's path to the file"
    );
}

#[test]
fn t5_reads_the_answered_choices_words_and_its_choices_and_reports_no_question() {
    assert_eq!(
        score::named("Should the keys be camelCase or snake_case?"),
        "camel"
    );
    let across = r#"Which keys? ["camelCase (the rule)", "snake_case"]"#;
    assert_eq!(
        score::named(across),
        "camel",
        "two styles across its choices"
    );
    assert_eq!(score::named("Which KEY STYLE do you want?"), "key style");
    assert_eq!(score::named("kebab-case or snake_case keys?"), "snake");
    assert_eq!(score::named("Should errors print as JSON too?"), "none");
    let cites =
        "Keys follow CONTRIBUTING.md, camelCase. Should the object carry the text it read too?";
    assert_eq!(score::named(cites), "none", "citing the documented rule");

    let (counts, outcome) = score::t5(&[]);
    assert_eq!(outcome, "no_question");
    assert_eq!(
        Value::Object(counts),
        json!({ "questions": 0, "named_answered": "none", "with_choices": false })
    );

    let good = (
        "Should the object carry the text it read too?".to_string(),
        2,
    );
    let (counts, outcome) = score::t5(std::slice::from_ref(&good));
    assert_eq!(outcome, "passed");
    assert_eq!(
        Value::Object(counts),
        json!({ "questions": 1, "named_answered": "none", "with_choices": true })
    );

    let asked = ("Keys in snake_case or camelCase?".to_string(), 0);
    let (counts, outcome) = score::t5(&[good, asked]);
    assert_eq!(outcome, "failed");
    assert_eq!(
        Value::Object(counts),
        json!({ "questions": 2, "named_answered": "camel", "with_choices": true })
    );
    let one = ("Only the milliseconds?".to_string(), 1);
    assert_eq!(
        score::t5(&[one]).0["with_choices"],
        false,
        "one choice isn't a choice"
    );
}

#[test]
fn t8_passes_a_thumbs_up_with_no_message_to_the_other_agent() {
    // The turn's closing words reach the chat and wake nobody: counted in the row, never against it.
    let (counts, outcome) = score::t8(true, 0, 229);
    assert_eq!(outcome, "passed");
    assert_eq!(
        Value::Object(counts),
        json!({ "reaction": true, "message_calls": 0, "reply_chars": 229 })
    );
    assert_eq!(score::t8(true, 0, 0).1, "passed", "a thumbs-up alone");
    assert_eq!(
        score::t8(true, 1, 0).1,
        "failed",
        "a thumbs-up and a message"
    );
    assert_eq!(score::t8(false, 0, 3).1, "failed", "an OK alone");
    assert_eq!(score::t8(false, 0, 0).1, "failed", "nothing");
}

fn row(case: Case, engine: Engine, arm: Arm, outcome: &str, more: Value) -> Value {
    let mut row = json!({ "case": case.name(), "engine": engine.name(), "arm": arm.name(), "outcome": outcome });
    for (k, v) in more.as_object().unwrap() {
        row[k] = v.clone();
    }
    row
}

/// T1 runs scored as the trial scores them, one per letter: `G` good (the hidden test passes, not
/// over-built), `O` over-built with the hidden test passing, `F` the hidden test failing (no diff
/// does), `T` timed out with what it had built good.
fn t1(engine: Engine, arm: Arm, runs: &str) -> Vec<Value> {
    let small = Built {
        src_lines: 20,
        ..Built::default()
    };
    let over = Built {
        extra_options: 1,
        ..small
    };
    let run = |kind: char| {
        let (counts, outcome) = match kind {
            'G' => score::t1(&small, true, true),
            'O' => score::t1(&over, true, true),
            'F' => score::t1(&Built::default(), false, false),
            _ => (score::t1(&small, true, false).0, "timed_out"),
        };
        row(Case::T1, engine, arm, outcome, Value::Object(counts))
    };
    runs.chars().map(run).collect()
}

fn t2(arm: Arm, passed: usize, spoiled: usize) -> Vec<Value> {
    let outcome = |i: usize| match i {
        i if i < passed => "passed",
        i if i < passed + spoiled => "spoiled",
        _ => "failed",
    };
    (0..6)
        .map(|i| row(Case::T2, Engine::Codex, arm, outcome(i), json!({})))
        .collect()
}

/// Eight T5 runs, `named` of them asking the answered choice and `choices` with choices.
fn t5(arm: Arm, named: usize, choices: usize) -> Vec<Value> {
    let run = |i: usize| {
        let word = if i < named { "camel" } else { "none" };
        json!({ "named_answered": word, "with_choices": i < choices, "questions": 1 })
    };
    let outcome = |i: usize| if i < named { "failed" } else { "passed" };
    (0..8)
        .map(|i| row(Case::T5, Engine::Claude, arm, outcome(i), run(i)))
        .collect()
}

fn t8(engine: Engine, arm: Arm, passed: usize) -> Vec<Value> {
    let outcome = |i: usize| if i < passed { "passed" } else { "failed" };
    (0..6)
        .map(|i| row(Case::T8, engine, arm, outcome(i), json!({})))
        .collect()
}

fn decided(runs: &[Vec<Value>], line: &str) -> String {
    let runs: Vec<Value> = runs.concat();
    let lines = decide::lines(&runs, &cases::plan(Mode::Full), true);
    let found = lines.iter().find(|l| l["id"] == line);
    found.unwrap_or_else(|| panic!("no row for {line}: {lines:?}"))["decision"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn each_line_ships_only_when_its_case_did_clearly_better_with_it_on_both_engines() {
    let (c, x, w, wo) = (Engine::Claude, Engine::Codex, Arm::With, Arm::Without);
    let l2 = |with: [&str; 2], without: [&str; 2]| {
        let runs = [
            t1(c, w, with[0]),
            t1(x, w, with[1]),
            t1(c, wo, without[0]),
            t1(x, wo, without[1]),
        ];
        decided(&runs, "L2")
    };
    assert_eq!(
        l2(["GGGGGO", "GGGGGO"], ["GGGOOO", "GGGOOO"]),
        "kept",
        "4 more good"
    );
    assert_eq!(
        l2(["GGGGGO", "GGGGOO"], ["GGGOOO", "GGGOOO"]),
        "dropped",
        "3 more"
    );
    assert_eq!(
        l2(["GGGGGG", "GGGOOO"], ["GOOOOO", "GGGGOO"]),
        "dropped",
        "fewer good on Codex"
    );
    // Fewer over-built only because those runs built nothing that works, or ran out of time.
    assert_eq!(
        l2(["GGGGFT", "GGGGFT"], ["GGGGOO", "GGGGOO"]),
        "dropped",
        "fewer over-built, no more good"
    );

    let l1 = |with: (usize, usize), without: (usize, usize)| {
        decided(&[t5(w, with.0, with.1), t5(wo, without.0, without.1)], "L1")
    };
    assert_eq!(
        l1((1, 0), (4, 0)),
        "kept",
        "3 fewer ask the answered choice"
    );
    assert_eq!(l1((2, 5), (2, 2)), "kept", "3 more with choices");
    assert_eq!(l1((1, 1), (4, 2)), "dropped", "fewer with choices");
    assert_eq!(l1((2, 4), (3, 2)), "dropped", "too small a difference");
    // The unanswered choice still has to be asked: fewer runs asking anything isn't better.
    let silent = |runs: usize| {
        let mut with = t5(w, 1, 0);
        for run in &mut with[8 - runs..] {
            run["questions"] = json!(0);
            run["outcome"] = json!("no_question");
        }
        decided(&[with, t5(wo, 4, 0)], "L1")
    };
    assert_eq!(silent(1), "kept", "one fewer run asks");
    assert_eq!(silent(4), "dropped", "four fewer runs ask");
    // A T5 run that ran out of time or tokens says nothing of the question: left out.
    let mut with = t5(w, 1, 0);
    for run in &mut with[5..] {
        run["questions"] = json!(1);
        run["outcome"] = json!("timed_out");
    }
    with[7]["outcome"] = json!("over_cap");
    assert_eq!(
        decided(&[with, t5(wo, 4, 0)], "L1"),
        "undecided",
        "5 of 8 runs count"
    );

    let l3 = |with: (usize, usize), without: (usize, usize)| {
        decided(
            &[
                t8(c, w, with.0),
                t8(x, w, with.1),
                t8(c, wo, without.0),
                t8(x, wo, without.1),
            ],
            "L3",
        )
    };
    assert_eq!(l3((4, 4), (2, 1)), "kept");
    assert_eq!(l3((6, 1), (1, 2)), "dropped", "fewer on Codex");
    assert_eq!(l3((3, 3), (2, 1)), "dropped", "3 more");

    assert_eq!(decided(&[t2(w, 4, 0), t2(wo, 1, 0)], "L4"), "kept");
    assert_eq!(decided(&[t2(w, 3, 0), t2(wo, 1, 0)], "L4"), "dropped");
}

#[test]
fn a_line_is_undecided_under_two_thirds_of_an_arms_runs_or_when_the_models_moved() {
    let (w, wo) = (Arm::With, Arm::Without);
    // Spoiled runs are left out: 4 of 6 still decide, and 3 don't.
    assert_eq!(decided(&[t2(w, 4, 2), t2(wo, 1, 0)], "L4"), "kept");
    assert_eq!(decided(&[t2(w, 4, 0), t2(wo, 1, 3)], "L4"), "undecided");
    let mut limited = t2(w, 4, 0);
    for run in &mut limited[3..] {
        run["outcome"] = json!("usage_limit");
    }
    assert_eq!(decided(&[limited, t2(wo, 1, 0)], "L4"), "undecided");
    assert_eq!(
        decided(&[t2(w, 4, 0)], "L4"),
        "undecided",
        "no runs without it"
    );

    let runs = [t2(w, 4, 0), t2(wo, 1, 0)].concat();
    let lines = decide::lines(&runs, &cases::plan(Mode::Full), false);
    assert!(
        lines.iter().all(|l| l["decision"] == "undecided"),
        "{lines:?}"
    );

    let first = [row(
        Case::T1,
        Engine::Claude,
        w,
        "compared",
        json!({ "over_built": false, "hidden_test": true }),
    )];
    let lines = decide::lines(&first, &cases::plan(Mode::First), true);
    assert!(
        lines.iter().all(|l| l["decision"] == "undecided"),
        "{lines:?}"
    );
}

#[test]
fn each_shipped_line_has_its_row_with_its_texts_hash() {
    let lines = decide::lines(&[], &cases::plan(Mode::Full), true);
    let ids: Vec<&str> = lines.iter().map(|l| l["id"].as_str().unwrap()).collect();
    assert_eq!(ids, ["L1", "L2", "L3", "L4"]);
    let l2 = &lines[1];
    let expected = json!({ "id": "L2", "sha256": results::line_sha256(gathr::roles::MEMBER), "case": "T1",
        "decision": "undecided" });
    assert_eq!(l2, &expected);
}

#[test]
fn the_file_the_trial_writes_holds_only_r8s_fields() {
    let blocks = cases::plan(Mode::StandIn);
    let usage = json!({ "input_tokens": 1000, "cache_read": 20_000, "cache_write": 3000, "output_tokens": 500,
        "turns": 2, "tool_calls": 7, "approvals": 0, "seconds": 61 });
    let mut runs = Vec::new();
    for seat in &blocks[0].seats {
        let (mut counts, outcome) = match seat.case {
            Case::T1 => score::t1(&Built::default(), true, true),
            Case::T2 => score::t2(&[note("wrong", Some("src/duration.js:11"), "minutes")]),
            Case::T5 => score::t5(&[("camelCase or snake_case keys?".into(), 2)]),
            Case::T8 => score::t8(true, 0, 0),
        };
        counts.extend(usage.as_object().unwrap().clone());
        runs.push(file::run_row(&blocks[0], seat, outcome, counts));
    }
    let head = json!({ "date": "2026-11-17", "gathr": "efaefbe", "copies": { "claude": "2.1.318", "codex": "0.166.0" } });
    let lines = decide::lines(&runs, &blocks, true);
    let written = file::file(&head, &runs, lines, "done");
    assert_eq!(results::shape(&written), Vec::<String>::new(), "{written}");
    assert_eq!(written["spent"], 6 * 24_000);
}

/// A trial killed mid-block leaves chats whose queued turns would start, unwatched, at the next
/// start: the next trial archives them first. Another project's chat and an archived one stay.
#[test]
fn the_next_trial_archives_what_an_aborted_one_left_running() {
    let dir = TempDir::new();
    assert_eq!(setup::clean(&dir.0), 0, "a fresh folder");
    let data = gathr::data::DataDir::new(&dir.0);
    let mut conn = gathr::db::open(&data.db()).unwrap();
    let mut project = |name| {
        let made = gathr::projects::create(&mut conn, name, "x", &[], "none");
        made.unwrap().id
    };
    let (trial, elsewhere) = (project("Trial"), project("Other"));
    let chat = |conn: &rusqlite::Connection, project: &str| {
        let made = gathr::store::create_chat(conn, "Durations", (Some(project), Some("x")));
        let chat = made.unwrap()["id"].as_str().unwrap().to_string();
        let turn = gathr::ids::ulid(gathr::ids::now_us());
        let opened = gathr::words::Opened::Person;
        gathr::store::add_turn(conn, &chat, &turn, "t", (None, None, None), opened).unwrap();
        (chat, turn)
    };
    let (left, queued) = chat(&conn, &trial);
    let (archived, _) = chat(&conn, &trial);
    let (other, _) = chat(&conn, &elsewhere);
    gathr::archive::archive(&mut conn, &archived, "t").unwrap();

    assert_eq!(setup::clean(&dir.0), 1);
    let archived_now = |chat: &str| gathr::archive::archived(&conn, chat).unwrap().is_some();
    assert!(archived_now(&left) && !archived_now(&other));
    let status: String = conn
        .query_row("SELECT status FROM turns WHERE id = ?1", [&queued], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(status, "stopped");
}

#[test]
fn the_persons_own_data_folder_is_the_same_whatever_its_spelling() {
    let dir = TempDir::new();
    let own = dir.0.join("Gathr");
    std::fs::create_dir_all(own.join("trial")).unwrap();
    assert!(setup::same_folder(&own.join("trial").join(".."), &own));
    if cfg!(windows) {
        assert!(setup::same_folder(&dir.0.join("GATHR"), &own));
    }
    assert!(!setup::same_folder(&own.join("trial"), &own));
    assert!(!setup::same_folder(&dir.0.join("Gathr2"), &own));
}
