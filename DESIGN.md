---
name: Gathr
description: A desktop conversation workspace for a team of agents.
tokens: designs/tokens.css
colors:
  frame: "#232323"
  background: "#1b1b1b"
  foreground: "oklch(.95 .002 106)"
  card: "#2c2c2c"
  popover: "#2f2f2f"
  primary: "oklch(.922 .002 106)"
  primary-foreground: "oklch(.215 .003 106)"
  secondary: "#2c2c2c"
  muted-surface: "#2c2c2c"
  muted-foreground: "oklch(.74 .004 106)"
  accent-surface: "#383838"
  destructive: "lab(63.7053% 60.745 31.3109)"
  border: "oklch(.985 .002 106 / 12%)"
  input: "oklch(.985 .002 106 / 15%)"
  ring: "transparent"
  attention: "#60a5fa"
typography:
  body: { fontFamily: "Hanken Grotesk Variable", fontSize: "15px", fontWeight: 450 }
  composer: { fontFamily: "Hanken Grotesk Variable", fontSize: "16px", fontWeight: 450 }
rounded: { sm: "8px", floating: "16px", composer: "24px", action: "9999px" }
spacing: { xs: "4px", sm: "8px", md: "16px", lg: "24px", xl: "32px" }
components:
  composer: { backgroundColor: "{colors.popover}", rounded: "{rounded.composer}" }
---

## Overview

The assistant-ui React prototype is the visual and interaction reference. Adapt
its look to Gathr's service and accepted decisions. Its simulated data, routing,
streaming, account controls and task transitions are not production contracts.
Operate mode: variance 2, motion 2, density 5. One centred conversation, a project
sidebar, a composer and a resizable workspace. No Rooms or Overview screen.

The source record and component map are in [prototype-reference.md](designs/prototype-reference.md).
The portable reference is [system.html](designs/system.html). It demonstrates
tokens and static component anatomy, not replacement runtime components.

## Colors

Surfaces take four steps. `shell` is the window's own surface, under the rail and the top line and
around the frame ([0070](docs/decisions/0070-the-windows-shell.md)): #2c2c2c in dark, a grey darker than the
frame in light, with `shell-edge` (7% foreground) as the frame's 1px ring. `frame` is navigation and the
workspace. `background` is the chat. `card` is what stands on the chat: an approval, a
notice. A message is no card: it stands in a bubble, the foreground at 8% over the chat
([0072](docs/decisions/0072-the-chat-reads-like-a-team-chat.md)). In dark the chat is darkest and the frame lighter ([0068](docs/decisions/0068-the-windows-colours.md)):
chat #1b1b1b, frame #232323, card #2c2c2c, with floating surfaces
(the composer, popovers, the dialog, Help) at #2f2f2f. `raised` and
`danger-surface` are tints, so they lift whatever they lie on, and
`accent-surface`, a menu's highlight, is lighter than the popover. Light is a white chat with a
light-grey frame and cards and a border dark enough to see. Each step differs
from the next in both themes. `tokens.css` records both themes. Foreground carries text and primary
actions. `muted-foreground` is metadata; `muted-surface` is a background.
`border` separates regions; `input` bounds inputs; `ring` is transparent.
Blue is attention, links and active work, not an agent palette; on the owner's word it also tints
the person's own bubble (22% over the chat) and draws the @Name a message to an agent opens with and
the names the person picked in their own message (0072). `free` (dark `oklch(0.74 0.17 148)`,
light `oklch(0.54 0.15 148)`) is the Free mark's green alone: green means Free and nothing else
([0071](docs/decisions/0071-an-agents-state-is-one-mark.md)).
`os-close` is Windows' own Close red, the same in both themes, under the pointer on the window's Close
alone.
Errors combine destructive text, a tinted surface, an icon and a reference.

The prototype calls its neutral surface tokens `muted` and `accent`. Existing
Gathr code uses those names for text. Here they are `muted-surface` and
`accent-surface`; the old names remain compatibility aliases to readable text.
The runtime adapter maps these two names back for assistant-ui. Do not paste the
prototype's conflicting names into the existing UI. Other aliases: canvas to
background, surface to card, text to foreground, danger to destructive.

## Typography

Hanken Grotesk Variable, self-hosted under the SIL Open Font License 1.1, then
Hanken Grotesk and the platform sans-serif. The unmodified Latin variable file
comes from `@fontsource-variable/hanken-grotesk` 5.3.0; the face exposes weights
300 to 700. Other scripts use fallback fonts. No remote font requests.
Replies 16px at weight 400 and line-height 1.5 (15px in a member's block and an
exchange); composer 16px; navigation and agent names 14px; text actions 13px;
metadata 12 to 13px monospace, except agent roles, which use the normal sans-serif
at 12px. Headings remain 26px for a new chat and 20px for a panel. Elsewhere the
base weight is 450 and body line-height 1.6; medium 500 and semibold 600 stay
unchanged. Code stays monospace. In a reply (the owner's call, 2026-11-12):
paragraphs 16px apart, list items 6px apart, bullets hanging so a wrapped line
lines up with its text, headings 20px/600, inline code 14px/500 on its fill. The text scale moves 11 to 12, 12 to 13,
13 to 14, 14 to 15 and 15 to 16px; existing larger headings keep their sizes.

In message headers, the name and role share a flex wrapper with baseline
alignment and an 8px gap. The avatar stays vertically centred against that
wrapper. Message text keeps its 34px indent. Do not centre the smaller role
against the name. The [typography check page](designs/typography/v1.html) shows
the workspace at the new scale. It was drawn before 0037 and 0038, so its split, plan bar and
Summary are history.

## Layout

- The window is at least 960 x 600; nothing is designed below that.
- The window's shell (0070): a 44px rail at the left and a 40px top line above, both on `shell`,
  around one frame that holds navigation, the chat and the workspace. The frame is rounded 12px at
  every corner, ringed 1px in `shell-edge`, with 6px of shell on its right and below. The desktop
  window has no system title bar: the top line is the whole band of shell above the frame, and a press
  on any part of it that isn't a control moves the window, a double click maximizes or restores it.
  On Windows its right end holds the window's three buttons, Minimize, Maximize (Restore while
  maximized) and Close, drawn as Windows draws them: each 46px wide and the band's 40px high, 10px
  glyphs at 1px stroke in the foreground colour, a foreground 7% tint under the pointer or keyboard
  focus, Close in `os-close` with a white glyph; they reach the window's top right corner, past the
  shell's margin, and the line's content ends 8px before them. On the Mac the system draws its own
  three buttons at the band's left, over the rail and the sidebar, where the line holds nothing. In a
  browser the line moves nothing and draws no buttons. The top line stays live over a dialog: the
  shared Dialog holds the same band, fixed over the window's top 40px, drawing nothing over the
  backdrop but Windows' three buttons at the same pixels, out of the dialog's keyboard order. Gathr's
  "Start failed" page keeps the system's title bar.
- The rail: Home at the top, a mark of where you are (the window has one place, so it is not a button),
  drawn as current (32px, 8px radius, foreground 9%), its centre on the word mark's line. At the foot,
  6px apart: Update, in the attention colour, only while a release is ready (a turning loader, muted,
  while it installs), above the Settings cog. Icons 18px at 1.75 stroke; Update 20px at 2.
- Navigation starts at 246px, resizes from 190 to 400px, and uses 190px below 760px (a browser
  only). With the workspace open in a narrow window it gives way to leave the chat and the
  workspace 360px each, never under 190px. Widths in the frame (the window less 50px), sidebar /
  chat / workspace: 246 / 686 / 458 at 1440, 246 / 542 / 362 at 1200, 230 / 360 / 360 at 1000 and
  190 / 360 / 360 at 960.
