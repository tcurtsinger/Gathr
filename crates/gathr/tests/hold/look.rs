//! I2a: only a held turn with a worktree is looked at, and what it changed is what git tracks
//! between its start and its end, and nothing else, in real throwaway repositories.

use std::path::{Path, PathBuf};

use gathr::hold::look::{self, Changed, Failed};
use serde_json::json;

use super::*;

/// I2a.1: a held turn in a project's chat, with a worktree, is looked at once; any other isn't.
#[test]
fn only_a_held_turn_with_a_worktree_is_looked_at() {
    let dir = TempDir::new();
    let (mut conn, _) = room(&dir);
    conn.execute_batch(
        "INSERT INTO agent_folders (path, kind, chat_id, agent_id, made_ts) VALUES
             ('w-mira', 'worktree', 'c', 'mira', 1), ('w-ann', 'worktree', 'c', 'ann', 1),
             ('f-mira', 'folder', 'd', 'mira', 1);",
    )
    .unwrap();
    let worktree = |conn: &Connection, turn: &str| {
        let found = look::to_look(conn, turn).unwrap();
        found.map(|l| l.worktree.display().to_string())
    };
    let mut seen = vec![("T, unsized", worktree(&conn, "T"), Some("w-mira"))];
    let l = look::to_look(&conn, "T").unwrap().unwrap();
    let start = look::Seen {
        head: "abc".into(),
        entries: Vec::new(),
    };
    look::before(&conn, &l, &start).unwrap();
    seen.push(("T, its start recorded", worktree(&conn, "T"), None));
    sent(&mut conn, ("d", "mira"), "D", "Fix the label");
    seen.push((
        "a held turn in a chat of no project",
        worktree(&conn, "D"),
        None,
    ));
    let m2 = sent(&mut conn, ("c", "mira"), "T2", "Fix the export");
    lead_sizes(&mut conn, "T2", "repair");
    seen.push(("a repair's turn", worktree(&conn, "T2"), None));
    let build = work(&mut conn, ("mira", "ann"), "T2", false);
    let build_turn = begin(&mut conn, build);
    asks::resize(&mut conn, &m2, "new", T).unwrap();
    seen.push(("a building task's turn", worktree(&conn, &build_turn), None));
    let wrong: Vec<_> = seen
        .iter()
        .filter(|(_, got, want)| got.as_deref() != *want)
        .collect();
    assert!(
        wrong.is_empty(),
        "looked at, as (what, got, wanted): {wrong:#?}"
    );
}

/// I2b.1, I2b.6, I2b.7: a caught change is Gathr's line in the held turn, listed for its chat and
/// for its ask's plan, and the ask was held.
#[test]
fn a_caught_change_is_listed_for_its_chat_and_its_plan() {
    let dir = TempDir::new();
    let (mut conn, m1) = room(&dir);
    let add = "INSERT INTO agent_folders (path, kind, chat_id, agent_id, made_ts) VALUES ('w', 'worktree', 'c', 'mira', 1)";
    conn.execute(add, []).unwrap();
    let wrote = plan(&mut conn, "T", "Add the tags table");
    let l = look::to_look(&conn, "T").unwrap().unwrap();
    let start = look::Seen {
        head: "abc".into(),
        entries: Vec::new(),
    };
    look::before(&conn, &l, &start).unwrap();
    let Some(look::Ending::Look(l, _)) = look::ending(&conn, "T").unwrap() else {
        panic!("T, still held, isn't looked at");
    };
    let files = changed(&["src/notes.rs", "src/tags.rs"], 1);
    let message = look::after(&mut conn, &l, Some(&files)).unwrap().unwrap();
    let body = "Mira changed 2 files before the plan was approved\nsrc/notes.rs\nsrc/tags.rs\nCommitted: 1";
    assert_eq!(
        (&message["turn"], &message["body"]),
        (&json!("T"), &json!(body))
    );
    let id = message["id"].clone();
    assert_eq!(json!(look::caught(&conn, "c").unwrap()), json!([id]));
    let shown = plans::get(&conn, wrote.plan).unwrap().unwrap();
    let row = json!({ "agent": "mira", "files": ["src/notes.rs", "src/tags.rs"], "message": id });
    assert_eq!(shown["changed"], json!([row]));
    assert!(held_ts(&conn, &m1).is_some(), "the ask wasn't held");
    assert!(
        look::ending(&conn, "T").unwrap().is_none(),
        "looked at twice"
    );
}

