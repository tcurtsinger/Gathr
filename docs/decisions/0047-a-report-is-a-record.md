# 0047: A task's report is a record, and Gathr works out where the task stands

Status: accepted (owner, 2026-11-13). Date: 2026-11-13.

Built: yes. Proof: `crates/gathr/tests/tasks/reports.rs`, `crates/gathr/tests/tasks/commands.rs`, `crates/gathr/tests/tasks/hear_backs.rs`, `crates/gathr/tests/coordination.rs`, `crates/gathr/tests/update_room.rs`, `ui/src/tasks/TaskReport.test.tsx`.

Amends [0010](0010-coordination-as-state.md)'s task rules. Serves promise 5 of [the charter](../charter.md#the-promises): work tells the truth.

## The choice

- **A report is kept.** `finish_task` records what a task's owner says of it: done, left or blocked, with a note,
  and for done what to look at (a PR, a commit, a file or a note). The report stands until the owner reports again
  or the giver or the person restarts the task. A "left" is cleared when a turn next starts on the task: its owner
  is at work on what was left.
- **A report is taken in any state,** from any turn of the owner's. While a turn runs on the task it applies when
  that turn ends; otherwise at once. A task that is done answers "Already recorded", and a cancelled one says so;
  neither is an error. What is turned away says too little or isn't the caller's: a done review with no commit,
  done work with nothing to look at, a left with no note, a blocked with neither a note nor a task.
- **Where the task stands is worked out,** when a turn on it ends well and when a report arrives outside one:
  blocked as reported; held while something Gathr can see is open, saying on what and what was reported; with
  nothing open, done if reported so, and stopped there if reported left, for its giver to hear.
- **A turn that ends is never a task that is done.** With nothing open and no report, Gathr starts one short turn
  asking for it. If that ends silent too, the task is blocked with "no report" and its giver hears.
- **A hold that ends** with no turn of its own (a PR merging, a background command ending during another turn)
  makes the task done only if its owner reported it done. Otherwise its owner is woken on it and told what ended,
  and that wake is the one ask. A PR that merges never makes "left" work done, and unreported work whose turn
  still runs when its PR merges is left to that turn's end. A review is done when its PR merges: nothing is left
  to review.
- **"Left" and "no report" are not states.** Gathr holds such a task as `blocked`, and the report says which.
- **A turn that is stopped with its carry-on waiting doesn't settle its task.** After Stop now, or a usage limit
  that cuts a turn once a tool has run, the task stays as it was and says why nothing runs on it. Its giver hears
  how the carry-on ends, not that it stopped. An update already worked this way ([0032](0032-service-updates.md)).
- **A hear-back says what is new.** It names only the tasks its giver hasn't heard about. A task that settles as
  something other than what its giver last heard is heard once more, by itself; the same outcome reached again
  is not. A task given `in_passing` doesn't hold the task whose turn gave it, and is heard back on by itself.
- **A member's request the person has answered can't be cancelled out from under the work that waits on it:**
  it settles as done, with the answer.
- **A task blocked for the one it comes after** records which one (`blocked_on`), and goes back in line when that
  one is restarted and finishes.

## Why

In the room's first three days in Gathr (2026-11-10 to 11-12), 14 turns closed a task as done with no word from
its owner, 33 truthful reports were refused for the state the task was in (held, done, blocked, ready), and work
reported as left came back to its giver as done when its PR merged. Each of these traced to one design: the
report lived only until the turn's end, and a turn ending with nothing said was read as done.

Keeping the report is what makes the rest small. With it, a later wake on the task (a wait firing, a hear-back)
doesn't erase "done", a hold ending can tell "done" from silence without reading it out of the reason's first
word, and a merge can tell finished work from left work.

## Turned down

- **A tenth state, `left`.** The states are a CHECK on the tasks table, so a new one means rebuilding that table
  on every installed database. The report already says "left"; the state says nobody is on it.
- **Asking on the owner's next turn** instead of waking it. A task whose owner has no other turn coming would sit
  until someone noticed. The ask would have fired 14 times in the three days.
- **Leaving tasks set after a stopped one waiting,** not blocked. The hear-back and the stale sweep would then
  have to follow whole chains of waiting tasks. Marking them blocked, with which task they wait for, needed no
  change to either.
- **Attempts, revisions and an "applied" acknowledgement** for each report and answer (the first audit's design):
  one kept report does the job.
- **"Left" starting its owner again at once** (0010 as built, from SerenityOS). An agent that can go on doesn't
  need a new turn to do it, and one that can't was started in a loop. SerenityOS's own record has an agent using
  "left" as a progress note in 20 of 35 turns.
