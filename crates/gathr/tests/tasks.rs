//! Tasks' and waits' transitions (0010) on a real database: who may make each, what each starts,
//! and what a task is held on.

mod common;
#[path = "tasks/reviews.rs"]
mod review_evidence;

use common::TempDir;
use gathr::Error;
use gathr::tasks::{self, Finish, Give, Kind, Outcome, Said, Start, State};
use gathr::waits::{self, Leave, Part};
use rusqlite::{Connection, params};

const T: &str = "test-trace";

/// A chat with a Supervisor (`lead`) and two members (`ann`, `bo`).
fn team(dir: &TempDir) -> Connection {
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    conn.execute_batch(
        "INSERT INTO accounts VALUES ('acc', 'claude', 'Main', 'main', 1);
         INSERT INTO agents (id, name, name_key, account_id, created_ts) VALUES
             ('lead', 'Lead', 'lead', 'acc', 1), ('ann', 'Ann', 'ann', 'acc', 1),
             ('bo', 'Bo', 'bo', 'acc', 1);
         INSERT INTO chats (id, title, created_ts) VALUES ('c', 'Chat', 1), ('other', 'Other', 1);
         INSERT INTO chat_members VALUES ('c', 'lead', 'supervisor', 1), ('c', 'ann', 'member', 1),
             ('c', 'bo', 'member', 1), ('other', 'lead', 'supervisor', 1),
             ('other', 'ann', 'member', 1);",
    )
    .unwrap();
    conn
}

/// A turn of `agent`'s in chat `c`, on `task`, with `status`.
fn turn(conn: &Connection, id: &str, agent: &str, task: Option<i64>, status: &str) {
    gathr::store::add_turn(conn, "c", id, T, (Some(agent), task, None)).unwrap();
    conn.execute(
        "UPDATE turns SET status = ?2 WHERE id = ?1",
        params![id, status],
    )
    .unwrap();
}

fn give(
    conn: &mut Connection,
    owner: &str,
    after: &[i64],
    given_in: Option<&str>,
) -> (i64, Vec<Start>) {
    let g = Give {
        chat: "c",
        giver: Some("lead"),
        owner,
        given_in,
        parent: None,
        kind: Kind::Work,
        what: "Fix the build",
        why: Some("CI is red"),
        after,
        pr: None,
    };
    let (task, starts) = tasks::give(conn, &g, T).unwrap();
    (task.number, starts)
}

fn state(conn: &Connection, task: i64) -> (State, Option<String>) {
    let t = tasks::get(conn, task).unwrap().unwrap();
    (t.state, t.reason)
}

/// Runs one turn on the task: started, the owner's finish if any, then ended well.
fn work(conn: &mut Connection, task: i64, finish: Option<Finish>, tools: i64) -> Vec<Start> {
    tasks::started(conn, task, T).unwrap();
    let owner = tasks::get(conn, task).unwrap().unwrap().owner.unwrap();
    if let Some(how) = finish {
        tasks::finish(conn, &owner, task, how, &Said::default()).unwrap();
    }
    tasks::ended(conn, task, Outcome::Ok { tools }, T).unwrap()
}

