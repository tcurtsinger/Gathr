# 0076: Replying in a thread

Status: accepted (owner, 2026-11-18: the threads mock-up's `#replybox` A and `#solo`, and the reply-react
mock-up's `#actions` B; the threads build plan's PR 4). Date: 2026-11-18.

Built: yes (the reply's target kept and checked, its turn placed and told, the box at a thread's foot). Proof: `crates/gathr/tests/replies.rs`, `crates/gathr/tests/words.rs`, `ui/src/chat/ReplyBox.test.tsx`, `ui/src/chat/under.test.tsx`.

Amends [0065](0065-questions-in-the-leads-words.md) (in the chat a question is talked over with Reply; the Discuss
switch stays in the Tasks view) and [0028](0028-team-chats.md) (the send takes what a reply answers). Builds on
[0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md). Serves promise 6 of
[the charter](../charter.md#the-promises) (6.5, the room's talk read as threads) and promise 4 (4.2, a question is
talked over where it was asked).

## The choice

- **Reply opens the one message box at the foot of a thread.** Reply sits in the row under a message's bubble,
  before the like, shown while the message is pointed at or holds keyboard focus (amended below). It opens the thread if it was folded and places the one `Composer` after the thread's
  last reply shown: one row, the words, who gets it by Send ("To Ann"), a cross, and Send. Focus goes into it;
  Escape (once no `@` list is open) or the cross closes it, and focus goes back to the Reply that opened it. One box
  at a time: Reply elsewhere moves it. Each thread keeps its own draft beside the chat's (by chat and thread, while
  the app runs, as the chat's draft is). The box at the chat's foot still sends new main messages, and keeps Stop.
- **Who gets it:** a message's author; a line of Gathr's, the lead (in a chat of one agent, that agent); a task's
  message, the task's owner; the person's own ask, once it has replies, the lead; an agent's
  question, its asker. `@` picks another, as in the main box. The person's own messages and Gathr's own question
  offer no Reply; a removed agent's message offers none, as no reply can reach it.
- **The target is kept on the reply:** `messages.reply_to` (the message it answers) or `messages.reply_task` (a
  task: a task's message has no message of its own), migration 0056, by `ADD COLUMN`. The send route checks it: a
  text message of this chat (Gathr's copy of a message stands for the message it carries, as a like's does) or a
  task of this chat, one of the two. Anything else is refused, `400` "Send failed" with why and a reference that
  `gathr diag trace` finds. History and the live message carry both fields; Retry sends a failed reply again with
  its target.
- **The reply's turn sits in the thread of what it answers** (`topics.rs`): a reply about a task sits with the task;
  a reply to the person's own message, under it; to a message in a turn, where that turn sits; to a line of Gathr's
  said in no turn, with what the line is recorded as about, else under the line. In the window a reply sits where
  what it answers is drawn, and what its turn says after it goes there too, as for any message of the person's that
  enters a turn (0075).
- **It serves the ask of what it answers** (`asks::for_reply`): the message's ask (the ask itself, its turn's ask,
  or the ask a line is about) or the task's. A reply never starts an ask, joins the newest open one, or asks a size.
  In a chat that is no team, a reply serves no ask, as any message there does.
