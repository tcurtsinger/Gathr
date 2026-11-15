# 0060: Gathr gives the review task

Status: accepted (owner, 2026-11-14, with Stage 3's spec, part E; the five departures below, 2026-11-15). Date: 2026-11-15.

Built: yes. Proof: `crates/gathr/tests/tasks/gathr_reviews.rs`, `crates/gathr/tests/gathr_review.rs`.

Builds on [0053](0053-a-review-asks-both-questions.md), [0047](0047-a-report-is-a-record.md) and [0049](0049-gathr-lands-pull-requests.md). Amends [0013](0013-one-chat-model.md) and [0043](0043-checks-and-merge.md), below. Serves promise 10 of [the charter](../charter.md#the-promises), part 10.8 of [the truth map](../truth-map.md): a PR's review doesn't depend on someone remembering to ask for it. Stage 3's part E.

## The choice

- **E1: Gathr gives the review.** When a work task's report first names its PR, and the report is `done`, in a chat with a Reviewer who isn't the task's owner, Gathr gives that Reviewer a review task for the PR, in the report's own transaction.
  - **First names.** The task had no PR before this report and the report names one. A task given with a PR (Gathr's own fix, a lead's work on an existing PR, the person's own PR) never first names one in a report. Gathr doesn't read a report's words for a PR, and it keeps no link between a branch's PR and a task.
  - **As the builder would give it.** Giver: the work task's owner. Owner: the Reviewer, the first member with that role. Parent: the work task. `Review #<pr>`, with the PR, no brief fields (a review takes none) and no `why`: the review's brief already carries the work's.
  - **After the work, in passing.** It comes `after` the work task, so it starts when the work is held only on its merge: the builder's turn has ended, and anything else the work waited on has ended. It is in passing, so the work isn't held on it. Without that the two would wait on each other: the work on the review, and the review on the work being held only on its merge. The work is held on its PR anyway, and the PR lands only on a clean review ([0049](0049-gathr-lands-pull-requests.md)).
  - **Once per PR.** None is given when a review in the chat that isn't cancelled covers the PR: one that names it, open or ended, or one with no PR that is the work's child or comes after it. A second look happens on the same review ([0053](0053-a-review-asks-both-questions.md)), and landing goes by the newest review of the PR. A cancelled review covers nothing.
  - **The turn it is given in.** The report's turn when that turn is on the work task; else the work task's newest turn; else the report's turn. Given in a turn on another task, the review's end would wake the builder there, and that task would move to working.
  - **A refusal fails nothing.** If the give is refused, the outcome says so and the report stands, as Gathr's fix does ([0053](0053-a-review-asks-both-questions.md)). A truthful report is never refused ([0047](0047-a-report-is-a-record.md)).
- **E2: a second review of a PR with an open one is refused.** `give_task` with kind `review` and a `pr`, when a review of that PR in the chat is waiting, ready, working or held: `PR #7 already has an open review: #2, Bo's, given by Ann. Give one once #2 has ended or its giver cancels it.` It names the giver because only the giver or the person may cancel a task, and for Gathr's review that is the builder. Not refused: a review of a PR whose reviews have all ended or been cancelled (a second look), a review with no `pr`, and Gathr's own gives.
- **The event.** Each report that first names a PR records `gathr.review.give`, a point event on the report's trace, with `gathr.review.outcome` and `gathr.review.pr`, and `gathr.task.review` (the review given, or the one that covers it) when there is one. The outcomes are `given`, `not_done` (the report was left or blocked), `no_reviewer`, `own_review`, `covered` and `refused: <why>`. A review given also has its own `gathr.task.give`, with the builder as giver, so `gathr diag` tells Gathr's review from an agent's and says why none was given.
- **The answer.** `finish_task`'s answer gains a second line when Gathr gave one: `Gathr gave #2 to Rev to review #7.` So the builder knows not to give one. `finish_task`'s `pr` says that naming it with a done report gives the Reviewer its review. No word that Gathr sends an agent changes, so no version of [0056](0056-gathr-records-which-words-opened-each-turn.md) moves.

No column, no migration, no window code. The task view shows Gathr's review as it shows any review, with "After: #<work>", and the chat's waiting line stops saying the PR "has no review".

## Different from the spec

The owner answered each on 2026-11-15.

- **Which reports give one.** The spec says "first names a PR". Only a `done` report gives a review; `left` and `blocked` record `not_done`. A review after a blocked report would be blocked at once, and its giver would hear on a blocked task. After a left report it would wait on a done nothing asks for, and it would turn the work's waiting line "which has no review", which wakes the lead, into one that doesn't, so the chat would go still with no wake. Any later report no longer first names the PR, so a PR first named in a left report gets none from Gathr and keeps the line that wakes the lead.
- **What counts as covered.** The spec says an open review that names the PR. Any review that isn't cancelled counts, and so does one with no PR that is tied to the work, so a work task that names a PR already reviewed gets no second review.
- **After, and in passing.** The spec doesn't say. See above.
- **Which turn.** The spec says the turn that made the report. It is that turn when it is on the work task, else the work task's newest turn, else the report's turn. A work task that never had a turn of its own is given in the report's turn, and its builder hears there.
- **A second report naming the PR says nothing.** The spec's scenario 3g says it "makes none and says so". It makes none and says nothing more: the first report's answer named the review, and one given by hand is refused naming it.

## Said plainly

- When Gathr lands the PR while the Reviewer's turn still runs, the merge sets the work done and then the review done, so the work isn't held when the review's hear-back runs, and its builder gets one hear-back turn saying the review is done. A review the builder gives itself does the same. When the review's turn ends first, the work is held with its done report, and the builder isn't woken.
- A PR first named in a report that isn't done gets no review from Gathr. Neither does a task that ends with changes and no PR: that review is not built.
- The Reviewer is the chat's first member with that role. A team holds one.

## Turned down

- **A review when GitHub shows a PR for the task's branch.** Gathr records no such link; the report is where the builder says which PR is its work.
- **A review at every report that names a PR.** It would make a second review of one PR, while the second look happens on the first.
- **The lead as giver.** The hear-back and the fix go to the builder, as when the builder gives it, and no lead turn is spent.
- **The review ready at once, without `after`.** It could start while the builder still pushes in the same turn.
- **Not in passing.** With `after`, the work and its review would wait on each other.
- **A review on a `left` report.** It waits on a done nothing asks for, and hides the line that wakes the lead. The other way, a column recording that the PR came from a report so a later `done` gives it, was turned down for its migration and its size.
- **A review for a task given with a PR.** Such a task may be about the person's own PR, and a clean review lands a PR.
- **A column or kind marking Gathr's reviews.** The event says it, and nothing reads the difference.
- **Refusing a review with no `pr` from a turn on a work task with an open review.** It names no PR; Gathr would guess.
- **Saying "#2 already reviews #7" on a second report.** More code for a line nothing needs.
- **A setting to turn it off.**

## Amends

- [0013](0013-one-chat-model.md): its Built line ("Not built: a review task that starts by itself") and the Reviewer row. A review task starts by itself for a PR named in a done report. A task that ends with changes and no PR does not get one.
- [0043](0043-checks-and-merge.md): its Built line and C9. The review task that starts by itself is built by this decision; the complete PR, check, review and next-step task view that C9 also names is not.
