# 0040. A chat's state in the chat list

Status: accepted (owner, 2026-11-11, on the plan for the chat redesign's fifth slice). Date: 2026-11-11.
The sidebar's dot is the one 0038 left for a later slice. 0039 is the conversation map's.

Built: yes. Proof: `crates/gathr/tests/chat_state.rs`, `ui/src/live-lists.test.tsx`.

## Choice

- **Each chat in the list says two things.** `working`: a turn of it is running. `needs`: it waits on the
  person, through an approval waiting for an answer, a question an agent asked them, or a message held for
  a sign-in. An archived chat says neither: archiving ended its work.
- **The service says it, the window draws it.** `GET /api/chats` carries both on every row, and the live
  socket sends `{"type": "chat_state", "chat", "working", "needs"}` to every window, only when either
  changed for that chat. The two are worked out in one place from the same query the list uses, so a
  reload and the live event agree.
- **One dot in the sidebar,** at the right end of the chat's title: grey and breathing while it works, the
  attention colour and breathing once it needs the person. Blue wins: a chat that works and needs the
  person shows one blue dot. Nothing otherwise. Under reduced motion the dot is there and still. Its name
  is "Agents working" or "Needs you". The open chat shows its dot too.
- **Grey follows running turns only** (the owner's call). A paused chat is grey while a turn of it still
  runs, then has no dot. An agent that only waits, on CI, a PR or another agent, isn't working: Summary
  says who waits on what. A turn queued behind another isn't working either.
- **Answering an approval is told to every window.** It changes the chat's `needs`, so a second window
  hears it; before, answering sent nothing.
- **After a restart the state is the list's.** Turns that ran when the service stopped are failed as
  before, approvals went with them, and questions and messages held for a sign-in still need the person.
- **Found from Diagnostics.** `gathr diag health` lists each chat that works or needs the person, by id
  (a title is conversation text, and a bundle is shared), and says who answered: the service, or the
  database with why the service didn't; a state that can't be worked out is recorded as `gathr.chat.state` on the turn's or the
  request's reference.

## Why

- Who is working and who waits on the person was only visible inside the open chat. With several chats
  going, the person had to open each one to find the one stopped on them.
- Only the service knows all of it: an approval lives in the memory of the turn that asked, and an answer
  given in another window sent no event.
- A new event type keeps the cost to one small message per change. The window reloads both lists on every
  `chat` event, which a turn's start and end must not cause.
- Two partial indexes by chat, `turns_live` on the turns running or waiting for a sign-in and
  `tasks_asked` on the questions open to the person, keep the list from reading any chat's turns or
  tasks: it stays as cheap however long chats grow.

## Turned down

- **The window working the state out from the events it already gets.** The same rules would live twice,
  and it couldn't know of an approval answered elsewhere.
- **A still grey dot on a paused chat with work held.** One more mark to learn, and the design has none.
- **Grey for an agent that waits.** A chat waiting a day on a PR would breathe all day, and waiting would
  read as working.
- **A count or a second dot** for a chat that both works and needs the person. What the person can act on
  wins.
- **Reusing the `chat` event.** Every window would reload both lists at each turn's start and end.
- **A new event in the record for every change.** The state follows from records already there: turn
  starts and ends, `gathr.approvals.*` and task changes.

## Known limits

- **A push or check Gathr asked about, whose caller gave up waiting,** stops counting only when its turn
  ends: nothing tells the service at the moment the caller leaves.
- **The dot says the chat, not who.** Which agent works or asks is in Summary (0038).

## Changes

- [0052](0052-a-quiet-chat-is-noticed.md), 2026-11-13: `needs` has a fourth cause. A chat Gathr told the
  person isn't moving needs them until that stretch ends: when something moves, or when they write in it. It
  is saved state (`quiet.told_ts`) read by the same query, so the list, the live event, the state after a
  restart and `gathr diag health` all have it.
