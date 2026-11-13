//! G165: a review covers canonical base merges, and remains done across a CI wake.
use super::*;

fn move_pr(room: &mut Room, head: &str) {
    git(&room.other, &["push", "-q", "origin", "room/five"]);
    let file = room.data.join("gh.json");
    let mut answers: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for key in ["out", "out_merged"] {
        let mut view: Value = serde_json::from_str(answers[0][key].as_str().unwrap()).unwrap();
        view["headRefOid"] = json!(head);
        answers[0][key] = json!(view.to_string());
    }
    answers[0]["merged_if"]["head"] = json!(head);
    std::fs::write(file, answers.to_string()).unwrap();
    room.heads[0] = head.to_string();
    room.checks_passed(head);
    // A real PR look's transition makes the old review outdated; merge must prove it safe.
    gathr::tasks::pr_looked(&mut room.db(), 1, "OPEN", Some(head), None, "pr-look").unwrap();
}

fn advance_main(room: &Room) -> String {
    git(&room.other, &["checkout", "-q", "main"]);
    std::fs::write(room.other.join("main-update.txt"), "from main\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "main update"]);
    git(&room.other, &["push", "-q", "origin", "main"]);
    let base = room.main();
    room.checks_passed(&base);
    base
}

#[test]
fn a_done_review_covers_a_clean_update_from_main() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let reviewed = room.heads[0].clone();
    room.reviewed(&vale, 1, &reviewed);
    advance_main(&room);
    git(&room.other, &["checkout", "-q", "room/five"]);
    git(
        &room.other,
        &["merge", "--no-ff", "-m", "update main", "main"],
    );
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    move_pr(&mut room, &head);
    let merged = room.call(&vale, "merge", json!({"pr":5}));
    assert_eq!(
        merged["isError"],
        false,
        "main-only update needs no new review: {}",
        text(&merged)
    );
    assert_eq!(room.main(), head, "the checked updated head is merged");
}

#[test]
fn an_author_commit_after_review_still_needs_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.reviewed(&vale, 1, &room.heads[0].clone());
    let base = room.main();
    git(&room.other, &["checkout", "-q", "room/five"]);
    std::fs::write(room.other.join("author.txt"), "not reviewed\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "author change"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    move_pr(&mut room, &head);
    room.refused(
        &room.call(&vale, "merge", json!({"pr":5})),
        "review it again",
    );
    assert_eq!(room.main(), base, "unreviewed author code wasn't merged");
}

#[test]
fn a_conflicted_main_merge_with_a_resolution_needs_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.reviewed(&vale, 1, &room.heads[0].clone());
    git(&room.other, &["checkout", "-q", "main"]);
    std::fs::write(
        room.other.join("feature.txt"),
        "main's different implementation\n",
    )
    .unwrap();
    git(&room.other, &["add", "."]);
    git(
        &room.other,
        &["commit", "-q", "-m", "main changed the same file"],
    );
    git(&room.other, &["push", "-q", "origin", "main"]);
    let base = room.main();
    room.checks_passed(&base);
    git(&room.other, &["checkout", "-q", "room/five"]);
    let merge = common::git_command()
        .arg("-C")
        .arg(&room.other)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "--no-ff",
            "-m",
            "update main",
            "main",
        ])
        .output()
        .unwrap();
    assert_eq!(
        merge.status.code(),
        Some(1),
        "fixture must conflict: {}",
        String::from_utf8_lossy(&merge.stdout)
    );
    std::fs::write(
        room.other.join("feature.txt"),
        "a manually changed resolution\n",
    )
    .unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "resolve conflict"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    move_pr(&mut room, &head);
    room.refused(
        &room.call(&vale, "merge", json!({"pr":5})),
        "review it again",
    );
    assert_eq!(
        room.main(),
        base,
        "unreviewed conflict resolution wasn't merged"
    );
}

#[test]
fn an_amended_clean_merge_tree_cannot_hide_author_changes() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.reviewed(&vale, 1, &room.heads[0].clone());
    let base = advance_main(&room);
    git(&room.other, &["checkout", "-q", "room/five"]);
    git(
        &room.other,
        &["merge", "--no-ff", "-m", "update main", "main"],
    );
    std::fs::write(room.other.join("hidden.txt"), "extra code in the merge\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "--amend", "--no-edit", "-q"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    move_pr(&mut room, &head);
    room.refused(
        &room.call(&vale, "merge", json!({"pr":5})),
        "review it again",
    );
    assert_eq!(
        room.main(),
        base,
        "merge ancestry alone cannot approve extra code"
    );
}