fn refusal(result: Result<impl std::fmt::Debug, Error>) -> String {
    match result {
        Err(Error::Refused(why)) => why,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

#[test]
fn a_task_waits_for_its_after_tasks_then_starts_its_owners_turn() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (first, starts) = give(&mut conn, "ann", &[], None);
    assert_eq!(state(&conn, first).0, State::Ready);
    assert_eq!(starts.len(), 1, "{starts:?}");
    assert_eq!(
        (starts[0].agent.as_str(), starts[0].task),
        ("ann", Some(first))
    );
    assert!(
        starts[0]
            .text
            .starts_with(&format!("Task #{first} from Lead: Fix the build"))
    );
    let (second, starts) = give(&mut conn, "bo", &[first], None);
    assert_eq!(state(&conn, second).0, State::Waiting);
    assert!(
        starts.is_empty(),
        "a waiting task starts nothing: {starts:?}"
    );
    assert_eq!(
        tasks::may_start(&conn, second).unwrap().as_deref(),
        Some(format!("task #{second} still waits on its `after` tasks").as_str())
    );
    let starts = work(&mut conn, first, Some(Finish::Done), 3);
    assert_eq!(state(&conn, first).0, State::Done);
    assert_eq!(state(&conn, second).0, State::Ready);
    let bo: Vec<_> = starts.iter().filter(|s| s.agent == "bo").collect();
    assert_eq!(bo.len(), 1, "{starts:?}");
    assert_eq!(bo[0].task, Some(second));
}

#[test]
fn an_after_task_that_fails_blocks_its_dependant() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (first, _) = give(&mut conn, "ann", &[], None);
    let (second, _) = give(&mut conn, "bo", &[first], None);
    tasks::started(&mut conn, first, T).unwrap();
    let why = "the engine exited".to_string();
    tasks::ended(&mut conn, first, Outcome::Failed(why.clone()), T).unwrap();
    assert_eq!(state(&conn, first), (State::Failed, Some(why)));
    assert_eq!(
        state(&conn, second),
        (State::Blocked, Some(format!("#{first} is failed")))
    );
}

#[test]
fn a_turn_that_fails_before_it_runs_fails_its_task_so_it_can_restart() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let (after, _) = give(&mut conn, "bo", &[task], None);
    let why = "Claude Code's path isn't a file".to_string();
    tasks::ended(&mut conn, task, Outcome::Failed(why.clone()), T).unwrap();
    assert_eq!(state(&conn, task), (State::Failed, Some(why)));
    assert_eq!(state(&conn, after).0, State::Blocked, "its dependant moves");
    let starts = tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    assert_eq!(starts.len(), 1, "{starts:?}");
    let (cancelled, _) = give(&mut conn, "ann", &[], None);
    tasks::cancel(&mut conn, None, cancelled, "dropped", T).unwrap();
    tasks::ended(&mut conn, cancelled, Outcome::Failed("late".into()), T).unwrap();
    assert_eq!(state(&conn, cancelled).0, State::Cancelled, "stays so");
}

#[test]
fn what_a_change_starts_is_saved_with_it_so_a_stop_right_after_loses_nothing() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let wait = wait_on(&mut conn, vec![part("check", "test -f done")]).unwrap();
    waits::met(&mut conn, &wait.id, "your check came back yes", T).unwrap();
    // The service stops here, before anything it was handed is queued in memory.
    drop(conn);
    let conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    let waiting = gathr::store::take_over(&conn).unwrap();
    let ann = waiting.iter().find(|w| w.agent.as_deref() == Some("ann"));
    let ann = ann.expect("the task's turn is taken over");
    assert_eq!(ann.task, Some(task));
    assert!(ann.text.starts_with(&format!("Task #{task} from Lead")));
    let lead = waiting.iter().find(|w| w.agent.as_deref() == Some("lead"));
    let lead = lead.expect("the wake-up is taken over");
    assert!(
        lead.text.starts_with("Your wait ended: your check"),
        "{}",
        lead.text
    );
}

#[test]
fn a_giving_turn_cut_off_by_a_stop_still_brings_its_hear_back() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    turn(&conn, "lead-turn", "lead", None, "running");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    work(&mut conn, a, Some(Finish::Done), 1);
    tasks::recover(&mut conn).unwrap();
    let waiting = gathr::store::take_over(&conn).unwrap();
    let back = waiting.iter().find(|w| w.agent.as_deref() == Some("lead"));
    let back = back.expect("the hear-back is taken over");
    assert!(back.text.contains(&format!("#{a} Ann")), "{}", back.text);
}

