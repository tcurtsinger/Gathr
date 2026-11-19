# 0078: Said to you is main, one bubble per turn

Status: accepted (owner, 2026-11-19: the chat-shape mock-up's structure A, "Said to you is main", and the build
plan's question 1 answered: Gathr's words that open a turn are quoted at the top of that turn's bubble). Date:
2026-11-19.

Built: yes (the rule in the service, one bubble per turn and its quote in the window, an unmarked thread seen up to the main line's mark, a plan check's findings chip in the checker's bubble; agents' own choice is a later step). Proof: `crates/gathr/tests/topics.rs`, `crates/gathr/src/topics/tests.rs`, `crates/gathr/tests/solo.rs`, `crates/gathr/tests/picked.rs`, `crates/gathr/tests/pauses.rs`, `crates/gathr/tests/coordination.rs`, `crates/gathr/tests/likes.rs`, `ui/src/chat/mains.test.tsx`, `ui/src/chat/reads.test.ts`, `ui/src/chat/Like.test.tsx`, `ui/src/chat/findings.test.tsx`.

Amends [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md) (the rule, the keys, a chat of one),
[0037](0037-one-timeline.md) (a plan version no longer splits its turn), [0062](0062-a-plan-is-checked-and-approve-hands-it-out.md) (the plan's chip in
the lead's bubble), [0073](0073-gathr-speaks-as-a-speaker.md) (Gathr's words that open a turn are its bubble's quote)
and [0077](0077-where-you-left-off.md) (a thread with no mark). Serves promise 6 of
[the charter](../charter.md#the-promises), part 6.5 of [the truth map](../truth-map.md): the room explains itself.

## The choice

- **What is said to the person is a main message:** their own message; the answer to it; every report of the lead's
  when it hears back; a turn that writes a plan; each task handed out; and the answer of each agent they picked with
  @. Replies fold under what they serve, as before: the work on a task, its PR, review and fix under the task; a plan's
  check under the lead's bubble that holds the version; a reply the person typed in a thread stays in it; messages
  between agents under what they are about.
- **The service's rule** (`crates/gathr/src/topics.rs`), in order: the result of an ask is its own `r:<turn>`; a turn
  that wrote a plan version is its own `b:<turn>` (a plan waiting is always main); the person's reply in a thread sits
  with what it answers; a turn the person's message opened, or Gathr's copy of it for an agent they picked, is its own
  `b:<turn>`; a turn that heard their answer or Discuss sits under the question; a PR Gathr can't land under its task;
  a teammate's message where the sender's sits; its task; then **the chat's lead on no task of its own** is its own
  `b:<turn>`, whatever opened it but a wake, a silent turn or stale tasks told (they serve their ask, as before), a
  turn the engine began, or a compaction (they follow the turn they continue): the lead's hear-back, a wait it set,
  a like of its message, a carry-on, a send-back, a size change, and an old turn that doesn't say what opened it.
  After that, as before: the turn it answers, the ask it serves, a compaction, else its own.
- **`b:<turn>` means the turn's bubble is a main message of its own**, said to the person or tied to nothing.
  `gathr diag threads` tells them apart by the fact that tied it: `person`, `picked`, `lead`, `plan`, or `nothing`,
  and counts the first four as `said_to_person`.
- **A plan check, and a check's or decision's line, sit with the turn that wrote the version**, by
  `plan_versions.turn_id`, not with the version's message. The send-back's own case goes: its turn is the lead's on no
  task.
- **History sends each turn's `result_of`**, and so does the live `turn` event; `team_since` goes.
- **One bubble per turn** (`ui/src/chat/groups.ts`): a version's message joins its turn's block, and its chip is one
  of the bubble's pieces where the version was written (`fold.ts`, `Replies.tsx` `Pieces`). While it waits the bubble's
  like is the version's (the like approves it), and its thumbs-ups include the checker's.
- **Gathr's words that open a turn and draw something** (a wait's result, a wake-up, a silent turn or stale tasks
  told, a carry-on, a landing hand-off, Sent back with the person's words) are the quote at the top of that turn's
  first bubble, up to three lines (`timeline.ts` `openings`, `Said.tsx` `Bubble`). Gathr draws no bubble for them.
  Until the turn has a bubble that draws, they draw as Gathr's line, as before.
- **The person's message into a running turn** still splits it, and what follows is a main message of its own, keyed
  `b:<turn>:<message>` by the window; after a reply typed in a thread, it stays in that thread. The service keeps
  the marks of that key as its turn's (`crates/gathr/src/reads.rs`; found when using it: they were refused).
- **A question's block is its main message;** when that block is its turn's own main message, the rest of the turn
  sits in the question's thread, which never loses its main.
- **One rule for every chat.** A chat of one reads as one timeline because what its agent says is said to the person;
  a reply to an older answer still makes a thread under it (0076). The flat path goes. A main message right after the
  same speaker's main message isn't named again, until a thread's replies show between them; in a chat of one, the
  agent's main messages keep their tick on the conversation map.
- **A thread with no mark** is seen up to the main line's mark (`reads.ts`): an update names threads anew (the lead's
  answers, a check under the version's turn), and what they held before the person last looked isn't new.
- **A plan check's findings are a chip at the end of the checker's bubble** (2026-11-19, the build plan's PR 2), on
  the last block of the check task's turns (`blocks.ts`, `TurnParts.tsx` `FindingsChip`), in the plan chip's shape with
  a chevron: "7 findings" (wrong and not asked for), "1 finding", or "2 suggestions" for a report of suggestions only.
  A press or Enter opens it in place to each note of the check's newest report, in order, as the Tasks view draws a
  note (`ui/src/tasks/Note.tsx`: its kind, "Wrong", "Not asked for" or "Suggestion", its file in monospace, its
  words); a press closes it. Gathr's findings line ("Plan v1:" "7 findings") is no longer drawn. A check that agrees
  has no chip, only its thumbs-up; a check that didn't finish or ran late keeps Gathr's line. Only a plan check has
  it: a code review's notes stay in the Tasks view and the fix's brief (the plan's question 3, settled so).

## Why

The owner read their own chat and found the lead's long answer behind "2 replies" under their message, and the plan
chip as a message of its own, cut off from the words that explain it. Both came from the person's message being the
topic of the lead's turn and from the version being a block and a main message of its own. Placement is computed when
history is read, never stored, so the new rule needs no migration and every old chat reads by it at once.

## Turned down

- **Keeping the flat path for a chat of one:** under the new rule a chat of one already reads flat; two paths drew the
  same thing two ways.
- **A Gathr bubble for the words that open a turn,** or a Gathr line in the bubble's thread (the plan's question 1, B):
  the owner picked the quote.
- **Recording the place on the turn:** it changes nothing a read can't work out; agents' own choice of place is a
  later step with its own column.

## Not in it

- A code review's notes as a chip in the reviewer's bubble.
- Agents choosing where their bubble goes (`post`).
- The window's split of a turn at the person's message is the window's alone: the service and `gathr diag threads`
  still name the whole turn `b:<turn>`, so a check or reply the service ties to that turn sits under its first part.
- Who led a chat is read from today's members: a replaced lead's old turns fall to `answers`, as before.
- Not verified: the owner's own data before and after (how many more main messages it gets), WebKit inside the real
  macOS window.
