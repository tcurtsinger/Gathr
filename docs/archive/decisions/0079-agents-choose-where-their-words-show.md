# 0079: Agents choose where their words show

Status: accepted (owner, 2026-11-19: the chat-shape build plan's section 5 and its question 2 answered A: agents learn
of `post` from the tool's own description and its name in the briefing's tool list, with no role line and no trial).
Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/post.rs`, `crates/gathr/tests/tools.rs`, `ui/src/chat/state.test.ts`.

Builds on [0078](0078-said-to-you-is-main-one-bubble-per-turn.md) (the default rule, which this choice changes for one
turn) and [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md) (the keys). Serves promise 6 of
[the charter](../charter.md#the-promises), part 6.5 of [the truth map](../truth-map.md): the room explains itself.

## The choice

- **One small tool, `post {"to"}`**, on Gathr's one MCP server, so both engines have it. `to` is `room` (the main
  list), a message's id (its thread: a copy of a message stands for the message it carries, as for a reply, 0076),
  `#12` (task 12's thread), or `person` (the thread of the person's message the turn answers, as `react` reads it).
- **It records the place on the turn,** `turns.placed`: `main`, `m:<message>` or `t:<n>`, the vocabulary of
  `messages.about` (0055). Migration `0059_placed.sql` adds the column; old turns have none.
- **The whole turn's bubble moves,** words written before the call too: Gathr can't tie the call to its place among
  the turn's steps, so it doesn't split the turn there. A second call moves it again. The description says to call it
  before writing, and the event counts the words saved before it (`words_before`).
- **The rule reads it third** (`crates/gathr/src/topics.rs`): after the result of an ask (`r:<turn>`) and a turn that
  wrote a plan version (`b:<turn>`), before everything else. `room` makes the turn its own `b:<turn>`; a message, a
  task or `person` puts it where a reply to that message, or the task, sits. A choice that leads nowhere (the message
  is gone) ties to nothing, as any broken link does.
- **The window moves the bubble when `post` answers:** the service sends the live `turn` event with the turn's new
  `topic` (status `running`, as the engine is calling a tool), which the window already merges into the turn it
  holds, so the bubble moves with no new event type. A plan written after a thread was chosen sends it again with
  the turn back in the room.
- **The answer** says where: "Your words this turn show in the room.", "… in the thread of Ann's message." (the
  person's message reads "the person's message"), "… in task #12's thread."
- **The briefing's tool list names it** ("post to choose where your words this turn show in a team chat"): Claude Code
  finds an MCP tool behind its tool search only when it is named there (0005). The window's step reads "Chose where
  to post".

## What it can't move

What needs the person stays in the room, whatever an agent chose: Gathr guards it, not the prompt.

- **Refused**, each answered `post failed: <why> (ref …)` and found with `gathr diag trace`: no turn running (`you
  have no turn running in this chat`); an archived chat; a chat of one agent (`post is for a team chat: this one has
  one agent.`); the result of an ask (`This turn is the result of the ask: it stays in the room.`); an id that isn't a
  text message of this chat, a step, or a task not in this chat (the words of `replies::check`, reused); `person`
  when the turn answers no message of the person's (the words of `react`); a message of the turn itself (`<id> is
  this turn's own message: choose another, or the room.`); a thread for a turn that already wrote a plan (`This turn
  holds a plan for the person: it stays in the room.`) or asked the person a question (`This turn asks the person a
  question: it stays in the room.`).
- **Overridden:** a plan written after `post` chose a thread puts the bubble back in the room, as the rule reads a plan
  first. A question asked after it is its own main message in the window, as before. Each override is a
  `gathr.turn.placed` event with `overridden: true` and `by` (`plan` or `question`).

## Diagnostics

- `gathr.turn.placed` on the turn's trace: `turn`, `to` (`room`, `message`, `task`, `person`), `placed`,
  `words_before` (the turn's own words saved before the call), `overridden` and, for an override, `by`. How often
  agents choose, and what, is read from these: `gathr diag query` on `event_log`.
- `gathr diag threads` names the fact `placed`: "the agent chose the room", "the agent chose to reply to message
  <id>", "the agent chose task #12's thread", and says how many turns agents placed.

## Why

The default (0078) puts nearly everything where it belongs, but only the agent knows when its turn answers a
teammate's message rather than its task, or is worth the room. One tool lets it say so for its whole bubble, on both
engines, while Gathr keeps what needs the person on the main list.

## Turned down

- **A parameter on `message`** (`to: "room"` or a reply id): it covers only words sent through the tool, as a second
  bubble beside the turn's, which breaks one bubble per turn, and the turn's own words still go by the default.
- **`message` with the person as `to`:** the same, and `message` refuses a chat of one.
- **Words in the agent's text ("@room"):** parsing words is guessing, and guards are built in, not set by prompting.
- **Splitting the turn at the call:** Gathr can't tie a call to its place among the turn's steps.
- **A role line telling agents to use it, tried on real engines** (the plan's question 2, B): the default already
  places things; a line would push agents to move them. Gathr counts the choices, so a need to tell them shows.
- **A new live event type for the move:** the `turn` event already carries `topic` and merges.

## Not in it

- Whether real agents use `post` unasked, on either engine: not run.
- That Claude Code needs `post` named in the briefing is 0005's finding for every tool, not run for this one.
- A Stop while `post` answers can send `running` after `stopped`; the turn's end event that follows sets it right.
