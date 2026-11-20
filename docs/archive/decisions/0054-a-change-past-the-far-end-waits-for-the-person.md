# 0054: A change past the far end waits for the person, and every landed change is recorded

Status: accepted (owner, 2026-11-14, "A: far end only"). Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/tasks/changes.rs`, `crates/gathr/tests/merge/changes.rs`, `crates/gathr/tests/archive.rs`, `ui/src/tasks/TasksTab.test.tsx`.

Builds on [0049](0049-gathr-lands-pull-requests.md) and [0053](0053-a-review-asks-both-questions.md). Serves promise 10 of [the charter](../charter.md#the-promises), part 10.7 of [the truth map](../truth-map.md): a change past a project's far end waits for the person, and Gathr keeps a record of every change it lands.

## The choice

- **S1: what a change adds.** Gathr counts the lines a PR adds with `git diff --numstat --no-renames -z <base>...<head>`.
  The count leaves out binary files, lock files by name (`Cargo.lock`, `package-lock.json`, `yarn.lock`,
  `pnpm-lock.yaml`, `poetry.lock`, `go.sum`, `Gemfile.lock`, `composer.lock`) and the paths the base's own
  attributes mark `linguist-generated` or `linguist-vendored` (`git check-attr --source=<base>`, value `set` or
  `true`; a git older than 2.40 has no `--source`, and then the checkout's own attributes are read). It also keeps the lines removed and the five largest counted files. The sign-off's list of changed
  files is read from the same diff, counted or not.
- **S2: the far end.** A project's line is the most lines any of its last 50 changes on the base added, counted as
  in S1 and never under 3,000. Below 10 changes it is 3,000. It is read with `git log --first-parent
  --diff-merges=first-parent -n 50 --numstat`, kept in memory for each project and base, and read again when the
  base moves.
- **S3: measured at landing.** `stand` in `landing.rs` and the merge tool measure the PR once its review covers
  the head and it is open into the default branch, before the sign-off. Landing's look and the merge tool's call
  are the only measures. A row of `changes` (migration 0041) keeps the PR's head, count and line, and is written
  only at a new head or line, with `gathr.change.measured`. A head with no change at the same line is read from
  the row, with no `check-attr`.
- **S4: over, and the question.** A PR is over when it adds more than the line, and, if the person let a count
  through, more than that count plus the line. An over PR gets one question for the person from Gathr: a task
  with no giver and no owner, `ready`, a choice, on the PR, under the work task it answers (the earliest one on
  the PR in the chat that isn't cancelled). It carries the numbers and the five largest files. The work task is
  untouched and no turn starts. The PR rests on it: landing says `waits on the person's answer to #9`, and the
  merge tool refuses with `#5 waits on the person's answer to #9` and a reference.
  - What: `#5 adds 3,200 lines, over this project's line of 3,000: let it go on?`
  - Why: the lines added and removed, the line and where it came from (`the most any of its last 50 changes
    added`, or `the least it can be` at 3,000), the largest files, and `It answers task #1 from Lead: Add a
    retry.` or `No task in this chat asks for it.`
  - Choices: `Let it go on` (`It lands once its review and checks pass.`) and `Send it back` (`#1 is blocked, and
    Lead is told.`, or `It doesn't land at this head.` with no work task). Gathr's question has no recommended
    choice.
- **S5: only the person answers it, with one choice.** An agent's answer to it is refused with `#9 is Gathr's
  question to the person: only the person answers it`. Anything but exactly one pick is refused with `Answer #9
  with one of its choices`. Each refusal carries a reference. The waiting answer says `#5 to land: waits on your
  answer to #9`, and the line waits on you.
