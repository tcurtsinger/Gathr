//! 0067: after a merge into the default branch, Gathr fast-forwards the project's own folder when
//! that's safe, by any method. Otherwise it leaves the folder as it is and tells the person why in
//! the chat, once, with a reference.
use super::*;

/// The project's own folder: the person's checkout, on main one commit behind origin.
fn folder(room: &Room) -> PathBuf {
    room.other.with_file_name("project")
}

/// What the folder is: its commit, and its changes as `git status` lists them.
fn state(room: &Room) -> (String, String) {
    let folder = folder(room);
    let head = git(&folder, &["rev-parse", "HEAD"]).trim().to_string();
    (head, git(&folder, &["status", "--porcelain"]))
}

/// PR #5 as GitHub has it, at `head` into `base`, its checks passed there.
fn five_at(room: &mut Room, head: &str, base: &str) {
    let pr = |state: &str| {
        json!({ "state": state, "headRefOid": head, "baseRefName": base, "statusCheckRollup": [], "headRefName": "room/five", "isCrossRepository": false })
            .to_string()
    };
    let when = json!({ "repo": room.origin, "base": base, "head": head });
    let view = json!({ "args": ["pr", "view", "5"], "out": pr("OPEN"), "out_merged": pr("MERGED"), "merged_if": when });
    put_answers(&room.data, &json!([view]));
    if head != room.heads[0] {
        room.checks_passed(head);
    }
    room.heads[0] = head.to_string();
}

/// #5, merged by `method` with the merge tool once `before` set up the room: its answer.
fn merge_five(room: &mut Room, method: &str, before: impl FnOnce(&mut Room)) -> String {
    let path = format!("/api/projects/{}", room.project);
    let set = room
        .service
        .send("PATCH", &path, json!({ "merge": method }));
    assert_eq!(set.status, 200, "{}", set.body);
    before(room);
    let lead = room.lead_turn();
    let vale = room.reviews(&lead, &[5]);
    room.review_done(1, &room.heads[0].clone());
    if method == "squash" {
        // GitHub squashes on its side, then shows the PR merged.
        let pr = |state: &str| {
            json!({ "state": state, "headRefOid": room.heads[0], "baseRefName": "main", "statusCheckRollup": [], "headRefName": "room/five", "isCrossRepository": false })
                .to_string()
        };
        let answers = json!([
            { "args": ["pr", "view", "5"], "outs": [pr("OPEN"), pr("OPEN"), pr("MERGED")] },
            { "args": ["api"], "squash": { "repo": room.origin, "base": "main" } }
        ]);
        put_answers(&room.data, &answers);
    }
    let merged = room.call(&vale, "merge", json!({ "pr": 5 }));
    let said = text(&merged);
    assert_eq!(merged["isError"], false, "{method}: {said}");
    said
}

/// The lines Gathr said in the chat about the project's folder.
fn told(room: &Room) -> Vec<String> {
    let db = room.db();
    let sql = "SELECT body FROM messages WHERE chat_id = ?1 AND role = 'gathr' AND body LIKE '%project folder%'";
    let mut lines = db.prepare(sql).unwrap();
    let lines = lines.query_map([&room.chat], |r| r.get(0)).unwrap();
    lines.collect::<Result<_, _>>().unwrap()
}

/// The folder kept as it was, and the person told once why, with a reference that traces.
fn kept(room: &Room, before: (String, String), why: &str) {
    assert_eq!(state(room), before, "the folder was changed");
    let told = told(room);
    assert_eq!(told.len(), 1, "{told:?}");
    assert!(told[0].contains(why), "expected \"{why}\": {}", told[0]);
    let reference = told[0].rsplit("(ref ").next().unwrap();
    let reference = reference.trim_end_matches([')', '.']);
    let (ok, trace) = gathr(&room.data, &["diag", "trace", reference]);
    assert!(
        ok && trace.to_string().contains("gathr.merge.checkout"),
        "{trace}"
    );
}

#[test]
fn each_method_fast_forwards_the_folder_and_keeps_an_unrelated_change() {
    for method in ["fast-forward", "merge", "squash"] {
        let (data, repos) = (TempDir::new(), TempDir::new());
        let mut room = room(&data.0, &repos.0, false);
        let said = merge_five(&mut room, method, |room| {
            std::fs::write(folder(room).join("README.md"), "mine\n").unwrap();
        });
        let main = room.main();
        if method == "squash" {
            let parent = git(&room.origin, &["rev-parse", "refs/heads/main^"]);
            assert_eq!(
                parent.trim(),
                room.base,
                "GitHub's squash isn't a new commit on main"
            );
        }
        assert_eq!(
            state(&room).0,
            main,
            "{method}: the folder isn't at the merge: {said}"
        );
        let readme = std::fs::read_to_string(folder(&room).join("README.md")).unwrap();
        assert_eq!(readme, "mine\n", "{method}: the person's change was lost");
        assert!(folder(&room).join("feature.txt").is_file(), "{method}");
        assert_eq!(told(&room), Vec::<String>::new(), "{method}");
        let sql = "SELECT count(*) AS n FROM event_log WHERE name = 'gathr.merge.checkout' AND error_type IS NULL";
        let (ok, rows) = gathr(&room.data, &["diag", "query", sql]);
        assert!(ok && rows["rows"][0]["n"] == 1, "{method}: {rows}");
    }
}

#[test]
fn a_folder_on_another_branch_is_kept_and_the_person_told() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        git(&folder(room), &["checkout", "-q", "-b", "mine"]);
        before = Some(state(room));
    });
    kept(&room, before.unwrap(), "on mine, not main");
}

