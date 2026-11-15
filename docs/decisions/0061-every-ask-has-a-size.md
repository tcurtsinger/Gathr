# 0061: Every ask in a team chat has a size

Status: accepted (owner, 2026-11-14, with Stage 3's spec, part G; its departures and two answers, 2026-11-15). Date: 2026-11-15.

Built: yes. Proof: `crates/gathr/tests/tasks/asks.rs`, `crates/gathr/tests/sizes.rs`, `crates/gathr/tests/archive.rs`, `crates/gathr/tests/tools.rs`, `ui/src/chat/asks.test.ts`, `ui/src/chat/Size.test.tsx`. Nothing holds new work yet: the size is recorded and shown, and the process doesn't differ by it until parts H to J.

Builds on [0010](0010-coordination-as-state.md), [0051](0051-the-giver-hears-when-all-is-in.md) and [0059](0059-a-work-task-carries-a-complete-brief.md). Serves promise 2 of [the charter](../charter.md#the-promises), parts 2.4 and 2.6 of [the truth map](../truth-map.md), and part 10.3: a repair and a new ask are told apart, and the person can correct the lead. Stage 3's part G.

In this code "ask" already means a turn's opening text (`store::ASK`) and Gathr's ask for a report (`tasks.asks`, `Task.asked`). So the new names are the table `asks`, the module `gathr::asks` and the columns `turns.ask_id` and `tasks.ask_id`. The JSON field on a task is `ask`.

## The choice

- **G1: an ask is a row, and every turn and task records the ask it serves.** A person's message in a team chat (two or more agents in `chat_members`) starts an ask or joins one. The ask's `id` is the id of the message that started it. A chat with fewer agents has no asks, no sizes and no lines.
  - A turn's ask is set when it is added (`store::add_turn`): the ask of its task, else the ask of the turn it answers. A task's ask is set right after its insert (`asks::inherit`): the ask of the turn that gave it, else of its parent. A hear-back with no task of its own serves the ask of the newest task it reports. A message moved into a turn of its own (`store::move_message`) brings its ask, a teammate's copy the ask of its original's turn.
  - A person's answer on a question carries the ask across. Rounds ([0051](0051-the-giver-hears-when-all-is-in.md)) are as they were.
  - Turns that serve no ask: a quiet chat's wake, a silent turn, stale tasks told to the lead, a PR handed to its reviewer, a compaction, every turn in a chat with fewer than two agents and every turn from before this migration.
  - It is inherited in code, not by triggers: a later migration that rebuilds `turns` or `tasks` would drop a trigger without a word.
- **G1a: a follow-up joins.** An ask has open work while an agent's task of it (one with an owner) isn't done or cancelled, or a turn of it is queued, waiting for sign-in or running. A person's message joins the ask its turn already serves (steered into a running turn), else the newest ask with open work, else it starts its own. Gathr writes `gathr.ask.message` on the turn's trace, with `gathr.ask.joined`. A message addressed to a member starts or joins an ask like any other.
- **G2: the lead sizes it, with `size`.** The tool takes `size` (`repair` or `new`, a string, so a wrong value gets Gathr's refusal and its reference) and, optionally, `message`. An ask with no size counts as new; every reader tests `size = 'repair'`, never `'new'`.
  - Only the chat's Supervisor may call it. It sizes the ask its turn serves, or, with `message`, the ask that message started. A `message` that joined another ask is **split** off as an ask of its own.
  - A split moves the turn the message opened (a turn with no task and no message before it) and the tasks that turn gave. A turn the message was steered into keeps its ask, and only the tasks it gave after the message move.
  - Refused, in this order, as `size failed: <why> (ref …)`, nothing written: a size that isn't one; an archived chat; a chat with one agent; a caller who isn't the Supervisor; no turn running; a `message` that isn't a message of the person's in this chat; a `message` that serves no ask; no `message` and a turn that serves no ask; and the person having sized the ask last, with the lead asking another size.
  - The same size again writes nothing and says so. Each change is `gathr.ask.resized`, with the size, what it was, who, whether it was a correction and, for a split, the ask it came from, whether its turn moved and how many tasks.
  - The briefing names `size` beside `react`. No kind of words in [0056](0056-gathr-records-which-words-opened-each-turn.md) changes, so no version moves. What the lead is told to do with it is part K's role text.
- **G3: the person changes the size.** `POST /api/asks/<ask>/size {"size"}` sets it with `sized_by` `person`, keeps `sized_in`, and answers the ask. It works on any ask of the chat, before or after its work. The change from repair to new is `gathr.ask.correction`, and nothing else is: the count for scorecard 1 is `gathr diag query "SELECT count(*) AS corrections FROM event_log WHERE name = 'gathr.ask.resized' AND attrs ->> '$.\"gathr.ask.correction\"' = 1"`. A resize starts no turn and moves no task. The lead can't change a size the person set.
- **G4: "Fixed".** When an ask is a repair, a work task of it is done, every task of it is done or cancelled, and no turn is queued, waiting for sign-in or running that has the ask or answers a turn that has it, Gathr says one line in no turn, once:

  ```
  Fixed: <the ask's title>
  #<n> <what>: PR #<pr>. Proof: <proof>
  ```

  One task line for each done work task, by number: the PR when it has one, else what its report names (`see`), then its proof when it has one. The title is the first line of the ask's message, as a chat's title is made. A repair with no task gets no line, and neither does one whose tasks failed or were blocked. Stage 2's line for a task that ends done stays, and "Fixed" comes after it. It is decided in the quiet check (`coord/quiet.rs`), in a writer job of its own after the check, so it sees what the last write committed, the giver's hear-back included; `asks.fixed` holds the line's message id, so a later resize or task says nothing again, and an ask resized from new to repair after its work is in gets its line at the next check. The event is `gathr.ask.fixed`; a failed check records it with `error("sqlite")` and why.
- **G5: the window.** One of Gathr's event lines, after a block, drawn by `Events` like the others, one line closed and opened in place by click or Enter.
  - Closed: "Mira: Fix" or "Mira: Plan" (the lead's name), "You: Fix" or "You: Plan" after the person changed it, "Plan" alone for an ask the lead hasn't sized. Opened: the shared `OptionList` as published, in its draft mode, named "Size": "Just fix it." and "Plan it with me first.", the current one marked. Choosing the other calls the route; choosing the marked one does nothing. A refusal shows under the choices with the shared failure notice: its words, and its reference.
  - **Where.** An ask's line is after the first block below its message of the lead's turn that sized it (`sized_in`). When that turn has none (the lead never sized it, or the message landed in a later turn), it's after the first block below the message of the message's own turn. So it never sits above the message it sizes. Two asks after one block each get a line, oldest first.
  - **No line for an unsized ask until it has a task** (any state or kind, a question to the person included), then "Plan". An ask that leads to no work (a "thanks") never has one.
  - The "Fixed" line is drawn as one line with the check icon: "Fixed:" and the title, opening to the task lines. No "Open task".
  - Icons: `Wrench` for Fix, `ClipboardList` for Plan.
- **The service tells the window.** `GET /api/chats/<id>/tasks` and the live `tasks` event carry `asks`, oldest first: `{"id", "title", "size", "by", "sized_in"}`, and each task carries its `ask`. Two temporary triggers on `asks` note the chat, so the event goes out after any change to an ask, whichever code made it.
- **The schema** (migration `0046_asks.sql`): the table `asks` (`id`, `chat_id`, `size`, `sized_by`, `sized_in`, `sized_ts`, `fixed`), the columns `turns.ask_id` and `tasks.ask_id`, and an index for each. No foreign key from `asks.id` to `messages`: deleting a chat removes messages before turns, and turns and tasks point at `asks`, so `archive::delete` removes the asks right after the tasks. **A database from before gets nothing.** Every turn and task keeps `ask_id` NULL, and the first message after the upgrade in a team chat starts a new ask even while old work is open. A backfill by rounds would make an unsized ask, counting as new, of everything in flight, and once part I lands all of it would be held.

## Why

- The size has to be a record to be held. A lead that says "this is a repair" in words is a lead the process can't check or correct, and the owner's count of corrections has nothing to read.
- A row, not a column on the first turn: an ask has four facts of its own, and a message steered into a running turn has no turn of its own to name a split ask by. `ask_id` on both turns and tasks, because a split moves the tasks a turn gave after the message while the turn stays.
- The person's open questions don't count as open work. Gathr never stale-cancels one, and the owner's room nearly always has one open, so every message would join one old ask.

## Different from the spec

The owner approved each with the detail on 2026-11-15.

- **Open work** is an agent's task not done or cancelled, or a turn queued, waiting for sign-in or running. The spec says "a task not settled", and settled includes blocked, failed and stopped; the person's own questions don't count (the owner: "No, new ask"). An answer typed as a message while only a question is open starts a new ask.
- **No line for an unsized ask until it has a task** (the owner: "No, wait"), where the spec draws "Plan" alone for every unsized ask. The cost: the person can't set a size while the lead's first turn runs and nothing is sized or given yet.
- **"Fixed"** needs done work and nothing of the ask queued or running, where the spec says "all its tasks have settled". A failed or blocked repair isn't fixed, and the lead's hear-back ends first.
- **The person's size stands.** The lead can't change an ask the person sized. The spec doesn't say.
- **A split moves a turn only when the message opened it.** The spec doesn't say.
- **Smaller:** no new live event, no ask on turns in the API, no backfill. `size` is one tool for sizing and splitting, as the spec says.
- **The spec is wrong that every turn inherits the ask of the turn it answers.** The person's turns, the turns the engine opens to read a message and Gathr's turns for the lead answer nothing, and a hear-back serves the tasks it reports.

## As built

Where the code differs from the detail the owner approved, the code stands.

- **Ordering is by insertion, not by id.** Two ids of one millisecond don't sort by time, so the newest ask with open work (`for_message`), the asks of a chat (`in_chat`) and the asks said "Fixed" at one check (`fixed`) are ordered by `rowid`.
- **The Supervisor's name in the refusal** is the agent's name, read with `team::members`, not `words::name_of`.
- **A resize refuses, in order,** `there's no ask <id>`, `` `<size>` isn't a size: repair or new ``, then an archived chat's own words, each as a 409 `Size failed` with a reference.
- **The window's refusal title** is the failure's own `error`: the route's "Size failed", or "Connection failed" when the call never reached the service. `SizeChoice` calls the route itself, and `Events` and `Event` take an optional `onReference`, so a refusal's reference opens Diagnostics.
- **The window reads `asks` as optional** in a chat's first load, so a stub that gives none draws no lines; the live event always has it.

## Said plainly

- **"Fixed" can come early.** A follow-up the person steers into the ask's last running turn, that the engine reads only after the turn ends, gets its turn's row when the engine reads it. Until then it waits only in the runner's `unread`, so nothing of the ask is queued or running in the database, and "Fixed" can be said before the follow-up's turn exists. The turn still serves the ask and runs; only the line came first. Closing it needs the runner's `unread` in the database.
- **It can come late.** The lists of touched chats are temporary tables, so a service that stops between the commit that ends the repair's last open thing and the check says the line at the chat's next write.
- **A message split off brings its own ask.** A message steered into a running turn, split off with `size`, and later read into a turn of its own, gives that turn its own ask, not the ask of the turn it was saved in.
- **A moved task's turns move with it.** A task the split moves takes its turns with it (its queued brief, a report ask, a carry-on), so a member's task turn always serves its task's ask. Tasks that a moved task had already given stay with the old ask.
- **A message joined to a queued turn** (a teammate's message appended to a turn already queued for the recipient) stays in it, and the turn keeps its ask: a turn serves one ask.
- **Failed, blocked and stopped tasks of agents still keep an ask open,** because "go on" after work stopped is a follow-up. The cost: an old failed task nobody cancels draws new messages into its ask until it is cancelled or the lead splits one off.
- **A question with no parent task serves no ask.** What that means for a hold or a growth stop is for parts I and J.
- **Same-millisecond ids** affect anything else that sorts asks or messages by id. Only `for_message`, `in_chat` and `fixed` are moved to `rowid`.
- **The cost of `fixed`'s query,** which runs on every quiet check, and of `for_message` in a chat with thousands of asks, was read from the plan, not timed.
- Whether a real lead calls `size`, and rightly, is part K's trial. No test here measures it.

## Turned down

- **`everyday` as a size in a team chat.** The assistant's, in Stage 6.
- **A column naming the ask's first turn instead of a table.**
- **Walking `answers` at read time, as rounds do.** Rounds break at a person's answer, so they don't give an ask's extent.
- **Triggers for the inheritance.**
- **A backfill by rounds.**
- **A new live event for asks.** The `tasks` event carries them.
- **Saying "Fixed" in the transaction that settles the last task.** The giver's hear-back is queued after it, so the rule couldn't see it.
- **The size as an enum in the tool's schema.** The refusal with a reference stays Gathr's.
- **The lead overriding the person's size.**
- **A split moving a turn it didn't open.**
- **The person's open questions as open work** (the owner's answer).
- **A "Plan" line for every unsized ask from its first block** (the owner's answer).
- **The mock-up's explaining words and its third choice.** The line says Fix or Plan, and nothing explains it.
- **A setting for any of it.**

## Amends

- [0010](0010-coordination-as-state.md): the rule that a Supervisor's turn giving two or more tasks is a plan goes. The size replaces it: what makes new work wait for a plan is an ask sized new, with parts H to J.