- The top line, from the sidebar's edge: a short divider on that edge (12px in from top and bottom),
  20px, the title, then More 4px after it in the same group; the title truncates with an ellipsis and
  More stays visible. The right holds view controls only: Summary, then the workspace's button, open or
  closed, 8px from the frame's end (from the window's buttons on Windows). These share one icon-button style: 28px, 8px radius, 4px apart,
  17px Lucide icons at 1.75 stroke (Summary's own icon too; More's dots radius 1.4). Summary's button
  widens to hold its stack of faces, 8px each side, at the same height. Pressed (the workspace's button
  while it is open): foreground 9% background, foreground icon, 12% on hover. The chat has no header of
  its own: the thread starts at the frame's top.
- Thread column is 42rem including 16px inner padding, leaving 640px (40rem) of text at
  the normal 16px root size (0037). Composer shares that content width. New chat centres
  its heading, optional project notch and composer.
- Workspace starts at 40% of available width, initially clamped to 360 to 760px.
  Resize minimum is 360px. It always sits beside the chat, never over it, and leaves the
  chat at least 360px: when the window narrows, the workspace gives way first, down to
  360px, then navigation. Only the drawn widths give way; widths the person dragged come
  back when the window widens. Its tabs sit in a 38px row of their own at its top, tabs 28px high.
- A document's adjacent file tree starts at 240px, resizes from 160 to 480px and
  leaves at least 200px for the reader. Reader padding is 24px, 20px narrow, or
  16px around highlighted source.
- Human task content and its pinned Back/Next/Send footer share a 600px column.
  Settings is the explicit layout exception described below.

ResizeHandle supports pointer capture, 10px arrows, 40px Shift+arrows and Home/End.
Maintain native scrolling, a thin themed rail and independent scrolling regions.
Desktop checks are at 1440 and 1024px, light and dark. Small screens reflow
controls; they do not create a second mobile product.

## Elevation

Flat chat and navigation. Floating popovers use a tonal surface and a soft shadow.
Summary anchors 8px below and 8px inside the top line's right end, independently of
workspace width. Its width is 348px, padding 8px, radius 16px and maximum height
560px bounded by the viewport. Navigation flyouts have no dimmed backdrop.
Blocking Settings uses the shared backdrop. Layers must not clip focused controls.

## Shapes

Text actions use the prototype's shared 32px pill Button. Icon controls keep their
contextual square or circular shape. Small controls use the 8px base radius;
floating surfaces 16px; ComposerBar 24px with 10px padding. Every message, the person's and
an agent's, is a bubble with 18px corners and 8px 13px padding that fits its words (0072).

## Components

Use actual assistant-ui primitives and the recorded official registry components
after the integration decision is approved. Do not build lookalikes in `ui/`.
The prototype gallery contains 64 examples, not 64 new Gathr service capabilities.
`ui/registry.json` records each adaptation with its original and adapted hash and why:
ElicitationForm, OptionList, Button, Tooltip and TodoList today. Retain source
notices and both hashes.
Dependency approval, pinned versions and file splitting belong to the integration
decision; this design update adds no JavaScript dependency.

### Conversation and composer

