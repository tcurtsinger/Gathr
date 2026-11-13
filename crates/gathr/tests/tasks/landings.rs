//! 0049: whose review lands a PR, the sign-off kept with a review, and what Gathr did at a head,
//! done and said once.
use super::*;
use gathr::landings;

/// The team's chats in a project each, as landing needs: Gathr lands a project's PRs.
fn in_projects(conn: &Connection) {
    conn.execute_batch(
        "INSERT INTO projects (id, name, name_key, folder, created_ts) VALUES ('p', 'Site', 'site', 'x', 1);
         UPDATE chats SET project_id = 'p';",
    )
    .unwrap();
}

fn done_at<'a>(commit: &'a str, signed_off: bool) -> Said<'a> {
    Said {
        commit: Some(commit),
        signed_off,
        ..Said::default()
    }
}

/// Scenario 10e: the newest review of a PR in the chat decides. An older approval never lands a PR
/// a newer review holds back, a cancelled review holds nothing back, and a review in another chat
/// is that chat's own.
#[test]
fn the_newest_review_of_a_pr_in_its_chat_is_the_one_that_lands_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    turn(&conn, "giving", "ann", None, "done");
    let first = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, first);
    let of = |conn: &Connection| landings::review_of(conn, "c", 5).unwrap().unwrap();
    assert_eq!(
        (of(&conn).task, of(&conn).reviewed.as_deref()),
        (first, Some("abc1234"))
    );

    let newer = review_by_bo(&mut conn, 5, "giving");
    assert_eq!(
        (of(&conn).task, of(&conn).reviewed),
        (newer, None),
        "an open review holds it"
    );
    tasks::cancel(&mut conn, Some("ann"), newer, "not needed", T).unwrap();
    assert_eq!(of(&conn).task, first, "a cancelled review holds nothing");

    assert_eq!(landings::review_of(&conn, "other", 5).unwrap(), None);
    let all = landings::reviews(&conn, Some("p")).unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
    // Once the PR has merged or closed there is nothing left to land.
    tasks::pr_looked(&mut conn, first, "MERGED", Some("abc1234"), None, T).unwrap();
    assert_eq!(landings::review_of(&conn, "c", 5).unwrap(), None);
}

/// Scenario 10e: sign-off is said with the review and kept with it. It stands while that review
/// does: a review of another commit, or one that is no longer done, needs it said again.
#[test]
fn sign_off_is_kept_with_the_review_it_was_given_with() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    turn(&conn, "giving", "ann", None, "done");
    let review = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, review);
    let signed = |conn: &Connection| {
        landings::review_of(conn, "c", 5)
            .unwrap()
            .unwrap()
            .signed_off
    };
    assert!(!signed(&conn));

    // The same done review recorded again with it is taken, though the task is done already.
    let (_, took, _) = tasks::finish(
        &mut conn,
        "bo",
        review,
        Finish::Done,
        &done_at("abc1234", true),
        T,
    )
    .unwrap();
    assert_eq!(took, tasks::Took::Applied);
    assert!(signed(&conn));
    // Said again without it, the same review keeps it.
    finish(
        &mut conn,
        "bo",
        review,
        Finish::Done,
        &done_at("abc1234", false),
    )
    .unwrap();
    assert!(signed(&conn));
    // A review of another commit is another review.
    finish(
        &mut conn,
        "bo",
        review,
        Finish::Done,
        &done_at("def5678", false),
    )
    .unwrap();
    assert!(!signed(&conn), "sign-off outlived its review");

    // A cancel withdraws the approval and its sign-off with it, and a restart starts clean.
    let second = review_by_bo(&mut conn, 6, "giving");
    begin(&mut conn, second);
    finish(
        &mut conn,
        "bo",
        second,
        Finish::Done,
        &done_at("abc1234", true),
    )
    .unwrap();
    let kept = |conn: &Connection| tasks::get(conn, second).unwrap().unwrap().signed_off;
    assert!(kept(&conn));
    tasks::cancel(&mut conn, Some("ann"), second, "not needed", T).unwrap();
    assert!(!kept(&conn), "sign-off outlived a cancel");
    conn.execute(
        "UPDATE tasks SET signed_off = 1 WHERE number = ?1",
        [second],
    )
    .unwrap();
    tasks::restart(&mut conn, Some("ann"), second, T).unwrap();
    assert!(!kept(&conn), "sign-off outlived a restart");

    // It goes with a review that is done, and nothing else.
    let work = give(&mut conn, "ann", &[], None).0;
    let said = done_at("abc1234", true);
    assert_eq!(
        refusal(tasks::finish(
            &mut conn,
            "ann",
            work,
            Finish::Done,
            &said,
            T
        )),
        "`signed_off` goes with a review that is done"
    );
    let left = Said {
        note: Some("one file left"),
        signed_off: true,
        ..Said::default()
    };
    assert_eq!(
        refusal(tasks::finish(
            &mut conn,
            "bo",
            review,
            Finish::Left,
            &left,
            T
        )),
        "`signed_off` goes with a review that is done"
    );
}

/// Scenario 10f: what Gathr does for a PR at one head it does once, whoever gets there first: two
/// looks at once each ask, and one is answered. Another head starts again.
#[test]
fn a_reviewer_is_told_and_a_pr_updated_once_for_a_head() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    in_projects(&conn);
    turn(&conn, "giving", "ann", None, "done");
    let review = review_by_bo(&mut conn, 5, "giving");
    reviewed(&mut conn, review);
    let review = landings::review_of(&conn, "c", 5).unwrap().unwrap();
    let wake = "Gathr can't land #5 at 0123456: CI failed.";

    let told = landings::tell(&mut conn, &review, "0123456", "CI failed", Some(wake), T).unwrap();
    let start = told.expect("the first look wakes the reviewer");
    assert_eq!(
        (start.agent.as_str(), start.task, start.text.as_str()),
        ("bo", None, wake),
        "on no task: its done review stands"
    );
    let again = landings::tell(&mut conn, &review, "0123456", "CI failed", Some(wake), T).unwrap();
    assert_eq!(again, None, "told twice for one head");
    assert!(landings::did(&conn, "c", 5, "0123456").unwrap().told);

    assert!(landings::updating(&conn, "c", 5, "0123456").unwrap());
    assert!(!landings::updating(&conn, "c", 5, "0123456").unwrap());
    assert!(landings::pushed(&conn, "c", 5, "0123456").unwrap());
    assert!(!landings::pushed(&conn, "c", 5, "0123456").unwrap());
    let did = landings::did(&conn, "c", 5, "0123456").unwrap();
    assert!(
        did.told && did.pushed && did.updated_us.is_some(),
        "{did:?}"
    );
    assert_eq!(
        landings::last_update(&conn, "c", 5).unwrap(),
        did.updated_us
    );

    // Told in the answer to its own report: recorded, with no turn.
    let quiet = landings::tell(&mut conn, &review, "89abcde", "CI failed", None, T).unwrap();
    assert_eq!(quiet, None);
    let other = landings::did(&conn, "c", 5, "89abcde").unwrap();
    assert!(
        other.told && !other.pushed && other.updated_us.is_none(),
        "{other:?}"
    );
    let woke = landings::tell(&mut conn, &review, "89abcde", "CI failed", Some(wake), T).unwrap();
    assert_eq!(woke, None, "woken for a head it was told of");
}
