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
        "brief",
        "brief: look into",
        "Task #1 from Mira: Survey tag libraries\n\
         Why: Notes need tags\n\
         Done when: Three libraries compared\n\
         Proof: a table in notes/tags.md\n\
         Where: no code\n\
         This is a look-into task: don't change the project's files or push. Write what you make in your own \
         folder or Shared, and name it or say what you found when you finish.\n\
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
        "role",
        "role: supervisor",
        "When you only agree with another agent's message, give it a thumbs-up with react instead of replying.",
    ),
    (
        "role",
        "role: member",
        "Make the smallest change that does what was asked: no fallback, guard, abstraction or compatibility \
         nobody asked for, and remove what it replaces.\n\
         When you only agree with another agent's message, give it a thumbs-up with react instead of replying.",
    ),
    (
        "role",
        "role: reviewer",
        "When you only agree with another agent's message, give it a thumbs-up with react instead of replying.",
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
         To ask the person, use ask_person with for_task 2 and your own `what`, your pick first. If an open \
         question of yours already asks it, use finish_task blocked with on_task set to that question: this request \
         comes back to you with its answer. To answer it yourself, use finish_task done with the answer in note.\n\
         If you ask the person, their answer reaches Ann by itself: after ask_person, end your turn with no finish_task.",
    ),
    (
        "request_brief",
        "request_brief: text",
        "Task #3 from Ann: Which port should the dev service use?\n\
         Ann needs the person's answer. Check it's the person's call, not already answered, and the pick is sound.\n\
         Answer: words\n\
         To ask the person, use ask_person with for_task 3 and your own `what`, your pick first. If an open \
         question of yours already asks it, use finish_task blocked with on_task set to that question: this request \
         comes back to you with its answer. To answer it yourself, use finish_task done with the answer in note.\n\
         If you ask the person, their answer reaches Ann by itself: after ask_person, end your turn with no finish_task.",
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
        "hear_back",
        "hear_back: a plan check left",
        "The task you gave is settled:\n\
         - #1 Bo, Check plan v1: Tags on notes: left: Two gaps. Findings: note 1, wrong, src/notes.rs: The list query \
         misses tags; note 2, unasked: Export isn't asked. Answer each in the next version's answers.\n\
         Read replies with read_chat, then continue.",
    ),
    (
        "answered",
        "answered: the agent's own question",
        "The person answered:\n\
         - #1 Which name should the app use?: Picked: Gathr. Their words: keep it short\n\
         Continue.",
    ),
    (
        "answered",
        "answered: a member's request",
        "The person answered:\n\
         - #4 What should the reconnect notice say?: Picked: Connection lost. Their words: and say we retry\n\
         #4 was asked for Ann (#2): Ann has the answer.\n\
         Continue.",
    ),
    (
        "answered",
        "answered: discussed",
        "The person wants to discuss:\n\
         - #4 What should the reconnect notice say?: Does it retry by itself?\n\
         #4 was asked for Ann (#2), who keeps waiting.\n\
         #4 is off the person's list until this turn ends. Reply to them here. To change the question, ask a new one \
         with ask_person: it takes #4's place.",
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
        "plan_check",
        "plan_check: first",
        "Task #10 from Mira: Check plan v1: Tags on notes\n\
         Check version 1 of Mira's plan \"Tags on notes\" before the person sees it. Say what is wrong or missing \
         (a wrong fact or file, a missed step or caller, a part given to the wrong member, a proof that proves \
         nothing) and what the ask doesn't need, with file and line.\n\
         The person asked: Add tags to notes\n\
         The plan, message <id>:\n\
         Plan: Tags on notes (version 1, 3 parts)\n\
         Let notes carry tags, shown in the list and used to filter.\n\
         \n\
         1. Add the tags table (Ann)\n\
         Why: Notes need tags\n\
         Done when: A note carries its tags\n\
         Proof: tests/tags.rs passes\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/tags.rs, about 60 lines\n\
         \n\
         2. Show tags in the list (Bo), after part 1\n\
         Why: Notes need tags\n\
         Done when: A note carries its tags\n\
         Proof: tests/tags.rs passes\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/tags.rs, about 60 lines\n\
         \n\
         3. Filter by tag (Ann), after part 2 and #9\n\
         Why: Notes need tags\n\
         Done when: A note carries its tags\n\
         Proof: tests/tags.rs passes\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/tags.rs, about 60 lines\n\
         If you agree, give message <id> a thumbs-up with react: that finishes this task. Else finish_task left, \
         with notes: wrong, unasked or suggestion, each with its evidence in says.",
    ),
    (
        "plan_check",
        "plan_check: again",
        "Task #2 from Mira: Check plan v2: Tags on notes\n\
         Check version 2 of Mira's plan \"Tags on notes\" for your findings on version 1 only, with Mira's answers:\n\
         - Note 1, wrong: src/notes.rs: The list query misses tags. Mira: changed: The list reads tags now\n\
         - Note 2, unasked: Export isn't asked. Mira: kept: The person asked for export last week\n\
         The plan, message <id>:\n\
         Plan: Tags on notes (version 2, 1 part)\n\
         Let notes carry tags, shown in the list and used to filter.\n\
         \n\
         1. Add the tags table (Ann)\n\
         Why: Notes need tags\n\
         Done when: A note carries its tags\n\
         Proof: tests/tags.rs passes\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/tags.rs, about 60 lines\n\
         If the answers hold, give message <id> a thumbs-up with react: that finishes this task. Else finish_task \
         left, with notes on what still fails.",
    ),
    (
        "plan_check",
        "plan_check: carry",
        "Task #2 from Mira: Check plan v1: Tags on notes\n\
         Check version 1 of Mira's plan \"Tags on notes\" before the person sees it. Say what is wrong or missing \
         (a wrong fact or file, a missed step or caller, a part given to the wrong member, a proof that proves \
         nothing) and what the ask doesn't need, with file and line.\n\
         The person asked: Add folders\n\
         The plan, message <id>:\n\
         Plan: Tags on notes (version 1, 1 part)\n\
         Let notes carry tags, shown in the list and used to filter.\n\
         \n\
         1. Add the tags table (Ann)\n\
         Why: Notes need tags\n\
         Done when: A note carries its tags\n\
         Proof: tests/tags.rs passes\n\
         Where: the project's worktree, on a new branch\n\
         Size: src/tags.rs, about 60 lines\n\
         Continue from here: your engine closed and ended your background command. Check what is already done \
         before repeating work.\n\
         If you agree, give message <id> a thumbs-up with react: that finishes this task. Else finish_task left, \
         with notes: wrong, unasked or suggestion, each with its evidence in says.",
    ),
    (
        "sent_back",
        "sent_back",
        "The person sent back version 1 of your plan \"Tags on notes\" (plan #1):\n\
         Split part 2\n\
         Write the next version with plan.",
    ),
    (
        "resized",
        "resized: new with tasks",
        "The person made \"Add tags to notes\" new, so it waits for a plan. Its tasks that hadn't started wait \
         for the plan to be approved: #1 (Ann). Write the plan with plan, and cancel a task the plan replaces.",
    ),
    (
        "resized",
        "resized: new",
        "The person made \"Fix the export\" new, so it waits for a plan. Write the plan with plan.",
    ),
    (
        "resized",
        "resized: repair with tasks",
        "The person made \"Add tags to notes\" a repair, so it goes ahead without a plan. Its tasks that waited \
         for the plan are back in line: #1 (Ann).",
    ),
    (
        "resized",
        "resized: repair",
        "The person made \"Fix the typo\" a repair, so it goes ahead without a plan.",
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
    (
        "liked",
        "liked: a message",
        "The person liked your message <id> of <time>: \"Hi!\". A like means they approve it or agree. \
         If it answers something you were waiting on, carry on. Otherwise end your turn without replying.",
    ),
    (
        "reply",
        "reply: a message",
        "The person replies to your message (message <id>, <time>): \"Hi!\"",
    ),
    (
        "reply",
        "reply: a task",
        "The person replies about your task #1 \"Write the exporter\".",
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
