# 0072: The chat reads like a team chat

Status: accepted (owner, 2026-11-17: the team-chat mock-up's look D, with "Tuned" text, Steps "Hover", Asked "Reply", and the person's own bubble tinted blue). Date: 2026-11-17.

Built: yes. Proof: `ui/src/chat/TeamChat.test.tsx` (a reply between agents under the message it answers, and the thread folding; the @Name a message to an agent opens with, and none when it quotes; a thumbs-up's count and who gave it on hover or focus; a turn's steps on its name line on hover or focus, kept for a failed step and a working agent), `ui/src/chat/messages.test.ts` (where a thread sits and which turn's steps go on a message), `ui/src/plans/plans.test.ts` (a check that agrees is the checker's thumbs-up), `ui/src/chat/Streaming.test.tsx` (a streamed reply draws only itself again), `ui/src/components/Markdown.test.tsx` (the tuned spacing), `ui/src/chat/Review.test.tsx` (a review and its fix under the PR's message, open while it blocks, folded once settled; still its reviewer's on the panel), `ui/src/chat/picks.test.tsx` and `crates/gathr/tests/picks.rs` (a picked name blue in the sent message, live, from history and after a failed send; a typed one plain). How a bubble looks is proved by pictures.

Amends [0037](0037-one-timeline.md): the one timeline stands, drawn as bubbles. What a turn did is on its message's name line, not a row above it; agents' messages to each other are threads, not a tinted block headed "Juno to Orin". Amends DESIGN.md: a message is no card, and blue also tints the person's own bubble and an @Name.

## The choice

- **Every message in a bubble beside its author's face, everyone on the left.** The person's words too, with a face of their own ("Y"), in a bubble tinted blue (the attention colour at 22% over the chat) so they are found when scrolling back. An agent's bubble is the foreground at 8% over the chat. A bubble fits its words; one with a table takes the column. A message with no words yet shows no bubble: the face sits beside the name line, and nothing empty shows.
- **One name line,** in one size and colour after the name: Supervisor for the lead of a team, the time, the task a member's turn works on, and a tick once that task is done. A turn's working state is its face's mark ([0071](0071-an-agents-state-is-one-mark.md)), never a word here.
- **A turn's steps and time taken are the end of its name line,** shown on hover or keyboard focus of the message, and always for a failed step, an agent still working, steps opened, or a block with no words, whose steps are all it has (else it would read as an empty message). They open under the name line.
- **Talk between agents is a thread under the message it belongs to.** A reply answers the message Gathr carried into its turn from the agent it writes to, and joins that message's thread; a message that answers none sits under the block its turn was writing when it sent it. The thread is a thin line with smaller faces, and folds to its faces and how many replies. A turn that only talked has no block of its own: its steps go on its message.
- **Work on a pull request is a thread under the PR's message** (the approved chat-lines mock-up's "Thread"): a review's turns and later work on the same PR (its fix) sit under the last message of the PR's own task (its earliest work task not cancelled, as the service names the builder) drawn before them, each whole, with its name line, its task and its tick, followed by any talk it began. The thread is open while anything in it is open, and folds once every task in it is done or cancelled; one settled when the chat opens starts folded. Without the PR's message drawn, the work stands where it began.
- **A message to an agent opens with a blue @Name;** when something came between it and what it answers, it quotes those words plainly instead, and names nobody.
- **A thumbs-up is the emoji and a count;** who gave it shows on hover or focus, as names. A plan's check that agrees is that thumbs-up on the version, with no line of its own.
- **The text is tuned for the bubble:** 15px at 450 and 1.45, paragraphs 10px apart, list items 3px, inline code 0.88em.

## Why

- With one timeline (0037) the team's talk was there but read as a log: a name only when the speaker changed, every agent the same, and talk between agents a card of "Juno to Orin" lines. In a bubble beside its face, who said what is seen, not read, as in the team chats people already use.
- The steps were a row above every message, the busiest line of the chat; on the name line, shown on hover, the words lead and the work is one point away. A failed step or an agent at work stays in view, as that is when they matter.
- A thread keeps a question and its answer together where the work they are about is, whatever happened in the chat between them.

## Turned down

- **Look A,** today's rules with more structure: every message named, a line between messages. Taller, and who is talking is still read rather than seen.
- **Look B,** a Slack-like thread with a coloured rail per agent and the steps under the words: no bubbles, and the steps always shown.
- **Look C,** a card per message as SerenityOS draws it, with a job and a footer of what it ran and edited: the tallest, and it needs a job for each agent and the files each turn changed, which Gathr doesn't keep.
- **Steps always shown,** under the words, in the name line or above the bubble: each puts a line of work on every message.
- **An asked question as a line, or inside the asking agent's bubble with the answer there:** the approved shape is the question in the agent's bubble and the answer as the person's own message, which comes with questions answered in the chat, a later change.
- **A colour per agent:** the drawn faces on the background the person picked ([0074](0074-an-agents-face.md)) tell agents apart, and blue stays the person's.

## What this leaves

Gathr speaking as itself, and questions answered in the chat, how a turn or a task ended, and an ask's size on the ask stay as 0037 and their own decisions draw them, inside the new bubbles, until their own changes. In the person's sent message, each name they picked from the `@` list stays blue, as it was in the message box (the owner, 2026-11-18: "The name of the agent the message went to stays blue in your sent message, as the mock-up drew it. A name you only typed, without picking it, stays plain."). The picks are kept with the message ([0028](0028-team-chats.md)) and only those spans are blue, in the box's colour; a name only typed stays plain, and so does every name in a message saved before picks were kept. A picked agent renamed or removed since keeps its blue on the name as it was sent.
