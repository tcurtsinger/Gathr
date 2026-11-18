# 0057: Agreeing is a thumbs-up, not a message

Status: accepted (owner, 2026-11-14: Stage 3's spec, the two narrowings and the look). Date: 2026-11-14.

Built: yes. Proof: `crates/gathr/tests/messages.rs`, `crates/gathr/tests/archive.rs`, `crates/gathr/tests/tools.rs`, `ui/src/chat/Timeline.test.tsx`, `ui/src/chat/state.test.ts`; the person's like (C4): `crates/gathr/tests/likes.rs`, `crates/gathr/tests/likes/acts.rs`, `ui/src/chat/Like.test.tsx`.

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
- **C4: the person's like** (owner, 2026-11-18: "Liking something means Approved or Agree with"). The same thumbs-up,
  given by the person from the row on a message's bubble or by pressing its count; it means "approved" or "I agree".
  - **Kept** in `reactions` with `agent_id` `person` (never an agent's id: those are 16 hex characters), no migration.
    `PUT /api/chats/<id>/messages/<message>/like` gives it, `DELETE` the same takes it back. History and the live
    `reaction` event carry it as `{"message", "agent": "person", "time"}`; a take-back sends the same with
    `"removed": true`. Each give writes `gathr.message.reaction` with `agent` `person` and `did`; a take-back,
    `gathr.message.reaction.remove`.
  - **On an agent's message, old or new**, it reaches that agent at once, exactly as a short message of the person's
    would: into the agent's running turn in that chat when one runs, else a turn of its own (`liked`, a kind of
    [0056](0056-gathr-records-which-words-opened-each-turn.md) with version 1). A pause never holds that turn, an
    update installing does, and a signed-out agent's waits for its sign-in, as the person's message does. The agent
    reads one line of Gathr's naming the message by its id, time and first words, saying the person approves it or
    agrees, then: carry on if it answers something it was waiting on, otherwise end the turn without replying. The
    line is Gathr's copy of the liked message (`copy_of`): the chat draws no bubble for it, nothing that looks for the
    person's message of a turn (Retry, asks, the quiet check) finds one, and a turn it opens serves the ask the liked
    message served. A turn a like opened that then says and does nothing did what its line asked: it ends done,
    not failed as other turns that say and do nothing are (G167), and the chat draws nothing for it.
  - **On a bubble holding a waiting question with a recommended choice** (the asker's pick: a `choice` question an
    agent asked, its first choice), it answers each such question with its pick, through the answer path a pick
    takes, so the asker hears it as it hears a pick and the answer stands like any answer. A bubble holding a waiting
    question with no pick offers no like; Gathr's own questions have no pick.
  - **On a version of the lead's plan waiting for the person**, it is Approve, through Approve's own path and checks.
  - **Never:** an approval (Allow, Deny) is not answered by a like. The person's own messages, Gathr's lines and a step
    take none. A removed agent's message, or one no longer in the chat's team, takes the mark and reaches nobody.
  - **Refused** with a reason and a reference, as the click it stands for refuses: another chat's or an unknown
    message, the person's own, Gathr's line, a step, a question not asked in that message, one already answered or
    with no pick, a superseded or already decided version, an archived chat.
  - **Given twice** while one stands: nothing more. **Taken back:** the mark goes and nothing else: no answer, approval
    or turn is undone, and nothing is sent. **Given again** after that: the mark alone. A message's like acts once,
    the first time: a person toggling it never wakes the agent twice. Whether it acted is read from the record
    (`gathr.message.reaction` with `agent` `person` on that message), so it needs no new column.
  - **Agents' thumbs-ups are unchanged:** they start no turn and can't be taken back. Agents liking the person's
    messages is its own change.
- **The two changed texts aren't kept samples** of [0056](0056-gathr-records-which-words-opened-each-turn.md): neither the teammate's message (a kind with no version) nor the briefing is one of the 24. No version moved, and the words tests pass unchanged.

## Why

- An agent that agrees by message costs a turn for the one it agrees with, and a line in the chat for everyone. A thumbs-up costs one row.
- An id in the delivered words makes agreeing one call. Without it, the member would first read the chat.
- The icon and a count take less room than a line of names, and the tooltip puts who gave it one hover away.
- The person's like (C4) is the owner's ask: agreeing with a recommendation, or a question waiting in a quiet room,
  without typing a reply. The known cost: a like on something that needs nothing costs one short turn of an idle
  agent, softened in the line it reads, not by a rule.

## Turned down

- **A reaction as a message row** (a new `kind`). It would count in the chat's messages, show in `read_chat` and search, and every rebuild of `messages` would have to carry it ([0035](0035-search.md)).
- **Other reactions, or an emoji parameter.**
- **An agent's thumbs-up that wakes its author or is delivered to it.** The person's does (C4).
- **Taking an agent's thumbs-up back.** The person takes theirs back (C4).
- **A thumbs-up on the person's messages or Gathr's lines.** Drawing under the person's message is window work nobody asked for.
- **The person's like delivered only on the agent's latest message, an older one read at its next turn** (the
  owner's review 2). It needed a rule for which message is latest and still left an old question needing a typed
  message to wake a quiet room (review 3).
- **A like given again after a take-back acting again.** A toggle would wake the agent, or try an answer, twice.
- **A like that answers a question with no pick, or Gathr's own question.** It would read as an answer nobody chose.
- **Drawing a folded text's thumbs-up under the block's shown text.**
- **`react` with no id, meaning the message that woke the agent.** It is ambiguous when several messages joined one turn.
- **Reading a message's words to decide it only agrees.**
- **`read_chat` showing reactions:** nothing asked for it.
- **A line of names under the message.** Offered to the owner, not picked.
- **A pill of initials.** Offered to the owner, not picked.
