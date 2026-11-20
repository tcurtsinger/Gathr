# 0073: Gathr speaks as a speaker

Status: accepted (owner, 2026-11-17: the UI pass's chat-lines mock-up, "Gathr's own voice" option A, "As a speaker", with its groups "Gathr's question" and "Answering in place"). Date: 2026-11-18.

Built: yes. Proof: `ui/src/chat/Asked.test.tsx` (a question answered in the chat sends what the Tasks view sends, from one block and one draft; one question is answered once though the chat and the Tasks view both offer it; after the answer the person's message shows once, below the question, which reads as answered), `ui/src/tasks/usePersonTasks.test.tsx` (a second Send of a question while its first is out sends nothing), `ui/src/chat/questions.test.ts` (an agent's question sits in its bubble and the answer is the person's message where it was sent, also when both were saved in the same millisecond; Gathr's question is Gathr's own message; each Discuss is the person's message; a request the answer settled reads no "completed" line; what the lead is told after a size change isn't drawn), `ui/src/chat/messages.test.ts` (Gathr's notes are its own messages, each with its time), `ui/src/plans/plans.test.ts` (a check that ran late is a sentence of Gathr's), `crates/gathr/tests/tasks/hear_backs.rs` (a line said in no turn reads where it was said, and after two turns that end together), `crates/gathr/tests/person_tasks.rs` (an answer and a Discuss keep when they were sent), `crates/gathr/src/store.rs` (the history order grows with the chat, not with its lines said in no turn), `crates/gathr/tests/hold/wait.rs` (the person reads what a size change held or released in a line of Gathr's own). How Gathr's message looks is proved by pictures.

Amends [0037](0037-one-timeline.md) and [0072](0072-the-chat-reads-like-a-team-chat.md): what nobody said is Gathr's message, not a one-line event. Amends [0051](0051-the-giver-hears-when-all-is-in.md): where a line said in no turn reads after a reload. Amends [0065](0065-questions-in-the-leads-words.md): F9's lines in the chat ("Mira asked you", "You answered Mira", "You to Mira") give way to the question in its asker's bubble and the person's own messages. Serves promises 4 and 6 of [the charter](../charter.md#the-promises), parts 4.2 and 4.4 of [the truth map](../truth-map.md).

## The choice

- **What nobody said is Gathr's message,** in the same layout as everyone's: Gathr's face (three dots, never an initial), its name and the time, and its words in an outlined bubble, a step quieter than an agent's (no fill, the words at 88% and 14.5px). Gathr is never an agent: it has no panel, no state mark and no task, and is never "he" or "she".
  - **Fixed:** the ask quoted, then "Fixed." and its tasks, quietly.
  - **A caught change:** its line, then each file.
  - **A check that ran late:** "Plan v1 came to you before Rev finished checking it."
  - **Gathr's notes** (a wait's result, a wake-up, a stale task): its words whole, never cut.
  - **A size change the person made:** when it held or released tasks, Gathr says so in a line of its own, "Tag picker waits for the plan." or "Tag picker is back in line.". The words the lead is told for it are its own and aren't drawn.
- **Gathr's question** is its message, with the question's why as its words and the one question block in its bubble. A pick answers it at once (0065).
- **An agent's question** sits in the bubble of the block that asked it, answered there with the same block as the Tasks view, with the same draft: a pick, the notes or Discuss started in one place is there in the other. While one Send of a question is out, another sends nothing.
- **The person's answer, and each Discuss, is their own message,** at the time they sent it, below the question. Once answered the question reads as answered: its words alone, no choices, no Waiting. Nothing else says the answer: no "You answered" line at the lead's later turn, and no "completed" line for a member's request the answer settled.
- **A line said in no turn reads where it was said,** on a reload as live: after every turn that had said something by then, by time among the latest-started one's messages, and first when no turn had.

## Service

- The person's answer and each Discuss keep when they were sent: `at`, in microseconds, in the task's `answer` and in each `discussed` entry. No migration: it is in the JSON the answer already keeps. An answer sent before this has no time: the window places it at the turn that heard it, or right under its question.
- A line said in no turn takes the place of the latest-started turn that had a message by then (it was the turn of whoever spoke last), and within a place messages are in time order. It is one window over the chat's messages in time order, so the cost grows with the chat, not with its lines in no turn times the chat (a subquery for each line read every earlier message: 225 ms for the query on a chat of 10,000 messages with 200 such lines, against 17 ms now). No migration: the order is read from what is stored.
- The person's change of size says what it held or released in a line of Gathr's own, in no turn (`plan_wait::noted`).
- Nothing new for a request the person's answer settled: the service never said a done line for it; the window drew "Mira completed" from the member's hear-back, and no longer does.

## Why

- Gathr's lines were loose one-line events between messages, cut with "…" and with an icon each. As a speaker with one shape for everyone, what Gathr says is read like any message, and Gathr's question has a bubble to be answered in.
- A question answered where it was asked, with the block the Tasks view uses, needs no trip to Tasks and no second form to keep in step.
- An answer drawn at the lead's later turn read below what it caused (the member's hear-back came first) and, for Gathr's own question, which no agent hears, not at all.
- On a reload a line sat under whoever spoke last before it, so a check's line could sit above its checker's reply.

## What was turned down

- **Option B, a quiet centred note** for Gathr's lines: still a loose line between messages, and Gathr's question needs a bubble.
- **The answer right under the question whenever it was sent:** it would read above what came between, out of time.
- **A second question component for the chat:** one block, as the Tasks view's.
- **A column for the answer's time:** the answer's JSON holds it.

## Said plainly

- **Lines that belong to later changes keep today's line inside Gathr's bubble:** a task heard back or done ("Ann completed: …"), and a plan's findings, Approved, Sent back and Dropped. An ask's size is no line: it sits on the person's own message ([0061](0061-every-ask-has-a-size.md)).
- **Gathr's notes keep their words,** which were written for the agent ("Your wait on CI … ended"); only a size change's are new.
- **A question's files** are opened from the Tasks view, not from the chat.
- **The agent's panel** shows an agent's earlier blocks without their questions.
- **An answer and what it started are saved in the same millisecond,** and a message's time is to the millisecond: the window places the answer before anything saved in that millisecond.
- **Not run:** macOS and WebKit, and a real engine (the proof answers with no engine signed in, so the lead's turn waits for sign-in).

## Amended by [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md)

- The person's answer and each Discuss sit in the question's thread, under the block that asked it, whenever they
  were sent, with the lead's turns that heard them. This reverses what was turned down here: the answer right under
  the question whenever it was sent. The question keeps its Discuss switch in the chat until Reply replaces it.

## Amended by [0078](0078-said-to-you-is-main-one-bubble-per-turn.md)

- Gathr's words that open a turn and draw something (a wait's result, a wake-up, a silent turn or stale tasks told, a
  carry-on, a landing hand-off, Sent back) are the quote at the top of that turn's bubble, up to three lines; Gathr
  draws no message for them until the turn has no bubble to quote them in.
- 2026-11-19: a plan's findings are no longer Gathr's line: the chip at the end of the checker's bubble says them
  ("3 findings") and opens to each note. A check that didn't finish or ran late keeps Gathr's line.