Compose ThreadPrimitive, MessagePrimitive and the shared ChatComposer. The full
upstream Thread has no Composer override. ChatComposer uses the modular Composer,
attachments, mentions, slash actions, selection quoting and Send/Stop through the
runtime adapter. There is no recipient picker (C24, 0028): the box's hint names who
gets a message that names nobody, "Message Mira" (the Supervisor, else the chat's
agent, else a new chat's first agent). Typing @ at a word's start, anywhere in the
message, opens the composer's one menu above the box's left edge: a 24px face with
its mark, the name, and the role muted in a chat of two or more; in a chat with no
project, every other agent follows under "Bring in" (12px muted, a rule above), up
to five in the chat. Up and Down move the lit row, Enter or Tab picks, Escape
closes it, and a click picks; the lit row has no Enter mark. The name goes in as
"@Name ". Only a name picked from the list counts, as @ and / work in other apps'
boxes: a typed @Ann with nothing picked is plain text. The box keeps each picked
name as a range of its text with its agent, through typing before and after it,
pasting and a draft kept while another chat is open; an edit inside it makes it
plain. Picked names show in the attention blue at the text's own weight: the box's
text is drawn again in one piece under the see-through textarea and the names
coloured by a highlight (a span would move the letters after it off the
textarea's by up to 1/16px). Once something is typed, a quiet sign by Send says
who gets it, the first picked name else the hint's agent, in 12.5px muted with the
name in the foreground at 550 and a bare 20px face: "To Ann", or "Bo joins" for an
agent picked from outside the chat. There is no Everyone option. Sent, the message
keeps its picked names, and its bubble draws each in the same attention blue at the
text's own weight (one rule, `picked.css`, for the box and the bubble); a typed
@Ann stays plain, as does every name in a message from before picks were kept.

Streamdown is the rendering reference, pending integration approval. Keep user
messages plain text; render agent Markdown, lists, headings, quotes, code, tables,
sources, files and tool results. Wide code and tables scroll inside the reply.
Raw HTML policy, link behavior and Diagnostics references stay governed by Gathr.
Tools are collapsed where they happened. Every message that can have one has one row under its bubble (`chat/Foot.tsx`,
0076, the owner's call: "They live in a dedicated spot under the bottom right of the bubble. On the same row as the
replies and reactions"), the same on main messages, replies, a chat of one agent and messages between agents. Left to
right: a main message's fold line when it has replies, the thumbs-up count when it has one, then at the right
MessageActions (`ui/src/components/MessageActions.tsx`): Reply (a reply arrow, named "Reply"), then the person's like
(a thumbs-up named "Like", pressed: the attention blue), 24px icon buttons with 15px icons, muted until pointed at,
icons only, no hover text. The buttons show while the message is pointed at or holds keyboard focus (by opacity, as
the steps are, so Tab reaches them) in a spot the row always keeps, so nothing above, below or beside moves when they
show. The bubble and the row share one box as wide as the wider of the two: the buttons end at the bubble's bottom
right edge, or, under a bubble narrower than the row's own content, right after the fold line and the count. Nothing
of the row covers the bubble's words, its name line, a state word, a steps button or the next message: it sits under
the bubble, 2px below it, 24px tall. A message with a row gives back 14px of the gap after it, so from a bubble's
bottom to the next name line is 32px after a main message and 26px after a reply in a thread (20px and 14px before,
with no row); a main message with replies is 26px from its first reply (48px when the fold line was a row of its own),
a reply with a count 32px from what follows (51px). Like: an agent's words, a message between agents, a bubble holding
a waiting question with the asker's pick, and a plan version waiting for the person offer it; the person's own
message, Gathr's lines and question, a bubble holding a waiting question with no pick, and words still streaming
offer none (0057 C4). Reply (one rule, `chat/answer.ts`): an agent's message, a message between agents, a plan
version, a task's message, a waiting question's bubble and a line of Gathr's offer it, and on a main message it
answers the main message as its thread's (a task's message: its owner; the person's own ask, once it has replies:
the lead); the person's other messages, their answers, Gathr's own question and a removed agent's message offer none.

The thread box (0076, `#replybox` A) is the same Composer, placed by `chat/ThreadBox.tsx` as an item after the
thread's last reply shown, inside the thread's rule where its replies sit: one row, 18px corners, 14.5px words, the
"To Ann" sign, a 24px cross named "Close" and a 28px Send, the field still growing with what is typed; where the
field would get less than 200px (the 360 column), the sign, the cross and Send go under it, at its right. Its field is
named "Reply" and so is its hint. Focus goes into it when Reply opens it; Escape (after an open @ list) or the cross
closes it and focus goes back to that Reply. It has no Stop. One at a time: Reply elsewhere moves it, and each
thread keeps its own draft.
Scroll-aware canvas fades appear only beyond the visible top/bottom edge,
pass pointer input through and leave the scrollbar clear.

### Team messages: main messages and threads

A team chat is main messages, each with its thread
([0075](docs/decisions/0075-a-team-chat-is-main-messages-each-with-its-thread.md), the threads mock-up's `#day`,
`#roots` A and `#result` A). A main message is something put to someone that starts a piece of work or needs the
person: the person's own message, each task the lead hands out, each version of a plan, a question to the person,
the result of an ask, and anything nothing ties. Main messages read in the order they were put; under each, its
thread: the answers, the agents' progress and talk, the PR, its review and fix, Gathr's lines about it. The service
says what each turn, task and line is about; the window never guesses. A chat of one agent is one timeline, as the
chat was before ([0037](docs/decisions/0037-one-timeline.md)), and so is what a chat said before it became a team.
The approved designs (Shared/designs) are a reference only: compose the existing registry components and Gathr
state in React; never port their HTML.

Inside a thread, and in a chat of one, a turn is one block where it began, in 0028's turn-start order between
turns, so replies streaming at once never interleave and a later reply lands in its own turn's block. Only a
person's message sent into a running turn splits that turn: one block before it, one after, and what the turn says
after it is about that message.

**The words are as wide as the message box** (the owner's rule): a main message's bubble starts at the box's left
edge and a full one ends at its right edge; its face hangs in a 48px gutter left of that width. Where the pane
leaves no room beside the column for the gutter (a narrow pane, the 360 column beside the side panel), the column's
left padding makes it and the box moves in with the words, so the two stay the same width at every size. The
conversation map shows only where it fits left of the faces.
Every message stands in a bubble beside its author's face, everyone on the left, as a team chat
reads ([0072](docs/decisions/0072-the-chat-reads-like-a-team-chat.md), the team-chat mock-up's look D):
the face 36px, 12px from the bubble, level with the bubble's top under the name line; the person's own
face is "Y" on the accent surface. The bubble fits its words up to the column: the foreground at 8% over
the chat, the person's tinted blue (the attention colour at 22%), 18px corners, 8px 13px padding, 15px
words at 450 and 1.45, paragraphs and what sits between them 10px apart, list items 3px, inline code
0.88em on a 6% tint, a code block on the chat's own surface. A bubble with a table takes the column, so
the table fills it as before. A message with no words yet (an agent at work) has no bubble: its face sits
beside its name line, and nothing empty shows. When anything else follows the name line (a notice, an
approval, open steps), the face sits level with it as with a bubble, 5px under the name line.

Above the bubble, one name line, 20px: the name at 15px/650, 8px, then every other piece in one size and
colour, 12.5px at 450, muted, tabular figures, with "·" between them 6px each side: Supervisor (the lead,
in a chat of two or more, on a main message; never in a thread, and not in the 360 column), the time, "Result"
on an ask's result, and for a member or reviewer "on" the task its turn works on, cut
with an ellipsis; not in that task's own thread, where the task's message says it. How the task ended is on its member's last message alone, after the task: a 12px tick once it
is done, else Cancelled (a circle with a slash) or Failed (a circle with !, in the danger colour) as words; blocked,
stopped and merging are the face's mark, never a tick. How a turn ended is a word after that, always shown, in the
words' colour: "Gathr restarted", or Gathr's reason when it ended the turn itself ("Needs plan", "Not needed",
"Usage limit", "Gathr ended it"); Failed, and Not sent on the person's own message, in the danger colour. The
person's own stop has no word: the face's mark says Stopped. Under the bubble, 13px muted, what can be done about
it: why, the reference (12px mono, dotted underline, opening Diagnostics) and one outline button 26px high,
Continue (until its carry-on starts), Retry or Sign in (0051). A block that follows its speaker's in the same
turn has no face or name line and lines up with the one above, unless it says how its task or turn ended. The face
opens the agent's panel in a team chat.

A turn's steps are the end of its name line: "· 10 steps · gave 2 tasks · 6m 40s · 1 failed ›", one
part omitted when it is zero, "1 failed" and "1 refused" in the danger colour, the time ticking while it runs. They show
while the message is pointed at or has keyboard focus, and always for a failed step, an agent still
working, steps opened or a block with no words (no text and no plan chip), whose steps are all it has; at rest the line holds its place. Click or Enter opens the steps under the name
line, above the bubble: each tool call as a step in plain words from the tool's name, Failed in the
danger colour on a failed one; a task it gave reads "Gave Cass a
task" with a "#119 Open the settings tools PR" chip that opens the task (a look-into task's chip reads
"#119 Look into: Survey tag libraries", 0063); what it wrote between
steps shows as muted lines. A step that kept what it ran
([0041](docs/decisions/0041-what-a-step-ran.md)) shows it in the same chip, in monospace: a
status icon, a short label, the chip ("Ran" then "git merge origin/main"; also
Read, Edited, Searched, Fetched, Searched the web, Ran a sub-agent). When the engine gave the
call its own words, they are the label ("Show what changed" then "git status --short"). The
chip is one line, cut with an ellipsis to the column it is in, never widening it, before the
label is cut; pointing at a cut chip, or focusing it (a cut chip is a tab stop), shows it whole in
the shared tooltip. A step that kept
nothing reads as before ("Ran a command"). A thought ([0042](docs/decisions/0042-thinking.md)) is a step
of its own, in order with the others: "Thought for 9s", with no chip and nothing to open; one under a
second is left out, and thoughts are not counted in "6 steps". Thoughts with nothing between them are one
step, with their time together. While a turn runs and its steps are closed, what the agent is doing now
(Running a command, Thinking) reads under its name line, 13px muted. The turn's last text is the
message. A turn that only wrote once has no steps, nor has one whose only work was a thought under a
second.

A main message says where it stands at its name line's right, 10px after the other pieces, which give way to it
(a long task is cut, never the state): the mark's shape still at 12px and its word, from the same function the
Tasks list reads (0071's marks). The person's ask: Working (the ring in the attention colour, the word in the words'
colour) or Done (a tick); Done is a link that scrolls to the ask's newest result. A task: its PR first ("PR #22")
when it has one, then its state as its row says it, Waiting saying on what when it waits on one thing ("Waiting on
PR #22 checks"). A plan version: Checking, Needs you, Working, Approved, Sent back or Dropped. A question: Needs you,
Discussing or Answered. Needs you is the attention blue at 550 with a 7px dot. Every mark's drawing sits 5px from its
word. A result and a turn nothing ties say none.

A task the lead hands out is drawn from its record, as no message carries the hand-out: the giver's face and name
(You for one the person gave) and the time it was given, then in a bubble as wide as the box the owner's blue
@Name and the task's title at 600 (a press opens it in the Tasks tab), and under it the first line of why it was
given, 14px at 78%, cut with an ellipsis, then "More" (13px muted, dotted underline) that opens the rest of why and
the brief's Done when, Proof, Where and Size in place; "Less" closes them. The lead's turn that gave it stays in
the ask's thread, its steps saying "gave 3 tasks".

An ask's result is the lead's own block, at the bottom, opening with a plain quote of the ask's first line (the
quote style below): a press on it scrolls up to the ask and holds it there, as a search hit does.

A main message with replies has one fold line, first in the row under its bubble (0076), 13px muted: a chevron
(turned down when open) and, folded, "1 reply" or "4 replies"; open, "Collapse replies". No faces and no time (the
owner's words: "I want just '3 replies'", "Collapse replies like in Zoom"). The thumbs-up count follows it on that
row, then the buttons. A thread's rule starts under the row. A press folds or
opens the thread; the line stays where it was, and a thread opened at the chat's foot scrolls up to show above the
box. A thread starts open; when the chat is opened, one whose main message is handled starts folded (a task done or
cancelled, a decided version, an answered question, an ask with nothing open, every turn of it over and no approval
waiting). Nothing folds or opens by itself while the chat is open, not for a new reply; the person's own fold holds
until the chat closes. Each reply is an item of the chat's own, 13px in from the main message's left edge: a 1px
border line at the edge joins the thread's items, 14px apart, faces at 28px, names at 14px, words at 14.5px, and its
words still end at the box's right edge. Threads never nest. An open thread of more than three replies shows its
newest three under "Show 4 earlier replies" (13px muted); a reply that holds an approval waiting, a failed step or a
running turn always shows. A press shows the rest, and focus goes to the first of them. A search hit, or the
status line's "needs you", opens the thread, and its earlier replies, to show what it leads to.

Messages agents write to each other sit in the thread of what the writer's work is about; a reply sits where the
message it answers sits (the one Gathr carried into its turn from the agent it writes to). A message to an agent
opens with the agent's @Name in the accent blue at the words' weight, no underline, unless it quotes what it
answers: when something came between the two, the answered words lead the bubble plainly, one line cut with an
ellipsis, 13px muted, after a 3px border line, no name. A turn that wrote no words and only sent messages has no
block of its own: its steps are on its first message's name line. Gathr's copy of a message in the receiver's turn
is never shown.
A thumbs-up is the emoji and a count under the words it was given to, 12.5px at 450 muted, no box.
Pointing at it or focusing it shows who gave it, as their names on one line beside the count, in a small box as tall
as the count, so it covers no message; with no room beside it (a narrow column, a thread) it wraps under the count, in
the flow, and what follows moves down rather than being covered. Names, never help, and never a target (the pointer
passes through it). The person's own like counts in it: "You"
first in the names, and the count in the attention blue on a 16% tint at 600. Where the person can like, the count
is a toggle (`aria-pressed`): a click gives theirs or takes it back. A plan's check that agrees is the checker's thumbs-up on the version, with no line of
its own (0062); a question stays a reply.

What nobody said is Gathr's message (0073): Gathr's face (three dots, never an initial), "Gathr" and the time
on the name line, and its words in an outlined bubble one step quieter than an agent's: a 1px border, no
fill, the words at 88% of the foreground and 14.5px. Gathr is never an agent: no panel, no mark, never "he"
or "she". When a repair's work is in it quotes the ask, says "Fixed." and lists the finished tasks with their
PRs, muted. A change Gathr caught in a held turn is its line, "Mira changed 3 files before the plan was
approved" ("on look-into task #12" for a look-into task; "1 file" for one), then one file per line, each with
a file icon in monospace, at most 20, then "and 2 more", then "Committed: 1" when some came from commits (0063).
A check that ran late is "Plan v1 came to you before Rev finished checking it.". When the person's change of
size held or released tasks: "Tag picker waits for the plan." or "Tag picker is back in line.". Anything else
(a wait's result, a wake-up, a stale-task nudge, a usage interruption) is its words, whole.

Gathr's question is its message too, a main message: the question's why as its words, and the question block (Human
tasks) in its bubble. An agent's question sits in the bubble of the block that asked it, with the same block; that
block is the question's main message, and the rest of its turn stays in the turn's thread. The person's answer, and
each Discuss, is their own message in the question's thread, whenever they sent it, with the lead's turns that heard
them; once answered the question reads as answered, its words alone. The question keeps its Discuss switch.

Two things Gathr tells an agent are never drawn: the brief of a member's own task (Gathr's message that opens
with "Task #<number> from", in a turn on that task), because the member's header says the task and the
giver's row lists it, and what the lead is told after the person changes a size (Gathr's own line says it);
nor is Gathr's copy of an agent's message, or what a turn that hears the person's answer opens with.

What a task's end told isn't drawn: a hear-back's opening and Gathr's line for a done task are for the agents, and
the end is on its member's own message (0051). Some of Gathr's lines are still one-line events, inside Gathr's
bubble: the plan's lines, with a clipboard: the lead-in "Plan v1:" and "3 findings" (a check that agrees is a thumbs-up
instead) (opening to the plan's title), "Sent back" (opening to the person's words), and "Plan v2:" "Approved"
or "Dropped"; each opens to the whole line and Open plan (0062). A line is 13px, muted, the lead-in in
the foreground colour at 500, an icon first. Closed, it stays on one line, with an ellipsis (…) for overflow.
Click or Enter opens it in place: the whole line, the reason, and Open task. Click or Enter again closes it.
Keyboard focus uses the hover highlight, without an outline or ring.

The size of an ask is the last piece of the person's own message's name line, in the line's style: the lead's
16px face when the lead sized it (none when the person did), "Fix" or "Plan", and a chevron (0061). It shows once
the ask is sized, has a task, or Gathr held something of it (0063), whoever has spoken. A click or Enter opens it
in place to the two sizes, "Just fix it." and "Plan it with me first.", in an outlined pair; the pressed one is
tinted and carries a blue tick, and the arrow keys move between them. Choosing one closes them, Escape too, and
nothing explains them. A refused change shows under the bubble with its reference. An archived chat shows the
face and the word alone.

Work on a pull request reads in the thread of the task whose PR it is: a review's turns and the fix Gathr gave after it,
each whole, with its name line, its task and its tick (0075).
An approval keeps its card where the agent asked, with Deny and Allow. What waits on you is said once,
on the status line ("Mira needs you"), and nowhere else above the composer (the owner, shown the panel that
said it a second time: "Keep the line"). "needs you" is a button of its own on the line: a press goes to
what is needed, the oldest first: an approval's card where it was asked, scrolled to and focused; else
the plan waiting, opened in its tab at its newest version; else the question's message. The rest of the
line opens its list as before. With nothing of the three in the chat, the line is one button.

### Status line

One quiet line just above the message box says how the team stands
([0050](docs/decisions/0050-what-a-chat-waits-on.md)), from each agent's state
([0071](docs/decisions/0071-an-agents-state-is-one-mark.md)). It always has its 28px slot, so nothing above or
below it moves when it comes or goes; the thread's last message ends 6px above it, and its words start where the
box's do. One font for every part, 12.5px/20px at 450, muted; who needs you is in the attention colour, who can't
run in the destructive one. Pointed at or open, it takes the raised tint as a 24px pill.

- **Its words**, in the marks' order: who needs you or can't run is named, up to two, and three or more are
  counted ("Mira needs you", "Mira and Bo need you"). The rest name one or two agents in all and count from three,
  "agents" and the verb once: "Ann is working, Bo is blocked", "3 agents are working", "2 agents are working, 1
  waiting". What a wait is on is said only when the line waits alone and every wait is on one thing: "Rev is
  waiting on PR #18 checks". A PR's waits read "PR #18 checks", "PR #18 to merge", "PR #18 to land". Never "he" or
  "she", a time, a step, a task's title or Free. Nothing when nobody, agent or Gathr, has anything going. A team
  holds at most five, so nothing folds.
- **Gathr is said like an agent, last** (the owner's pick, 2026-11-18): its update, a PR it lands and a command
  an agent's ended turn left running are Gathr's, in the most pressing of their states, by the same rule: "Gathr is
  waiting on PR #18 to land" when every agent is free, "Ann and Gathr are working". Gathr is never counted as an
  agent: beside a count it is said after it, "2 agents and Gathr are working", "Mina needs you, 1 agent and Gathr
  are working, 2 waiting", and alone in its state, "3 agents are working, Gathr is waiting".
- **Three dots** end it while an agent works: 4px, 3px apart, at 28% of the text colour, with a light that passes
  along them once per turn of the Working ring (1.6s) and rests; still, at 50%, under reduced motion. Without
  them, nothing runs.
- **Pressed**, it opens a 340px popover above it, 6px off, with a row for each agent that isn't free, the most
  pressing first: its 24px face, its name at 14px, then its state in the mark's shape at 12px and its word, what
  and how long, as its face's card says it, at 12.5px muted ("Needs you · Question · 1m", "Working · Fix the
  review of PR #18 · 1m 12s"); Needs you in the attention colour, Can't run in the destructive one. Seconds count
  under a minute, each second. A row never loses its time: a what too long for the row is cut with an ellipsis, and
  the time stays at its end. A silent turn's row says "silent 12m" for how long it has made no progress. A wait
  whose looks keep failing says "Can't read" after what it waits on, in the foreground colour, a shade brighter
  than the rest, no colour of its own (the owner's pick, 2026-11-18): "Waiting · PR #18 checks · Can't read · 1h
  4m", and its face's card says the same. Why it fails isn't in the list. Gathr's rows come after the agents', in
  the same row with Gathr's 24px three-dot face: "Working · Update to 0.1.240", "Waiting · PR #18 to land · 4m",
  "Working · Ann: Run the test suite · 12m" (a command, with the agent that left it). An agent's row opens that
  agent's tab in the panel, and the keyboard follows; Gathr's rows open nothing and take no focus. Escape closes it and gives focus back to the line.
  Who and what the line says is announced from beside it, not by a live region around the button.

### Conversation map

The registry's ConversationMap as published ([0039](docs/decisions/0039-conversation-map.md)): a 24px rail
14px inside the chat pane's left edge. Its length is at most half the pane's height below the header and
never more than 616px (44 ticks at 14px); it is centred in that height, with the same space above it as
below it down to the pane's bottom edge. It shows from three ticks up and only with 44px left of the
faces that hang left of the column's words (0075), so it never lies over a face, the text, the composer or the scroll-to-bottom button.
Only its ticks take the pointer.

- Ticks: 12px long and 2px thick for a message from the person (foreground 22%), 6px for an ask's result in a
  team chat, or in a chat of one the agent's reply to the person (15%), 12px long and 4px thick for a folded day (22%). On screen 50%; current 3px thick at 90%.
  Pointing at the rail lengthens the ticks on screen by 6px, and the current one and the one under the
  pointer by 12px; a reply's stays 6px shorter than the others throughout. Reduced motion removes the
  transition. No other agent's turn, work row, exchange or event has a tick, nor a turn still running.
- At least 6px a tick, 14px at most, so the rail holds its length divided by 6px. Short of room, the oldest
  days fold first into one tick each, the latest day's replies go next, then its oldest messages fold
  into one tick.
- Card, to the right, on hover or keyboard focus: "You · 16:04" or the lead's name and time, 13px medium,
  two lines; then the message's first lines as plain text, muted, three lines. A day: "Monday · 14 messages
  from you" (the date once a week old), alone. The tick's accessible name is
  the card's title and its description the preview. Escape closes the card.
- A tick opens its message at the top of the view, 32px under the edge, and focus stays on the tick.
  Up, Down, Home and End move along the rail. At the end of the chat the latest message on screen is the
  current tick; elsewhere, the latest at or above the middle of the view.

### Summary

Summary holds what the plan bar and a status bar would say, so neither exists
([0038](docs/decisions/0038-summary-holds-the-plan.md)). A chat with more than one agent has it.

Its button in the top line carries no words. It is the roster: every agent of the chat at 20px, in the
chat's order, 6px apart so no face covers another's mark, each with its state's mark (see **State marks**
below); a Working mark turns here. Five agents are one 140px button. Its accessible name says who needs the
person and who works: "Summary: Vale needs you, Juno and Pia working", or "Summary".

The button opens a 348px popover under the top line. Escape closes it and returns focus to the button; a press
outside closes it. Top to bottom:

- **Plan**, when the Supervisor has a round. The heading's right holds one 12px segment for each step
  (done 80% foreground, at work attention, failed a destructive outline, the rest 12%) and "1 of 3
  done". Then one row for each step: its state's icon, the task's title cut with an ellipsis, and who
  has it in 12px muted, with " · needs you" in the attention colour while the step is at work and that
  agent needs the person, or " · blocked", " · failed", " · stopped". Icons: a circle-check for done, an
  attention spinner at work, an empty circle for not started, a square-minus for blocked, a danger
  circle-exclamation for failed, a square for stopped. A done row is muted.
- **Team**, with "2 working · 1 needs you" at the heading's right, a part omitted when it is zero.
  One line for each agent: its 24px face with its mark, still, its name, what it is doing in one
  13px phrase, and its tasks in this chat as done/total (read out as "1 of 2 tasks done", "1 of 1 task done"), cancelled ones left out and
  nothing when it has none. The phrase is the running turn's activity as its row in the timeline labels it (Running a
  command); "Needs your approval" or "Asked you a question" in the attention colour; "Waiting for
  Juno", a wait on CI or a PR by name, Held, Paused, Stopped, Signed out, Its last turn failed, "#24 is blocked"
  or a usage limit with its end; else Free, the mark's word.
  No role, no start time and no chevron: those are in the agent's panel. Order: who needs the person,
  then the marks' order (can't run, works, stopped, blocked, waits, free), the Supervisor first among equals.
- **Pause or resume**, at the foot: a menu with Pause or Resume for each agent alone.

A step or an agent's row is a button that closes Summary and opens that agent's panel, and the
keyboard follows to the panel's tab.

A round is what the service says it is on each task (`round`, 0051): the Supervisor's turn the
person's message or answer began, with every task given since in answer to it, nested tasks
included. Exclude person questions and cancelled tasks; order by task number. Only done tasks count.
A round is finished once every task is settled and the Supervisor's hear-backs on it have ended; it
then stays until the person's next message, and after that there is no Plan until the next hand-off.
Never invent a step from prose, and never infer success from a finished turn: blocked, failed and
stopped stay their own states and never count done. All steps, counts, working agents, assignments
and timing come from service tasks, turns, waits and approvals. Missing records never become
inferred state. Reduced motion stops spinners and rings.

### State marks

An agent's state is one mark on its face ([0071](docs/decisions/0071-an-agents-state-is-one-mark.md)), the
same on every face of it in a chat: the most pressing that holds of Needs you (a blue dot), Can't run (a red
disc with !), Working (a three-quarter blue ring), Stopped (a grey disc with a square; a plain square at 24 and
under), Blocked (a grey disc with a bar), Waiting (a grey disc with a clock's hand) and Free (a plain green
dot). It sits at the face's top right, 1px over its edge: 12px at 36, 10px at 28, 9px at 24, 8px at 20, cut
out of the surface behind it by a ring of that surface's colour (2px at 36 and 28, 1.5px below), so it reads on
any background a face is drawn on. Faces of 24px and up carry it, and the top line's at 20; 16px faces (the
panel's tab) and Settings' faces don't. A Working mark turns once in 1.6s on the top line and on the agent's
latest message only; elsewhere, and under reduced motion, it is still.

Pointing at a face, or focusing the button that holds it, shows its card above it: a 10px-radius popover
surface, the name at 13.5px/600 (and Supervisor after it, muted), then the mark at 10px and the state's word,
what and since when: "Can't run · Failed · Show cook times · 1m", "Working · Show cook times · 4 steps · 38s".
Free says only "Free". It is information, the one hover that shows something in the window besides who
reacted. An archived chat shows no marks; an agent no longer in the chat has none, is off the top line, and
its card says Removed. A chat's sidebar row shows its most pressing of Needs you, Can't run and Working as the
same mark at 9px, still, 8px after its cut title.

### Agent panel

An agent opens beside the chat as a workspace tab, after Tasks: its 16px face and name, with a
close button. One agent at a time: opening another replaces it. While another tab shows, the panel
keeps its scroll position and its opened rows, and is not drawn again until it shows. It opens from a step or a row in
Summary, a row of the status line's list, and from an agent's face in the timeline. 20px 24px 32px padding, 24px between sections, each under a 13px muted
heading; a section with nothing in it is left out, and an agent with nothing at all reads "Nothing in
this chat yet".

Its face in the timeline is a button the face's size, which takes the hover tint. An agent that has
left the chat keeps its words, and its steps in Plan, but has nothing to open.

- **Header:** two lines beside the 36px face with its mark, still, that never wrap. First the name at
  20px/600 (26px line) and the role 8px after it in 13px muted; at the right, Message, the chat icon
  (lucide `message-square-text`, named "Message Juno", no words), which puts "@Juno " first in the
  composer as a picked name, unless its first picked name is already Juno, and focuses it; then More.
  Under them, in the chat's name line words (12.5px muted, "·" between), the state as its mark says it,
  what, and since when: "Working · Tag picker · since 16:06", "Needs you · Approval · since 14:52",
  "Waiting · PR #18 checks · since 09:42", "Free". The state's word is in the foreground colour, Needs
  you in blue and Can't run in red. The name and what are cut first; the role, the state's word, the
  time and the context bar never are. At that line's right, the context bar: a 48 x 4px track and the
  percent, amber from 65% and red past 85% (the registry's colours), its numbers on hover. Until the
  conversation's first call is measured, the track is empty and says New; a resumed conversation shows
  its last fill from the moment it opens; while a compaction asked here runs, it says Compacting. An
  archived chat, or an agent that can't be sent to, has no Message.
- **Task:** the task its running turn works on, else its first open one: the title, "From Orin" ("From Gathr" for a fix Gathr gave) and
  Open task, which opens it in the Tasks tab.
- **Now:** its running turn's row, the timeline's own, with its steps showing. A turn that has done
  nothing yet reads Working; one that only waits on the person has no Now.
- **Earlier in this chat:** its earlier blocks, newest first, drawn by the timeline's own components
  at 15px, each headed by its name and time: its turns, and what it wrote to other agents. The latest
  10; older ones stay in the chat.
- **Tasks in this chat:** every task of its own, cancelled ones too, open ones first, five before Show more, as
  the Tasks tab's rows without the face or the name: "#22 · Merging". A row opens the task.

### Navigation and workspace

New, Search and Tasks sit above projects and their chats; Search shows no key words, and Ctrl K (Cmd K
on a Mac) still opens it. Settings and Update are on the rail. Rows are 30px; the word mark's block
36px, 8px under the frame's top. A chat's row and a project's row are each one 7px-rounded box that greys
as a whole on hover; their + and More are 24px icons inside it with no box of their own, shown only on
hover, keyboard focus or while the menu is open, the open chat's row too (C70, C71). A chat with
something going on carries one 9px mark 8px after its title, which is cut before it (0040, C65, 0071): the
faces' own Needs you, Can't run or Working mark, the most pressing of the three, still. The mark never moves
More. Its name is the mark's word. An agent that only waits, a queued turn and a paused chat with nothing
running carry no mark. Archived uses Lucide Archive when collapsed and the same icon with its lid
lifted when expanded, never ArchiveX. Rotate the lid -14 degrees about its left
hinge (2px 8px), translate it up 1.5px, over 160ms ease-out.
Search and the pending human Tasks list share a contextual flyout, at most
340px wide, beside navigation. Escape, outside click, Close or the same trigger
dismisses it. Search matches titles and message text, supports arrow selection and
Enter, and retains its query. A task routes to its exact chat and task.
Archived chats are included and labelled Archived. Chats precede Messages; one
keyboard cursor spans both groups. A message hit centres its saved message and
holds the scroll there until the person returns to the bottom. The matched
message gets a 1.6-second attention tint; reduced motion uses a static tint until
the person leaves that result. No outline or ring. In-chat find is separate work.

Search uses [title first](designs/search/v1.html), selected 2026-11-10, matching
Tasks. Snippet first (v2) is retired. Search and Tasks share the icon-button close.
The portable reference demonstrates empty, loading, no matches and referenced errors.

Workspace has a permanent Home tab, then Tasks, Changes, Terminal, Files, Web and
canonical document tabs as opened. Plus returns to Home. Each chat retains its
tabs, selection and tool state; canonical paths reuse a tab, terminals may have
several instances. Close other tabs retains Home and Recents. Arrow keys,
Home/End and Delete operate tabs. Summary and the one timeline (0037) replace
0015's earlier Team tab, inline thread and split. The agent's panel is a tab after
Tasks (0038). Other tab workflows remain governed by their decisions.

Agents show their face beside their name, replacing the shared robot icon (0074): a drawn face on the
background the person picked, the same everywhere, at 36 heading its panel, 24 on a message, in Summary's
rows and a project's team, 20 in a stack and 16 on its tab. The face is hidden from screen readers
beside the name and carries the name where it stands alone. The background shows as picked, the same in light
and dark; until one is picked it is a grey, #CECECE. The person and Gathr keep their own marks.
Roles such as Supervisor, Member and Reviewer use the normal sans-serif.

Team status appears in Summary, and each agent's detail in its panel. Progress must
derive from real task completion, not a decorative timer. Plans have their own
identity and tab. A typed note is never approval.

A plan's version is the lead's own block in the timeline: the speaker header, then a chip, a
button with the clipboard icon, "Plan · version 2" and "9 parts" in muted small type, then the
thumbs-up names under it, the same as under a reply. Text the lead writes after the plan reads
after the chip, in a block of its own. The chip opens the plan's tab, after the agent's tab, labelled
"Plan v2" with a close button. The tab reads top to bottom as a task's page does, in the same classes:
"Version 2" at the top left and, when there is more than one, the versions at the right, `v1 · v2`, muted
and underlined, the one shown plain; the title; one line with the lead's 16px face and name, the time and
the plan's state word (Checking, Waiting in the attention colour, Revising, Approved, Dropped, or
Replaced on an older version); the plan's text; then its fields: Asked, Check ("Bo agrees", "3 findings
from Bo", "Bo is checking", "Bo didn't finish"), the version's Findings (a suggestion is left out) in a
task's note format, "Wrong · src/notes.rs: ...", the lead's Answers under the findings they answer
("Changed: ..." or "Kept: ..."), the ask's Questions, each with its answer or "Open", Sent back with the
person's words on a version they sent back, and Changed, one row for each turn of the ask in which Gathr
caught a change, "Ann: src/notes.rs, src/tags.rs", shown only when there is one. Then "Parts 9", the
Tasks list's muted heading and count, and each part as the one task row: the owner's plain face, its
what, and one line "Part 2 · Ann · After 1", with Changed or New at its end in the words' colour, against
the version before and matched by what it says, so a part moved in order is not changed. A part that went
is an ended row after the parts, its what struck through, "Ann · Removed". Version 1 has no marks. A part
opens in place, one at a time and none at first: its row and its brief on one tint, the brief in a task's
fields (Why, Done when, Proof, Where, Size, After); a changed field keeps its old words under its new
ones, muted, "Was: ...", a changed owner has an Owner row, and After's Was names a part that went by its
what. Once approved, a part that became a task is that task's row, as in Tasks ("#24 · Bo · Blocked"),
and opens it. The tab opens at its top on another version and after the person's own
Send back, Approve or Drop; a state that changes from elsewhere (a check that ends) keeps their place. The foot shows only when something in it acts, at a task foot's 64px: Drop
(outline) at the left on the newest version while the plan is open, and at the right, on the newest
version once it has reached you, Send back (outline) and Approve. Send back opens the answer form's field
over the buttons, "What to change" as its placeholder, with Cancel and Send back, which is off until the
field has words; Enter sends, as in the answer form. A refusal sits over the buttons as one sentence at
the right, as the chat says one: "Not approved" ("Not sent back", "Not dropped"), why, and its reference.
No sentence explains a button. The foot fits the workspace at its narrowest, 360px, in both states. The
tab carries the attention mark while the plan waits on you, as Tasks does.

### Human tasks

The navigation count shows pending human tasks across chats, in the attention
colour. The workspace Tasks
tab lists the chat's tasks (the tasks mock-up, New) in three groups, For you, Open and Closed, each under a
12.5px muted heading with its count: the questions waiting on the person, oldest first; what is under way;
what is over, newest first. Each task is one row, the one task row an agent's tab uses too: a plain 24px face
(the asker's for a question, Gathr's three dots for its own, else the owner's; never a state mark), its what at
14px, and one 12.5px muted line, "#24 · Bo · Blocked", the same pieces in the same order on every row: number,
who, Look into, state. The state is in the chat's words, in the line's colour: a state the marks draw (Working,
Blocked, Stopped, Waiting) in the mark's shape, still, at 12px; Merging, Failed (in the danger colour) and
Cancelled with their icons; "After #27", "Not started", "Discussing" as words; Waiting in the attention colour
for a question waiting on you; a done task none, with a tick after its what, and an ended task's what muted.
Rows are 6px 10px with a 10px radius and take the hover tint.
A row opens its task in the tab, under Back ("< Tasks") and its number. An agent's task: its what at 20px,
the line "Ann · PR #18 · Working" (its owner's 16px face first), then the fields From (the giver's 20px face,
name and time; Gathr for its own question and for a fix it gave after a review), Why, the brief, Report ("None
yet" while it works) and the rest; the foot holds only what acts: none when done, Cancel while open, Cancel and
Restart (primary) once blocked, failed or stopped, Restart once cancelled.
A question opens as itself, from its row: the asker's message (its 36px face, name,
role, time, "for Ann") with the one question block in its bubble, answered there, each question by itself
(0065). Only the Supervisor asks the person (the sole agent in a one-agent chat). The block: the question with
its icon and "Waiting" in the attention colour; for a question an agent gave, the shared Switch, small, labelled
Discuss, right under it, so nothing above it moves; the choices, each a bordered row with a mark (a ring, or a
square where several can be picked) that fills in the attention colour once picked; and one growing field with
a round Send. Gathr's own question has no field and no switch, so a pick answers it at once; an agent's has a
field (Notes, or Answer when it has no choices), so a pick marks the choice and Send or Enter sends it. Notes
alone answer any agent's question in words, so "none of these" can be said. Discuss swaps the choices and notes
for its own field, "Message to Mira"; each is kept while the other shows. The first choice of an agent's choice
question reads "<lead>’s pick", then " · " and its detail ("Mira’s pick · it comes back by itself"); Gathr's own
question has no pick. A refused Send stays in the block under its field: "Not sent" in the danger colour, why,
and its reference, with what was typed kept. Once sent the block closes, reads Discussing while discussed, and
the person's own message follows the asker's. A discussed question leaves the For you count and the chat's dot
until the lead's turn that got the message ends, or is replaced by a new question (0065).
In the chat the same block sits in the asker's bubble, with the same draft as here: a pick, notes or Discuss
started in one place is there in the other, and while one Send of a question is out, another sends nothing.
The answer, and each Discuss, is the person's own message in the chat, at the time it was sent (0073).
Lifecycle/history policy and persistence are service work, not prototype rules.

### Archive and stale (F11)

The selected archive location is a folded Archived group after the active chats
in the sidebar. It starts closed, shows a count, keeps its fold while the window
is open and shares the chat-list scroll region. Archived rows use 34px left padding;
More appears on hover and keyboard focus. Restore returns a
chat to its original project or Chats without restarting cancelled work.
The separate Archived button and flyout are retired. See
[the selected reference](designs/archive-stale/v3.html), a refinement of v1;
v1 and v2 remain unchanged as the compared proposals.

The top line's More keeps Pause/Resume/Restart first, then Archive, then Delete after a
separator. Row menus use the same shared menu. An archived chat offers Restore
and Delete; its composer becomes a quiet Archived line with Restore. Deleting
the current chat selects its next sidebar neighbour, then its previous one,
then New. Other chats retain their drafts and reading position.

Confirm is a shared Dialog variant: "Archive <title>?" or "Delete <title>?",
Cancel and the verb. The title wraps, Cancel has initial focus, Escape cancels
and focus returns to the trigger. Archive uses the neutral primary Button;
Delete uses the outlined danger Button. A refused action keeps the confirm
open with "Archive failed" or "Delete failed" and its Diagnostics reference.
A confirm that needs it has a short body under the question, in muted 14px: a
count and what happens to it, then what is kept ("1 project and 2 chats will be
deleted. Files are kept."). Every thing named carries its count, never its name;
no line per fact, no paths. A project's or an agent's Delete (0069) takes its
body from the service's counts when it opens; the verb waits for them, and
counts that fail show the shared failure notice with the verb off. No Undo.
Restore asks nothing; its failure uses the shared referenced Notice.

The Tasks flyout has Questions, then an expanded Stale group. Each task appears
once. Its navigation badge still counts only open human questions, including
stale human questions. Stale rows name the chat and owner (For you for a human),
and say "Idle 4 days" for time since last progress. An agent's real hold or state
stays visible. This is not a countdown to cancellation.

A stale human question uses the existing question pager and per-question drafts.
It never implies automatic cancellation. A stale agent task opens that chat's
Tasks tab at its title, giver, owner, state and reason, with Cancel and Restart.
Restart follows the service's eligibility; a waiting task is not restartable
just because it is stale. Agent tasks never receive a human answer form.

General has Stale after (days) and Auto-cancel after (days), defaults 3 and 7.
Both accept 1 to 365 whole days and save on Enter or blur. Retain invalid input
with "Enter a whole number from 1 to 365"; retain a refused edit with its reference.
Help explains progress and the open work that prevents automatic cancellation.
The person's questions stay open until answered or explicitly cancelled.
With the defaults, an eligible agent task cancels after 10 days without progress.
The service owns that timing and every archive effect; see 0010.

### Settings, accounts and agents

Keep Gathr's accepted centred Settings layout (0023), not the prototype's example:
at most 1024 by 760px, 32px horizontal and 48px vertical clearance, a fixed header,
a 200px category rail and an X at top right (the shared icon button, 16px from the top
and right, named Close Settings; 0023). Below 768px categories move above
content. Escape/the X return focus and preserve the chat's draft and position.
Apply the new shared tokens, type and controls within that layout.

General starts with Theme for This computer: System (default, follows the system's
setting), Light or Dark, applied at once without a reload. Then Open at login for
This computer, applied immediately with Switch. The window keeps the theme and the
navigation and workspace widths across restarts. General ends with the version and, under it, the faces'
credit (0074): "Faces", then "Avatar Illustration System by Micah Lanier, CC BY 4.0, changed", the work
and the licence linked.

Agent folder follows Open at login: the current path, Change and shared ? Help.
It belongs to the service host, with no This computer label. In the desktop
window, Change opens the system folder picker at the current folder. Selecting a
folder saves immediately; cancelling saves nothing. Browsers and failed pickers
use the path editor: Save/Enter saves; Cancel/Escape returns focus to Change
without closing Settings. A refused save shows its reason and error reference;
the path editor also preserves the typed input. Help explains each project's
Shared and per-agent folders and that changes apply to new work. Current work
stays where it is.

Engines manages named accounts and one Gathr-owned program per engine. No custom
path. A new Gathr has no accounts and does not reuse external logins. Agents has
one shared editor: Face, Gender, Skin tone, Background, Account, Model, Effort, Approvals. Face is the agent's
56px face and Shuffle, which draws it from a new seed and keeps the gender and tone shown; Gender is Man or Woman; Skin tone is six swatches,
light to deep, named Skin tone 1 to 6; Background is twelve 26px swatches (Rose to Orchid, round the wheel, names
for screen readers only) and a last + swatch, dashed with a plus, that opens the shared colour picker under the
row: a saturation and brightness square, a hue bar, the colour now and its hex, on the popover surface. A colour
not among the twelve shows on the +, pressed, and stays on it, not pressed, while a ready colour is picked. Escape or a click outside closes the picker, not Settings. Each
redraws the face at once and is saved with Save. A gender or tone never set shows what the face's seed picked;
a background never set is the grey, with no swatch pressed (0074). Account implies engine;
model/effort choices come from that installed engine. Family Latest follows new
releases; numbered versions stay pinned. Do not ship prototype fixture models,
temperature or extra toggles as product settings. Save affects future turns in
all chats using the agent; running turns retain their assignment. Roles are
chat-specific, with one Supervisor for two to five members and an optional,
distinct Reviewer. Edit agent opens this same editor from every entry point.

Approvals is Ask, Edits, Auto (new-agent default) or Full, per 0029. Detail stays
behind Help beside the label. The shared ? opens an in-window popover; Escape
closes Help before Settings and preserves focus. Switch remains the existing
accessible immediate setting, with pending save and referenced failure states.

Projects (0043, option B) lists projects as Agents lists agents. A project's page has
two sections in one draft with one Save: General (name, folder, default team) and
Checks & merge. Checks are read only, in run order, each with who set it (the
agent's name, or Found in the project) and when: the Supervisor sets them, the
person never types one. Required checks (CI, Gathr's checks, Both) and Merge method
(Fast-forward, Merge commit, Squash via GitHub API) are editable, each with one line
saying what it means. A project's More in the sidebar has Project settings, which
opens Projects at that project, then Delete after a separator. Projects and Agents
have Delete, the outlined danger Button, at the left of the form's foot; Agents'
Delete replaced Remove and its inline question. Checks & merge ends with the checks cache: its size,
"Unused caches are removed after 14 days" and Clear, which acts at once, apart from Save.

An agent's task opens in the workspace's Tasks tab from its title in the agent
panel's task list, the panel's Open task, or Open task in its hand-off line: who has it, its state and reason (a task
that waits for its plan reads "waits for its plan"), a number line that reads "Task #12 · Look into" for a look-into
task (0063), and, with a PR, Gathr's checks run at the PR's head (passed, failed with its reference, or
running), each command with its exit, time and log, the failing one's open. Back
returns to the person's questions.

Diagnostics stays a Settings category: a 210px turn list beside the independently
scrolling timeline. Failure links open its matching turn and reset conflicting
filters. Preserve category/filter state and make long turns usable. No normal
header Diagnostics action. The prototype's component gallery is a reference tool.

### Recovery, focus and motion

App-level failures appear at the top of the chat in the shared Notice, with the reason, reference and Dismiss.

Stopped, waiting for sign-in and failed-send notices stay at their affected reply
per 0023. Preserve partial text and drafts. Neutral for stopped/waiting; destructive
for failure, with the reference, Diagnostics and Retry. Errors end in failed.
Sign in opens Engines in the same Settings panel.

No focus rings anywhere, including the composer. Keyboard focus uses the same
quiet highlight as hover, so the current item stays visible. Keep native keyboard
behavior. Icon buttons have accessible names. Dialogs trap focus; Escape closes
the top layer and returns focus. Reduced motion removes movement. Keep registry hover/press feedback and
purposeful transitions, never animate a simulated service result into truth.

## Copy

Gathr is an AI developer tool. Use Enable / Disable for settings actions and Enabled / Disabled for their states. Preserve technical terms users configure or troubleshoot, such as model, endpoint, tool call, and JSON. Use JetBrains’ Writing Short and Clear to remove repeated context, filler, and explanations of obvious controls. For notifications, include the affected item, what happened, and the next action. Include a workflow rule only when it changes what the user needs to do about that item. Gathr’s terminology takes precedence over examples in external guides.

References: [JetBrains: Writing Short and Clear](https://plugins.jetbrains.com/docs/intellij/writing-short.html), [Paste: Word list](https://paste-dsys.com/foundations/content/word-list/), [Paste: Alert composition notes](https://paste-dsys.com/components/alert/#composition-notes).

## Do's and Don'ts

- Capitalize Gathr. Keep one component for each repeated pattern.
- No hover help line on a control, anywhere: an icon button carries its name for screen readers only. Hover may show data, such as who reacted.
- Use the prototype's real components with Gathr adapters and real state.
- Preserve identifiers, references, recovery steps and the tool arguments an agent needs.
- Stale notices use "No progress in 3 days: #72 Approve the wording? (waiting for your answer). Finish, restart or cancel it."
  Agent tasks name the agent and state. Only a list with agent tasks explains when those tasks are cancelled automatically.
  Questions for the person stay open. Multiple tasks use one row each and one shared instruction.
- Keep screenshots and fixture data invented; never port personal content.
- Do not replace accepted architecture, membership, safety or lifecycle policy
  with a prototype callback. Conflicts go to the decision/plan, not into styling.
- Do not claim the current production screens already match this reference.

## Check

Run `node designs/check.cjs`. Open `designs/system.html` at 1440 and 1024px,
dark and light (`#light` or Switch theme). Exercise composer, tool disclosure,
tabs, Summary, Settings categories, Help, Switch, Escape and keyboard focus.
Run `node designs/check-team.cjs` after the UI dependencies are available. It
checks the retired split's preview (kept as history, 0037): running, finished and late
replies, split retention, independent scrolling, plan progress, Summary and keyboard
controls at both sizes and themes.
An optional output folder saves screenshots;
keep evidence outside the repo.
The old selected pages are retired as layout authority; see designs/README.md.
No engine, account sign-in, real command or Settings service connects to this page.