#[test]
fn a_stop_between_a_giving_turns_end_and_its_round_still_brings_its_hear_back() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    work(&mut conn, a, Some(Finish::Done), 1);
    // The service stops after the turn's end is written, before its round runs.
    conn.execute_batch(
        "CREATE TEMP TRIGGER stop BEFORE UPDATE ON tasks BEGIN SELECT RAISE(ABORT, 'stopped'); END;",
    )
    .unwrap();
    let moved = tasks::Moved {
        chat: "c",
        turn: "lead-turn",
        agent: Some("lead"),
        task: Some(lead_task),
        trace: T,
    };
    assert!(tasks::turn_moved(&mut conn, &moved, "done", None).is_err());
    drop(conn);
    let mut conn = gathr::db::open(&dir.0.join("gathr.db")).unwrap();
    tasks::recover(&mut conn).unwrap();
    let waiting = gathr::store::take_over(&conn).unwrap();
    let to_lead: Vec<&str> = waiting
        .iter()
        .filter(|w| w.agent.as_deref() == Some("lead"))
        .map(|w| w.text.as_str())
        .collect();
    let heard = format!("#{a} Ann");
    assert!(
        to_lead.iter().any(|t| t.contains(&heard)),
        "no hear-back taken over: {to_lead:?}"
    );
}

#[test]
fn a_finish_applies_when_the_turn_ends_and_only_the_owner_gives_one() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let early = tasks::finish(&mut conn, "ann", task, Finish::Done, &Said::default());
    assert_eq!(
        refusal(early),
        format!("task #{task} is ready, not being worked on")
    );
    tasks::started(&mut conn, task, T).unwrap();
    let theirs = tasks::finish(&mut conn, "bo", task, Finish::Done, &Said::default());
    assert_eq!(refusal(theirs), format!("task #{task} isn't yours"));
    let said = Said {
        note: Some("needs the lead's call on naming"),
        ..Said::default()
    };
    tasks::finish(&mut conn, "ann", task, Finish::Blocked, &said).unwrap();
    assert_eq!(
        state(&conn, task).0,
        State::Working,
        "not before the turn ends"
    );
    tasks::ended(&mut conn, task, Outcome::Ok { tools: 2 }, T).unwrap();
    let reason = Some("needs the lead's call on naming".to_string());
    assert_eq!(state(&conn, task), (State::Blocked, reason));
}

#[test]
fn a_turn_that_ends_with_work_open_holds_its_task_and_says_on_what() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let (handed, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let wait = Leave {
        chat: "c",
        agent: "lead",
        task: Some(lead_task),
        left_in: Some("lead-turn"),
        cwd: None,
        then_do: "merge it",
        parts: vec![Part {
            kind: "ci".into(),
            target: "#54".into(),
            repo: None,
        }],
    };
    waits::leave(&mut conn, &wait, T).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok { tools: 4 }, T).unwrap();
    let why = format!("done, waiting on CI on #54, #{handed} (Ann)");
    assert_eq!(state(&conn, lead_task), (State::Held, Some(why)));
    // With nothing open, the same end is done.
    let (plain, _) = give(&mut conn, "bo", &[], None);
    work(&mut conn, plain, None, 1);
    assert_eq!(state(&conn, plain).0, State::Done);
}

fn to_lead(starts: &[Start]) -> Vec<&Start> {
    starts.iter().filter(|s| s.agent == "lead").collect()
}

#[test]
fn the_giver_hears_back_once_its_turn_has_ended_and_every_task_from_it_is_settled() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    let starts = work(&mut conn, a, Some(Finish::Done), 1);
    assert!(
        to_lead(&starts).is_empty(),
        "the lead is still giving: {starts:?}"
    );
    let (b, _) = give(&mut conn, "bo", &[], Some("lead-turn"));
    gathr::store::set_turn(&conn, "lead-turn", "done", None).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok { tools: 1 }, T).unwrap();
    assert_eq!(state(&conn, lead_task).0, State::Held);
    let ended = tasks::turn_ended(&mut conn, "lead-turn", T).unwrap();
    assert!(to_lead(&ended).is_empty(), "#{b} is open: {ended:?}");
    let starts = tasks::cancel(&mut conn, Some("lead"), b, "not needed", T).unwrap();
    let back = to_lead(&starts);
    assert_eq!(back.len(), 1, "one hear-back: {starts:?}");
    assert_eq!(back[0].task, Some(lead_task), "on the lead's own task");
    assert!(
        back[0]
            .text
            .contains(&format!("#{a} Ann, Fix the build: done"))
    );
    assert!(
        back[0]
            .text
            .contains(&format!("#{b} Bo, Fix the build: cancelled: not needed"))
    );
    let again = tasks::turn_ended(&mut conn, "lead-turn", T).unwrap();
    assert!(again.is_empty(), "once: {again:?}");
}

