# 0075: A team chat is main messages, each with its thread

Status: accepted (owner, 2026-11-18: the threads mock-up's `#roots` A and `#result` A, and the build plan's two questions answered: Gathr records which ask a turn about the whole room serves, and a chat's messages from before it became a team stay flat). Date: 2026-11-18.

Built: partly (the service: what every turn, task and line said in no turn is about; the window draws it next). Proof: `crates/gathr/tests/topics.rs`, `crates/gathr/src/topics/tests.rs`, `crates/gathr/tests/questions_flow.rs`, `crates/gathr/tests/messages.rs`, `crates/gathr/tests/likes.rs`, `crates/gathr/tests/pauses.rs`, `crates/gathr/tests/coordination.rs`, `crates/gathr/tests/quiet_room.rs`, `crates/gathr/tests/stale.rs`, `crates/gathr/tests/merge/lands.rs`, `crates/gathr/tests/merge/review_questions.rs`.

Amends [0051](0051-the-giver-hears-when-all-is-in.md) (the hear-back that closes an ask is its result) and [0028](0028-team-chats.md) (each turn says what it is about). Serves promise 6 of [the charter](../charter.md#the-promises), part 6.5 of [the truth map](../truth-map.md): the room explains itself, its talk read as threads.

## The choice

- **A main message is something put to someone that starts a piece of work or needs the person:** the person's
  own message; each task the lead hands out (work with an owner, given by the chat's Supervisor or by the person);
  a question to the person; each version of a plan; the result of an ask; and anything nothing ties, as its own.
  Everything else is a reply in the thread of the main message it serves.
- **The service works out what everything is about, in one place** (`crates/gathr/src/topics.rs`). It reads a
  chat's turns, tasks, plan versions, asks, landings and lines once, one query a table by the chat's own indexes,
  and follows the links in memory: never a query for each row (0073's lesson). Each answer is a key: `t:<task>`
  (a task or question), `m:<message>` (the person's message, a plan version, a main line of Gathr's), `r:<turn>`
  (a result) or `b:<turn>` (a turn tied to nothing).
- **A task's topic,** from its own row, in order: a question is its own; a plan check sits with the version it
  checks; a task with a parent with its parent (Gathr's review and its fix after a review); a review given by
  hand with the task its PR was built in; the lead's or the person's work is its own; anything else with the turn
  that gave it; given in no turn, its own.
- **A turn's topic,** in order: the result of an ask is its own; a turn the person's message opened sits under
  that message; one that heard the person's answer or Discuss under that question; one Gathr queued for a PR it
  can't land with the PR's task; one that carries a copy (a teammate's message, a like) where that message sits;
  then its task's topic; then the turn it answers; then the ask it serves; a compaction where its agent's turn
  before it was; else its own.
- **A line said in no turn** sits with what it is about: the task of a done line or of a merge that didn't update
  the folder, the ask of a size change's note or of "Fixed", the version of a check's or decision's line, the
  ask or task of a caught change. "Nothing moves" needs the person: a main line of its own.
- **Three facts are recorded where they happen,** as nothing kept them before. Migration 0055 adds them by `ADD
  COLUMN`: `turns.result_of`, the ask whose closing hear-back the turn is (0051, below); `landings.turn`, the
  reviewer's turn Gathr queued for a PR it can't land; `messages.about`, what a line in no turn is about
  (`t:<task>`, `m:<message>`, or `main`).
- **The lead's turn on the room as a whole serves the ask of what it names** when that is one ask: a quiet room's
  wake, a member's silent turn told, stale tasks told. Its `ask_id` is set, so it sits in that ask's thread;
  naming work of several asks, or of none, it stays its own.
- **An ask says whether it is open** (`open` with each ask), from the same rule that already decides it.
- **The answer is in history** (`topic` on each turn and on each line said in no turn), **the tasks list** (`topic`
  on each task) and **the live `turn` event**. `gathr diag threads --chat <id>` lists each turn, task and line, its
  topic and the fact that tied it, or why nothing did.
- **The fallback is safe.** Anything nothing ties is a main message of its own. A link that leads nowhere (a turn or
  task that is gone), a chain that loops, or one longer than 32 links ends there, tied to nothing, and history
  records it as an error event on its request's reference.
- **Old chats are read the same way,** with no migration of old rows: by `task` and `answers` where they are set,
  and by the person's message that opened a turn. An old chat has no results.

Settled while building, each the smaller choice:

- **A check's line, a decision's line and "Fixed" are already recorded** (`plan_versions.check_line`,
  `decided_line`, `asks.fixed`), so their writers set no `about`; the rule reads those columns.
- **A review given by hand ties by its PR** to the task the PR was built in, where the plan had it sit with the turn
  that gave it: the PR's thread is where its work, Gathr's own review and the landing are.
- **A turn that heard the answers to two questions** sits under the lowest-numbered one.
- **When a chat became a team** for a window that keeps a one-agent chat flat is what the members' `added_ts`
  already says; nothing new is recorded for it.

## Why

The chat read every message in one line, and the window guessed what a block was about from what it could see:
tested only on states written by hand, and missing three facts. The service holds the facts, so it is the one
place to decide, and the tests are on real sequences of work through its API, where a wrong idea of what is
recorded would show. One answer serves the chat, the Tasks views and `gathr diag`.

## Turned down

- **The window working topics out from history,** as it placed threads, reviews and questions before: it lacks
  what opened a turn, its ask and the landings, and it was tested only on hand-made states.
- **A `topic` column on turns or messages.** It changes nothing a read can't work out, and old rows would need a
  second path.
- **The result as the last reply under the ask** (`#result` B): it moves as replies come, and the person looks for
  the outcome at the bottom.
- **Only the person's messages as main messages** (`#roots` B): a task the lead hands out and a question waiting
  on the person would hide inside threads.
- **The service naming the topic of every message as it streams:** the split where the person's message enters a
  running turn is the window's, and the turn worker would keep a second copy of the rule.

## Not in it

- How a thread is drawn, folded and marked handled (the next PR), replying in a thread, and where the person left
  off.
- One topic per turn: a lead's turn that answers the person, hands out tasks and messages an agent sits in one
  thread. Only the person's message entering a turn splits it.