- **What the agent reads,** before the person's words: one line of Gathr's, the new turn kind `reply`, version 1,
  with its kept samples. `The person replies to your message (message <id>, <time>): "<first 200 characters>"`
  (to Ann's message, to Gathr's message, to their own message), or `The person replies about your task #12
  "<title>".` (about Ann's task, about task). The time is the message's UTC time, as the like's line gives it.
  Joining the agent's running turn in this chat, the same line goes in with the words. A reply turn left waiting
  across a restart reads the line again (`replies::of_turn`).
- **A question waiting on the person is talked over by replying in its thread.** The box sends a Discuss through
  the existing route (`POST /api/person-tasks/answer`, 0065's F5): the question stays open and comes back when the
  lead's turn ends. Picking a choice still answers it; the like still answers it with the asker's pick. Once
  answered, the box sends an ordinary reply about the question, to its asker. In the chat the question block has
  no Discuss switch; the Tasks view keeps it.
- **A chat drawn flat** (one agent, or before it became a team, 0075): a reply to an older message makes a thread
  under it, and what its turn says after it goes there; threads never nest, so a reply to a reply joins the same
  thread. No state words: the chat stays flat around it.
- **Diagnosable:** `gathr.chat.send` records `gathr.reply.to` and `gathr.reply.task`; a refused target is the
  request's own error with its reason; `gathr diag threads` names a reply's turn's fact `reply`.

## Why

- Typing where the reply will show keeps the person's eye on the thread, and who gets it shows by Send before it
  goes (`#replybox` A, the owner's pick over B).
- The target is a fact only the moment of sending knows; keeping it on the message means history, the window, the
  topics and a restart all read the same answer, with nothing to keep in step.
- A question's thread is where its talk already sits (0075), so talking it over there needs no second control.

## Turned down

- **The one box at the bottom with a "Reply to" line and a ring on the message** (`#replybox` B, the owner's earlier
  pick): with the thread far up, the ring is out of sight while typing, and the same box makes new main messages
  when the line is closed.
- **A link table for replies.** One message answers one thing: two columns on the message say it, and a table
  would be a join on every history read.
- **Reusing `copy_of`.** It means "Gathr's copy of another message, never drawn"; a reply is drawn and is the
  person's own. Every place that skips copies would skip replies.
- **The reply as a main message with a quote.** It would answer something far up from the bottom of the chat,
  the opposite of a thread.

## Said plainly

- **A reply to a line of Gathr's that the window places by another fact** (a repair's Fixed line, a plan's line, a
  caught change, read in 0075's rule for lines) sits under that line in the service's topic, while the window
  draws it where the line is. Rare: such lines offer Reply, but few are answered.
- **A reply to a message from before a chat became a team** is drawn as its own main message in the team part:
  the message it answers is drawn flat, above where threads begin.
- **The reply's own state** is its thread's: it has no state word of its own, as no reply has.
- **A failed reply** shows with Retry at the chat's foot, as a failed message does, not in its thread.
- **Not verified:** a real engine reading the `reply` line, macOS and WebKit, and the box with a screen reader.

## Amended: the buttons under the bubble (owner, 2026-11-18)

The owner's words: "Hovering over a message shows them. They live in a dedicated spot under the bottom right of the
bubble. On the same row as the replies and reactions." This replaces the corner row (`#actions` B) and the plain word
Reply on a thread's line:

- **One row under a message's bubble** (`ui/src/chat/Foot.tsx`), the same on every message that can have one: a main
  message's fold line when it has replies, the thumbs-up count, then Reply and Like at the right, icons only. The fold
  line is no item of its own any more: it is the main message's, drawn with it.
- **The buttons show while the message is pointed at or holds keyboard focus,** by opacity, in a spot the row always
  keeps, so nothing moves when they show. The bubble and the row share one box as wide as the wider of the two, so the
  buttons end at the bubble's bottom right edge, or follow the fold line and the count under a narrower bubble. CSS
  alone: no layout measured in script, nothing only Chromium has.
- **Reply on a main message answers it as its thread's,** folded or open, as the word did: a task's message, about the
  task to its owner; an agent's question, to its asker; the person's own ask, once it has replies, to the lead.
- **What it costs:** the row is 24px, 2px under the bubble, and gives back 14px of the gap after it. From a bubble's
  bottom to the next name line: 32px after a main message with nothing under it (20px before), 26px after a reply in a
  thread (14px before); 26px from a main message with replies to its first reply (48px when its fold line was a row of
  its own), and 32px after a message with a count (51px). Measured in the browser at 900 wide.
- **It closes the corner row's faults:** over the name line and its steps button on a short bubble, and over a main
  message's state word.
