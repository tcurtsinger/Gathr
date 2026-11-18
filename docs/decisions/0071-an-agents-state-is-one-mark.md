# 0071: An agent's state is one mark

Status: accepted (owner, 2026-11-17: the status mock-up, all seven marks, the card, Spin "Latest"). Date: 2026-11-17.

Built: yes. Proof: `crates/gathr/tests/waiting.rs` (each state, its words and since when), `crates/gathr/tests/chat_state.rs` (a chat's row's mark), `ui/src/chat/glance.test.ts` (the most pressing of what the chat and the service say), `ui/src/chat/marks.test.tsx` (the top line, a message's face, the card by pointer and by focus, an agent that left, an archived chat, reduced motion), `ui/src/chat/Summary.test.tsx`, `ui/src/sidebar.test.tsx`, `ui/src/live-lists.test.tsx`. How each mark is drawn is proved by pictures.

Amends [0050](0050-what-a-chat-waits-on.md): where each agent stands is one of seven states, each with since when.

## The choice

- **Seven states, one mark each, most pressing first.** An agent shows the first that holds, the same on every face of it in the chat:
  1. **Needs you**, a blue dot: an approval of its waits, a question of its to the person is open, or a plan of its was sent to the person and isn't decided.
  2. **Can't run**, a red disc with !: a turn of its waits for a sign-in, or its last turn in the chat failed and nothing was queued after it.
  3. **Working**, a three-quarter blue ring: a turn of its runs, starting and thinking included.
  4. **Stopped**, a grey disc with a square (a plain square at 24 and under): the person stopped the chat, a pause holds the agent (the agent's, the chat's or the app's), a restart cut its last turn, or the person stopped that turn. A turn Gathr stopped itself (its task cancelled, held for its plan) doesn't stop the agent.
  5. **Blocked**, a grey disc with a bar: a task of its is blocked.
  6. **Waiting**, a grey disc with a clock's hand: it has work of its own, a turn to come or a task under way, and waits: on another agent, a check, a PR, a merge, a usage limit, a download.
  7. **Free**, a plain green dot: nothing running, nothing waiting. Green means Free only (`--free`, dark `oklch(0.74 0.17 148)`, light `oklch(0.54 0.15 148)`); blue means the person's attention.
- **Read from what Gathr keeps, no new table.** The service's `standings` (`crates/gathr/src/waiting/standing.rs`) reads each from turns (status, why, `stop_why`, created and ended times), tasks, pauses, plans and the approvals the running service knows. The answer's agents carry `state`, `words`, `what` (a word or two for the card), `task` and `since_us` for every state. The live `waiting` event says when to ask again, as before.
- **The window adds what streams.** A face shows the most pressing of what the chat as it streams says (an approval or a question waits, a turn runs) and the service's answer. The answer is a moment behind: it never makes an agent Working with no turn running here, nor Free with work of its own to come, and a turn running here outranks all it said but Needs you.
- **Where.** On every agent face of 24px and up, and on the top line's faces at 20, cut out of the surface it sits on by a ring of that surface's colour, so it reads on any face background. One `Face` component draws it, given the state.
- **The top line shows every agent of the chat**, in the chat's order, 6px apart so no face covers a mark. It is still Summary's button. A team holds 1 to 5 agents, so no overflow is drawn.
- **Working turns only on the top line and on the agent's latest message;** the same ring is still on older messages, Summary's rows, the panel and the card. Under reduced motion it is still everywhere.
- **The card.** Pointing at a face, or keyboard focus on the button that holds it, shows its card: the name (the Supervisor's role after it), then the state in the mark's shape and word, what, and since when: "Can't run · Failed · Show cook times · 1m", "Working · Show cook times · 4 steps · 38s". Free says only its word. It is information, so it shows on hover; it is the conversation map's card (Base UI's preview card), one per chat. One state only: no second line.
- **A chat's row** in the sidebar shows the chat's most pressing of Needs you, Can't run and Working, worked out by the service with the list (`mark`), still, after the cut title.
- **Edge cases.** A paused chat: every agent Stopped. An archived chat: no marks anywhere. An agent no longer in the chat: no mark, off the top line, its card says Removed ([0069](0069-deleting-a-project-or-an-agent.md)).

## Why

The owner's ask: every agent's state at a glance, on its face, the same everywhere, with the detail one point away. Before, the window worked out four standings and the service said `waiting` for a sign-in, a usage stop, a stop and a pause alike; a plan waiting on the person, a blocked or failed task and a failed or stopped last turn read Ready.

## Turned down

- **A second state on the card ("Also").** The status line's list (a later change) says each agent's one state; one state per agent stays simple.
- **Working turning on every face.** Many turning rings in a long chat pull the eye from what is happening now.
- **A colour per state on a sidebar dot.** The row uses the faces' own marks, so a state has one shape everywhere.
- **"+8" for many agents on the top line.** A team holds at most five.