fn task_state(room: &Room) -> Value {
    let tasks = room
        .service
        .call("GET", &format!("/api/chats/{}/tasks", room.chat))
        .json();
    tasks["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["number"] == 1)
        .unwrap()["state"]
        .clone()
}

fn held_review(data: &Path, repos: &Path) -> (Room, String, PathBuf, Value) {
    let mut room = room_replay(data, repos, false, true);
    let file = data.join("gh.json");
    let mut answers: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    answers.as_array_mut().unwrap().push(json!({"args":["api"],"out":json!({"name":"CI","status":"in_progress","context":"CI","state":"pending"}).to_string()}));
    std::fs::write(&file, answers.to_string()).unwrap();
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let lead_cwd = PathBuf::from(nth(&data.join("env.jsonl"), 0)["cwd"].as_str().unwrap());
    let vale_cwd = PathBuf::from(nth(&data.join("env.jsonl"), 1)["cwd"].as_str().unwrap());
    std::fs::write(lead_cwd.join("go"), "").unwrap();
    let waiting = room.call(
        &vale,
        "wait",
        json!({"ci":"#5","then":"Merge the reviewed PR"}),
    );
    assert_eq!(waiting["isError"], false, "{}", text(&waiting));
    room.reviewed(&vale, 1, &room.heads[0]);
    std::fs::write(vale_cwd.join("go"), "").unwrap();
    common::until(20, "review held on its pending CI", || {
        (task_state(&room) == "held").then_some(())
    });
    (room, vale, file, answers)
}

#[test]
fn a_ci_wait_waking_the_review_task_keeps_its_completed_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let (room, vale, file, mut answers) = held_review(&data.0, &repos.0);
    answers[2]["out"] = json!(json!({"name":"CI","status":"completed","conclusion":"SUCCESS","context":"CI","state":"success"}).to_string());
    std::fs::write(&file, answers.to_string()).unwrap();
    common::until(20, "CI wakes the same review task into working", || {
        (task_state(&room) == "working").then_some(())
    });
    let merged = room.call(&vale, "merge", json!({"pr":5}));
    assert_eq!(
        merged["isError"],
        false,
        "CI wake must not erase the completed review: {}",
        text(&merged)
    );
    assert_eq!(room.main(), room.heads[0]);
}

#[test]
fn the_owner_can_record_done_again_on_its_existing_held_review() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let (room, vale, _, _) = held_review(&data.0, &repos.0);
    let done = room.call(
        &vale,
        "finish_task",
        json!({"task":1,"how":"done","commit":room.heads[0]}),
    );
    assert_eq!(
        done["isError"],
        false,
        "refresh held review without a new task: {}",
        text(&done)
    );
    assert_eq!(
        task_state(&room),
        "held",
        "only review evidence changes, not task lifecycle"
    );
    let denied = room.call(&vale, "finish_task", json!({"task":1,"how":"left"}));
    assert_eq!(
        denied["isError"], true,
        "the exception only records a done review"
    );
    for extra in [json!({"pr":6}), json!({"on_task":99})] {
        let mut args = json!({"task":1,"how":"done","commit":room.heads[0]});
        args.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        assert_eq!(room.call(&vale, "finish_task", args)["isError"], true);
    }
    let lead = nth(&data.0.join("env.jsonl"), 0)["GATHR_AGENT_TOKEN"]
        .as_str()
        .unwrap()
        .to_string();
    let denied = room.call(
        &lead,
        "finish_task",
        json!({"task":1,"how":"done","commit":room.heads[0]}),
    );
    assert_eq!(denied["isError"], true, "{}", text(&denied));
    assert!(text(&denied).contains("isn't yours"), "{}", text(&denied));
}

#[test]
fn an_explicit_unfinished_review_revokes_its_prior_approval() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.reviewed(&vale, 1, &room.heads[0]);
    let left = room.call(
        &vale,
        "finish_task",
        json!({"task":1,"how":"left","note":"Found an issue"}),
    );
    assert_eq!(left["isError"], false, "{}", text(&left));
    let base = room.main();
    room.refused(&room.call(&vale, "merge", json!({"pr":5})), "isn't done");
    assert_eq!(room.main(), base);
}

/// The review task once Gathr's own look has seen the PR at `head` and judged the review so. A
/// look that finds git busy keeps no verdict and asks again, so the first answer may not be the last.
fn looked_at(room: &Room, head: &str, outdated: bool) -> Value {
    let what = format!("Gathr's look to find the review at {head} out of date: {outdated}");
    common::until(20, &what, || {
        let tasks = room
            .service
            .call("GET", &format!("/api/chats/{}/tasks", room.chat))
            .json();
        let task = tasks["tasks"]
            .as_array()?
            .iter()
            .find(|t| t["number"] == 1)?;
        (task["pr_head"] == head && task["outdated"] == outdated).then(|| task.clone())
    })
}

