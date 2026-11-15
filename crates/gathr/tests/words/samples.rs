//! Samples of Gathr's words to agents (Stage 3, part A): each kind's main forms, as the real code
//! builds them from fixed made-up input. A sample's kind has a version in `crates/gathr/src/words.rs`,
//! and `tests/words.rs` keeps the hash of each version's samples: a text that changes without a new
//! version fails there. `<head>` and `<version>` stand for what no made-up input can fix.
// Each test crate that includes it uses part of it.
#![allow(dead_code)]

/// (kind, name, text), each kind's samples together.
pub const SAMPLES: &[(&str, &str, &str)] = &[
    (
        "brief",
        "brief: no why",
        "Task #1 from Lead: Fix the build\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "brief",
        "brief: why",
        "Task #2 from Lead: Fix the build\n\
         Why: CI is red\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "brief",
        "brief: carry",
        "Task #2 from Lead: Fix the build\n\
         Why: CI is red\n\
         Continue from here: the engine exited. Check what is already done before repeating work.\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "brief",
        "brief: complete",
        "Task #1 from Lead: Fix the build\n\
         Why: CI is red\n\
         Done when: CI on main is green\n\
         Proof: the CI run on the PR\n\
         Where: the project's worktree, on a new branch\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "brief",
        "brief: size",
        "Task #2 from Lead: Fix the build\n\
         Why: CI is red\n\
         Done when: CI on main is green\n\
         Proof: the CI run on the PR\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/build.rs, about 40 lines\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "review_brief",
        "review_brief: head seen, a task",
        "Task #4 from Lead: Review #70\n\
         For a review, include the commit you reviewed when you finish the task.\n\
         PR #70, at abc1234 as last seen.\n\
         It answers task #3 from Lead: Fix the label. Why: the label is cut off.\n\
         Say what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true.\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "review_brief",
        "review_brief: no PR",
        "Task #5 from Lead: Review the notes\n\
         For a review, include the commit you reviewed when you finish the task.\n\
         Say what in the change the ask doesn't need, as unasked notes, or nothing_unasked: true.\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "request_brief",
        "request_brief: choice",
        "Task #2 from Ann: What should the reconnect notice say?\n\
         Why: the notice ships today\n\
         Ann needs the person's answer. Check it's the person's call, not already answered, and the pick is sound.\n\
         Answer: one of its choices\n\
         1. Reconnecting… (their pick): it comes back by itself\n\
         2. Connection lost\n\
         Files: notice.png\n\
         Use ask_person with for_task 2 to file it; omit `what` to forward it unchanged. If an existing question \
         covers it, use finish_task blocked with on_task set to that question. To answer it yourself, use \
         finish_task done with the answer in note. When the person answers, finish #2 done with their answer in \
         note to pass it back to Ann.\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "request_brief",
        "request_brief: text",
        "Task #3 from Ann: Which port should the dev service use?\n\
         Ann needs the person's answer. Check it's the person's call, not already answered, and the pick is sound.\n\
         Answer: words\n\
         Use ask_person with for_task 3 to file it; omit `what` to forward it unchanged. If an existing question \
         covers it, use finish_task blocked with on_task set to that question. To answer it yourself, use \
         finish_task done with the answer in note. When the person answers, finish #3 done with their answer in \
         note to pass it back to Ann.\n\
         Use finish_task with done, left or blocked. Say what remains or which agent or task blocks it.",
    ),
    (
        "report_ask",
        "report_ask: no report",
        "Your last turn on task #1 (Fix the build) ended with no report. Use finish_task to say done, left or blocked.",
    ),
    (
        "hear_back",
        "hear_back: one settled, done",
        "The task you gave is settled:\n\
         - #1 Ann, Fix the build: done\n\
         Read replies with read_chat, then continue.",
    ),
    (
        "hear_back",
        "hear_back: several settled",
        "The 2 tasks you gave are settled:\n\
         - #2 Ann, Fix the build: done: see the change\n\
         - #3 Bo, Fix the build: cancelled: not needed\n\
         Read replies with read_chat, then continue. This turn is on your task #1: end it with finish_task.",
    ),
    (
        "hear_back",
        "hear_back: one stands",
        "Where the task you gave stands:\n\
         - #2 Ann, Fix the note: held: done, waiting on #90 to merge\n\
         Read replies with read_chat, then continue. This turn is on your task #1: end it with finish_task.",
    ),
    (
        "answered",
        "answered: the agent's own question",
        "The person answered:\n\
         - #1 Which name should the app use?: Gathr: keep it short\n\
         Continue.",
    ),
    (
        "answered",
        "answered: a member's request",
        "The person answered:\n\
         - #4 What should the reconnect notice say?: Connection lost: and say we retry\n\
         #4 answers the request from Ann. Use finish_task to mark #2 done and put the answer in note.\n\
         Continue.",
    ),
    (
        "wake",
        "wake: one, at once",
        "Not moving: #1 (Ann). Nothing waits on the person.\n\
         - #1 (Ann) is ready, and no turn is on it, for under a minute\n\
         Move it on: give, restart or cancel work, or ask the person. If nothing can be done, say why.",
    ),
    (
        "silent",
        "silent: on a task",
        "Ann's turn on #1 has made no progress for 12 min. It is still running. \
         A message to them reaches that turn.",
    ),
    (
        "wait_fired",
        "wait_fired: on a task",
        "Your wait ended: CI on #54 finished: 4 passed. You planned to do this: merge it. Check the current \
         status, then continue or explain what is blocking you. End the turn with finish_task for task #1.",
    ),
    (
        "wait_fired",
        "wait_fired: no task",
        "Your wait ended: your check came back yes. You planned to do this: merge. Check the current status, \
         then continue or explain what is blocking you.",
    ),
    (
        "wait_fired",
        "wait_fired: expired",
        "Your wait ended: You have waited 24 hours for a check (the last check failed: bash didn't start). \
         You planned to do this: merge. Check the current status, then continue or explain what is blocking you.",
    ),
    (
        "stale",
        "stale: one task, one day",
        "No progress in 1 day: #1 Fix the build (assigned to Ann, ready). Finish, restart or cancel it; \
         agent tasks auto-cancel after 1 more day without progress unless work is ongoing or paused.",
    ),
    (
        "stale",
        "stale: several tasks, several days",
        "No progress in 3 days: #1 Fix the build (assigned to Ann, ready)\n\
         #2 Fix the build (assigned to Ann, ready). Finish, restart or cancel them; \
         agent tasks auto-cancel after 7 more days without progress unless work is ongoing or paused.",
    ),
    (
        "landing",
        "landing: can't land",
        "Gathr can't land #5 at <head>: CI on <head> has a failure, which the project doesn't require to pass: \
         1 passed, 1 failed (Rust (macOS)). Read it with ci_failures. Your review stands, and this is yours to \
         judge: give its author a fix, or merge it with `merge`.",
    ),
    (
        "carry_on",
        "carry_on: update",
        "Gathr's update to <version> ended your turn. Check what is already done before repeating work, \
         then continue.",
    ),
    (
        "carry_on",
        "carry_on: Stop now",
        "Stop now ended your turn. Check what is already done before repeating work, then continue.",
    ),
    (
        "carry_on",
        "carry_on: usage limit",
        "The account's usage limit interrupted your turn. Check what is already done before repeating \
         work, then continue.",
    ),
];

/// Asserts that `built`, the text the real code built for the sample `name`, is that sample's text
/// byte for byte.
pub fn same(name: &str, built: &str) {
    let Some((kind, _, kept)) = SAMPLES.iter().find(|s| s.1 == name) else {
        panic!("there's no sample named {name:?} in tests/words/samples.rs");
    };
    assert!(
        built == *kept,
        "{kind}'s words changed: sample {name:?} no longer matches.\nbuilt: {built:?}\nkept:  {kept:?}\n\
         If the change is meant: copy the built text into tests/words/samples.rs, raise {} in \
         crates/gathr/src/words.rs by one, and run the words test for the row to add to KEPT. If it isn't, \
         the code is wrong.",
        kind.to_uppercase()
    );
}

/// The kind and version a turn recorded.
pub fn opened(conn: &rusqlite::Connection, turn: &str) -> (Option<String>, Option<i64>) {
    let sql = "SELECT opened_by, opened_version FROM turns WHERE id = ?1";
    conn.query_row(sql, [turn], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap_or_else(|e| panic!("turn {turn}'s kind: {e}"))
}
