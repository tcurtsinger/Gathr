# 0082: A message reads as SerenityOS's card

Status: accepted (owner, 2026-11-19: "How SerenityOS structures / draws each message is basically what I am looking
for too"; asked where narration shows, "Inside the steps"; and on the text, "can we do whatever SerenityOS does to make
the formatting better?"). Date: 2026-11-19.

Built: yes. Proof: `ui/src/chat/turn.test.tsx`, `ui/src/chat/cardLive.test.tsx`, `ui/src/chat/state.test.ts`,
`ui/src/chat/nameLine.test.tsx`, `crates/gathr/tests/plans/check.rs`, `crates/gathr/tests/plan_flow.rs`,
`crates/gathr/tests/steps.rs`, `crates/gathr/tests/words.rs`.

Ports SerenityOS's turn card (`desktop/src/components/RoomTurnCard.tsx`, added 2026-10-19) and message text
(`desktop/src/components/Markdown.tsx`, added 2026-10-19, and the sizes in `desktop/src/styles.css`), the owner's own,
in Gathr's own code. Amends [0078](0078-said-to-you-is-main-one-bubble-per-turn.md) (every piece in order) and
[0080](0080-the-team-checks-the-leads-answer-in-the-open.md) (the check's words, what the last word carries). Serves
promise 6 of [the charter](../charter.md#the-promises).

## The choice

- **A turn's message** is the words it wrote after its last step. Words written before a later step are narration.
  Gathr's tools that only report or place (`react`, `finish_task`, `post`) are no step for this, and neither is a
  thought under a second, which isn't drawn. A finished turn that wrote nothing after its last step has its last
  words as its message. One rule in two places, each with its tests: the window (`ui/src/chat/fold.ts`, which must
  apply it live, before a turn ends) and the service (`gathr::open_check`, for the last word). The service doesn't
  mark messages: a saved text becomes narration only when a later step arrives, so a mark would need rewriting.
- **The bubble** draws the message as Markdown, a plan version's chip where it was written, a question and a findings
  chip as before, and at its foot **one steps line**: every step and every piece of narration in order, closed to
  what the turn did counted by kind ("Read 1 file, ran 2 commands"; a thumbs-up and finishing a task aren't counted),
  open to each in order, narration grey (`work-say`). While the turn runs it is the working line ("Working · 12s");
  the agent's newest words show as words above it, staying there through the steps that follow until newer words
  come, and under "Working" the line shows its latest step. (As first built the newest words moved into that line
  once a step followed; the owner, the same day: "I have to decipher out of the shimmering stuff".) The bubble shows
  from the moment the turn runs and never blanks.
- **The name line**: the name, then the agent's title in grey, then the time, New, and once its turn is done a tick
  and how long it took ("14s", "4m 12s"), from when the turn was made to when it ended, as the working line counts.
  Gathr keeps no title of an agent's own, so the title is its role in a team: "Lead" and "Reviewer"; a member's says
  nothing, and nothing in a chat of one. History and the live `turn` event carry `ended`.
- **`@Name`** of an agent of the chat in an agent's words reads as the blue mention a message to an agent opens with
  (0072): a remark plugin passed to the registry's Markdown from `components/Markdown.tsx`, case-insensitive,
  longest name first, never in code or a link. The registry files stay as published.
- **A quote** in an agent's words: a 2px rule at its left at a quarter of the text's colour, upright, at 70% of it.
- **A message's text** takes SerenityOS's proportions in Gathr's palette (`markdown.css`, `said.css`): 15.5px at 400
  with a 1.65 line height (a reply in a thread 15px, the ratio it had); a heading of any level a section label,
  semibold, a step larger (1.0625em), 16px clear above, never a page title; blocks 10px apart; its own 0.4em dot for
  a bullet, the text 10px after it, each level in by 1.1rem; a number in the muted colour, 8px before its text; bold
  at 600; inline code a soft chip (7% of the foreground, 0.84em, 4px corners); a code block's code and a table at
  13px, a table's cells 10px by 6px and its header row tinted and semibold; links underlined 2px under.
- **The check's words** (`check` version 2): the thumbs-up is the whole check, "so write nothing, not even what you
  checked", and the answer is named `answer` for `react`, never by its id, which an agent copied into its prose.
  `react` takes `answer` in a check turn (`gathr::open_check::answer_of`), refused in any other turn. The lead's last
  word (`last_word` version 2) carries each objecting member's message by the rule above, not its narration, and no
  message id either. Untried on engines, as 0080's words were.

## Why

- The owner ran the first real open check: agents open with narration ("I'll check the tests…") before their message,
  and Gathr drew every piece in order, so the narration read as the message and reached the lead in its last word.
- One member gave the thumbs-up and still wrote two messages restating what it checked; one put a raw message id in
  its prose because the words named the id for `react`.
- SerenityOS's long messages read in sections; Gathr's ran together at one weight and spacing.

## Turned down

- **The service marking each text as message or narration**: the mark would change after it was sent.
- **The duration from the first step** rather than when the turn was made: it would leave out the engine's first
  wait, and the working line already counts from when it was made.
- **The duration at the far right of the column**: Gathr's name line spans the column above a bubble that fits its
  words, so it would float far from a short message. It ends the name line's pieces instead.
- **A field or setting for an agent's title.**
- **SerenityOS's heading sizes for a document read on its own** (its `doc` mode): a chat bubble has no document.

## Said plainly

- **A search hit on narration** opens the bubble, but the words it found are inside the closed steps line.
- **The lead's title is "Lead"** on the name line, as the brief asked, while the face's card still says "Supervisor".
- **Not verified:** how real engines follow the new check words; the window was looked at only in the pictures made
  for the PR, at 1280 wide.
- **Old chats** read by the same rules; nothing is migrated.
