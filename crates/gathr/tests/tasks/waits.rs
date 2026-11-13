//! Waits (0010): an all-wait fires once, an agent wait when that agent is free, a day-old one
//! anyway, and whose wait is whose.

use super::*;

#[test]
fn an_all_wait_fires_once_when_every_part_is_met() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let all = wait_on(&mut conn, vec![part("ci", "#54"), part("pr", "55")]).unwrap();
    assert_eq!((all.kind.as_str(), all.parts.len()), ("all", 2));
    let first = waits::met(
        &mut conn,
        &all.parts[0].id,
        "CI on #54 finished: 4 passed",
        T,
    )
    .unwrap();
    assert!(first.is_empty(), "{first:?}");
    let fired = waits::met(&mut conn, &all.parts[1].id, "#55 was merged", T).unwrap();
    assert_eq!(fired.len(), 1, "{fired:?}");
    assert!(
        fired[0]
            .text
            .contains("CI on #54 finished: 4 passed; #55 was merged")
    );
    assert!(fired[0].text.contains("You planned to do this: merge"));
    let again = waits::met(&mut conn, &all.parts[1].id, "#55 was merged", T).unwrap();
    assert!(again.is_empty(), "fires once");
    assert_eq!(waits::read(&conn, &all.id).unwrap().state, "fired");
}

#[test]
fn an_agent_wait_fires_when_that_agent_has_nothing_left_in_the_chat() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let idle = wait_on(&mut conn, vec![part("agent", "ann")]);
    assert_eq!(
        refusal(idle),
        "Ann has nothing running or queued in this chat"
    );
    turn(&conn, "t1", "ann", None, "running");
    turn(&conn, "t2", "ann", None, "queued");
    wait_on(&mut conn, vec![part("agent", "ann")]).unwrap();
    gathr::store::set_turn(&conn, "t1", "done", None).unwrap();
    let early = waits::agent_free(&mut conn, "c", "ann", T).unwrap();
    assert!(
        early.is_empty(),
        "a queued turn is still Ann's work: {early:?}"
    );
    gathr::store::set_turn(&conn, "t2", "failed", Some("boom")).unwrap();
    let woken = waits::agent_free(&mut conn, "c", "ann", T).unwrap();
    assert_eq!(woken.len(), 1);
    assert!(
        woken[0]
            .text
            .contains("Ann is done here: its last turn failed")
    );
}

#[test]
fn a_wait_a_day_old_wakes_its_agent_anyway() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let wait = wait_on(&mut conn, vec![part("check", "test -f done")]).unwrap();
    waits::looked(&conn, &wait.id, None, None, Some("bash didn't start")).unwrap();
    let now = gathr::ids::now_us();
    assert!(waits::expire(&mut conn, now, T).unwrap().is_empty());
    let woken = waits::expire(&mut conn, now + waits::WAIT_FOR_US, T).unwrap();
    assert_eq!(woken.len(), 1);
    assert!(woken[0].text.contains(
        "You have waited 24 hours for a check (the last check failed: \
         bash didn't start)"
    ));
    assert_eq!(waits::read(&conn, &wait.id).unwrap().state, "expired");
}

#[test]
fn an_agent_ends_only_its_own_waits() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let wait = wait_on(&mut conn, vec![part("pr", "54")]).unwrap();
    let again = wait_on(&mut conn, vec![part("pr", "54")]).unwrap();
    assert_eq!(again.id, wait.id, "the same wait, said again");
    let theirs = waits::end(&mut conn, "c", "ann", &wait.id, T);
    assert_eq!(
        refusal(theirs),
        format!("wait {} isn't yours in this chat", wait.id)
    );
    assert_eq!(waits::in_chat(&conn, "c", Some("lead")).unwrap().len(), 1);
    waits::end(&mut conn, "c", "lead", &wait.id, T).unwrap();
    assert_eq!(waits::read(&conn, &wait.id).unwrap().state, "cancelled");
    assert_eq!(
        refusal(wait_on(&mut conn, vec![part("ci", "main")])),
        "a CI wait takes a PR's number or a commit's id, not \"main\""
    );
    let nowhere = Leave {
        chat: "c",
        agent: "lead",
        task: None,
        left_in: None,
        cwd: None,
        then_do: "merge",
        parts: vec![part("check", "true")],
    };
    assert_eq!(
        refusal(waits::leave(&mut conn, &nowhere, T)),
        "a check runs in its agent's folder, and this wait has none"
    );
}

#[test]
fn a_wait_on_an_agent_doing_your_task_is_refused_since_the_hear_back_covers_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    turn(&conn, "t1", "ann", Some(task), "queued");
    let doubled = wait_on(&mut conn, vec![part("agent", "ann")]);
    assert_eq!(
        refusal(doubled),
        format!(
            "Ann is on your task #{task}: you hear back when it's settled, so don't wait on Ann"
        )
    );
}
