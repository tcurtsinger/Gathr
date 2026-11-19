# 0075: A team chat is main messages, each with its thread

Status: accepted (owner, 2026-11-18: the threads mock-up's `#roots` A and `#result` A, and the build plan's two questions answered: Gathr records which ask a turn about the whole room serves, and a chat's messages from before it became a team stay flat). Date: 2026-11-18.

Built: yes (the service says what every turn, task and line said in no turn is about; the window draws the chat as main messages with threads). Proof: `ui/src/chat/mains.test.tsx` and `ui/src/chat/layout.test.tsx` (on the service's answers its own tests recorded), `ui/src/chat/Streaming.test.tsx`, `crates/gathr/tests/topics.rs`, `crates/gathr/src/tasks/touched.rs`, `crates/gathr/src/store.rs`, `crates/gathr/src/topics/tests.rs`, `crates/gathr/tests/questions_flow.rs`, `crates/gathr/tests/messages.rs`, `crates/gathr/tests/likes.rs`, `crates/gathr/tests/pauses.rs`, `crates/gathr/tests/coordination.rs`, `crates/gathr/tests/quiet_room.rs`, `crates/gathr/tests/stale.rs`, `crates/gathr/tests/merge/lands.rs`, `crates/gathr/tests/merge/review_questions.rs`.

Amends [0051](0051-the-giver-hears-when-all-is-in.md) (the hear-back that closes an ask is its result) and [0028](0028-team-chats.md) (each turn says what it is about). Serves promise 6 of [the charter](../charter.md#the-promises), part 6.5 of [the truth map](../truth-map.md): the room explains itself, its talk read as threads.

## The choice

- **A main message is something put to someone that starts a piece of work or needs the person:** the person's
  own message; each task the lead hands out (work with an owner, given by the chat's Supervisor or by the person);
  a question to the person; each version of a plan; the result of an ask; and anything nothing ties, as its own.
  Everything else is a reply in the thread of the main message it serves.
- **The service works out what everything is about, in one place** (`crates/gathr/src/topics.rs`). It reads a
  chat's turns, tasks, plan versions, asks, landings and lines once, one query a table by the chat's own indexes,
  and follows the links in memory: never a query for each row (0073's lesson). The live `turn` event's topic and
  the tasks list's use the same rule fed one row at a time instead: each follows only that turn's or task's own
  chain, each link by an index, so its cost doesn't grow with the chat, on the busiest path. A line said in no turn
  is the exception: its topic as it goes out live reads the chat once, as history does, once per write that said
  one (a few a task), about 7 ms on a chat of 10,030 messages (release build: `gathr diag threads`, which reads
  the same, took 16 ms against 9 ms for a command that reads nothing). One resolver,
  two sources of its facts (`topics/load.rs`, `topics/rows.rs`). Each answer is a key: `t:<task>`
  (a task or question), `m:<message>` (the person's message, a plan version, a main line of Gathr's), `r:<turn>`
  (a result) or `b:<turn>` (a turn tied to nothing).
- **A task's topic,** from its own row, in order: a question is its own; a plan check sits with the version it
  checks; a task with a parent with its parent (Gathr's review and its fix after a review); a review given by
  hand with the task its PR was built in; the lead's or the person's work is its own; anything else with the turn
  that gave it; given in no turn, its own.
- **A turn's topic,** in order: the result of an ask is its own; a turn the person's message opened sits under
  that message; one that heard the person's answer or Discuss under that question; one Gathr queued for a PR it
  can't land with the PR's task; one a send-back queued with the version sent back (its opening line is that
  version's decided line); one that carries a copy (a teammate's message, a like) where that message sits;
  then its task's topic; then the turn it answers; then the ask it serves; a compaction where its agent's turn
  before it was; else its own.
- **A line said in no turn** sits with what it is about: the task of a done line or of a merge that didn't update
  the folder, the ask of a size change's note or of "Fixed", the version of a check's or decision's line, the
  ask or task of a caught change. "Nothing moves" needs the person: a main line of its own.
- **Three facts are recorded where they happen,** as nothing kept them before. Migration 0055 adds them by `ADD
  COLUMN`: `turns.result_of`, the ask whose closing hear-back the turn is (0051, below); `landings.turn`, the
  reviewer's turn Gathr queued for a PR it can't land; `messages.about`, what a line in no turn is about
  (`t:<task>`, `m:<message>`, or `main`). It adds three partial indexes for the live reads too, a turn's first
  message of the person's or Gathr's, the question a turn heard, a PR's tasks: no row is rewritten.
- **The lead's turn on the room as a whole serves the ask of what it names** when that is one ask: a quiet room's
  wake, a member's silent turn told, stale tasks told. Its `ask_id` is set, so it sits in that ask's thread;
  naming work of several asks, or of none, it stays its own.
- **An ask says whether it is open** (`open` with each ask), from the same rule that already decides it.
- **The answer is in history** (`topic` on each turn and on each line said in no turn), **the tasks list** (`topic`
  on each task), **the live `turn` event**, and each line said in no turn as it goes out live. `gathr diag threads --chat <id>` lists each turn, task and line, its
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
- **When a chat became a team** is what the members' `added_ts` already says; nothing new is recorded for it. History
  says it as `team_since`, the second member's `added_ts` (none for a chat of one), as members carry no time.

## The window

- **It draws what the service named,** and keeps its two rules inside a turn: a turn splits where the person's message
  entered it, and what the turn says after it is about that message; a message between agents that answers one Gathr
  carried into its turn sits where what it answers sits (`ui/src/chat/topics.ts`). A turn not named yet is its own.
- **Main messages in time order, each followed by its thread** (`ui/src/chat/mains.ts`, `timeline.ts`). A task is drawn
  from its record, as no message carries it (`TaskMain.tsx`): its giver to its owner, its title, the first line of why,
  and More for the rest of the brief. A question's main message is the block that asked it. A result is the lead's own
  block, never Gathr's words that opened its turn; it quotes its ask, and the ask's Done leads to it.
- **Each main message says its state** at its name line's right, from `tasks/rows.ts`, the same function the Tasks
  list reads, in the marks' words (0071): the person's ask Working or Done; a task as its row says it, with its PR;
  a version Checking, Needs you, Approved, Sent back or Dropped; a question Needs you, Discussing or Answered.
- **Each reply is an item of its own** in the chat's list, so a streamed word draws only its reply. A thread has one fold
  line, the same open or folded; one longer than three shows its newest three under "Show N earlier replies", never
  hiding a reply that holds an approval waiting, a failed step or a running turn.
- **Folding never happens under the person's eyes:** a thread whose main message is handled when the chat is opened
  starts folded, every other open, and nothing folds or opens by itself while it is open; the person's own fold holds
  until the chat closes. A search hit and the status line's "needs you" open the thread they lead to (since
  2026-11-18 a needs-you row in Summary and the New pill, below).
- **A chat of one agent is one timeline,** as before; so is what a chat said before it became a team.
- **The words are as wide as the message box,** the faces hang left of that width (the owner's rule): a reply's smaller
  face and the thread's rule sit inside it, its words still end at the box's right edge. Where the pane leaves no room
  beside the column, the column's left padding makes it, and the box moves in with the words.

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

- Replying in a thread, every picked agent getting a message, and where the person left off (seen, the New line,
  folds kept across a reload): later steps. Until then a fold lasts while the chat is open.
- One topic per turn: a lead's turn that answers the person, hands out tasks and messages an agent sits in one
  thread. Only the person's message entering a turn splits it.

## Changes

- 2026-11-18, the owner's look at the test window: the status line and the line at the foot go ([0050](0050-what-a-chat-waits-on.md),
  [0077](0077-where-you-left-off.md)). A search hit, a needs-you row in Summary and the New pill open the thread they
  lead to. The message box is wider than the words and the faces; the words stay as wide as the column. A thread's rule
  is drawn once: each item's piece reaches up to where the item above ends, never over it (a piece drawn twice read
  thicker in the dark theme, where the border is see-through).
