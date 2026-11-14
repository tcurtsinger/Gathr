//! 0043 C4: agents don't merge; a PR's reviewer calls `merge`, which Gathr refuses, with a
//! reference, unless the review is done on the PR's head, the project's checks passed there and
//! the default branch is in it. One merge per repository at a time; the PR's tasks settle at once.

mod common;
#[path = "merge/lands.rs"]
mod lands;
#[path = "merge/review_questions.rs"]
mod review_questions;
#[path = "merge/review_updates.rs"]
mod review_updates;

use std::path::{Path, PathBuf};

use common::{
    TempDir, TestService, call_tool, fake_gh, gathr, git, held_recording, nth, project, text,
};
use serde_json::{Value, json};

/// Lets the held engines go however a test ends: dropped first, before the service.
struct Release(PathBuf, bool);

impl Drop for Release {
    fn drop(&mut self) {
        if self.1 {
            let starts =
                std::fs::read_to_string(self.0.with_file_name("env.jsonl")).unwrap_or_default();
            for line in starts.lines() {
                if let Ok(start) = serde_json::from_str::<Value>(line)
                    && let Some(cwd) = start["cwd"].as_str()
                {
                    let _ = std::fs::write(Path::new(cwd).join("go"), "");
                }
            }
        }
        let _ = std::fs::write(&self.0, "");
    }
}

struct Room {
    _release: Release,
    service: TestService,
    data: PathBuf,
    chat: String,
    project: String,
    lead: String,
    vale: String,
    ann: String,
    /// The clone that pushes PR branches and moves main, as other people would.
    other: PathBuf,
    origin: PathBuf,
    /// The default branch's commit the PRs start from.
    base: String,
    /// Each PR's head: #5 and #6.
    heads: [String; 2],
    /// Every engine started so far.
    started: usize,
}

/// A project with a Supervisor (Lead) and a Reviewer (Vale), two PRs on origin (#5 adds
/// `feature.txt`, #6 `other.txt`, or `tests/check.sh` with `test_change`), the project's checks
/// passed on main and on both heads, and `gh` answering for both PRs.
fn room(data: &Path, repos: &Path, test_change: bool) -> Room {
    room_replay(data, repos, test_change, false)
}

/// In the CI-wake fixture, each engine consumes its own gate and can hold a second turn.
fn room_replay(data: &Path, repos: &Path, test_change: bool, waking: bool) -> Room {
    let (folder, _) = project(repos);
    // A merge commit is made as the person who owns the checkout.
    git(&folder, &["config", "user.name", "Test"]);
    git(&folder, &["config", "user.email", "test@example.invalid"]);
    let other = repos.join("other");
    let base = git(&other, &["rev-parse", "HEAD"]).trim().to_string();
    let mut heads = Vec::new();
    for (branch, file) in [
        ("room/five", "feature.txt"),
        (
            "room/six",
            if test_change {
                "tests/check.sh"
            } else {
                "other.txt"
            },
        ),
    ] {
        git(&other, &["checkout", "-q", "-b", branch, &base]);
        let path = other.join(file);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "made up\n").unwrap();
        git(&other, &["add", "."]);
        git(&other, &["commit", "-q", "-m", file]);
        git(&other, &["push", "-q", "origin", branch]);
        heads.push(git(&other, &["rev-parse", "HEAD"]).trim().to_string());
    }
    git(&other, &["checkout", "-q", "main"]);
    let answers = data.join("gh.json");
    let origin = repos.join("origin.git");
    let view = |n: usize| {
        let pr = |state: &str| {
            json!({ "state": state, "headRefOid": heads[n], "baseRefName": "main", "statusCheckRollup": [], "headRefName": if n == 0 { "room/five" } else { "room/six" }, "isCrossRepository": false })
                .to_string()
        };
        let when = json!({ "repo": origin, "base": "main", "head": heads[n] });
        json!({ "args": ["pr", "view", (n + 5).to_string()], "out": pr("OPEN"), "out_merged": pr("MERGED"), "merged_if": when })
    };
    std::fs::write(&answers, json!([view(0), view(1)]).to_string()).unwrap();
    let recording = held_recording(data);
    if waking {
        let held = std::fs::read_to_string(&recording)
            .unwrap()
            .replace("{\"dir\":\"hold\"}", "{\"dir\":\"hold\",\"line\":\"take\"}");
        std::fs::write(&recording, format!("{}\n{}", held.trim_end(), held)).unwrap();
    }
    let service = service(data, waking);
    let account = service.add_account("Team");
    common::sign_in_account(data, &account);
    let agent = |name: &str| {
        let made = service.send(
            "POST",
            "/api/agents",
            json!({ "name": name, "account": account }),
        );
        made.json()["id"].as_str().unwrap().to_string()
    };
    let (lead, vale, ann) = (agent("Lead"), agent("Vale"), agent("Ann"));
    let members = json!([
        { "agent": lead, "role": "supervisor" },
        { "agent": vale, "role": "reviewer" },
        { "agent": ann, "role": "member" }
    ]);
    let body = json!({ "name": "Site", "folder": folder, "members": members });
    let project = service.send("POST", "/api/projects", body).json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let chat = service
        .send("POST", "/api/chats", json!({ "project": project }))
        .json()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let room = Room {
        _release: Release(data.join("go"), waking),
        service,
        data: data.to_path_buf(),
        chat,
        project,
        lead,
        vale,
        ann,
        other,
        origin: repos.join("origin.git"),
        base,
        heads: [heads[0].clone(), heads[1].clone()],
        started: 0,
    };
    room.checks_passed(&room.base);
    room.checks_passed(&room.heads[0]);
    room.checks_passed(&room.heads[1]);
    room
}

