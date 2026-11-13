# 0039. A conversation map on the chat's left edge

Status: accepted (owner, 2026-11-11: asked for assistant-ui's conversation map on the left, on the chat
redesign's approved design; `@base-ui/react` approved with it; the rail's length and centring the same day).
Date: 2026-11-11.

Built: yes. Proof: `ui/src/chat/map.test.ts`, `ui/src/chat/ChatMap.test.tsx`.

## Choice

- **assistant-ui's ConversationMap, as published** (`elements-conversation-map`, recorded in
  `ui/registry.json`): a 24px rail of ticks on the left edge of the chat pane, just inside navigation.
  Each tick is a button with a hover card, which also opens on keyboard focus and closes on Escape. Up,
  Down, Home and End move along the ticks, and Enter opens one.
- **Its length is capped and it is centred.** The rail is at most half the height of the chat pane below
  the header, and never more than 616px (44 ticks at 14px). It is centred in that pane: the space above
  it equals the space below it, measured to the pane's bottom edge, not to the composer. Fewer ticks make
  a shorter rail, each tick 14px at most.
- **What gets a tick:**
  - a long tick for each message from the person;
  - a short tick for each reply the lead wrote to the person: the last words of each of the Supervisor's
    own main-line turns, once the turn has ended. In a chat with one agent, of each of its turns;
  - nothing for another agent's turn, a work row, a message between agents or an event line. A turn that
    ended without words has none, and a running turn has none yet, so a streaming reply never redraws
    the map.
- **Never cramped.** Ticks keep at least 6px each, so the rail holds its capped length divided by 6px.
  Past that, the oldest calendar days fold first, each into one thick tick ("Monday · 14 messages from
  you"; the date, "3 Nov", once the day is a week old) that opens that day's first message. The chat's
  latest day, today's as a rule, keeps its own ticks. If that day alone is too many, its short ticks go
  and the person's stay. If even those are too many, the oldest fold into one thick tick, "Earlier · 60
  messages from you", so the whole chat stays in reach. The fold is worked out again when the pane's
  height changes.
- **The card** says who and when ("You · 16:04", "Orin · 16:05", the time as the chat shows times) over
  the message's first lines as plain text: Markdown's marks dropped, anything in angle brackets kept, as
  the chat shows an agent's HTML as text. A day's card has its
  title and "Jumps to the start of that day". The title is the tick's accessible name and the preview
  its description.
- **Where the chat is.** The current tick is the latest ticked message at or above the middle of what
  shows above the composer; at the end of the chat, the latest one on screen. Ticks whose messages are on
  screen are brighter. Messages not drawn (0030 draws the latest 100 and older pages on scroll) count as
  further up.
- **A tick opens its message** as a search hit opens (0035): the pages up to it are drawn, it is put at
  the top of the view and the view holds there until the person is back at the bottom or sends. It is not
  tinted, and focus stays on the tick. Whichever was picked last wins, a tick, a search hit or the
  dock's Show, and an older one is dropped for good. `gathr.ui.map.open` times it; `gathr.ui.map.error`
  records a message that could not be drawn.
- **When it shows.** From three ticks up, and only while the pane has 44px left of the column: the
  rail's 14px inset, its 24px and 6px before the column. It never lies over the text, the composer, the
  approval dock or the scroll-to-bottom button, which all sit in the column. Only the ticks take the
  pointer: the wheel over the rest of the rail scrolls the chat.
- **One new dependency:** `@base-ui/react` 1.9.0 (MIT), pinned exactly. The published component builds
  its hover card on Base UI's PreviewCard, and the registry file stays as published (0030).
- **Kinds are marked from outside.** The published rail draws every tick alike and gives a tick no mark
  of its own to tell a message from a reply or a day. `ChatMap` sets `data-kind` and `aria-description`
  on each tick after the rail draws, in the entries' order, and `map.css` styles length and thickness
  from the kind. A test fails if the long and short ticks stop measuring 12px and 6px, or a day's 4px.

## Why

- A long chat has no landmarks. In the owner's real chat, 57 messages from the person and 432 agent
  turns over about 32 hours, finding "where I asked about X" meant scrolling through all of it.
- The person's own messages are the landmarks they remember. The lead's replies are what they came
  back to read.
- One tick per turn in that chat would be about 1.5px each on a 700px rail; one per message from the
  person is about 12px each.
- Folding by day keeps the recent part of the chat exact and the older part reachable in one click.
- Centred between the header and the composer, a full rail left a gap below it and none above. Capped
  and centred in the pane, the space above and below it is the same.

## Turned down

- **A tick per agent turn.** 432 ticks in the chat above: unreadable, and most are work the person never
  asked to see.
- **A tick every N messages.** It lands nowhere meaningful: neither a question nor an answer.
- **The right edge.** It would sit beside the scroll bar and under the workspace when that is open.
- **A rail the whole height of the pane, or centred between the header and the composer.**
- **A tick for a reply while it streams.** Its preview would change with every token and redraw the rail.
- **Editing the registry file to mark kinds.** An adaptation to carry through every update, for what a
  few CSS rules and two attributes do from outside.
- **Writing our own hover card on the existing tooltip.** The published component would have to be
  edited to use it.

## Known limits

- **Resting ticks are faint, as the owner approved them.** Measured against the chat's background: a
  reply's tick 1.5:1 in dark and 1.4:1 in light, a message's or a day's 1.9:1 and 1.7:1. On screen they
  are 4.4:1 and 3.7:1, the current one 11:1 and 16:1.
- **A former lead's replies lose their ticks when the lead changes.** Only the present Supervisor's
  replies are short ticks; an agent that left the chat has none.
- **A card near the top can lie over the header.** Base UI places it beside its tick and keeps it inside
  the window, not below the chat header, and the published component passes no setting for that.
- **A page search (Ctrl+F) still finds only what is drawn** (0030); the map does not change that.
