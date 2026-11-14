//! 0049: Gathr lands a pull request. A clean review is the word to land: once the newest review of
//! a PR covers its head and what the project asks for passed there, Gathr merges it, with nobody's
//! turn. Scenarios 10a, 10b, 10e and 10f, through the service with GitHub's stand-in.
use super::*;

/// The look the service makes at each task's PR, as it asks GitHub.
const PR_LOOK: &str = "state,headRefOid,headRefName,baseRefName,isCrossRepository";

/// A room whose service looks every 100 ms, as it does every 2 minutes, with GitHub answering an
/// update: it takes it, and the test makes the commit GitHub would.
fn looking(data: &Path, repos: &Path, test_change: bool) -> Room {
    let room = room_replay(data, repos, test_change, true);
    room.answer(
        json!({ "args": ["repo", "view"], "out": r#"{"defaultBranchRef":{"name":"main"}}"# }),
    );
    room
}

pub(super) fn passed(name: &str) -> Value {
    json!({ "__typename": "CheckRun", "name": name, "status": "COMPLETED", "conclusion": "SUCCESS" })
}

pub(super) fn running(name: &str) -> Value {
    json!({ "__typename": "CheckRun", "name": name, "status": "IN_PROGRESS", "conclusion": "" })
}

impl Room {
    fn answers(&self) -> Value {
        let text = std::fs::read_to_string(self.data.join("gh.json")).unwrap();
        serde_json::from_str(&text).unwrap()
    }

    /// One more answer of GitHub's.
    fn answer(&self, answer: Value) {
        let mut all = self.answers();
        all.as_array_mut().unwrap().push(answer);
        put_answers(&self.data, &all);
    }

    /// GitHub's view of PR `n` (0 is #5, 1 is #6), changed in place.
    fn view(&self, n: usize, change: impl Fn(&mut Value)) {
        let mut all = self.answers();
        for key in ["out", "out_merged"] {
            let mut view: Value = serde_json::from_str(all[n][key].as_str().unwrap()).unwrap();
            change(&mut view);
            all[n][key] = json!(view.to_string());
        }
        put_answers(&self.data, &all);
    }

    /// GitHub's CI on PR `n`'s head, as its view sums it up.
    pub(super) fn ci(&self, n: usize, checks: Value) {
        self.view(n, |view| view["statusCheckRollup"] = checks.clone());
    }

    fn set(&self, setting: &str, value: &str) {
        let path = format!("/api/projects/{}", self.project);
        let changed = self.service.send("PATCH", &path, json!({ setting: value }));
        assert_eq!(changed.status, 200, "{}", changed.body);
    }

    /// A review of `pr` given to Vale, as its turn left it: done on `commit`, or still open.
    fn review(&self, task: i64, pr: i64, commit: Option<&str>) {
        let sql = "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, pr, finish, reviewed,
                                      review_done, finish_ts, created_ts, updated_ts)
                   VALUES (?1, ?2, ?3, ?4, 'review', 'Review it', ?5, ?6, ?7, ?8, ?9, ?1, 1, 1)";
        let (state, finish) = match commit {
            Some(_) => ("done", Some("done")),
            None => ("ready", None),
        };
        let row = rusqlite::params![
            task,
            self.chat,
            self.lead,
            self.vale,
            state,
            pr,
            finish,
            commit,
            commit.is_some()
        ];
        self.db().execute(sql, row).unwrap();
    }

    /// A newer run of Gathr's checks at `head`, in `state`.
    fn run(&self, head: &str, state: &str) {
        let sql = "INSERT INTO check_runs (id, project_id, head, state, trace_id, started_ts)
                   VALUES (?1, ?2, ?3, ?4, 't', 5)
                   ON CONFLICT (id) DO UPDATE SET state = excluded.state";
        let row = rusqlite::params![format!("newer-{head}"), self.project, head, state];
        self.db().execute(sql, row).unwrap();
    }

    fn calls(&self, has: &str) -> usize {
        let calls = std::fs::read_to_string(self.data.join("gh.json.calls")).unwrap_or_default();
        calls.lines().filter(|line| line.contains(has)).count()
    }

    /// Returns once the service has looked at its PRs again, from start to end, landing included.
    fn looked(&self) {
        let before = self.calls(PR_LOOK);
        let prs: i64 = self
            .db()
            .query_row(
                "SELECT count(*) FROM tasks WHERE pr IS NOT NULL AND pr_closed = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        // A look asks once for each task with a PR, then lands what it can (reading the tasks and
        // GitHub afresh), and the next look's first ask shows that landing ended. What is proved
        // is that one landing began after this call and ended. With `prs` asks and one more: a look
        // with asks still to make now lands after them, and the next look's first ask comes within
        // the count; a look already landing now may have begun before the test's change, and the
        // count runs through the whole next look to the ask after it.
        let enough = before + prs as usize + 1;
        common::until(120, "the service to look at its PRs again", || {
            (self.calls(PR_LOOK) >= enough).then_some(())
        });
    }

    pub(super) fn landed(&self, head: &str) {
        common::until(120, "the PR to land", || {
            (self.main() == head).then_some(())
        });
    }

    fn chat(&self) -> Value {
        self.service
            .call("GET", &format!("/api/chats/{}", self.chat))
            .json()
    }

    /// What Gathr told a reviewer of a PR it can't land.
    fn told(&self) -> Vec<String> {
        let chat = self.chat();
        let said = chat["messages"].as_array().unwrap().iter();
        said.filter(|m| m["role"] == "gathr")
            .filter_map(|m| m["body"].as_str())
            .filter(|body| body.contains("can't land"))
            .map(String::from)
            .collect()
    }

    fn told_once(&self) -> String {
        common::until(120, "Gathr to tell the reviewer", || {
            self.told().into_iter().next()
        })
    }

    /// The default branch moves on, as when another PR merges.
    fn main_moves(&self) -> String {
        git(&self.other, &["checkout", "-q", "main"]);
        std::fs::write(self.other.join("main-update.txt"), "from main\n").unwrap();
        git(&self.other, &["add", "."]);
        git(&self.other, &["commit", "-q", "-m", "main update"]);
        git(&self.other, &["push", "-q", "origin", "main"]);
        self.main()
    }
}

/// Scenario 10e: a done review is the word to land. The reviewer records it and Gathr merges, in
/// that call: no wait on CI, no `merge`, no turn to be told.
#[test]
fn a_clean_review_lands_the_pr_and_its_reviewer_is_told_in_the_answer() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let done =
        json!({ "how": "done", "task": 1, "commit": room.heads[0], "nothing_unasked": true });
    let finished = room.call(&vale, "finish_task", done);
    let said = text(&finished);
    assert_eq!(finished["isError"], false, "{said}");
    assert!(said.contains("Merged #5"), "{said}");
    assert_eq!(room.main(), room.heads[0], "main isn't the PR's head");
    let closed: bool = room
        .db()
        .query_row("SELECT pr_closed FROM tasks WHERE number = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(closed, "the PR's tasks weren't settled");
}

/// Scenario 10a, and the room's #100 and #102, which sat green and unmerged until someone noticed
/// (constructed): whatever the project asks for holds the PR until it passed at that commit, and
/// then it lands with no agent's turn. With Required checks at none, the review lands it once no
/// CI run is going.
#[test]
fn a_reviewed_pr_lands_by_itself_once_what_the_project_asks_for_passed() {
    for ci in ["gathr", "ci", "both", "none"] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let room = looking(&data.0, &repos.0, false);
        room.set("ci", ci);
        let head = room.heads[0].clone();
        if ci == "gathr" {
            room.run(&head, "running");
        } else {
            room.ci(
                0,
                json!([passed("Rust (Windows)"), running("Rust (macOS)")]),
            );
        }
        room.review(1, 5, Some(&head));
        room.looked();
        assert_eq!(room.main(), room.base, "{ci}: landed before it passed");
        if ci == "both" {
            // GitHub's CI passes while Gathr's own run is still going: both, not either.
            room.run(&head, "running");
            room.ci(0, json!([passed("Rust (Windows)"), passed("Rust (macOS)")]));
            room.looked();
            assert_eq!(room.main(), room.base, "landed on CI alone");
        }
        if ci == "gathr" || ci == "both" {
            room.run(&head, "passed");
        } else {
            room.ci(0, json!([passed("Rust (Windows)"), passed("Rust (macOS)")]));
        }
        room.landed(&head);
        let turns = room.chat()["turns"].as_array().unwrap().len();
        assert_eq!(turns, 0, "{ci}: landing took an agent's turn");
    }
}

/// Scenarios 10a and 10f: a CI failure the project doesn't require leaves the PR to its reviewer,
/// who is woken on no task, once for that head, with what Gathr saw: across looks and a restart.
/// Its approval stands: once the job passes at that head, the PR lands.
#[test]
fn a_pr_that_cant_land_wakes_its_reviewer_once_for_that_head() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = looking(&data.0, &repos.0, false);
    room.set("ci", "none");
    let head = room.heads[0].clone();
    let failed = json!({ "__typename": "CheckRun", "name": "Rust (macOS)", "status": "COMPLETED", "conclusion": "FAILURE" });
    room.ci(0, json!([passed("Rust (Windows)"), failed]));
    room.review(1, 5, Some(&head));
    let told = room.told_once();
    assert!(
        told.contains("#5")
            && told.contains(&head[..7])
            && told.contains("1 failed (Rust (macOS))"),
        "{told}"
    );
    let chat = room.chat();
    let woken: Vec<&Value> = chat["turns"].as_array().unwrap().iter().collect();
    assert_eq!(woken.len(), 1, "{chat}");
    assert_eq!(woken[0]["agent"], room.vale.as_str(), "{chat}");
    assert!(woken[0]["task"].is_null(), "woken on a task: {chat}");
    room.looked();
    assert_eq!(room.told().len(), 1, "told again at a later look");
    assert_eq!(room.main(), room.base, "landed over a CI failure");

    let _ = room.service.call("POST", "/api/shutdown");
    room.service.child.wait().unwrap();
    room.service = service(&room.data, true);
    room.looked();
    assert_eq!(room.told().len(), 1, "told again after a restart");

    room.ci(0, json!([passed("Rust (Windows)"), passed("Rust (macOS)")]));
    room.landed(&head);
}