/// GitHub's answers, replaced whole: the service asks every 100 ms in some rooms, and the stand-in
/// must never read half a file (it would answer nothing, and Gathr would say GitHub failed).
fn put_answers(data: &Path, answers: &Value) {
    let next = data.join("gh.json.next");
    std::fs::write(&next, answers.to_string()).unwrap();
    common::until(10, "GitHub's answers to be replaced", || {
        std::fs::rename(&next, data.join("gh.json")).ok()
    });
}

/// The service on `data`, with the held recording and GitHub's stand-in the room wrote there: at a
/// room's start, and again after a stop.
fn service(data: &Path, waking: bool) -> TestService {
    let gate = if waking {
        PathBuf::from("go")
    } else {
        data.join("go")
    };
    TestService::with_engine(
        data,
        &[
            ("GATHR_REPLAY", &data.join("held.jsonl")),
            ("GATHR_REPLAY_HOLD", &gate),
            ("GATHR_REPLAY_ENV", &data.join("env.jsonl")),
            ("GATHR_GH_PATH", &fake_gh()),
            ("GATHR_FAKE_GH", &data.join("gh.json")),
            // GitHub marks a pushed merge in a few seconds; the stand-in does at once.
            ("GATHR_MERGE_CONFIRM_MS", Path::new("1500")),
            (
                "GATHR_WAIT_EVERY_MS",
                Path::new(if waking { "100" } else { "120000" }),
            ),
        ],
    )
}

impl Room {
    fn db(&self) -> rusqlite::Connection {
        let db = rusqlite::Connection::open(self.data.join("gathr.db")).unwrap();
        // The service writes at each of its looks: a test's own write waits its turn.
        db.busy_timeout(std::time::Duration::from_secs(10)).unwrap();
        db
    }

    /// The project's checks, passed at `head`, as a run of Gathr's (C3) leaves them.
    fn checks_passed(&self, head: &str) {
        let db = self.db();
        db.execute(
            "INSERT OR IGNORE INTO project_checks (project_id, position, command, source, set_ts)
             VALUES (?1, 0, 'cargo test', 'found', 1)",
            [&self.project],
        )
        .unwrap();
        let id = format!("run-{head}");
        db.execute(
            "INSERT INTO check_runs (id, project_id, head, state, trace_id, started_ts, ended_ts)
             VALUES (?1, ?2, ?3, 'passed', 't', 1, 2)",
            rusqlite::params![id, self.project, head],
        )
        .unwrap();
    }

    fn token(&mut self) -> String {
        let started = nth(&self.data.join("env.jsonl"), self.started);
        self.started += 1;
        started["GATHR_AGENT_TOKEN"].as_str().unwrap().to_string()
    }

    /// Lead's turn: its engine's token.
    fn lead_turn(&mut self) -> String {
        let path = format!("/api/chats/{}/messages", self.chat);
        let sent = self.service.send(
            "POST",
            &path,
            json!({ "text": "Ship it", "agent": self.lead }),
        );
        assert_eq!(sent.status, 200, "{}", sent.body);
        self.token()
    }