#[test]
fn a_giver_whose_tasks_settle_while_it_gives_hears_back_when_its_turn_ends() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (lead_task, _) = give(&mut conn, "lead", &[], None);
    tasks::started(&mut conn, lead_task, T).unwrap();
    turn(&conn, "lead-turn", "lead", Some(lead_task), "running");
    let (a, _) = give(&mut conn, "ann", &[], Some("lead-turn"));
    work(&mut conn, a, Some(Finish::Done), 1);
    gathr::store::set_turn(&conn, "lead-turn", "done", None).unwrap();
    tasks::ended(&mut conn, lead_task, Outcome::Ok { tools: 1 }, T).unwrap();
    assert_eq!(
        state(&conn, lead_task),
        (State::Held, Some(format!("done, waiting on #{a} (Ann)"))),
        "not done before it has heard back"
    );
    let back = tasks::turn_ended(&mut conn, "lead-turn", T).unwrap();
    assert_eq!(back.len(), 1, "{back:?}");
    assert_eq!(back[0].task, Some(lead_task));
}

#[test]
fn left_twice_by_turns_that_did_nothing_is_blocked() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let starts = work(&mut conn, task, Some(Finish::Left), 5);
    assert_eq!(
        state(&conn, task).0,
        State::Ready,
        "work done: another turn"
    );
    assert!(starts[0].text.contains("Continue from here: work remains"));
    work(&mut conn, task, Some(Finish::Left), 0);
    assert_eq!(state(&conn, task).0, State::Ready);
    let starts = work(&mut conn, task, Some(Finish::Left), 0);
    assert!(starts.is_empty(), "{starts:?}");
    assert_eq!(state(&conn, task).0, State::Blocked);
}

#[test]
fn a_task_blocked_on_an_existing_task_carries_on_when_it_is_done() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (fix, _) = give(&mut conn, "bo", &[], None);
    let (task, _) = give(&mut conn, "ann", &[], None);
    tasks::started(&mut conn, task, T).unwrap();
    let said = Said {
        on_task: Some(fix),
        ..Said::default()
    };
    tasks::finish(&mut conn, "ann", task, Finish::Blocked, &said).unwrap();
    tasks::ended(&mut conn, task, Outcome::Ok { tools: 1 }, T).unwrap();
    assert_eq!(state(&conn, task).0, State::Blocked);
    let starts = work(&mut conn, fix, Some(Finish::Done), 2);
    assert_eq!(state(&conn, task).0, State::Ready);
    let ann: Vec<_> = starts.iter().filter(|s| s.agent == "ann").collect();
    assert!(
        ann[0]
            .text
            .contains(&format!("the task blocking this one, #{fix}, is done"))
    );
}

#[test]
fn a_restart_stops_working_tasks_and_a_restart_by_the_giver_carries_on() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    let (after, _) = give(&mut conn, "bo", &[task], None);
    tasks::started(&mut conn, task, T).unwrap();
    assert_eq!(tasks::recover(&mut conn).unwrap(), 1);
    let stopped = Some("Gathr stopped during its turn".to_string());
    assert_eq!(state(&conn, task), (State::Stopped, stopped));
    assert_eq!(
        state(&conn, after).0,
        State::Waiting,
        "its dependant isn't broken"
    );
    let refused = tasks::restart(&mut conn, Some("bo"), task, T);
    assert_eq!(
        refusal(refused),
        format!("only task #{task}'s giver or the person may restart it")
    );
    let starts = tasks::restart(&mut conn, Some("lead"), task, T).unwrap();
    assert!(
        starts[0]
            .text
            .contains("Continue from here: Gathr stopped during its turn")
    );
    assert_eq!(state(&conn, task).0, State::Ready);
}