/// The PR moves to `head` on GitHub, and nothing tells the service: its own look finds out.
fn publish(room: &mut Room, head: &str) {
    git(&room.other, &["push", "-q", "origin", "room/five"]);
    let file = room.data.join("gh.json");
    let mut answers: Value =
        serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    for key in ["out", "out_merged"] {
        let mut view: Value = serde_json::from_str(answers[0][key].as_str().unwrap()).unwrap();
        view["headRefOid"] = json!(head);
        answers[0][key] = json!(view.to_string());
    }
    std::fs::write(file, answers.to_string()).unwrap();
    room.heads[0] = head.to_string();
}

/// Scenario 5f: Gathr's own look at a PR agrees with the merge tool. A commit that only brings in
/// the base leaves the review standing; a commit that changes code puts it out of date, and
/// `list_tasks` says so with both commits.
#[test]
fn gathrs_look_puts_a_review_out_of_date_only_when_the_code_changed() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room_replay(&data.0, &repos.0, false, true);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    let reviewed = room.heads[0].clone();
    room.reviewed(&vale, 1, &reviewed);

    advance_main(&room);
    git(&room.other, &["checkout", "-q", "room/five"]);
    git(
        &room.other,
        &["merge", "--no-ff", "-m", "update main", "main"],
    );
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    publish(&mut room, &head);
    // Only main came in.
    let seen = looked_at(&room, &head, false);
    assert_eq!(seen["reviewed"], reviewed.as_str(), "{seen}");

    std::fs::write(room.other.join("author.txt"), "not reviewed\n").unwrap();
    git(&room.other, &["add", "."]);
    git(&room.other, &["commit", "-q", "-m", "author change"]);
    let head = git(&room.other, &["rev-parse", "HEAD"]);
    publish(&mut room, &head);
    // The author changed code.
    looked_at(&room, &head, true);
    let listed = text(&room.call(&vale, "list_tasks", json!({})));
    let says = format!(
        "reviewed at {}, out of date: #5 is at {}",
        &reviewed[..7],
        &head[..7]
    );
    assert!(listed.contains(&says), "{listed}");
}

/// Scenario 6g through the service (G144, G181, G101): a CI wait keeps what its last look saw, for
/// the window and for its agent, and its wake tells a cancelled job from a failed one.
#[test]
fn a_ci_wait_keeps_what_its_last_look_saw_and_wakes_with_each_job_by_name() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let (room, vale, file, mut answers) = held_review(&data.0, &repos.0);
    let head = room.heads[0].clone();
    let path = |what: &str| format!("repos/{{owner}}/{{repo}}/commits/{head}/{what}?per_page=100");
    let jobs = |macos: (&str, &str)| {
        [
            json!({"name":"UI (Windows)","status":"completed","conclusion":"success"}),
            json!({"name":"UI (macOS)","status":macos.0,"conclusion":macos.1}),
        ]
        .map(|job| job.to_string())
        .join("\n")
    };
    // GitHub's check runs for the head, and no commit statuses: ahead of the answer for any `api`.
    answers[2] = json!({"args":["api","--paginate",path("check-runs")],"out":jobs(("queued", ""))});
    answers
        .as_array_mut()
        .unwrap()
        .push(json!({"args":["api","--paginate",path("status")],"out":""}));
    std::fs::write(&file, answers.to_string()).unwrap();
    let waits = || {
        let tasks = room
            .service
            .call("GET", &format!("/api/chats/{}/tasks", room.chat))
            .json();
        tasks["waits"].clone()
    };
    let saw = common::until(20, "the wait to say a job is queued", || {
        let wait = waits()[0].clone();
        wait["seen"]
            .as_str()
            .is_some_and(|seen| seen.contains("queued"))
            .then_some(wait)
    });
    assert_eq!(
        saw["seen"],
        format!(
            "1 of 2 checks on #5 at {} finished: 1 passed; queued: UI (macOS)",
            &head[..7]
        ),
        "{saw}"
    );
    assert!(saw["looked_ts"].as_i64().is_some(), "when it looked: {saw}");
    let listed = text(&room.call(&vale, "list_tasks", json!({})));
    assert!(
        listed.contains("Last look: 1 of 2 checks on #5") && listed.contains("queued: UI (macOS)"),
        "its agent reads the same: {listed}"
    );

    answers[2]["out"] = json!(jobs(("completed", "cancelled")));
    std::fs::write(&file, answers.to_string()).unwrap();
    let woke = common::until(20, "the wake that names the cancelled job", || {
        let chat = room
            .service
            .call("GET", &format!("/api/chats/{}", room.chat))
            .json();
        chat["messages"].as_array()?.iter().find_map(|m| {
            let body = m["body"].as_str()?;
            (m["role"] == "gathr" && body.contains("Your wait ended")).then(|| body.to_string())
        })
    });
    assert!(
        woke.contains("finished: 1 passed, 1 cancelled (UI (macOS))") && !woke.contains("failed"),
        "cancelled, not failed: {woke}"
    );
}