    /// Lead gives Vale the review of each PR; Vale's token, once its first review runs.
    fn reviews(&mut self, lead: &str, prs: &[i64]) -> String {
        let port = self.service.info.port;
        for pr in prs {
            let review = json!({ "to": "Vale", "what": format!("Review #{pr}"), "kind": "review", "pr": pr });
            let given = text(&call_tool(port, lead, "give_task", review));
            assert!(given.starts_with("Task #"), "{given}");
        }
        self.token()
    }

    fn call(&self, token: &str, tool: &str, arguments: Value) -> Value {
        call_tool(self.service.info.port, token, tool, arguments)
    }

    /// A review finished done on `commit` in a turn not running here, as its turn would record it.
    /// Nothing looks at its PR then, so the test merges it by hand: a review reported with
    /// `finish_task` lands it (0049, `lands.rs`).
    fn review_done(&self, task: i64, commit: &str) {
        let done =
            "UPDATE tasks SET finish = 'done', reviewed = ?2, review_done = 1 WHERE number = ?1";
        self.db()
            .execute(done, rusqlite::params![task, commit])
            .unwrap();
    }

    /// The reviewer's review of task `task` done on `commit`.
    fn reviewed(&self, vale: &str, task: i64, commit: &str) {
        let done =
            json!({ "how": "done", "task": task, "commit": commit, "nothing_unasked": true });
        let finished = self.call(vale, "finish_task", done);
        assert_eq!(finished["isError"], false, "{finished}");
    }

    /// Returns once Gathr's own look after a merge (0049) has dealt with `pr`: the look holds the
    /// repository's lock, and a `merge` called meanwhile waits behind it, on a loaded machine for
    /// longer than a test's request does.
    fn looked_at(&self, pr: i64) {
        let sql = "SELECT count(*) FROM landings WHERE pr = ?1 AND looked_ts IS NOT NULL";
        common::until(120, "Gathr's look after the merge", || {
            let seen: i64 = self.db().query_row(sql, [pr], |r| r.get(0)).ok()?;
            (seen > 0).then_some(())
        });
    }

    fn main(&self) -> String {
        git(&self.origin, &["rev-parse", "refs/heads/main"])
            .trim()
            .to_string()
    }

    /// A refusal that says `why` and whose reference traces.
    fn refused(&self, answer: &Value, why: &str) {
        let said = text(answer);
        assert_eq!(answer["isError"], true, "{said}");
        assert!(said.contains(why), "expected \"{why}\": {said}");
        let reference = said.rsplit("(ref ").next().unwrap().trim_end_matches(')');
        let (ok, trace) = gathr(&self.data, &["diag", "trace", reference]);
        assert!(ok && trace.to_string().contains("merge"), "{trace}");
    }
}

#[test]
fn each_unmet_condition_refuses_with_a_reference_and_merges_nothing() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let five = json!({ "pr": 5 });

    room.refused(&room.call(&lead, "merge", five.clone()), "reviewer");
    room.refused(&room.call(&vale, "merge", five.clone()), "isn't done");
    room.reviewed(&vale, 1, &room.base.clone());
    room.refused(&room.call(&vale, "merge", five.clone()), "review it again");
    // Vale, #120: the reviewed commit is resolved, never matched as text.
    let reviewed = |commit: &str| {
        let sql = "UPDATE tasks SET reviewed = ?1 WHERE number = 1";
        room.db().execute(sql, [commit]).unwrap();
    };
    for (commit, why) in [
        (&room.heads[0][..5], "too short"),
        ("zzzzzzz", "isn't a commit"),
        (&format!("{}0", room.heads[0])[..], "isn't a commit"),
    ] {
        reviewed(commit);
        room.refused(&room.call(&vale, "merge", five.clone()), why);
    }
    reviewed(&room.heads[0][..7]);
    room.db()
        .execute("DELETE FROM check_runs WHERE head = ?1", [&room.heads[0]])
        .unwrap();
    room.refused(&room.call(&vale, "merge", five.clone()), "checks");
    room.checks_passed(&room.heads[0]);
    std::fs::write(room.other.join("moved.txt"), "main moved\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "moved"]);
    git(&room.other, &["push", "-q", "origin", "main"]);
    room.refused(&room.call(&vale, "merge", five.clone()), "up to date");
    git(
        &room.other,
        &[
            "push",
            "-q",
            "--force",
            "origin",
            &format!("{}:main", room.base),
        ],
    );
    assert_eq!(room.main(), room.base, "a refusal merged something");

    let merged = room.call(&vale, "merge", five);
    assert_eq!(merged["isError"], false, "{}", text(&merged));
    assert_eq!(room.main(), room.heads[0], "main isn't the PR's head");
}