#[test]
fn a_cancelled_tasks_queued_turn_never_starts_and_its_waits_end() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let (task, _) = give(&mut conn, "ann", &[], None);
    turn(&conn, "bo-turn", "bo", None, "running");
    let wait = Leave {
        chat: "c",
        agent: "ann",
        task: Some(task),
        left_in: None,
        cwd: None,
        then_do: "carry on",
        parts: vec![Part {
            kind: "agent".into(),
            target: "bo".into(),
            repo: None,
        }],
    };
    let left = waits::leave(&mut conn, &wait, T).unwrap();
    tasks::cancel(&mut conn, None, task, "the person dropped it", T).unwrap();
    assert_eq!(
        tasks::may_start(&conn, task).unwrap().as_deref(),
        Some(format!("task #{task} is cancelled").as_str())
    );
    assert_eq!(waits::read(&conn, &left.id).unwrap().state, "cancelled");
}

#[test]
fn a_tasks_pr_settles_it_and_a_review_follows_its_commit() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let review = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: None,
        parent: None,
        kind: Kind::Review,
        what: "Review #54",
        why: None,
        after: &[],
        pr: Some(54),
    };
    let (task, _) = tasks::give(&mut conn, &review, T).unwrap();
    tasks::started(&mut conn, task.number, T).unwrap();
    let done = tasks::finish(&mut conn, "bo", task.number, Finish::Done, &Said::default());
    assert_eq!(
        refusal(done),
        "Include the reviewed commit when you finish a review"
    );
    let said = Said {
        commit: Some("abc1234"),
        ..Said::default()
    };
    tasks::finish(&mut conn, "bo", task.number, Finish::Done, &said).unwrap();
    tasks::ended(&mut conn, task.number, Outcome::Ok { tools: 3 }, T).unwrap();
    assert_eq!(
        state(&conn, task.number).0,
        State::Done,
        "the PR is what was reviewed, not work the reviewer owes"
    );
    let followed = |conn: &Connection| {
        let prs = tasks::with_prs(conn).unwrap();
        prs.iter().any(|p| p.0 == task.number)
    };
    assert!(followed(&conn), "its PR is still followed for G73");
    tasks::pr_looked(&mut conn, task.number, "OPEN", Some("def5678aaaa"), T).unwrap();
    let looked = tasks::get(&conn, task.number).unwrap().unwrap();
    assert!(looked.outdated, "G73");
    // The head the look saw, so the window shows the checks run at that commit (0036 C6).
    assert_eq!(looked.pr_head.as_deref(), Some("def5678aaaa"));
    tasks::pr_looked(&mut conn, task.number, "MERGED", Some("def5678aaaa"), T).unwrap();
    assert_eq!(state(&conn, task.number).0, State::Done);
    assert!(!followed(&conn), "a merged PR is looked at no more");
    let work_pr = Give {
        owner: "ann",
        kind: Kind::Work,
        what: "Fix #61",
        pr: Some(61),
        ..review
    };
    let (work_pr, _) = tasks::give(&mut conn, &work_pr, T).unwrap();
    work(&mut conn, work_pr.number, Some(Finish::Done), 2);
    let held = Some("done, waiting on #61 to merge".to_string());
    assert_eq!(state(&conn, work_pr.number), (State::Held, held), "G75");
    tasks::pr_looked(&mut conn, work_pr.number, "MERGED", None, T).unwrap();
    assert_eq!(
        state(&conn, work_pr.number),
        (State::Done, Some("#61 merged".into()))
    );
    let (other, _) = give(&mut conn, "ann", &[], None);
    conn.execute("UPDATE tasks SET pr = 60 WHERE number = ?1", [other])
        .unwrap();
    tasks::pr_looked(&mut conn, other, "CLOSED", None, T).unwrap();
    let why = Some("#60 was closed without merging".to_string());
    assert_eq!(state(&conn, other), (State::Cancelled, why));
    // S72: a review under way on a PR that closes is cancelled too, not only the work task.
    let under_way = Give {
        pr: Some(62),
        what: "Review #62",
        ..review
    };
    let (under_way, _) = tasks::give(&mut conn, &under_way, T).unwrap();
    tasks::started(&mut conn, under_way.number, T).unwrap();
    tasks::pr_looked(&mut conn, under_way.number, "CLOSED", None, T).unwrap();
    let why = Some("#62 was closed without merging".to_string());
    assert_eq!(state(&conn, under_way.number), (State::Cancelled, why));
}

