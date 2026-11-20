# 0064: A change far past its Fix or its part stops and asks

Status: accepted (owner, 2026-11-14, with Stage 3's spec, part J; counted when the task says it is done, owner, 2026-11-16, "When it says done"; its detail and its departures, 2026-11-16, "Approve J"; the answer on a new message sent while a Fix is open, "Keep it"). Date: 2026-11-16.

Built: yes. Proof: `crates/gathr/tests/merge/report_count.rs`, `crates/gathr/tests/tasks/lines.rs`, `ui/src/chat/messages.test.ts`.

Builds on [0054](0054-a-change-past-the-far-end-waits-for-the-person.md), [0060](0060-gathr-gives-the-review-task.md) and [0061](0061-every-ask-has-a-size.md). Amends each, below. Serves promise 10 of [the charter](../charter.md#the-promises), parts 10.3 and 10.7 of [the truth map](../truth-map.md). Stage 3's part J.

In the window and in everything Gathr says to the person the sizes are **Fix** and **Plan**; in the code, events and tools they stay `repair` and `new`. A **building task** is a task of kind `work` that isn't a look-into task. **J's question** is Gathr's question to the person filed when a task reports done: 0054's question, given in a turn. "The line" is the count past which a change is asked about.

## The choice

When a building task tells Gathr it is done and names its pull request, Gathr counts the lines that pull request adds, the way it counts one before landing it, before the review is given.

- **J1. When Gathr counts.** In the `finish_task` handler, before its database job, and only for a done report that first names the PR (or reports it again after a send-back), from the caller's own building task, with no review given for the PR yet, in a project's chat, for a task that has a line. Gathr counts in the project's folder under the repository's lock, as landing does, within 20 seconds. A count that fails (GitHub or git doesn't answer, the head isn't on origin) or takes longer is recorded, and the report goes on as it did: the review is given, and landing counts against the same line.
- **J2. A Fix's line.** A task on a repair ask has the most lines any of the project's last 50 landed Fixes added, never under **1,000** (1,000 until 10 have landed). It is measured from lines, never from the lead's word: a repair task's own size is never its line. Each landed change records the size of its ask (`changes.size`) so this can be read.
- **J3. A part's line.** A task on a new or unsized ask that has a size in lines has `max(3 × size, size + 300)`: asked past three times its size and more than 300 over it. A task with no ask has no line, and a task on a new or unsized ask with no size has none. Landing still measures its PR against the project's line.
- **J4. Past the line, one question, and the task waits.** Gathr asks the person once with 0054's question, given in the turn the review would be given in, so the task waits on it with the hold every task already has on what its turns gave. The question is written in the report's own transaction, before the task settles. No review is given until the person answers. Meanwhile what the chat waits on shows the question and the task's wait on it, and no line saying the PR waits to merge with no review (`tests/tasks/gathr_reviews.rs`). The title names its line: `#7 adds 1,240 lines, over a Fix's line of 1,000: let it go on?` or `#7 adds 1,240 lines, over its part's line of 900: let it go on?`. The head the person sent back, or one whose question they cancelled, rests: the report blocks the task with why, and its giver hears.
- **J5. The answers.** "Let it go on" lets that count through, so neither a report nor landing asks again about it, settles the task as `done, waiting on #7 to merge`, and gives the review. "Send it back" blocks the task and tells its giver, as 0054 does, and the PR never merges at that head. The person cancelling the question rests the PR at that head and blocks the task too. A task cancelled while its question is open withdraws it; a question ended by landing or a merge releases its task.
- **J6. Landing takes the lower line.** Landing and the merge tool measure against the lower of the project's line and the PR's task's line, so a PR that grew after its report, one whose count failed, and a review's fix are caught at merging. What was let through at the report holds there.
- **J7. In the window.** The chat's line for the question is its own: `Gathr asked you: …`, from the shared asker. No new kind, icon or colour.
- **J8. Diagnosable.** `gathr.change.report` (new, on the report's trace, with its errors), and 0054's events with `line_of` and `gathr.turn.id`. A failed count is found from its reference.

A new message sent while a Fix is open still joins it, as before (the owner's answer, "Keep it").

## Why

- A change counted when its work says it is done saves the review and the merge of an oversized change. It can't save the building, and the owner chose the time with that said: in the room's record about two tasks in three finish in one turn.
- The pull request is counted, not the agent's worktree: the count is landing's own, so a report and landing agree, a rebase changes nothing, and nothing new is stored but two columns.
- The question is 0054's: one question, one answer, one place a count is let through. J adds where it is asked and which line it reads.
- The task waits with no new state, table or map: a question given in a turn of the task is already held on.
- A Fix's floor stays 1,000: the repository's own history has no repair past it ([the detail's reading](#said-plainly)).

## Different from the spec

- **Counted when the task reports done, before its review, not at each turn's end** (the owner's decision). Growth over several turns isn't stopped until the report.
- **The pull request is counted, not the agent's worktree from a commit kept at its first turn.** No stored commit, no new untracked files, no counting of one agent's open tasks together, no new table.
- **The task waits and no review is given until the person answers.** The spec says the stop asks.
- **Landing takes the lower of the project's line and the task's.** The spec left 0054 as it is.
- **The chat line is the question's own, `Gathr asked you: …`,** not "Stopped at 640 lines: #41": "stopped" already means a stopped turn in the chat.
- **The part's title names its line,** `over its part's line of 900`, not its size.
- **A cancelled J question rests the PR at that head, as 0054's does, and also blocks the task and tells its giver.**
- **A cancelled work task withdraws its open J question.**
- **A task with a size on a new or unsized ask is held to it, a plan's part or not.** A repair task's own size is never its line.
- **A chat with no project isn't counted.**
- **A Fix's floor is the spec's 1,000, and the part's numbers stay 3× and 300** (the detail's reading of the repository's history, below).

## As built

What differs from the detail, from the code as it landed:

- **A rested head blocks through the report itself** (`crates/gathr/src/changes/report.rs`, `reported`): the report is turned into a `blocked` one with the words `#5 was sent back by the person at <short>. Push a smaller change, then report #1 done.`, so the task is blocked when it settles, at once when no turn runs or when the running turn ends (0047), and its giver hears through settle's cascade, not `tasks::followed` at report time. Setting the state during a running turn would have left the task blocked after a later done report in the same turn. The tool's answer shows the block's words either way.
- **What the handler passes is `Counted`,** not `Count`: `NotCounted` (the default), `Failed` or `Measured(Measure, ms)`. It carries the count's milliseconds, so `duration_ms` is written inside the report's transaction with the outcome. A failed count is recorded by the handler (`crates/gathr/src/bin/gathrd/changes.rs`, `failed`).
- **`counted` takes an optional kept count:** landing passes the stored count when the head and line match, so it keeps its existing shortcut for a count it already made, and the report passes none. Landing reads the project's line before it reads the diff, to know the line before it chooses the stored count.
- **The task is found once, by the same order as before** (the one named, else the turn's, else the caller's one open task) in `reported` in `crates/gathr/src/bin/gathrd/tools/coord.rs`, and passed to `to_count`. `review::covered` takes a connection for it, as `to_count` runs outside a transaction.
- **The words follow `line_of` where the detail named none:** a withdrawn question reads `#5 is now +500, under a Fix's line of 1,000` or `under its part's line of 400` (a project's keeps 0054's words), and a part's question for work with no size in lines says `Its part's line is N.`. That case isn't reached through a report, which counts only a task with a line.
- **The tool's answer line `Gathr asked the person about #5's size (#N). Its review waits for the answer.`** is read after the report, from the PR's standing, when the report was counted and no review was given, not returned from `finish`.
- **A task with no ask has no line,** even with lines in its brief.
- **A part's `size_lines` on a repair ask is never its line.**
- **The window's change is one line,** not two: the shared asker replaces the name in `questions` (`ui/src/chat/messages.ts`), and keeps `An agent` for an agent the crew can't name.
- **Size.** About 690 lines of service code (the detail said 300), 690 of tests (560) and 20 of window code (5): about 1,400 in all, inside the 1,200 to 1,550 the detail gave after its factor.
- **The first count on a test repository** took 990 ms against the local stand-in for GitHub (`gathr.change.report`'s `duration_ms`).

The tests as they are: `crates/gathr/tests/tasks/lines.rs` (the Fix line, the recorded size, a task's line), `crates/gathr/tests/merge/report_count.rs` (eight tests in a held room: the question and the wait, a send-back and its smaller head, "Let it go on" and landing, under the line and no line, a failed count found from its reference, a count past its time, a cancelled question or task, landing against the task's line), and `ui/src/chat/messages.test.ts` (the question's asker). The first test of the held room also checks a second task naming the PR while the question is open: no second question, no review, and no hold of that task. The room is a doubled replay recording with a gate for each turn, so a test ends exactly one agent's turn, and `GATHR_REPORT_COUNT_MS` sets the bound in a test.

## Said plainly

In the owner's words, what this does not stop:

- **Growth over several turns before a report.** A task that takes several turns keeps growing until it reports done. In the first version's reading of the room's record (not checked again), 54 of 79 done work tasks reported done in their first turn, so about a third took more.
- **Work whose pull request stays under its line,** new work that rode in on a Fix included. A new message sent while a Fix is open joins it, and Gathr asks only once the Fix's pull request passes its line.
- **A task with no line:** a new or unsized ask's task given with no size, a task with no ask, a task in a chat with no project. Landing still measures its PR against the project's line.
- **A task that never reports done:** its PR isn't on the task, so only landing's project line applies.
- **Work not pushed** isn't in the PR, so it isn't counted.
- **The building is spent.** The stop saves the review and the merge, not the agent's turns that wrote it.
- **One Fix let through raises the Fix line.** One repair the person lets through at 2,000 lines and lands raises the project's Fix line to 2,000 for its next 50 repairs, as 0054 chose.
- **A moved or split file counts whole,** as 0054 counts (`--no-renames`). At 1,000, splitting a file near the 1,000-line limit is just under the floor.
- **A pull request carrying several tasks** is counted once, against the first task's line.
- **A failed count** (GitHub down, over 20 seconds) lets the review go on; landing then counts.
- **The floor.** This repository has 205 merged pull requests (at the start of part J). By hand, from each one's title and branch, 49 were plainly repairs, and none added more than 679 lines; 1,000 stops no repair in this history, and 700 would have stopped a 975-line one if it is a repair. Not sure for any single small pull request.
- **The part's numbers.** 3× and 300 are the spec's. The room's database can't settle them (no sized task yet). Against the detail's sizes, nothing in Stage 3 landed past 1.8 times its size; against the rough sizes, five of nine parts would have been asked about, each of which the owner in fact let go on. Once plans have run, `tasks.size_lines` and `changes.counted` are both on record: if more than one in ten parts are asked about and let go on, the factor is too low.
- **Not verified:** that dropping the count's future at 20 seconds ends a `gh` or `git` it started (read in `gathr::run`: dropping a running program kills it; no test sees the kill, and the test of the bound times out before `gh` starts); whether either engine limits an MCP tool call to under 20 seconds; GitHub's real time for `gh pr view` and `git fetch` (only the stand-in was timed); the history's repair classification pull request by pull request; whether 1,000 fits other projects; whether 3× and 300 fit Gathr's own plan parts. The paths that release a work task when its question is withdrawn or ended by landing, and a resize of the ask while asked beyond the case in the tests, are built and have no test of their own.
- **The counts, from the record:** `gathr diag query "SELECT attrs ->> '$.line_of' AS line, attrs ->> '$.outcome' AS outcome, count(*) AS n FROM event_log WHERE name = 'gathr.change.report' GROUP BY 1, 2" --data <folder>`.

## Turned down

- **Counting at every turn's end** (the owner's decision): it saved neither the building nor the review for two tasks in three, and needed a table, git at the end of turns and a stored base.
- **A floor of 700:** it would have stopped a repair this repository merged.
- **A line from a repair's own size:** it would be the lead's word, not a measure.
- **Giving the review and leaving it waiting:** it would block and wake its builder at a send-back.
- **A new chat line kind "Stopped at …".**
- **Leaving let-through repairs out of the Fix line:** 0054's largest is kept.
- **A setting for any line.**
- **A third choice on the question,** such as "Make it new work".
- **A rule that a Fix takes new messages only until its task says it is done** (the owner's answer, "Keep it").

## Amends

- [0054](0054-a-change-past-the-far-end-waits-for-the-person.md): its question is also asked when a building task reports done, in a turn of that task, against the task's line; landing and the merge tool take the lower of the project's line and the task's; `changes` gains `size` and `line_of`; and a cancelled J question blocks its task.
- [0060](0060-gathr-gives-the-review-task.md): the review is not given while Gathr's question about the PR is open, and is given when the person lets it go on.
- [0061](0061-every-ask-has-a-size.md): a landed change records the size of its ask.