#[test]
fn of_two_prs_on_one_base_the_second_is_refused_once_the_first_merges() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5, 6]);
    room.review_done(1, &room.heads[0]);
    room.review_done(2, &room.heads[1]);
    let first = room.call(&vale, "merge", json!({ "pr": 5 }));
    assert_eq!(first["isError"], false, "{}", text(&first));
    room.looked_at(6);
    room.refused(&room.call(&vale, "merge", json!({ "pr": 6 })), "up to date");
    assert_eq!(room.main(), room.heads[0]);
}

/// The work the PR is for, held on its merge, settles when it merges, at once: the PR looks run
/// every 2 minutes, and this doesn't wait for one. Its giver hears back when another agent merged;
/// the one that merged gets no turn to be told what it did (G159). How many hear-backs it got.
fn merged_with_giver(giver_merges: bool) -> (usize, usize) {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let giver = if giver_merges {
        room.vale.clone()
    } else {
        room.lead.clone()
    };
    let path = format!("/api/chats/{}/messages", room.chat);
    let say = |room: &Room, agent: &str| {
        let sent = room
            .service
            .send("POST", &path, json!({ "text": "Go", "agent": agent }));
        assert_eq!(sent.status, 200, "{}", sent.body);
    };
    say(&room, &giver);
    let giving = room.token();
    // The giving turn ends, as it has long before a merge; later turns hold again.
    let go = room.data.join("go");
    std::fs::write(&go, "").unwrap();
    let history = room
        .service
        .wait_for(&room.chat, 20, "the giving turn to end", |h| {
            h["turns"][0]["status"] == "done"
        });
    std::fs::remove_file(&go).unwrap();
    let gave = history["turns"][0]["id"].as_str().unwrap().to_string();
    // Ann's work on #5, done and held on its merge; Vale's review of it, done on its head.
    let db = room.db();
    let task =
        "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, reason, pr, given_in,
                                   finish, reviewed, review_done, created_ts, updated_ts)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 5, ?9, 'done', ?10, ?5 = 'review', 1, 1)";
    let held = "done, waiting on #5 to merge";
    let none = None::<String>;
    let work = rusqlite::params![
        1, room.chat, giver, room.ann, "work", "Build it", "held", held, gave, none
    ];
    db.execute(task, work).unwrap();
    let review = rusqlite::params![
        2,
        room.chat,
        giver,
        room.vale,
        "review",
        "Review #5",
        "done",
        none,
        none,
        room.heads[0]
    ];
    db.execute(task, review).unwrap();
    say(&room, &room.vale);
    // Vale's engine stays up between its turns: a giver that merges uses the same one.
    let vale = if giver_merges { giving } else { room.token() };
    // Vale's own task, given to itself in this turn and blocked on the merged work, carries on
    // once that work settles: its start answers this turn, where Vale gave tasks, yet it's work,
    // not a hear-back, and must start (Vale, #120). A task merely after #1 is ready already, as #1
    // has only its merge left.
    let running: String = room
        .db()
        .query_row(
            "SELECT id FROM turns WHERE agent_id = ?1 AND status = 'running'",
            [&room.vale],
            |r| r.get(0),
        )
        .unwrap();
    let blocked = "INSERT INTO tasks (number, chat_id, giver, owner, kind, what, state, reason, blocked_on,
                                      given_in, heard, created_ts, updated_ts)
                   VALUES (3, ?1, ?2, ?2, 'work', 'Follow up', 'blocked', 'waiting on #1', 1, ?3, 1, 1, 1)";
    room.db()
        .execute(blocked, rusqlite::params![room.chat, room.vale, running])
        .unwrap();

    let merged = room.call(&vale, "merge", json!({ "pr": 5 }));
    assert_eq!(merged["isError"], false, "{}", text(&merged));
    let state: String = room
        .db()
        .query_row("SELECT state FROM tasks WHERE number = 1", [], |r| r.get(0))
        .unwrap();
    assert_eq!(state, "done", "the merged work isn't settled");
    let history = room
        .service
        .call("GET", &format!("/api/chats/{}", room.chat))
        .json();
    let turns = history["turns"].as_array().unwrap();
    let heard = turns
        .iter()
        .filter(|t| t["agent"] == giver.as_str() && t["answers"] == gave.as_str());
    let follow_up = turns
        .iter()
        .filter(|t| t["agent"] == room.vale.as_str() && t["task"] == 3);
    (heard.count(), follow_up.count())
}