/// Ann's fix with its PR, #69.
fn fix_with_pr(conn: &mut Connection) -> i64 {
    let fix = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix the label",
        why: None,
        after: &[],
        pr: Some(69),
    };
    tasks::give(conn, &fix, T).unwrap().0.number
}

// Found live: the review of #69 waited on the fix, held until #69 merged, which waited on the review.
#[test]
fn a_task_done_and_held_only_on_its_pr_lets_the_tasks_after_it_start() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = fix_with_pr(&mut conn);
    let (review, _) = give(&mut conn, "bo", &[fix], None);
    let starts = work(&mut conn, fix, Some(Finish::Done), 2);
    let held = Some("done, waiting on #69 to merge".to_string());
    assert_eq!(
        state(&conn, fix),
        (State::Held, held),
        "settled by the merge"
    );
    assert_eq!(
        state(&conn, review).0,
        State::Ready,
        "the review of #69 starts while #69 waits on it"
    );
    let bo: Vec<_> = starts.iter().filter(|s| s.agent == "bo").collect();
    assert_eq!(bo.len(), 1, "{starts:?}");
    let merged = tasks::pr_looked(&mut conn, fix, "MERGED", None, T).unwrap();
    assert_eq!(state(&conn, fix).0, State::Done);
    let again: Vec<_> = merged.iter().filter(|s| s.agent == "bo").collect();
    assert!(again.is_empty(), "the review starts once: {again:?}");
}

#[test]
fn a_task_held_on_more_than_its_pr_or_left_still_holds_the_tasks_after_it() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = fix_with_pr(&mut conn);
    let (review, _) = give(&mut conn, "bo", &[fix], None);
    tasks::started(&mut conn, fix, T).unwrap();
    let wait = Leave {
        chat: "c",
        agent: "ann",
        task: Some(fix),
        left_in: None,
        cwd: Some("."),
        then_do: "check the logs",
        parts: vec![part("ci", "#69")],
    };
    waits::leave(&mut conn, &wait, T).unwrap();
    tasks::ended(&mut conn, fix, Outcome::Ok { tools: 2 }, T).unwrap();
    let held = Some("done, waiting on CI on #69, #69 to merge".to_string());
    assert_eq!(state(&conn, fix), (State::Held, held));
    assert_eq!(state(&conn, review).0, State::Waiting, "held on a wait");
    let left = fix_with_pr(&mut conn);
    let (after_left, _) = give(&mut conn, "bo", &[left], None);
    work(&mut conn, left, Some(Finish::Left), 2);
    assert_eq!(
        state(&conn, after_left).0,
        State::Waiting,
        "its owner left work"
    );
}

#[test]
fn a_task_is_refused_with_why() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let mut g = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "nobody",
        given_in: None,
        parent: None,
        kind: Kind::Work,
        what: "Fix it",
        why: None,
        after: &[],
        pr: None,
    };
    let outsider = refusal(tasks::give(&mut conn, &g, T));
    assert_eq!(
        outsider,
        "that agent isn't in this chat: its agents are Lead, Ann, Bo"
    );
    g.owner = "ann";
    let long = "x".repeat(81);
    g.what = &long;
    assert_eq!(
        refusal(tasks::give(&mut conn, &g, T)),
        "`what` is 81 characters: cut 1 to fit 80, and put the details in `why`"
    );
    g.what = "Fix it\nnow";
    assert_eq!(
        refusal(tasks::give(&mut conn, &g, T)),
        "`what` is one line of 1 to 80 characters"
    );
    g.what = "Fix it";
    g.chat = "other";
    let (elsewhere, _) = tasks::give(&mut conn, &g, T).unwrap();
    g.chat = "c";
    let after = [elsewhere.number];
    g.after = &after;
    let missing = refusal(tasks::give(&mut conn, &g, T));
    assert_eq!(
        missing,
        format!("there's no task #{} in this chat", elsewhere.number)
    );
}

