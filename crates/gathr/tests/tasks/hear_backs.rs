//! A hear-back says what is new, and what really happened (0047, docs/scenarios.md 5e): only the
//! tasks its giver hasn't heard about, a changed outcome once more, and a task given in passing by
//! itself.

use super::*;

fn note(text: &str) -> Said<'_> {
    Said {
        note: Some(text),
        ..Said::default()
    }
}

/// G188: a hear-back repeated every task its giver gave in that turn. It names only what is new.
#[test]
fn a_hear_back_names_only_what_its_giver_has_not_heard() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "done");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    work(&mut conn, a, Some(Finish::Blocked));
    let starts = work(&mut conn, b, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    assert!(
        back[0]
            .text
            .starts_with("The 2 tasks you gave are settled:"),
        "{}",
        back[0].text
    );

    tasks::restart(&mut conn, Some("lead"), a, T).unwrap();
    let starts = work(&mut conn, a, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(said.starts_with("The task you gave is settled:"), "{said}");
    assert!(said.contains(&format!("#{a} Ann")), "{said}");
    assert!(!said.contains(&format!("#{b} Bo")), "heard already: {said}");
}

/// 5e: an outcome that changes after its giver heard it is heard once more. The same outcome,
/// reached again by a turn nobody asked for, is not. After a restart it is: the giver asked for
/// that run, and hears how it ended.
#[test]
fn an_outcome_that_changes_after_it_was_heard_is_heard_once_more() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "done");
    let (task, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let starts = work(&mut conn, task, Some(Finish::Blocked));
    assert_eq!(to_lead(&starts).len(), 1, "heard blocked: {starts:?}");

    // A turn lands on it and ends with nothing new: blocked as before, and nothing more to hear.
    let starts = work(&mut conn, task, None);
    assert_eq!(state(&conn, task).0, State::Blocked);
    assert!(
        to_lead(&starts).is_empty(),
        "the same outcome twice: {starts:?}"
    );

    // Restarted by its giver, it ends the same way: the giver hears how the run it asked for ended.
    tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    let starts = work(&mut conn, task, Some(Finish::Blocked));
    assert_eq!(to_lead(&starts).len(), 1, "{starts:?}");

    // Its owner reports it done from another turn: its giver hears that.
    let (_, _, starts) = tasks::finish(
        &mut conn,
        "ann",
        task,
        Finish::Done,
        &note("fixed in #9"),
        T,
    )
    .unwrap();
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let line = format!("#{task} Ann, Fix the build: done: fixed in #9");
    assert!(back[0].text.contains(&line), "{}", back[0].text);
}

/// G164: a task given in passing doesn't hold the task whose turn gave it, the hear-back on the
/// others doesn't wait for it, and it is heard back on by itself.
#[test]
fn a_task_given_in_passing_holds_nothing_and_is_heard_back_on_by_itself() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    let giving = begin(&mut conn, lead_task);
    let (needed, _) = give(&mut conn, "ann", &[], Some(&giving));
    let aside = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: Some(&giving),
        parent: None,
        kind: Kind::Work,
        what: "Tidy the notes",
        why: None,
        after: &[],
        pr: None,
        in_passing: true,
    };
    let aside = tasks::give(&mut conn, &aside, T).unwrap().0.number;
    gathr::store::set_turn(&conn, &giving, "done", None).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok, T).unwrap();
    let held = Some(format!("waiting on #{needed} (Ann)"));
    assert_eq!(state(&conn, lead_task), (State::Held, held));

    let starts = work(&mut conn, needed, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "it doesn't wait for #{aside}: {starts:?}");
    assert!(back[0].text.contains(&format!("#{needed} Ann")));
    assert!(
        !back[0].text.contains(&format!("#{aside} Bo")),
        "{}",
        back[0].text
    );

    let starts = work(&mut conn, aside, Some(Finish::Done));
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "{starts:?}");
    let said = &back[0].text;
    assert!(said.starts_with("The task you gave is settled:"), "{said}");
    assert!(said.contains(&format!("#{aside} Bo")), "{said}");
}
