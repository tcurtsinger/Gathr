//! Stage 4: a turn opens with what Gathr already holds from GitHub, so its agent needn't ask. A turn
//! woken by CI that failed, a wait on a PR, a commit or several things, here or in another repository,
//! or a PR Gathr can't land on red CI, opens with the failed jobs' last lines at the commit that
//! failed; a turn on a task's PR, the lead's too, with its branch, head and files, told once per head.
//! A GitHub read that fails or stalls lets the turn through without them, found from its reference.
use common::replay::{escaped, read_in};

use super::lands::{looking, passed, running};
use super::*;

/// The calls Gathr's `failed_at` makes, as the stand-in matches them.
const RUNS: &str = ".check_runs[] | {name, conclusion: (.conclusion // \"\" | ascii_upcase), detailsUrl: .details_url}";
const STATUSES: &str =
    ".statuses[] | {context, state: (.state | ascii_upcase), targetUrl: .target_url}";
/// The fields Gathr's read of a PR asks for: the `pr` tool's, and a turn's opening's.
const VIEW: &str = "number,title,state,headRefOid,headRefName,baseRefName,mergeable,reviewDecision,statusCheckRollup,files";

fn at(repo: Option<&str>, commit: &str, what: &str) -> String {
    let repo = repo.unwrap_or("{owner}/{repo}");
    format!("repos/{repo}/commits/{commit}/{what}?per_page=100")
}

/// GitHub's answers for the failed checks at `commit`, in `repo` or the folder's own: one Actions
/// job, `job`, whose log ends in an error naming the commit.
fn failed_in(repo: Option<&str>, commit: &str, job: &str) -> Vec<Value> {
    let run = json!({ "name": "Rust (macOS)", "conclusion": "FAILURE",
        "detailsUrl": format!("https://github.com/o/r/actions/runs/7/job/{job}") });
    let log = format!("step\t2026-11-19T10:00:00.0000000Z ##[error]test failed at {commit}\n");
    let mut read_log = vec!["run", "view", "--job", job, "--log-failed"];
    read_log.extend(repo.map(|r| ["-R", r]).into_iter().flatten());
    let notes = format!(
        "repos/{}/check-runs/{job}/annotations",
        repo.unwrap_or("{owner}/{repo}")
    );
    vec![
        json!({ "args": ["api", "--paginate", at(repo, commit, "check-runs"), "--jq", RUNS], "out": run.to_string() }),
        json!({ "args": ["api", "--paginate", at(repo, commit, "status"), "--jq", STATUSES], "out": "" }),
        json!({ "args": read_log, "out": log }),
        json!({ "args": ["api", notes], "out": "[]" }),
    ]
}

fn failed_jobs(commit: &str, job: &str) -> Vec<Value> {
    failed_in(None, commit, job)
}

/// What the agent reads at a turn's opening for those failed checks.
fn opened_with(commit: &str) -> String {
    format!(
        "Failed checks at {}:\n\n## Rust (macOS) (failure)\n##[error]test failed at {commit}",
        &commit[..7]
    )
}

const FIVE: &str = "#5 is room/five at";

/// Vale's review of #5 (task #1) runs, with #5 changing one file, and in it Vale leaves `wait`, on
/// CI that is still running; `more` of GitHub's answers come ahead of that, and `env` is more of the
/// service's environment. Vale's engine holds its turn: `review_ends` lets it go.
fn waiting(
    (data, repos): (&Path, &Path),
    env: &[(&str, &Path)],
    wait: impl Fn(&Room) -> Value,
    more: impl Fn(&Room) -> Vec<Value>,
) -> (Room, String, PathBuf) {
    let mut room = room_replay_with(data, repos, false, true, env);
    room.ci(0, json!([running("CI")]));
    let one = json!([{ "path": "feature.txt", "additions": 1, "deletions": 0 }]);
    room.view(0, |view| view["files"] = one.clone());
    let mut answers = room.answers();
    let all = answers.as_array_mut().unwrap();
    all.splice(2..2, more(&room));
    let going =
        json!({ "name": "CI", "status": "in_progress", "context": "CI", "state": "pending" });
    all.push(json!({ "args": ["api"], "out": going.to_string() }));
    put_answers(data, &answers);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let lead_cwd = PathBuf::from(nth(&data.join("env.jsonl"), 0)["cwd"].as_str().unwrap());
    let vale_cwd = PathBuf::from(nth(&data.join("env.jsonl"), 1)["cwd"].as_str().unwrap());
    std::fs::write(lead_cwd.join("go"), "").unwrap();
    let left = room.call(&vale, "wait", wait(&room));
    assert_eq!(left["isError"], false, "{}", text(&left));
    (room, vale, vale_cwd)
}

impl Room {
    /// The first turn opened by `opened`, once there is one.
    fn opened_by(&self, opened: &str) -> String {
        let sql = "SELECT id FROM turns WHERE opened_by = ?1 ORDER BY created_ts LIMIT 1";
        common::until(120, &format!("a turn opened by {opened}"), || {
            self.db().query_row(sql, [opened], |r| r.get(0)).ok()
        })
    }