#[test]
fn a_merge_into_a_branch_other_than_the_default_leaves_the_folder() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        let release = format!("{}:refs/heads/release", room.base);
        git(&room.other, &["push", "-q", "origin", &release]);
        let head = room.heads[0].clone();
        five_at(room, &head, "release");
        before = Some(state(room));
    });
    assert_eq!(room.main(), room.base, "main moved");
    kept(
        &room,
        before.unwrap(),
        "into release, not the default branch, main",
    );
}

/// Vale: with `merge.autostash` on, a plain fast-forward stashes the change, moves, and leaves
/// conflict markers in the person's file.
#[test]
fn a_change_to_a_file_the_merge_touches_is_kept_even_with_autostash_on() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        let other = room.other.clone();
        git(&other, &["checkout", "-q", "room/five"]);
        std::fs::write(other.join("README.md"), "theirs\n").unwrap();
        git(&other, &["commit", "-q", "-am", "readme"]);
        git(&other, &["push", "-q", "origin", "room/five"]);
        let head = git(&other, &["rev-parse", "HEAD"]).trim().to_string();
        git(&other, &["checkout", "-q", "main"]);
        five_at(room, &head, "main");
        let folder = folder(room);
        git(&folder, &["config", "merge.autostash", "true"]);
        std::fs::write(folder.join("README.md"), "mine\n").unwrap();
        before = Some(state(room));
    });
    assert_eq!(room.main(), room.heads[0], "#5 wasn't merged");
    let readme = std::fs::read_to_string(folder(&room).join("README.md")).unwrap();
    assert_eq!(readme, "mine\n");
    kept(&room, before.unwrap(), "README.md");
}

#[test]
fn local_commits_not_on_origin_keep_the_folder() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        let folder = folder(room);
        std::fs::write(folder.join("local.txt"), "mine\n").unwrap();
        git(&folder, &["add", "."]);
        git(&folder, &["commit", "-q", "-m", "local"]);
        before = Some(state(room));
    });
    kept(&room, before.unwrap(), "commits that aren't on origin");
}

#[test]
fn a_rebase_under_way_keeps_the_folder() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        let folder = folder(room);
        git(&folder, &["checkout", "-q", "-b", "side"]);
        std::fs::write(folder.join("README.md"), "side\n").unwrap();
        git(&folder, &["commit", "-q", "-am", "side"]);
        git(&folder, &["checkout", "-q", "main"]);
        std::fs::write(folder.join("README.md"), "main\n").unwrap();
        git(&folder, &["commit", "-q", "-am", "main"]);
        let rebase = common::git_command()
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(["rebase", "side"])
            .current_dir(&folder)
            .output()
            .unwrap();
        assert!(
            !rebase.status.success(),
            "the rebase didn't stop on its conflict"
        );
        before = Some(state(room));
    });
    kept(&room, before.unwrap(), "a rebase is under way");
}

/// Vale, #216: a cherry-pick of several commits whose first conflict was resolved and committed
/// has no `CHERRY_PICK_HEAD`, only its sequencer, and that commit can be on origin already.
#[test]
fn a_cherry_pick_with_commits_still_to_pick_keeps_the_folder() {
    let (data, repos) = (TempDir::new(), TempDir::new());
    let mut room = room(&data.0, &repos.0, false);
    let mut before = None;
    merge_five(&mut room, "fast-forward", |room| {
        let folder = folder(room);
        git(&folder, &["fetch", "-q", "origin"]);
        git(&folder, &["merge", "-q", "--ff-only", "origin/main"]);
        git(&folder, &["checkout", "-q", "-b", "picks"]);
        std::fs::write(folder.join("README.md"), "picked\n").unwrap();
        git(&folder, &["commit", "-q", "-am", "a"]);
        std::fs::write(folder.join("b.txt"), "b\n").unwrap();
        git(&folder, &["add", "."]);
        git(&folder, &["commit", "-q", "-m", "b"]);
        git(&folder, &["checkout", "-q", "main"]);
        std::fs::write(folder.join("README.md"), "mine\n").unwrap();
        git(&folder, &["commit", "-q", "-am", "mine"]);
        let pick = common::git_command()
            .args([
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
            ])
            .args(["cherry-pick", "picks~1", "picks"])
            .current_dir(&folder)
            .output()
            .unwrap();
        assert!(
            !pick.status.success(),
            "the cherry-pick didn't stop on its conflict"
        );
        std::fs::write(folder.join("README.md"), "both\n").unwrap();
        git(&folder, &["add", "README.md"]);
        git(&folder, &["commit", "-q", "--no-edit"]);
        git(&folder, &["push", "-q", "origin", "main"]);
        let other = room.other.clone();
        git(&other, &["fetch", "-q", "origin"]);
        git(&other, &["checkout", "-q", "room/five"]);
        git(&other, &["merge", "-q", "--no-edit", "origin/main"]);
        git(&other, &["push", "-q", "origin", "room/five"]);
        let head = git(&other, &["rev-parse", "HEAD"]).trim().to_string();
        git(&other, &["checkout", "-q", "main"]);
        five_at(room, &head, "main");
        let git_dir = folder.join(".git");
        assert!(!git_dir.join("CHERRY_PICK_HEAD").exists() && git_dir.join("sequencer").is_dir());
        before = Some(state(room));
    });
    assert_eq!(room.main(), room.heads[0], "#5 wasn't merged");
    assert!(
        folder(&room).join(".git/sequencer").is_dir(),
        "the cherry-pick was ended"
    );
    kept(
        &room,
        before.unwrap(),
        "a cherry-pick or revert is under way",
    );
}
