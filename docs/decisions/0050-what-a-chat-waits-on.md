# 0050: One answer to what a chat waits on

Status: accepted (owner, 2026-11-13, with Stage 2's spec; the line's place is the owner's pick). Date: 2026-11-13.

Built: yes. Proof: `crates/gathr/tests/waiting.rs`, `crates/gathr/tests/messages.rs`, `ui/src/chat/Waiting.test.tsx`.

Serves promise 6 of [the charter](../charter.md#the-promises): the room explains itself (parts 6.1 and 6.2 of [the truth map](../truth-map.md)).

## The choice

- **One function gives the answer for a chat,** from saved state plus what only the running service knows. Each
  line says what it is, who or what can end it (it moves by itself, it waits on you, nobody is on it, or it
  follows other lines), since when, and where Gathr knows it from. Where Gathr can't tell, the line says
  unknown, and why.
- **What it lists:** each running turn; each queued turn with why it hasn't started; each open wait with what its
  last look saw and its error when looks fail; each question, approval and sign-in that waits on you; each
  background command; each PR Gathr may land, as its last look left it ([0049](0049-gathr-lands-pull-requests.md));
  each task no turn is on, with what it waits for or that nothing is on it; and Gathr's own update.
- **Then where each agent stands:** needs you, working, waiting (for whom or what, since when) or free.
- **A task's reason and its lines come from one list of its holds** (`tasks::holds`), so the two never say
  different things. The reason's words don't change: other code reads them.
- **What only the service knows is said by the queues.** Five reasons a queued turn waits live only in its
  agent's queue, which is one for the agent across all its chats: the turn it is behind, an engine that is
  closing, a message its engine hasn't read yet, a compaction waiting for background commands. Each queue tells
  the service why each of its queued turns waits whenever that changes. A queued turn nothing has spoken for is
  said as unknown. A turn behind one that waits for a sign-in waits on you too, whichever chat that turn is in.
- **Served three ways, one answer:** `GET /api/chats/<chat>/waiting`; a tool for every agent, `waiting`, for its
  own chat; and `gathr diag waiting --chat <id>`, from the service when it runs and from the database when it
  doesn't, where what only the service knew is said as it was when it stopped and nothing is said to move by
  itself.
- **In the window, a line at the foot of the chat,** where "Working" shows, while no turn runs and the answer
  has a line. Its words are the first of: "Waiting on you: 2 questions"; "Waiting: …" with what has waited
  longest among the things that move by themselves; "Not moving: …" with what nobody is on. Then "and N more".
  It opens the whole list in a popover of Summary's kind.
- **Summary and the agent panel keep what they work out** (who needs you, who works and on what). The words for
  an agent that does neither ("Waiting for CI on #157", "Held", "Paused", "Usage limit until 14:05") are the
  answer's, and the window's own working-out of them is gone.
- **It follows the service without a reload.** The service sends one live message naming the chat whenever its
  answer may have changed, and the window asks again. Ages tick in the window.
- **A tool that names a hold names the real one:** `message` says whether an update, or the pause of the app,
  the chat or the agent, holds the turn it queued.

## Why

In the team's first three days, every agent once waited on a CI job no runner had taken, and nothing said so;
the owner asked the lead what it was waiting for seven times. What a chat waited on was put together in the
window, agent by agent, from tasks and waits alone: it couldn't see a turn queued behind another chat's turn,
a wait whose look kept failing, or a PR that sat reviewed. Agents couldn't ask at all.

## Turned down

- **Keep working it out in the window.** Half of it is in the service's memory, and an agent has no window.
- **A table of holds.** A second state to keep in step with tasks, turns and waits. The answer is read from what
  is already there.
- **Push the whole answer live.** It changes with every turn; one small message and a request when a window
  shows that chat costs less and can't arrive out of order.
- **A new screen.** The line and a popover say it where the person already looks.

## Changes

- [0049](0049-gathr-lands-pull-requests.md): each look at a PR to land keeps what it saw and whether the PR moves
  by itself from there (`landings.saw`, `moving`, `looked_ts`, migration 0038).
- `ui/src/chat/waits.ts` is gone.
- [0052](0052-a-quiet-chat-is-noticed.md), 2026-11-13: a running turn that has made no progress for ten minutes,
  twenty while a tool is open, is a line nobody is on, with since when (`silent_us`), and the line at the foot
  reads "Working: Cass silent 12m" in place of "Working". A task whose owner is paused, by the agent's pause,
  the chat's or the app's, waits on you and says ", paused", as a wait under a pause does; its line didn't
  name the pause before. Gathr now acts on a chat where nothing moves, which is 0052's own.