#[test]
fn a_merged_pr_settles_the_work_held_on_it_and_wakes_its_giver_at_once() {
    let (heard, follow_up) = merged_with_giver(false);
    assert_eq!(heard, 1, "the giver didn't hear back");
    assert_eq!(follow_up, 1, "the merger's next work didn't start");
}

#[test]
fn the_agent_that_merged_isnt_woken_to_hear_of_its_own_merge() {
    let (heard, follow_up) = merged_with_giver(true);
    assert_eq!(heard, 0, "the merger got a turn to hear of its own merge");
    assert_eq!(
        follow_up, 1,
        "the merger's next work was dropped with its hear-back"
    );
}

#[test]
fn a_pr_that_changes_tests_or_brings_the_first_checks_needs_the_reviewers_sign_off() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, true);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[6, 5]);
    room.reviewed(&vale, 1, &room.heads[1].clone());
    room.review_done(2, &room.heads[0]);
    room.refused(
        &room.call(&vale, "merge", json!({ "pr": 6 })),
        "tests/check.sh",
    );
    let signed = room.call(&vale, "merge", json!({ "pr": 6, "signed_off": true }));
    assert_eq!(signed["isError"], false, "{}", text(&signed));
    room.looked_at(5);

    // A project with no passed checks on its main yet: its first PR is the baseline.
    room.db()
        .execute("DELETE FROM check_runs WHERE head = ?1", [&room.heads[1]])
        .unwrap();
    room.db()
        .execute("DELETE FROM check_runs WHERE head = ?1", [&room.base])
        .unwrap();
    git(&room.other, &["fetch", "-q", "origin"]);
    git(&room.other, &["checkout", "-q", "room/five"]);
    git(&room.other, &["merge", "-q", "--no-edit", "origin/main"]);
    git(&room.other, &["push", "-q", "origin", "room/five"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]).trim().to_string();
    room.checks_passed(&head);
    room.db()
        .execute("UPDATE tasks SET reviewed = ?1 WHERE number = 2", [&head])
        .unwrap();
    let answers = room.data.join("gh.json");
    let pr = json!({ "state": "OPEN", "headRefOid": head, "baseRefName": "main", "statusCheckRollup": [], "headRefName": "room/five", "isCrossRepository": false });
    std::fs::write(
        &answers,
        json!([{ "args": ["pr", "view", "5"], "out": pr.to_string() }]).to_string(),
    )
    .unwrap();
    room.refused(&room.call(&vale, "merge", json!({ "pr": 5 })), "first");
    let signed = room.call(&vale, "merge", json!({ "pr": 5, "signed_off": true }));
    assert_eq!(signed["isError"], false, "{}", text(&signed));
}

#[test]
fn a_project_merges_by_its_own_method() {
    for method in ["merge", "squash"] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = room(&data.0, &repos.0, false);
        let path = format!("/api/projects/{}", room.project);
        assert_eq!(
            room.service
                .send("PATCH", &path, json!({ "merge": method }))
                .status,
            200
        );
        let lead = room.lead_turn();
        let vale = room.reviews(&lead, &[5]);
        room.review_done(1, &room.heads[0]);
        if method == "squash" {
            let answers = room.data.join("gh.json");
            let mut all: Vec<Value> =
                serde_json::from_str(&std::fs::read_to_string(&answers).unwrap()).unwrap();
            all.push(json!({ "args": ["api"], "out": "{\"merged\": true, \"sha\": \"5a5a5a5\"}" }));
            std::fs::write(&answers, Value::Array(all).to_string()).unwrap();
        }
        let merged = room.call(&vale, "merge", json!({ "pr": 5 }));
        assert_eq!(merged["isError"], false, "{method}: {}", text(&merged));
        match method {
            "merge" => {
                let parents = git(
                    &room.origin,
                    &["rev-list", "--parents", "-n", "1", "refs/heads/main"],
                );
                let expected = format!("{} {}", room.base, room.heads[0]);
                assert!(
                    parents.trim().ends_with(&expected),
                    "not a merge commit of the head onto the base: {parents}"
                );
            }
            _ => {
                let calls = std::fs::read_to_string(room.data.join("gh.json.calls")).unwrap();
                let api = calls
                    .lines()
                    .find(|l| l.contains("\"api\""))
                    .unwrap_or_default();
                assert!(
                    api.contains("merge_method=squash")
                        && api.contains(&format!("sha={}", room.heads[0])),
                    "{calls}"
                );
                assert_eq!(room.main(), room.base, "a squash pushed the branch itself");
            }
        }
    }
}