/// Scenario 10b: a PR behind its base is brought up to date by Gathr, once for that head, and
/// lands when its checks pass again at the new one. Nobody is woken.
#[test]
fn a_pr_behind_its_base_is_brought_up_to_date_and_lands_when_its_checks_pass_again() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    room.answer(json!({ "args": ["api", "-X", "PUT"], "out": r#"{"message":"Updating pull request branch."}"# }));
    let base = room.main_moves();
    room.checks_passed(&base);
    room.review(1, 5, Some(&room.heads[0]));
    common::until(120, "Gathr to ask GitHub for the update", || {
        (room.calls("update-branch") > 0).then_some(())
    });
    let pinned = format!("expected_head_sha={}", room.heads[0]);
    assert_eq!(room.calls(&pinned), 1, "pinned to the head Gathr saw");
    // The commit GitHub makes: the base merged into the PR's branch. Its checks run again.
    git(&room.other, &["checkout", "-q", "room/five"]);
    git(&room.other, &["merge", "--no-ff", "-m", "update", "main"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]).trim().to_string();
    room.run(&head, "running");
    git(&room.other, &["push", "-q", "origin", "room/five"]);
    let mut all = room.answers();
    all[0]["merged_if"]["head"] = json!(head);
    put_answers(&room.data, &all);
    room.view(0, |view| {
        view["headRefOid"] = json!(head);
        view["statusCheckRollup"] = json!([passed("Rust (Windows)")]);
    });
    room.looked();
    assert_eq!(room.main(), base, "landed before its checks passed again");
    assert_eq!(room.calls("update-branch"), 1, "updated more than once");
    room.run(&head, "passed");
    room.landed(&head);
    assert_eq!(room.told(), Vec::<String>::new(), "someone was woken");
}

/// Scenario 10f: an update that conflicts is the reviewer's to judge: woken once, and the update
/// isn't asked for again at that head.
#[test]
fn an_update_that_conflicts_wakes_the_reviewer_once() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    let conflict = "HTTP 422: merge conflict between base and head";
    room.answer(json!({ "args": ["api", "-X", "PUT"], "err": conflict, "code": 1 }));
    let base = room.main_moves();
    room.review(1, 5, Some(&room.heads[0]));
    let told = room.told_once();
    assert!(told.contains("#5") && told.contains("conflict"), "{told}");
    // One ask, which `gh` tries once more when it fails (G5).
    let asked = room.calls("update-branch");
    room.looked();
    assert_eq!(room.told().len(), 1, "told again");
    assert_eq!(room.calls("update-branch"), asked, "asked again");
    assert_eq!(room.main(), base);
}

/// Scenario 10e: an older approval never lands a PR a newer review holds back, and an approval
/// never reaches past an author's commit.
#[test]
fn a_pr_never_lands_on_an_older_approval_or_past_its_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    // #6: its author commits after the review.
    git(&room.other, &["checkout", "-q", "room/six"]);
    std::fs::write(room.other.join("author.txt"), "not reviewed\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "author change"]);
    git(&room.other, &["push", "-q", "origin", "room/six"]);
    let moved = git(&room.other, &["rev-parse", "HEAD"]).trim().to_string();
    room.checks_passed(&moved);
    room.view(1, |view| view["headRefOid"] = json!(moved));
    room.review(1, 6, Some(&room.heads[1]));
    // #5: approved, then given a newer review that is still open.
    room.review(3, 5, None);
    room.review(2, 5, Some(&room.heads[0]));
    room.looked();
    assert_eq!(room.main(), room.base, "landed");
    assert_eq!(
        room.told(),
        Vec::<String>::new(),
        "nobody is woken for either"
    );

    let done = "UPDATE tasks SET state = 'done', finish = 'done', reviewed = ?1, review_done = 1, finish_ts = 9
                WHERE number = 3";
    room.db().execute(done, [&room.heads[0]]).unwrap();
    room.landed(&room.heads[0]);
}

/// Scenario 10e: Gathr lands into the default branch only. Another base is left to its reviewer.
#[test]
fn a_pr_into_a_branch_that_isnt_the_default_is_left_to_its_reviewer() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    git(&room.other, &["push", "-q", "origin", "main:release"]);
    room.view(0, |view| view["baseRefName"] = json!("release"));
    room.review(1, 5, Some(&room.heads[0]));
    let told = room.told_once();
    assert!(told.contains("release") && told.contains("main"), "{told}");
    room.looked();
    assert_eq!(room.told().len(), 1, "told again");
    let release = git(&room.origin, &["rev-parse", "refs/heads/release"]);
    assert_eq!(release.trim(), room.base, "landed into another branch");
    assert_eq!(room.main(), room.base);
}