/// I2b.5: the words the agent's next turn opens with name at most 20 files, as Gathr's line does,
/// and count the rest.
#[test]
fn the_told_words_name_twenty_files_and_count_the_rest() {
    let mut seen = Vec::new();
    for n in [25, 20] {
        let dir = TempDir::new();
        let (mut conn, _) = room(&dir);
        let add = "INSERT INTO agent_folders (path, kind, chat_id, agent_id, made_ts) VALUES ('w', 'worktree', 'c', 'mira', 1)";
        conn.execute(add, []).unwrap();
        plan(&mut conn, "T", "Add the tags table");
        let l = look::to_look(&conn, "T").unwrap().unwrap();
        let start = look::Seen {
            head: "abc".into(),
            entries: Vec::new(),
        };
        look::before(&conn, &l, &start).unwrap();
        let Some(look::Ending::Look(l, _)) = look::ending(&conn, "T").unwrap() else {
            panic!("T, still held, isn't looked at");
        };
        let files: Vec<String> = (1..=n).map(|i| format!("f{i:02}.txt")).collect();
        let files: Vec<&str> = files.iter().map(String::as_str).collect();
        look::after(&mut conn, &l, Some(&changed(&files, 0))).unwrap();
        let place = hold::Place {
            worktree: Path::new("w"),
            own: None,
            shared: None,
        };
        let words = look::untold(&mut conn, "T", "c", "mira", &place).unwrap();
        let words = words.expect("nothing told");
        seen.push(words.lines().next().unwrap_or_default().to_string());
    }
    let named = "f01.txt, f02.txt, f03.txt, f04.txt, f05.txt, f06.txt, f07.txt, f08.txt, f09.txt, f10.txt, \
                 f11.txt, f12.txt, f13.txt, f14.txt, f15.txt, f16.txt, f17.txt, f18.txt, f19.txt, f20.txt";
    assert_eq!(
        seen,
        [
            format!(
                "In your last turn you changed 25 files in w: {named} and 5 more. The person sees them."
            ),
            format!("In your last turn you changed 20 files in w: {named}. The person sees them."),
        ]
    );
}

/// A bare remote whose `main` has `a.txt` and a `.gitignore` of `out/`, a branch `x` with `x.txt`,
/// and a PR head `refs/pull/7/head` with `p.txt`; a second clone, `other`, pushes to it.
fn remote(root: &Path) {
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    common::git(&seed, &["init", "-q", "-b", "main"]);
    std::fs::write(seed.join("a.txt"), "a\n").unwrap();
    std::fs::write(seed.join(".gitignore"), "out/\n").unwrap();
    common::git(&seed, &["add", "."]);
    common::git(&seed, &["commit", "-q", "-m", "first"]);
    common::git(&seed, &["checkout", "-q", "-b", "x"]);
    std::fs::write(seed.join("x.txt"), "x\n").unwrap();
    common::git(&seed, &["add", "."]);
    common::git(&seed, &["commit", "-q", "-m", "x"]);
    common::git(&seed, &["checkout", "-q", "--detach", "main"]);
    std::fs::write(seed.join("p.txt"), "p\n").unwrap();
    common::git(&seed, &["add", "."]);
    common::git(&seed, &["commit", "-q", "-m", "pr"]);
    common::git(&seed, &["update-ref", "refs/pull/7/head", "HEAD"]);
    common::git(&seed, &["checkout", "-q", "main"]);
    common::git(root, &["clone", "-q", "--bare", "seed", "origin.git"]);
    let pr = "refs/pull/7/head:refs/pull/7/head";
    common::git(&seed, &["push", "-q", "../origin.git", pr]);
    common::git(root, &["clone", "-q", "origin.git", "other"]);
}

/// A new clone of the remote, as an agent's worktree.
fn clone(root: &Path, name: &str) -> PathBuf {
    common::git(root, &["clone", "-q", "origin.git", name]);
    root.join(name)
}

/// `other` pushes a commit of `file` with `text` to `main`.
fn pushed(root: &Path, file: &str, text: &str) {
    let other = root.join("other");
    common::git(&other, &["pull", "-q", "--ff-only"]);
    std::fs::write(other.join(file), text).unwrap();
    common::git(&other, &["add", "."]);
    common::git(&other, &["commit", "-q", "-m", file]);
    common::git(&other, &["push", "-q", "origin", "main"]);
}

/// git in `dir`, as `common::git`, whether or not it works.
fn try_git(dir: &Path, args: &[&str]) {
    let named = [
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.invalid",
    ];
    let ran = common::git_command()
        .args(named)
        .args(args)
        .current_dir(dir)
        .output();
    ran.unwrap();
}