    /// What the engine read in `turn`, once it holds `words`.
    fn read(&self, turn: &str, words: &str) -> String {
        common::until(120, &format!("turn {turn} to read {words:?}"), || {
            let read = read_in(&self.data, turn);
            read.contains(&escaped(words)).then_some(read)
        })
    }

    /// Vale's review is done on #5's head and its turn ends, held on the wait. Then every check at
    /// every commit fails, with `then` of GitHub's answers ahead of all: the wake comes next.
    fn review_ends(&self, vale: &str, vale_cwd: &Path, then: Vec<Value>) {
        self.reviewed(vale, 1, &self.heads[0]);
        std::fs::write(vale_cwd.join("go"), "").unwrap();
        let sql = "SELECT state FROM tasks WHERE number = 1";
        common::until(120, "the review held on its wait", || {
            let state: String = self.db().query_row(sql, [], |r| r.get(0)).ok()?;
            (state == "held").then_some(())
        });
        let mut answers = self.answers();
        let all = answers.as_array_mut().unwrap();
        let red = json!({ "name": "CI", "status": "completed", "conclusion": "failure", "context": "CI", "state": "failure" });
        all.last_mut().unwrap()["out"] = json!(red.to_string());
        all.splice(0..0, then);
        put_answers(&self.data, &answers);
    }

    /// The wake's opening read GitHub and failed: it went on without the lines, and the read's
    /// trace says why.
    fn woke_without_lines(&self) -> String {
        let read = self.read(&self.opened_by("wait_fired"), "Your wait ended");
        assert!(!read.contains("Failed checks at"), "{read}");
        let sql = "SELECT ref FROM event_log WHERE name = 'gathr.github.read' AND status = 'error'";
        let found = common::until(30, "the failed read's event", || {
            let (ok, rows) = gathr(&self.data, &["diag", "query", sql]);
            assert!(ok, "{rows}");
            rows["rows"][0]["ref"].as_str().map(String::from)
        });
        let (ok, trace) = gathr(&self.data, &["diag", "trace", &found]);
        assert!(ok, "{trace}");
        trace.to_string()
    }
}

/// A wait on a PR's CI: the wake opens with the failed job at the head that failed, and the review's
/// conversation isn't told #5's branch and files again at the same head.
#[test]
fn a_failed_ci_wait_on_a_pr_opens_with_its_failed_jobs_and_the_pr_is_told_once() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let wait = |_: &Room| json!({ "ci": "#5", "then": "Merge it" });
    let failed = |room: &Room| failed_jobs(&room.heads[0], "88");
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, failed);
    let head = room.heads[0].clone();
    let first: String = room
        .db()
        .query_row(
            "SELECT id FROM turns WHERE task = 1 ORDER BY created_ts LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let told = format!(
        "{FIVE} {}, into main, and changes 1 file:\n- feature.txt (+1 -0)",
        &head[..7]
    );
    room.read(&first, &told);
    room.review_ends(&vale, &cwd, vec![]);

    let woke = room.opened_by("wait_fired");
    let read = room.read(&woke, &opened_with(&head));
    assert!(
        !read.contains(FIVE),
        "told #5 again at the same head: {read}"
    );
}

/// The PR's head moves after CI failed at it, before its wake opens: the failures are read at the
/// commit that failed, not at the head GitHub now shows. Only a read of the PR itself sees the push,
/// so Gathr's looks don't put the review out of date meanwhile.
#[test]
fn a_head_pushed_after_ci_failed_reads_the_failures_where_ci_failed() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let wait = |_: &Room| json!({ "ci": "#5", "then": "Merge it" });
    let failed = |room: &Room| failed_jobs(&room.heads[0], "88");
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, failed);
    let [failed, pushed] = room.heads.clone();
    let view = room.answers()[0]["out"].as_str().unwrap().to_string();
    let mut moved: Value = serde_json::from_str(&view).unwrap();
    moved["headRefOid"] = json!(pushed);
    let read_pr = json!({ "args": ["pr", "view", "5", "--json", VIEW], "out": moved.to_string() });
    room.review_ends(&vale, &cwd, vec![read_pr]);

    let woke = room.opened_by("wait_fired");
    room.read(&woke, &opened_with(&failed));
}

#[test]
fn a_failed_ci_wait_on_a_commit_opens_with_its_failed_jobs() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let wait = |room: &Room| json!({ "ci": room.heads[1], "then": "Merge it" });
    let failed = |room: &Room| failed_jobs(&room.heads[1], "90");
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, failed);
    room.review_ends(&vale, &cwd, vec![]);
    room.read(&room.opened_by("wait_fired"), &opened_with(&room.heads[1]));
}