/// Found in #184's review. A fork's PR is never landed, though its head is in the project (someone
/// fetched it): its reviewer is told. And a review that doesn't reach the head wakes nobody,
/// whatever else is wrong with the PR.
#[test]
fn a_forks_pr_is_left_to_its_reviewer_and_a_stale_review_wakes_nobody() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    // #6 is into another branch, and reviewed at a commit its head has moved past.
    git(&room.other, &["push", "-q", "origin", "main:release"]);
    room.view(1, |view| view["baseRefName"] = json!("release"));
    room.review(1, 6, Some(&room.base));
    room.looked();
    assert_eq!(
        room.told(),
        Vec::<String>::new(),
        "woken for a stale review"
    );
    // #5 is a fork's, reviewed at its head, with everything else in order.
    room.view(0, |view| view["isCrossRepository"] = json!(true));
    room.review(2, 5, Some(&room.heads[0]));
    let told = room.told_once();
    assert!(told.contains("#5") && told.contains("fork"), "{told}");
    room.looked();
    assert_eq!(room.told().len(), 1, "told again, or for #6");
    assert_eq!(room.main(), room.base, "a fork's PR landed");
}

/// Scenario 10e: a PR that changes tests needs its reviewer's sign-off, said with the review and
/// kept with it. Without it the answer says so and nothing lands; the same review recorded again
/// with it lands the PR.
#[test]
fn sign_off_given_with_the_review_lands_a_pr_that_changes_tests() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, true);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[6]);
    let done =
        json!({ "how": "done", "task": 1, "commit": room.heads[1], "nothing_unasked": true });
    let unsigned = room.call(&vale, "finish_task", done.clone());
    let said = text(&unsigned);
    assert_eq!(unsigned["isError"], false, "{said}");
    assert!(
        said.contains("tests/check.sh") && said.contains("signed_off: true"),
        "{said}"
    );
    assert_eq!(room.main(), room.base, "landed without sign-off");

    let mut signed = done;
    signed["signed_off"] = json!(true);
    let finished = room.call(&vale, "finish_task", signed);
    let said = text(&finished);
    assert_eq!(finished["isError"], false, "{said}");
    assert!(said.contains("Merged #6"), "{said}");
    assert_eq!(room.main(), room.heads[1]);
    assert_eq!(room.told(), Vec::<String>::new(), "told twice: no turn too");
}