fn write(dir: &Path, file: &str, text: &str) {
    let path = dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn commit(dir: &Path, file: &str, text: &str) {
    write(dir, file, text);
    common::git(dir, &["add", "--", file]);
    common::git(dir, &["commit", "-q", "-m", file]);
}

fn env() -> gathr::platform::env::Vars {
    look::env(gathr::platform::env::fresh(std::env::vars_os().collect()).vars)
}

/// What a held turn changed: `before` runs before its start, `during` in it.
async fn caught(w: &Path, before: impl FnOnce(&Path), during: impl FnOnce(&Path)) -> Changed {
    let env = env();
    before(w);
    let start = match look::seen(w, &env).await {
        Ok(start) => start,
        Err(failed) => panic!("{}: the start wasn't read: {failed:?}", w.display()),
    };
    during(w);
    let (changed, _) = look::changes(w, &env, &start).await.unwrap();
    changed
}

fn changed(files: &[&str], committed: usize) -> Changed {
    Changed {
        files: files.iter().map(|f| f.to_string()).collect(),
        committed,
    }
}

/// I2a.3 to I2a.6, test 17: each row a new clone, a change before the turn and one in it.
#[tokio::test]
async fn a_held_turns_change_is_what_git_tracks_and_nothing_else() {
    let dir = TempDir::new();
    let root = dir.0.as_path();
    remote(root);
    let none = changed(&[], 0);
    let mut seen: Vec<(&str, Changed, Changed)> = Vec::new();
    let w = clone(root, "new");
    let got = caught(&w, |_| {}, |w| write(w, "new.txt", "n\n")).await;
    seen.push(("a new file", got, changed(&["new.txt"], 0)));
    let w = clone(root, "ignored");
    let got = caught(&w, |_| {}, |w| write(w, "out/build.o", "o\n")).await;
    seen.push(("a new ignored file", got, none.clone()));
    let w = clone(root, "untouched");
    let got = caught(&w, |w| write(w, "a.txt", "before\n"), |_| {}).await;
    seen.push(("a.txt changed before, untouched", got, none.clone()));
    let w = clone(root, "again");
    let got = caught(
        &w,
        |w| write(w, "a.txt", "before\n"),
        |w| write(w, "a.txt", "again\n"),
    )
    .await;
    seen.push((
        "a.txt changed before and again",
        got,
        changed(&["a.txt"], 0),
    ));
    let w = clone(root, "reverted");
    let revert = |w: &Path| _ = common::git(w, &["checkout", "-q", "--", "a.txt"]);
    let got = caught(&w, |w| write(w, "a.txt", "before\n"), revert).await;
    seen.push((
        "a.txt changed before and reverted",
        got,
        changed(&["a.txt"], 0),
    ));
    let w = clone(root, "commit");
    let got = caught(&w, |_| {}, |w| commit(w, "b.txt", "b\n")).await;
    seen.push(("a commit of b.txt", got, changed(&["b.txt"], 1)));
    let w = clone(root, "two");
    let two = |w: &Path| {
        commit(w, "d.txt", "d\n");
        write(w, "d.txt", "d2\n");
        commit(w, "e.txt", "e\n");
        common::git(w, &["commit", "-q", "-am", "d again"]);
    };
    let got = caught(&w, |_| {}, two).await;
    seen.push((
        "two commits, d.txt twice",
        got,
        changed(&["d.txt", "e.txt"], 2),
    ));
    let w = clone(root, "names");
    let names = |w: &Path| {
        commit(w, "dir é/x y.txt", "x\n");
        write(w, "dir é/u v.txt", "u\n");
    };
    let got = caught(&w, |_| {}, names).await;
    let whole = changed(&["dir é/u v.txt", "dir é/x y.txt"], 1);
    seen.push(("names with a space and an accent", got, whole));

    // A moved head alone is no change.
    let w = clone(root, "pull");
    pushed(root, "n.txt", "n\n");
    let got = caught(
        &w,
        |_| {},
        |w| _ = common::git(w, &["pull", "-q", "--ff-only"]),
    )
    .await;
    seen.push(("a fast-forward pull", got, none.clone()));
    let w = clone(root, "merge");
    let merge = |w: &Path| _ = common::git(w, &["merge", "-q", "--no-edit", "origin/x"]);
    let got = caught(&w, |w| commit(w, "l.txt", "l\n"), merge).await;
    seen.push((
        "a merge of origin/x over a commit from before",
        got,
        none.clone(),
    ));
    let w = clone(root, "checkout");
    let checkout = |w: &Path| _ = common::git(w, &["checkout", "-q", "-b", "y", "origin/x"]);
    let got = caught(&w, |_| {}, checkout).await;
    seen.push(("a checkout of origin/x", got, none.clone()));
    let w = clone(root, "over");
    let over = |w: &Path| {
        common::git(w, &["merge", "-q", "--no-edit", "origin/x"]);
        commit(w, "f.txt", "f\n");
    };
    let got = caught(&w, |w| commit(w, "l.txt", "l\n"), over).await;
    seen.push(("a commit on top of that merge", got, changed(&["f.txt"], 1)));
    let w = clone(root, "survives");
    pushed(root, "n2.txt", "n2\n");
    let pull = |w: &Path| _ = common::git(w, &["pull", "-q", "--ff-only"]);
    let got = caught(&w, |w| write(w, "a.txt", "mine\n"), pull).await;
    seen.push((
        "an uncommitted a.txt that survives a pull",
        got,
        none.clone(),
    ));
    let w = clone(root, "conflict");
    pushed(root, "a.txt", "theirs\n");
    let conflict = |w: &Path| try_git(w, &["pull", "-q", "--no-rebase"]);
    let got = caught(&w, |w| commit(w, "a.txt", "ours\n"), conflict).await;
    seen.push((
        "a pull that stops on a conflict",
        got,
        changed(&["a.txt"], 0),
    ));

    // The limits, said plainly in I2a.3.
    let w = clone(root, "inside");
    let inside = |w: &Path| {
        common::git(w, &["merge", "-q", "--no-ff", "--no-commit", "origin/x"]);
        write(w, "z.txt", "z\n");
        common::git(w, &["add", "z.txt"]);
        common::git(w, &["commit", "-q", "--no-edit"]);
    };
    let got = caught(&w, |_| {}, inside).await;
    seen.push((
        "a file added inside a merge commit (a limit)",
        got,
        none.clone(),
    ));
    let w = clone(root, "fetched");
    let fetched = |w: &Path| {
        common::git(w, &["fetch", "-q", "origin", "refs/pull/7/head"]);
        common::git(w, &["checkout", "-q", "--detach", "FETCH_HEAD"]);
    };
    let got = caught(&w, |_| {}, fetched).await;
    seen.push((
        "a PR head in FETCH_HEAD (a limit)",
        got,
        changed(&["p.txt"], 1),
    ));
    let w = clone(root, "notes");
    let second = |w: &Path| write(w, "notes/two.md", "2\n");
    let got = caught(&w, |w| write(w, "notes/one.md", "1\n"), second).await;
    seen.push((
        "a file added in an untracked folder from before (a limit)",
        got,
        none.clone(),
    ));
    let w = clone(root, "drafts");
    let got = caught(&w, |_| {}, |w| write(w, "drafts/x.md", "x\n")).await;
    seen.push(("a new untracked folder", got, changed(&["drafts/"], 0)));

    let wrong: Vec<_> = seen.iter().filter(|(_, got, want)| got != want).collect();
    assert!(
        wrong.is_empty(),
        "changed, as (what, got, wanted): {wrong:#?}"
    );
}

/// The repository's `core.fsmonitor` never runs in a look (I2a.4).
#[tokio::test]
async fn a_look_runs_no_fsmonitor_the_repository_names() {
    let dir = TempDir::new();
    remote(&dir.0);
    let w = clone(&dir.0, "hostile");
    common::git(
        &w,
        &["config", "core.fsmonitor", "echo > fsmonitor; exit 1"],
    );
    write(&w, "a.txt", "changed\n");
    common::git(&w, &["status", "--porcelain"]);
    let mark = w.join("fsmonitor");
    assert!(
        mark.exists(),
        "git status didn't run it: the test proves nothing"
    );
    std::fs::remove_file(&mark).unwrap();
    let start = look::seen(&w, &env()).await;
    assert!(start.is_ok(), "{start:?}");
    assert!(!mark.exists(), "the look ran the repository's fsmonitor");
}

/// I2a.6: over 2,000 entries at the start, nothing is kept.
#[tokio::test]
async fn a_worktree_over_two_thousand_entries_is_too_many() {
    let dir = TempDir::new();
    remote(&dir.0);
    let w = clone(&dir.0, "many");
    for n in 0..2_001 {
        std::fs::write(w.join(format!("f{n}.txt")), "f\n").unwrap();
    }
    match look::seen(&w, &env()).await {
        Err(Failed::TooMany(n)) => assert_eq!(n, 2_001),
        other => panic!("expected too_many, got {other:?}"),
    }
}