/// A wait of several parts: its CI part failed, and its wake opens with that part's failed jobs.
#[test]
fn a_combined_wait_whose_ci_failed_opens_with_its_failed_jobs() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let wait = |room: &Room| json!({ "ci": room.heads[1], "agent": "Lead", "then": "Merge it" });
    let failed = |room: &Room| failed_jobs(&room.heads[1], "91");
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, failed);
    room.review_ends(&vale, &cwd, vec![]);
    room.read(&room.opened_by("wait_fired"), &opened_with(&room.heads[1]));
}

/// A wait on CI in another repository: its failures are read there, its job's log too.
#[test]
fn a_failed_ci_wait_on_another_repository_reads_its_jobs_there() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let elsewhere = "made-up/elsewhere";
    let wait = |room: &Room| json!({ "ci": room.heads[1], "repo": elsewhere, "then": "Merge it" });
    let failed = |room: &Room| failed_in(Some(elsewhere), &room.heads[1], "94");
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, failed);
    room.review_ends(&vale, &cwd, vec![]);
    room.read(&room.opened_by("wait_fired"), &opened_with(&room.heads[1]));
}

/// The lead's task runs in its one conversation, not a task's own: its turn opens with the PR too.
#[test]
fn the_leads_task_turn_on_a_pr_opens_with_its_branch_head_and_files() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room_replay(&data.0, &repos.0, false, true);
    room.ci(0, json!([running("CI")]));
    let one = json!([{ "path": "feature.txt", "additions": 1, "deletions": 0 }]);
    room.view(0, |view| view["files"] = one.clone());
    let lead = room.lead_turn();
    crate::common::fix_it(room.service.info.port, &lead);
    let own = common::briefed(json!({ "to": "Lead", "what": "Add the feature", "pr": 5 }));
    let given = text(&room.call(&lead, "give_task", own));
    assert!(given.starts_with("Task #1 for Lead"), "{given}");
    let lead_cwd = PathBuf::from(nth(&data.0.join("env.jsonl"), 0)["cwd"].as_str().unwrap());
    std::fs::write(lead_cwd.join("go"), "").unwrap();
    let sql = "SELECT id FROM turns WHERE task = 1 ORDER BY created_ts LIMIT 1";
    let turn: String = common::until(120, "the lead's task turn", || {
        room.db().query_row(sql, [], |r| r.get(0)).ok()
    });
    let head = &room.heads[0][..7];
    room.read(
        &turn,
        &format!("{FIVE} {head}, into main, and changes 1 file"),
    );
}

/// A job's log that can't be read fails the read: no partial lines, and its why is traced.
#[test]
fn a_failed_log_read_at_a_turns_opening_is_found_from_its_reference() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let wait = |room: &Room| json!({ "ci": room.heads[1], "then": "Merge it" });
    let down = |room: &Room| {
        let log = json!({ "args": ["run", "view", "--job", "95"], "code": 1, "err": "HTTP 502: Bad Gateway" });
        [vec![log], failed_jobs(&room.heads[1], "95")].concat()
    };
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &[], wait, down);
    room.review_ends(&vale, &cwd, vec![]);
    let trace = room.woke_without_lines();
    let why = "Rust (macOS)'s log wasn't read";
    assert!(
        trace.contains(why) && trace.contains("Bad Gateway") && trace.contains(&room.heads[1]),
        "{trace}"
    );
}

/// GitHub stalls: the turn waits no longer than the limit, then starts without the lines.
#[test]
fn a_stalled_github_read_lets_the_wake_through_and_says_why() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let env = [("GATHR_OPENING_GITHUB_MS", Path::new("1500"))];
    let wait = |room: &Room| json!({ "ci": room.heads[1], "then": "Merge it" });
    let stalls = |room: &Room| {
        let runs = at(None, &room.heads[1], "check-runs");
        let slow = json!({ "args": ["api", "--paginate", runs, "--jq", RUNS], "out": "", "sleep_ms": 20000 });
        vec![slow]
    };
    let (room, vale, cwd) = waiting((&data.0, &repos.0), &env, wait, stalls);
    room.review_ends(&vale, &cwd, vec![]);
    let trace = room.woke_without_lines();
    assert!(trace.contains("GitHub took over 1.5 s"), "{trace}");
}

/// Both stops on red CI, required or not, wake the reviewer with the failed jobs at that head.
#[test]
fn a_pr_gathr_cant_land_on_red_ci_wakes_its_reviewer_with_the_failed_jobs() {
    for ci in ["none", "ci"] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let room = looking(&data.0, &repos.0, false);
        let head = room.heads[0].clone();
        let mut answers = room.answers();
        answers
            .as_array_mut()
            .unwrap()
            .splice(2..2, failed_jobs(&head, "92"));
        put_answers(&data.0, &answers);
        room.set("ci", ci);
        let failed = json!({ "__typename": "CheckRun", "name": "Rust (macOS)", "status": "COMPLETED", "conclusion": "FAILURE" });
        room.ci(0, json!([passed("Rust (Windows)"), failed]));
        room.review(1, 5, Some(&head));
        room.told_once();
        room.read(&room.opened_by("landing"), &opened_with(&head));
    }
}
