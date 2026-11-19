# 0077: Where you left off

Status: accepted (owner, 2026-11-18: the threads mock-up's `#new` and `#day` B; the threads build plan's PR 6).
Date: 2026-11-18.

Built: yes (the marks kept by the service and read with the history, the New line, the blue counts, each reply's New, the line at the chat's foot, kept folds). Proof: `crates/gathr/tests/reads.rs`, `ui/src/chat/seen.test.tsx`, `ui/src/chat/reads.test.ts`.

Builds on [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md) (threads, and which start folded) and
[0076](0076-replying-in-a-thread.md). A chat's own delete, which [0069](0069-deleting-a-project-or-an-agent.md) uses for a project's chats, removes its
marks too. Serves no promise as [the charter](../charter.md#the-promises) words it: it is the owner's own ask, "where the
person left off". The truth map keeps it as 6.10, beside the chat's other ways of explaining itself.

## The choice

- **What "seen" is:** half of a message has been in view, between the chat's top and the message box, while the
  window is in front (a message taller than the view: half the view's height of it). The person's own messages are
  always seen and never new.
- **Kept by the service, per thread:** table `reads` (migration 0057): `chat_id`, `topic`, `seen_ts`, `folded`, one
  row per thread the person has looked at, and one for the chat's main line (topic `''`). `topic` is the thread's key
  as history names it (0075: `m:`, `t:`, `r:`, `b:`). `seen_ts` is when the newest message seen there was said:
  everything at or before it counts as seen. `folded` is the person's own fold (1) or open (0), or null for Gathr's
  rule. History carries the rows as `reads`; `PUT /api/chats/{id}/reads` keeps changes; the live `reads` event sends
  each row as kept to every window. **A mark never moves back** (`MAX`); a fold is the person's latest. A thread that
  isn't the chat's refuses the whole write, `409` "Marks failed" with why and a reference that `gathr diag trace`
  finds. Deleting the chat removes its rows.
- **The window marks as it shows:** the browser says which items reach the view; only those are measured, at most
  once a frame, as the chat scrolls, grows or comes to the front (`ui/src/chat/seen.ts`). A mark moves in memory at
  once, with no redraw of the chat; only the fold lines and the line at the foot that read it are drawn again, and
  only when what they say changes. The window writes at most **once a second**, the newest mark of each thread that
  moved, and once more when the chat closes or the page goes (`ui/src/chat/reads.ts`).
- **New, as the person comes back:** what the person had seen is taken when the chat opens and when the window comes
  back to the front. The New line sits above the first main message said after the main line's mark and before they
  came; in an open thread, each reply said then says New on its name line, in the attention blue. Both hold while the
  person reads, and go when they next come back. What arrives while they are there is not New: the line at the foot
  points to it instead.
- **A folded thread's count:** while folded, its fold line says how many replies are newer than its mark, in blue
  ("2 new replies"); plain ("3 replies") once none is. New replies never open a folded thread.
- **The line at the chat's foot,** directly above the status line: the newest reply not seen whose place isn't in
  view (its own block, or its thread's main message while the thread is folded), with its face, "Ann replied:", its
  first words and its time. A press opens its thread, centres the reply, and marks it seen; the line goes once
  nothing is left to name. It says nothing the status line says.
- **Folding, whole:** when the chat opens, a thread folds when its main message is handled and every reply in it is
  seen; one with a reply not seen opens open. Nothing folds or opens by itself while the chat is open. The person's
  own fold or open is kept by the service and wins, in every window and after a reload; a search hit, the status
  line or the line at the foot opens a thread for as long as the chat is open, and is not kept.
- **An old chat:** a chat with no marks yet (every chat from before 0077) is taken as seen up to its first open: its
  newest message or now, whichever is later. Nothing old shows as New, every handled thread starts folded, and the
  window keeps that as the marks of the main line and of every thread it draws.

## Why

- The window's preferences live in cookies, as localStorage is kept per origin and the service may come back on
  another port (`ui/src/kept.ts`). A cookie can't hold a mark per thread across chats, and two windows on one chat, or
  a browser on the same service (0017), would each keep their own. The table loses nothing on a reload, a restart or
  an update.
- A mark by time is one number per thread, whatever the thread's length; a mark per message would be a row per
  message read.
- Taking what is New when the person comes, not as they scroll, keeps the New line and the words where they were
  while they are read: marked seen at once, they would go before they are noticed.

## Turned down

- **The window's own storage** (cookies, localStorage): above.
- **Marks shared between people:** one person uses Gathr; a mark is theirs.
- **A mark per message:** a row per message read, and nothing gained over a time per thread.
- **New words that go as soon as each reply is seen:** they would go while being read.

## Said plainly

- **Marks are by time:** a reply jumped past (opened from the line at the foot) marks the earlier replies of its
  thread seen too.
- **A thread that appears later with older replies** (a topic the window had not drawn on the first open of an old
  chat) has no mark, and its replies count as not seen.
- **The time compared** is when each block was said as the window draws it (its first message, or its turn's start);
  a long turn's later words share its block's time.
- **Not verified:** macOS's own window (focus and the front, under WebKit in a real app), a screen reader on the New
  line and the line at the foot, two real windows on one service.
