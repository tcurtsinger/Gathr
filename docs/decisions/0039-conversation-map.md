# 0039. A conversation map on the chat's left edge

Status: accepted (owner, 2026-11-11: asked for assistant-ui's conversation map on the left, on the chat
redesign's approved design; `@base-ui/react` approved with it). Date: 2026-11-11.

## Choice

- **assistant-ui's ConversationMap, as published** (`elements-conversation-map`, recorded in
  `ui/registry.json`): a 24px rail of ticks on the left edge of the chat pane, just inside navigation,
  centred between the header and the composer. Each tick is a button with a hover card. Up, Down, Home and
  End move along the ticks, and Enter opens one.
- **What gets a tick:**
  - a long tick for each message from the person;
  - a short tick for each reply the lead wrote to the person: the last words of each of the lead's own
    main-line turns, or of each turn in a chat with one agent;
  - nothing for a member's turn, a work row, a message between agents or an event line. A turn that ended
    without words has none either.
- **Never cramped.** Ticks keep at least 6px each. When the rail is too short for that, the oldest calendar
  days fold first, each into one thick tick ("Monday · 14 messages from you"; the date, "3 Nov", once the
  day is a week old) that opens that day's first message. The chat's latest day, today's as a rule, keeps
  its own ticks. If that day alone is too many, its short ticks go and the person's stay. If even those
  are too many, the oldest fold into one thick tick, "Earlier · 60 messages from you", so the whole chat
  stays in reach. The fold is worked out again when the pane's height changes.
- **The card** says who and when ("You · 16:04", "Orin · 16:05", the time as the chat shows times) over
  the message's first lines as plain text. A day's card has its title and "Jumps to the start of that day".
- **Where the chat is.** The current tick is the latest ticked message at or above the middle of what
  shows above the composer. Ticks whose messages are on screen are brighter. Messages not drawn (0030
  draws the latest 100 and older pages on scroll) count as further up.
- **A tick opens its message** as a search hit opens (0035): the pages up to it are drawn, it is put at
  the top of the view and the view holds there until the person is back at the bottom or sends. It is not
  tinted, and focus stays on the tick. `gathr.ui.map.open` times it; `gathr.ui.map.error` records a
  message that could not be drawn.
- **When it shows.** From three ticks up, and only while the pane has 44px left of the column: the
  rail's 14px inset, its 24px and 6px before the column. It never lies over the text.
- **One new dependency:** `@base-ui/react` 1.9.0 (MIT), pinned exactly. The published component builds
  its hover card on Base UI's PreviewCard, and the registry file stays as published (0030).
- **Kinds are marked from outside.** The published rail draws every tick alike and gives a tick no mark
  of its own to tell a message from a reply or a day. `ChatMap` sets `data-kind` on each tick after the
  rail draws, in the entries' order, and `map.css` styles length and thickness from it. A test fails if
  the long and short ticks stop measuring 12px and 6px, or a day's tick 4px.

## Why

- A long chat has no landmarks. In the owner's real chat, 57 messages from the person and 432 agent
  turns over about 32 hours, finding "where I asked about X" meant scrolling through all of it.
- The person's own messages are the landmarks they remember. The lead's replies are what they came
  back to read.
- One tick per turn in that chat would be about 1.5px each on a 700px rail; one per message from the
  person is about 12px each.
- Folding by day keeps the recent part of the chat exact and the older part reachable in one click.

## Turned down

- **A tick per agent turn.** 432 ticks in the chat above: unreadable, and most are work the person never
  asked to see.
- **A tick every N messages.** It lands nowhere meaningful: neither a question nor an answer.
- **The right edge.** It would sit beside the scroll bar and under the workspace when that is open.
- **Editing the registry file to mark kinds.** An adaptation to carry through every update, for what
  three CSS rules and one attribute do from outside.
- **Writing our own hover card on the existing tooltip.** The published component would have to be
  edited to use it.

## Known limits

- **A card near the top can lie over the header.** Base UI places it beside its tick and keeps it inside
  the window, not below the chat header, and the published component passes no setting for that.
- **A page search (Ctrl+F) still finds only what is drawn** (0030); the map does not change that.
