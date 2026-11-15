# 0057: Agreeing is a thumbs-up, not a message

Status: accepted (owner, 2026-11-14: Stage 3's spec, the two narrowings and the look). Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/messages.rs`, `crates/gathr/tests/archive.rs`, `crates/gathr/tests/tools.rs`, `ui/src/chat/Timeline.test.tsx`, `ui/src/chat/state.test.ts`.

Builds on [0010](0010-coordination-as-state.md) and [0013](0013-one-chat-model.md). Serves promise 6 of [the charter](../charter.md#the-promises), part 6.6 of [the truth map](../truth-map.md): an agent that only agrees posts no message and starts no turn. Stage 3's part C.

## The choice

- **C1: the `react` tool.** An agent puts its thumbs-up on another agent's message in its own chat, by the message's id. It takes one parameter, `message`. There is no parameter for which reaction: the thumbs-up is the only one.
  - The record is one row in `reactions` (migration `0044_reactions.sql`: message, agent, chat and time; one row for each agent and message) and one event, `gathr.message.reaction`, with `message` and `agent`. Both are written in one transaction, the event on the caller's turn's trace.
  - It posts no message, counts no hand-off, starts no turn and wakes nobody. The chat's message count, search and the quiet check don't see it.
  - **A second thumbs-up** from the same agent on the same message is not an error. It adds no row, writes no event and sends nothing live. The answer says the message already has it.
  - **Gathr's copy** of a teammate's message (the one in the member's turn) stands for the message it carries: the thumbs-up lands on the original.
  - **Refused**, each answered `react failed: <why> (ref …)` and found with `gathr diag trace`: no turn running for the caller in the chat; no text message with that id in the chat (an id from another chat, an unknown id, a tool or thinking row); a message that isn't an agent's (the person's, or one of Gathr's own lines); the caller's own message; an archived chat.
  - **The id comes with the words.** What Gathr tells a member when a teammate messages it carries the id: "Lead wrote to you (message <id>): …". `read_chat` already shows ids. The briefing names `react` among the tools, so Claude Code finds it behind its tool search.
  - **Deleting a chat** removes its reactions with its messages. Archiving keeps them.
- **C2: the window shows it, live.**
  - `GET /api/chats/<id>` carries `reactions`, oldest first. The live socket sends `{"type": "reaction", "chat", "reaction"}` when a row is new.
  - **The look is the thumbs-up icon and a count.** The names are in the shared tooltip, under the mark so it never covers the words, on hover and on keyboard focus, in the order the thumbs-ups were given. The accessible name says them too: "Thumbs-up from Vale and Juno". No names and no initials in the line, and no label.
  - The count is the number of givers the window can name. A giver Gathr no longer knows isn't counted, so the count always equals the names in the tooltip.
  - It sits under the words it was given to: a reply block's shown text, and each message of an exchange between agents. A reply block shows its turn's last text. A thumbs-up on an earlier text of the turn, which folds into the turn's steps, is kept and the API carries it, but the window draws none under the block: it would sit under words the giver didn't agree with.
- **C3: no role text.** The briefing names the tool and says what it does, as it does for every tool. It doesn't tell agents to agree this way, and nothing refuses a message for its wording. That agents agree with a thumbs-up and not a message is measured later, in Stage 3's trial, so 6.6 stays Partly.
- **The two changed texts aren't kept samples** of [0056](0056-gathr-records-which-words-opened-each-turn.md): neither the teammate's message (a kind with no version) nor the briefing is one of the 24. No version moved, and the words tests pass unchanged.

## Why

- An agent that agrees by message costs a turn for the one it agrees with, and a line in the chat for everyone. A thumbs-up costs one row.
- An id in the delivered words makes agreeing one call. Without it, the member would first read the chat.
- The icon and a count take less room than a line of names, and the tooltip puts who gave it one hover away.

## Turned down

- **A reaction as a message row** (a new `kind`). It would count in the chat's messages, show in `read_chat` and search, and every rebuild of `messages` would have to carry it ([0035](0035-search.md)).
- **Other reactions, or an emoji parameter.**
- **A thumbs-up that wakes its author or is delivered to it.**
- **Taking a thumbs-up back.**
- **A thumbs-up on the person's messages or Gathr's lines.** Drawing under the person's message is window work nobody asked for, and the person's own reactions come later.
- **Drawing a folded text's thumbs-up under the block's shown text.**
- **`react` with no id, meaning the message that woke the agent.** It is ambiguous when several messages joined one turn.
- **Reading a message's words to decide it only agrees.**
- **`read_chat` showing reactions:** nothing asked for it.
- **A line of names under the message.** Offered to the owner, not picked.
- **A pill of initials.** Offered to the owner, not picked.