- **S6: the window names Gathr.** A question for the person with no giver is shown as asked by Gathr (any other
  task with no giver is the person's), with no "Recommended", and
  Send needs a pick: notes alone don't answer it.
- **S7: "Let it go on".** The count asked about is let through. If the PR's count is past it plus the line, the next
  question is filed in the same transaction. Gathr looks at landing at once, so the PR lands without waiting for
  the next sweep.
- **S8: "Send it back".** The PR rests at that head. The work task it answers is blocked with `The person sent #5
  back as bigger than asked: +3,200 lines; this project's line is 3,000. Narrow #1's ask and restart it, or
  cancel it.`, and its giver hears, as for any blocked task. A finished or cancelled work task isn't touched.
  Restarting a blocked task now carries its reason into the brief, for every blocked task, not only these.
- **S9: a sent-back head never lands.** A later review at that head changes nothing. A new head is measured: past
  the line it is asked again, otherwise it lands on a review that reaches it. The waiting line is `#5 to land:
  you sent it back; waits on a new head`, and it moves others, so a send-back wakes no one.
- **S10: other endings.**
  - A smaller head, or a higher line, withdraws an open question when landing measures it: the question is
    cancelled with `#5 is now +500, under this project's line of 3,000`, and the PR goes on.
  - A question the person cancels rests the PR at that head: `waits on a new head: the person cancelled #9`. A
    new head can be asked again.
  - Archiving a chat cancels its question like any task.
  - A PR seen merged or closed ends its question (`#5 merged`, `#5 was closed`). Closed removes the row, with
    `gathr.change.closed`. Merged keeps it as the record. `gathr.change.merged_unasked` says a PR merged with the
    question open, and `gathr.change.merged_sent_back` one merged at a head the person sent back.
  - `settle` no longer reads the person's tasks as the PR's.
- **S11: the record.** When a PR merges, its row is filled once: the builder (the owner of its first work task)
  and the reviewer (its newest review that isn't cancelled), each with the engine and model they run on, the
  count of `unasked` notes of its reviews, the commit its first review named (`tasks.first_reviewed`, set by a
  review's first report), whether Gathr merged it (`by_gathr`), and when. It is written by the look that sees
  the merge, and by `settle` after Gathr's own merge. `push_merge` marks the row as Gathr's once its push succeeds,
  before GitHub is asked, so a merge a look sees first, or one GitHub shows only later, counts as Gathr's. `gathr.change.landed` carries the same.
  Deleting an archived chat removes its unlanded rows and keeps the landed ones.

## Why

A PR far bigger than anything the project has landed, and bigger than the ask, is where an unasked change
costs most, and today it lands like any other. The line is the project's own: it moves with what the project
lands, so a project that lands big changes isn't asked about each one. The question comes at landing, when the
work is built and reviewed and there is nothing left to hold. It asks the person once, with the numbers and the
task it answers, and doesn't land the change until they answer. In this repository's 180 merged PRs the rule would
have asked twice, both among the first 25 merges. The record holds what git can't give: who built and who
reviewed, on what, and how much was unasked, so the room can be measured against a single agent.

## Turned down

- **The turn-end stop.** Stopping an agent's turn that leaves a change past the line moves to Stage 3, with the
  rest of the checks at a turn's end.
- **A line fixed at 1,000 lines,** and the second largest of the last 50. The owner chose the largest, never under
  3,000.
- **Asking again whenever a cancelled question's PR is seen.** It would be filed again at every look. A cancelled
  question rests the PR at that head.
- **A setting for the line.** It comes from what the project lands.

## Left out, on the owner's word

Two parts of the rules as first approved are not built, and the owner agreed on 2026-11-14 ("Leave them out"):
the stop asks at landing, when the work is built and reviewed, and doesn't need them.

1. **Measuring a PR when it appears and at each new head.** It would add one `git fetch` for each new head in
   the 2-minute look, a map of failures so one isn't recorded again and again, and an event
   (`gathr.change.unmeasured`). It would give: a question asked as soon as a PR goes over, not once its review is
   in, and a record row for a PR merged by hand before any landing.
   Without it, a question about a head the builder has since made smaller stays open until a review reaches the
   new head, and a PR merged by hand before any landing gets no record row.
2. **Holding turns on the PR's tasks while it is asked about or sent back.** It would keep an in-memory map of
   held tasks in `Chats`, read again from eight places (an answer, a task given or finished, a take-over, a
   verdict, a merge or close), and new waiting words for a held turn. It would give: no turn on the PR's work
   while the person decides, and none after a send-back until the giver restarts the work.
   Without it, work on the PR goes on while its question is open, and a send-back blocks the work task and tells
   its giver, but doesn't hold a turn that was already queued.

## Different from the approved rules

1. `changes` has a `removed` column. The question says how many lines the PR removes, and "Let it go on" asks
   again from the stored count.
2. A question the person cancelled rests the PR at that head, and it is asked again only at a new head.
3. A question is withdrawn when the PR is measured and is no longer over, so the person isn't asked about lines
   that are gone. `stand` therefore measures before it rests on a question.
4. A sent-back or cancelled PR's waiting line moves others, so quiet doesn't wake the lead for it.
5. `git check-attr` takes the paths as arguments, in batches of up to 20,000 characters, because Gathr's own
   process runner gives a command no stdin.
6. `push_merge` marks the row (`changes::pushed`) right after its push, so a look that sees the merge before
   `settle` doesn't record it as by hand. A failed write of that mark fails the merge like its other steps.
7. `settle` records the merge as Gathr's whether or not its own write succeeded. A failure of the record's own
   write is `gathr.change.ended` with an error.
8. A diff that fails at landing takes the path it took when the sign-off read it: `gathr.land.look` with an
   error, and no row.
9. The words follow the facts: the line is said to be the largest of the last 50, or the least it can be.
   Words for an agent say "the person"; words for the person say "you".

## Changes

- [The scenarios](../scenarios.md): 10h is new.
- [The truth map](../truth-map.md): part 10.7 is new, and 10.6 no longer says the stop is the next change.