/// Scenario 10f: a push GitHub doesn't confirm settles nothing and isn't tried again: the PR looks
/// settle the tasks once GitHub reports the merge.
#[test]
fn a_push_github_doesnt_confirm_settles_nothing_and_isnt_tried_again() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let room = looking(&data.0, &repos.0, false);
    room.set("merge", "merge");
    // GitHub goes on showing the PR open.
    let mut all = room.answers();
    all[0].as_object_mut().unwrap().remove("merged_if");
    put_answers(&room.data, &all);
    room.review(1, 5, Some(&room.heads[0]));
    let unconfirmed = || {
        let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.merge.done'
                   AND error_type = 'unconfirmed'";
        let (ok, rows) = gathr(&room.data, &["diag", "query", sql]);
        assert!(ok, "{rows}");
        rows["rows"][0]["n"].as_i64().unwrap()
    };
    common::until(120, "the push GitHub doesn't confirm", || {
        (unconfirmed() > 0).then_some(())
    });
    let pushed = room.main();
    assert_ne!(pushed, room.base, "nothing was pushed");
    room.looked();
    assert_eq!(unconfirmed(), 1, "tried again");
    assert_eq!(room.main(), pushed, "pushed again");
    let closed: bool = room
        .db()
        .query_row("SELECT pr_closed FROM tasks WHERE number = 1", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert!(!closed, "a PR GitHub shows open was settled as merged");
}