fn wait_on(conn: &mut Connection, parts: Vec<Part>) -> Result<waits::Wait, Error> {
    let leave = Leave {
        chat: "c",
        agent: "lead",
        task: None,
        left_in: None,
        cwd: Some("."),
        then_do: "merge",
        parts,
    };
    waits::leave(conn, &leave, T)
}

fn part(kind: &str, target: &str) -> Part {
    Part {
        kind: kind.into(),
        target: target.into(),
        repo: None,
    }
}

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
    waits::looked(&conn, &wait.id, None, Some("bash didn't start")).unwrap();
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

/// Ann's work on `pr`, given in Lead's turn, which has ended: Lead is the one to hear back.
fn work_on_pr(conn: &mut Connection, pr: i64) -> i64 {
    let giving = format!("lead-{pr}");
    turn(conn, &giving, "lead", None, "running");
    let fix = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "ann",
        given_in: Some(&giving),
        parent: None,
        kind: Kind::Work,
        what: "Fix the summary",
        why: None,
        after: &[],
        pr: Some(pr),
    };
    let fix = tasks::give(conn, &fix, T).unwrap().0.number;
    conn.execute("UPDATE turns SET status = 'done' WHERE id = ?1", [&giving])
        .unwrap();
    assert!(tasks::turn_ended(conn, &giving, T).unwrap().is_empty());
    fix
}

/// Bo's review of `pr`, given by Ann in `given_in`.
fn review_by_bo(conn: &mut Connection, pr: i64, given_in: &str) -> i64 {
    let review = Give {
        chat: "c",
        giver: Some("ann"),
        owner: "bo",
        given_in: Some(given_in),
        parent: None,
        kind: Kind::Review,
        what: "Review the fix",
        why: None,
        after: &[],
        pr: Some(pr),
    };
    tasks::give(conn, &review, T).unwrap().0.number
}

fn reviewed(conn: &mut Connection, review: i64) -> Vec<Start> {
    tasks::started(conn, review, T).unwrap();
    let said = Said {
        commit: Some("abc1234"),
        ..Said::default()
    };
    tasks::finish(conn, "bo", review, Finish::Done, &said).unwrap();
    tasks::ended(conn, review, Outcome::Ok { tools: 3 }, T).unwrap()
}

fn to(starts: &[Start], agent: &str) -> Vec<String> {
    let mine = starts.iter().filter(|s| s.agent == agent);
    mine.map(|s| s.text.clone()).collect()
}

// Found live (#85, #86): the work was held on its merge after its review, and its giver, the one
// to see it merged, never heard.
#[test]
fn a_task_held_only_on_its_merge_wakes_its_giver_once_its_review_is_done() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = work_on_pr(&mut conn, 85);
    tasks::started(&mut conn, fix, T).unwrap();
    turn(&conn, "ann-turn", "ann", Some(fix), "running");
    let review = review_by_bo(&mut conn, 85, "ann-turn");
    tasks::finish(&mut conn, "ann", fix, Finish::Done, &Said::default()).unwrap();
    tasks::ended(&mut conn, fix, Outcome::Ok { tools: 2 }, T).unwrap();
    conn.execute("UPDATE turns SET status = 'done' WHERE id = 'ann-turn'", [])
        .unwrap();
    let starts = reviewed(&mut conn, review);
    assert_eq!(to(&starts, "ann").len(), 1, "Ann hears back on the review");
    let starts = work(&mut conn, fix, Some(Finish::Done), 1);
    let held = Some("done, waiting on #85 to merge".to_string());
    assert_eq!(state(&conn, fix), (State::Held, held));
    let lead = to(&starts, "lead");
    assert_eq!(
        lead.len(),
        1,
        "Lead hears it waits on the merge: {starts:?}"
    );
    assert!(lead[0].contains("waiting on #85 to merge"), "{}", lead[0]);
    let merged = tasks::pr_looked(&mut conn, fix, "MERGED", None, T).unwrap();
    let lead = to(&merged, "lead");
    assert_eq!(lead.len(), 1, "Lead hears the merge too: {merged:?}");
    assert!(lead[0].contains("#85 merged"), "{}", lead[0]);
}