/// Vale, #120: only the latest run at the head counts, and only one that ran the checks as set now.
#[test]
fn an_older_pass_never_outweighs_a_newer_run_or_changed_checks() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.review_done(1, &room.heads[0]);
    let five = json!({ "pr": 5 });
    for (state, why) in [("failed", "failed"), ("running", "still running")] {
        let newer = "INSERT INTO check_runs (id, project_id, head, state, trace_id, started_ts) VALUES (?1, ?2, ?3, ?4, 't', 5)";
        room.db()
            .execute(
                newer,
                rusqlite::params![format!("newer-{state}"), room.project, room.heads[0], state],
            )
            .unwrap();
        room.refused(&room.call(&vale, "merge", five.clone()), why);
        room.db()
            .execute(
                "DELETE FROM check_runs WHERE id = ?1",
                [format!("newer-{state}")],
            )
            .unwrap();
    }
    room.db()
        .execute("UPDATE project_checks SET set_ts = 9", [])
        .unwrap();
    room.refused(&room.call(&vale, "merge", five), "changed");
    assert_eq!(room.main(), room.base, "a refusal merged something");
}

/// Vale, #120: a PR whose head moves while Gathr merges it is refused before the push; once the
/// push is made, a head that moved after it leaves the PR open, so nothing is settled as merged.
#[test]
fn a_head_that_moves_during_a_merge_is_refused_or_left_unsettled() {
    for moved_after_push in [false, true] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = room(&data.0, &repos.0, false);
        let lead = room.lead_turn();
        let vale = room.reviews(&lead, &[5]);
        room.review_done(1, &room.heads[0]);
        let view = |head: &str| {
            json!({ "state": "OPEN", "headRefOid": head, "baseRefName": "main", "statusCheckRollup": [], "headRefName": "room/five", "isCrossRepository": false }).to_string()
        };
        let (h, moved) = (room.heads[0].clone(), room.base.clone());
        let outs = match moved_after_push {
            false => json!([view(&h), view(&moved)]),
            true => json!([view(&h), view(&h), view(&moved)]),
        };
        let answers = json!([{ "args": ["pr", "view", "5"], "outs": outs }]);
        std::fs::write(room.data.join("gh.json"), answers.to_string()).unwrap();
        let answer = room.call(&vale, "merge", json!({ "pr": 5 }));
        if moved_after_push {
            let said = text(&answer);
            assert_eq!(answer["isError"], false, "{said}");
            assert!(
                said.contains("moved") && !said.contains("settled."),
                "{said}"
            );
            assert_eq!(room.main(), h, "the checked head wasn't pushed");
        } else {
            room.refused(&answer, "moved");
            assert_eq!(room.main(), room.base, "a moved head was merged");
        }
        let closed: bool = room
            .db()
            .query_row("SELECT pr_closed FROM tasks WHERE number = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!closed, "a PR still open was settled as merged");
    }
}

/// Vale, #120: settling the tasks can fail after the merge, which can't be undone: the answer says
/// so, with a reference, instead of claiming them settled.
#[test]
fn tasks_that_fail_to_settle_after_a_merge_are_reported_with_a_reference() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.review_done(1, &room.heads[0]);
    let refuse = "CREATE TRIGGER no_settling BEFORE UPDATE OF pr_closed ON tasks BEGIN SELECT RAISE(ABORT, 'forced'); END";
    room.db().execute(refuse, []).unwrap();
    let answer = room.call(&vale, "merge", json!({ "pr": 5 }));
    let said = text(&answer);
    assert_eq!(room.main(), room.heads[0], "{said}");
    assert!(
        said.contains("weren't settled") && said.contains("(ref "),
        "{said}"
    );
    let reference = said
        .rsplit("(ref ")
        .next()
        .unwrap()
        .trim_end_matches([')', '.']);
    let (ok, trace) = gathr(&room.data, &["diag", "trace", reference]);
    assert!(ok && trace.to_string().contains("forced"), "{trace}");
}

