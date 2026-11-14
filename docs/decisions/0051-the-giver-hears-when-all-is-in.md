# 0051: The giver hears when all is in, or at once on trouble

Status: accepted (owner, 2026-11-13, with Stage 2's spec: "All in, or trouble"). Date: 2026-11-13.

Built: yes. Proof: `crates/gathr/tests/tasks/hear_backs.rs`, `crates/gathr/tests/tasks.rs`, `crates/gathr/tests/coordination.rs`, `ui/src/chat/plan.test.ts`, `ui/src/chat/messages.test.ts`.

Amends [0047](0047-a-report-is-a-record.md)'s rule for hear-backs. Serves promise 5 of [the charter](../charter.md#the-promises), part 5.7 of [the truth map](../truth-map.md), and the charter's rule that a turn buys something: work, a check or a decision.

## The choice

What a hear-back is stays as 0047 left it: a turn Gathr starts for whoever gave tasks, saying what became of the ones it hasn't heard about. When it goes changes.

- **Trouble is heard at once, by itself.** A task that ends blocked, failed or stopped wakes its giver while the
  others still run. A task its owner left unfinished and one that ended with no report are among these: Gathr
  holds both as blocked.
- **The rest is heard when all of it is in.** Done and cancelled tasks are heard once everything given from the
  same place is settled. The same place is one of two things:
  - **A task of the giver's own.** Everything given in that task's turns is one group, and the hear-back is a
    turn on that task.
  - **No task, for a lead that coordinates: the round.** A round is the turn the person's word began (their
    message, or their answer to a question) and every turn that answers it, directly or through another. One
    hear-back comes for the round, answering the newest turn that gave some of it. Work from the person's next
    message is another round and holds nothing of an older one back.
- **No turn when there is nothing to do.** When the tasks that are in all ended done, and the giver's own task
  is held with its owner's word that it is done, no turn starts. The tasks count as heard, and the giver's task
  is looked at again as when any hold ends: its reason names what is left, or it is done. This is a builder
  whose review came back clean.
- **A task that ends done is a line in the chat when it ends,** said by Gathr in no turn: "Cass completed: …".
  The person reads it and nobody is woken. The window shows each end once: a hear-back that comes later doesn't
  repeat it.
- **Work held only on its merge is no longer told to its giver.** Gathr lands it
  ([0049](0049-gathr-lands-pull-requests.md)), and its giver hears when it has landed, or when its PR closed
  unmerged or it was cancelled.
- **The service says which round a task is in** (`round` on each task in a chat's list), and the window's plan
  reads that. The window used to work it out from the turns by its own rule.

Unchanged: the person's answers reach whoever asked at once; a task given in passing is heard by itself when
it settles; a task its giver restarted, or one that settles as something other than was heard, is heard again.

Settled while building, each the smaller choice:

- **One case of "held only on its merge" still wakes its giver:** a fix given by the reviewer of the same PR.
  The merge waits on that review, and the review waits to hear of the fix, so the reviewer hears when the fix
  is reported done (found in #100's own review; the test for it stood and still does). Stage 2's spec said the
  whole rule goes.
- **A round also begins at the person's answer to a question,** as the window already drew it (G158). The spec
  named only their message.
- **A hear-back goes once every turn that gave some of its group has ended,** as before for the one giving
  turn: its giver may still be handing out work.
- **No schema change.** A round is read from the turns' links when it is asked for.
- **The line is a message with role `gathr` and no turn,** in the words a hear-back uses for a task, so the
  window reads it with the code it had.
- **A line reads where it was said.** A chat reads in the order its turns began, and a line has no turn, so
  it reads after the block of whoever spoke last before it. Found in real use: on a reload every line sat
  at the foot of the chat.
- **A seeded team chat says these lines too** (`gathr seed`), so the window can be looked at with them.

## Why

In the room's first three days in Gathr, the lead had 78 turns opened by a hear-back, and 30 of them called no
tool: there was nothing to do. Turns Gathr started on no task took about 21% of the room's tokens. Meanwhile
trouble waited: a blocked task was told to its giver only when the last task of its group had settled.

A done task rarely needs its giver before the rest is in, and the person can see it end in the chat. A blocked
one needs its giver now.

## Turned down

- **A hear-back for every task as it ends.** That is the cost this decision removes.
- **Keeping a group as one giving turn.** A lead that gives work in three turns of one round then gets three
  hear-backs for one piece of coordinating.
- **A `round` column on tasks.** It would be written once and never change, but a migration is not needed to
  follow links that are already kept.
- **A new kind of message for the line.** The window, search and the API would each need to learn it.

## Not in it

- A reviewer still gets a turn when a fix it gave lands, though that tells it nothing new.
- Whether leads give follow-on work with `after`, so that it starts without waiting for the round's hear-back,
  is for the first days of real use.