#[test]
fn a_review_still_open_holds_the_hear_back_until_it_settles() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = work_on_pr(&mut conn, 86);
    turn(&conn, "ann-other", "ann", None, "running");
    let review = review_by_bo(&mut conn, 86, "ann-other");
    let starts = work(&mut conn, fix, Some(Finish::Done), 2);
    assert!(
        to(&starts, "lead").is_empty(),
        "the review leads to the merge"
    );
    let starts = reviewed(&mut conn, review);
    let lead = to(&starts, "lead");
    assert_eq!(lead.len(), 1, "{starts:?}");
    assert!(lead[0].contains("waiting on #86 to merge"), "{}", lead[0]);
}

/// Ann's work on `pr` held only on its merge, already heard by Lead (Vale's #100 review).
fn heard_while_held(conn: &mut Connection, pr: i64) -> i64 {
    let fix = work_on_pr(conn, pr);
    let starts = work(conn, fix, Some(Finish::Done), 2);
    assert_eq!(to(&starts, "lead").len(), 1, "{starts:?}");
    fix
}

#[test]
fn a_heard_task_whose_pr_closes_or_is_cancelled_wakes_its_giver_once_more() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let fix = heard_while_held(&mut conn, 87);
    let closed = tasks::pr_looked(&mut conn, fix, "CLOSED", None, T).unwrap();
    let lead = to(&closed, "lead");
    assert_eq!(lead.len(), 1, "{closed:?}");
    assert!(
        lead[0].contains("#87 was closed without merging"),
        "{}",
        lead[0]
    );
    let again = tasks::pr_looked(&mut conn, fix, "CLOSED", None, T).unwrap();
    assert!(to(&again, "lead").is_empty(), "heard once: {again:?}");
    let fix = heard_while_held(&mut conn, 88);
    let cancelled = tasks::cancel(&mut conn, None, fix, "not needed", T).unwrap();
    assert_eq!(to(&cancelled, "lead").len(), 1, "{cancelled:?}");
}

// Found in #100's own review: Vale's review gave the fix on the same PR. The fix, held on the merge,
// waited on that open review while the review waited to hear back on the fix.
#[test]
fn a_fix_given_by_the_review_of_its_pr_wakes_that_reviewer() {
    let dir = TempDir::new();
    let mut conn = team(&dir);
    let review = Give {
        chat: "c",
        giver: Some("lead"),
        owner: "bo",
        given_in: None,
        parent: None,
        kind: Kind::Review,
        what: "Review #90",
        why: None,
        after: &[],
        pr: Some(90),
    };
    let review = tasks::give(&mut conn, &review, T).unwrap().0.number;
    tasks::started(&mut conn, review, T).unwrap();
    turn(&conn, "bo-turn", "bo", Some(review), "running");
    let fix = Give {
        chat: "c",
        giver: Some("bo"),
        owner: "ann",
        given_in: Some("bo-turn"),
        parent: None,
        kind: Kind::Work,
        what: "Fix the note",
        why: None,
        after: &[],
        pr: Some(90),
    };
    let fix = tasks::give(&mut conn, &fix, T).unwrap().0.number;
    tasks::ended(&mut conn, review, Outcome::Ok { tools: 2 }, T).unwrap();
    conn.execute("UPDATE turns SET status = 'done' WHERE id = 'bo-turn'", [])
        .unwrap();
    let starts = work(&mut conn, fix, Some(Finish::Done), 2);
    let held = Some("done, waiting on #90 to merge".to_string());
    assert_eq!(state(&conn, fix), (State::Held, held));
    assert_eq!(to(&starts, "bo").len(), 1, "Bo hears back: {starts:?}");
}