/// Vale, #120: a PR retargeted to another base with the same head is refused before the push; one
/// retargeted after it isn't shown merged, so nothing is settled as merged.
#[test]
fn a_pr_retargeted_during_a_merge_is_refused_or_left_unsettled() {
    for after_push in [false, true] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = room(&data.0, &repos.0, false);
        let lead = room.lead_turn();
        let vale = room.reviews(&lead, &[5]);
        room.review_done(1, &room.heads[0]);
        let view = |base: &str| {
            let h = &room.heads[0];
            json!({ "state": "OPEN", "headRefOid": h, "baseRefName": base, "statusCheckRollup": [] })
                .to_string()
        };
        let outs = match after_push {
            false => json!([view("main"), view("release")]),
            true => json!([view("main"), view("main"), view("release")]),
        };
        let answers = json!([{ "args": ["pr", "view", "5"], "outs": outs }]);
        std::fs::write(room.data.join("gh.json"), answers.to_string()).unwrap();
        let answer = room.call(&vale, "merge", json!({ "pr": 5 }));
        if after_push {
            let said = text(&answer);
            assert!(
                said.contains("doesn't show #5 merged") && said.contains("(ref "),
                "{said}"
            );
            assert_eq!(room.main(), room.heads[0], "the checked head wasn't pushed");
        } else {
            room.refused(&answer, "retargeted");
            assert_eq!(room.main(), room.base, "a retargeted PR was merged");
        }
        let closed: bool = room
            .db()
            .query_row("SELECT pr_closed FROM tasks WHERE number = 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!closed, "a PR still open was settled as merged");
    }
}

#[test]
fn merge_queues_cleanup_at_once_without_waiting_for_a_pr_look() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let cwd = PathBuf::from(nth(&data.0.join("env.jsonl"), 1)["cwd"].as_str().unwrap());
    git(&cwd, &["checkout", "-q", "-b", "room/five", &room.heads[0]]);
    room.review_done(1, &room.heads[0]);
    let merged = room.call(&vale, "merge", json!({"pr":5}));
    assert_eq!(merged["isError"], false, "{}", text(&merged));
    // The replay engine still holds this worktree. No periodic PR look runs in this fixture.
    let sql = "SELECT ref FROM event_log WHERE name='gathr.cleanup.skip' AND error_type='in_use'
        AND json_extract(attrs,'$.branch')='room/five'";
    common::until(
        20,
        "immediate merge cleanup protects the reviewer's running engine",
        || {
            let (ok, rows) = gathr(&data.0, &["diag", "query", sql]);
            assert!(ok, "{rows}");
            rows["rows"][0]["ref"].as_str().map(String::from)
        },
    );
    assert!(
        cwd.join(".git").is_file(),
        "a live engine's folder was removed"
    );
}

/// 0045 (owner, room tasks 167-169): with Required checks `none`, a clean review on the head is
/// enough: CI still pending and no checks run don't hold the merge. Under `ci` the same PR waits.
#[test]
fn review_only_merges_on_a_clean_review_while_ci_is_pending() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let answers = room.data.join("gh.json");
    // GitHub's view of the PR, as the stand-in answers it: CI still running on the head.
    let pending =
        r#""statusCheckRollup":[{"name":"Rust (macOS)","status":"IN_PROGRESS","conclusion":null}]"#;
    let all = std::fs::read_to_string(&answers).unwrap();
    let all = all.replace(
        r#"\"statusCheckRollup\":[]"#,
        &pending.replace('"', r#"\""#),
    );
    std::fs::write(&answers, all).unwrap();
    let set = |room: &Room, ci: &str| {
        let path = format!("/api/projects/{}", room.project);
        let changed = room.service.send("PATCH", &path, json!({ "ci": ci }));
        assert_eq!(changed.status, 200, "{}", changed.body);
    };
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.review_done(1, &room.heads[0]);
    let five = json!({ "pr": 5 });

    set(&room, "ci");
    room.refused(
        &room.call(&vale, "merge", five.clone()),
        "haven't all passed",
    );
    set(&room, "none");
    room.db()
        .execute("DELETE FROM check_runs WHERE head = ?1", [&room.heads[0]])
        .unwrap();
    let merged = room.call(&vale, "merge", five);
    assert_eq!(merged["isError"], false, "{}", text(&merged));
    assert_eq!(room.main(), room.heads[0], "main isn't the PR's head");
}
