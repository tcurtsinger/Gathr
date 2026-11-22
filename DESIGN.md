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
Operate mode: variance 2, motion 2, density 5. One centred chat (the room, `ui/src/room`), the
chats' sidebar, the message box and a resizable sidebar of tabs beside the chat. No Overview screen.

The source record and component map are in [prototype-reference.md](designs/prototype-reference.md).
The portable reference is [system.html](designs/system.html). It demonstrates
tokens and static component anatomy, not replacement runtime components.

## Colors

Surfaces take four steps. `shell` is the window's own surface, under the rail and the top line and
around the frame ([0070](docs/archive/decisions/0070-the-windows-shell.md)): #2c2c2c in dark, a grey darker than the
frame in light, with `shell-edge` (7% foreground) as the frame's 1px ring. `frame` is navigation and the
workspace. `background` is the chat. An agent's message is a card on the chat: a `border` ring and the
foreground at 3% over the chat. The person's message is a bubble in `accent-surface`. In dark the chat is darkest
and the frame lighter ([0068](docs/archive/decisions/0068-the-windows-colours.md)):
chat #1b1b1b, frame #232323, card #2c2c2c, with floating surfaces
(the composer, popovers, the dialog) at #2f2f2f. `raised` and
`danger-surface` are tints, so they lift whatever they lie on, and
`accent-surface`, a menu's highlight, is lighter than the popover. Light is a white chat with a
light-grey frame and cards and a border dark enough to see. Each step differs
from the next in both themes. `tokens.css` records both themes. Foreground carries text and primary
actions. `muted-foreground` is metadata; `muted-surface` is a background.
`border` separates regions; `input` bounds inputs; `ring` is transparent.
Blue is attention, links and active work, not an agent palette: Needs you and Working, a waiting question's
ring, Doing on the board, the replies' fold, a thumbs-up the person gave, the names the person picked from the @
list and an @Name of the chat's agents in an agent's words. `free` (dark `oklch(0.74 0.17 148)`,
light `oklch(0.54 0.15 148)`) is Done on the board and the check on a design's picked option; `review` (amber) is
the board's In review.
`os-close` is Windows' own Close red, the same in both themes, under the pointer on the window's Close
alone.
A failure is a small card in the corner (Recovery, focus and motion): the popover surface, a hairline border, a small
danger mark, its title, its reason muted and its reference; never a red fill or a red border.

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
The base is 15px at weight 450 and line-height 1.6. An agent's card: its name 650, its words 15px; a reply
14.5px under a 13.5px line of name and time; the person's bubble at the base size; the message box 16px; chat rows
in the sidebar 14px; the time, the row under a message and a card's state 12 to 13px. Code stays monospace.
An agent's words (`markdown.css`, ported from SerenityOS): blocks 10px apart; a heading of any level a section
label, 1.0625em at 600, 16px clear above; list dots 0.4em with the text 10px after, numbers muted, items 3px apart;
a quote a 2px rule at its left, muted; inline code 0.84em on a 7% foreground chip with 4px corners; a code block
and a table 13px; a table is one frame with 16px corners, its header small, monospace and muted over a hairline,
its cells 10px by 16px with no lines between rows.

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
  three buttons at the band's left, over the rail and the sidebar. After them (from the band's left, 8px,
  on Windows and in a browser) the line holds the sidebar's button, Back and Forward. In a
  browser the line moves nothing and draws no buttons. The top line stays live over a dialog: the
  shared Dialog holds the same band, fixed over the window's top 40px, drawing nothing over the
  backdrop but Windows' three buttons at the same pixels, out of the dialog's keyboard order. Gathr's
  "Start failed" page keeps the system's title bar.
- The rail: Home at the top, its centre on the word mark's line, then Agents, Plugins, Designs and Memory, 6px apart; the
  screen that is open is drawn as current (32px, 8px radius, foreground 9%). At the foot,
  6px apart: Update, in the attention colour, only while a release is ready (a turning loader, muted,
  while it installs), above the Settings cog. Icons 18px at 1.75 stroke; Update 20px at 2.
- Navigation starts at 246px, resizes from 190 to 400px, and uses 190px below 760px (a browser
  only). With the workspace open in a narrow window it gives way to leave the chat and the
  workspace 360px each, never under 190px. Widths in the frame (the window less 50px), sidebar /
  chat / workspace: 246 / 686 / 458 at 1440, 246 / 542 / 362 at 1200, 230 / 360 / 360 at 1000 and
  190 / 360 / 360 at 960.
- The top line, from the sidebar's edge: a short divider on that edge (12px in from top and bottom),
  20px, the title, then More 4px after it in the same group; the title truncates with an ellipsis and
  More stays visible. The right holds the Agents button (a chat bubble holding three rising bars), the questions
  button, then the sidebar's button, open or closed, 8px from the frame's end (from the window's buttons on Windows). These share one icon-button style: 28px, 8px radius, 4px apart,
  17px Lucide icons at 1.75 stroke (More's dots radius 1.4). Pressed (the sidebar's button
  while it is open): foreground 9% background, foreground icon, 12% on hover. The chat has no header of
  its own: the thread starts at the frame's top.
- The chat's column is at most 760px, centred, 24px in from the chat's sides. The message box is 12px wider
  than the column on each side (at most 784px), 16px above the frame's foot; the chat runs behind its top edge,
  and the newest message rests 16px above it. A new chat centres the face of who its message goes to (the
  assistant, or the picked project's lead; none without one) at 56px over one line at 18px/550; with no agents
  at all, Add agent under it. With no project it is a greeting by the
  computer's clock, "Good morning, Sam" (05:00), "Good afternoon" (12:00), "Good evening" (17:00), "Still up,
  Sam?" (00:00), the person's name from Settings > General; with a project picked it is "What should we build
  in <the project's name>?". While the @ list is open they sit at the column's top.
- Workspace starts at 40% of available width, initially clamped to 360 to 760px.
  Resize minimum is 360px. It always sits beside the chat, never over it, and leaves the
  chat at least 360px: when the window narrows, the workspace gives way first, down to
  360px, then navigation. Only the drawn widths give way; widths the person dragged come
  back when the window widens. Its tabs sit in a 38px row of their own at its top, tabs 28px high.
  It opens by itself in a chat of more than one agent; after that, as the person left it in that chat.
  Settings is the explicit layout exception described below.

ResizeHandle supports pointer capture, 10px arrows, 40px Shift+arrows and Home/End.
Maintain native scrolling, a thin themed rail and independent scrolling regions.
Desktop checks are at 1440 and 1024px, light and dark. Small screens reflow
controls; they do not create a second mobile product.

## Elevation

Flat chat and navigation. Floating popovers use a tonal surface and a soft shadow.
Navigation flyouts have no dimmed backdrop.
Blocking Settings uses the shared backdrop. Layers must not clip focused controls.

## Shapes

Text actions use the prototype's shared 32px pill Button. Icon controls keep their
contextual square or circular shape. Small controls use the 8px base radius;
floating surfaces 16px; ComposerBar 24px with 10px padding. The person's bubble has 16px corners and
10px 14px padding, at most 76% of the column; an agent's card 14px corners; a code block 8px.

## Components

The window uses the recorded official registry components, wrapped from outside: assistant-ui's composer
and approval card, and shadcn's Button and dropdown menu. Do not build lookalikes in `ui/`.
The prototype gallery contains 64 examples, not 64 new Gathr service capabilities.
`ui/registry.json` records each adaptation with its original and adapted hash and why:
Button today. Retain source
notices and both hashes.
Dependency approval, pinned versions and file splitting belong to the integration
decision; this design update adds no JavaScript dependency.

### Conversation and composer

The room (`ui/src/room/Room.tsx`, `room.css`; the owner's picks of 2026-11-20 and 2026-11-22) lists, by when
they started, the person's messages, each agent's main message, an agent's question to the person and, of
Gathr's own lines, only a compaction's, quiet, centred and 13px. Items are 18px apart.

- **The person's message** is a bubble at the right in `accent-surface`, its words as typed (no Markdown),
  its files' tiles under them; files alone sit at the right with no bubble. A note on a design shows above
  the words as a small 7% tint block: the pin's number on a blue disc, the design and option, then the part.
  A message the service didn't take stays where it would be, dimmed, with a quiet Retry under it.
- **An agent's main message** is a card with its 28px picture in a 44px left margin and, when the next item
  is a card or question too, a thin line down from the picture to it. The card's 40px head has the name, Lead
  when it leads, and at the right "Working · 40s" in blue with the turning mark, counting every second, then
  a check and how long it took ("1m 12s"), or Stopped, Waiting for sign-in or Failed in words. Then its last
  words; while the turn runs they are muted, and a turn with no words shows its reason muted. A turn that
  ended with nothing to say shows nothing.
- **What an agent showed** with `show` (`room/Shown.tsx`) sits under its words and above the kept lines: a
  table as assistant-ui's Data table (a click on a header sorts, dates by date, empty cells blank and last; a
  link as its name, or its address without the scheme, cut in the middle), a chart as its Chart (charts in a
  row sit side by side), a flow as its Flow graph (laid out by what follows what) or math as its Math block
  (`^` and `_` raised and lowered), each with its title as a quiet 12.5px line above.
- **Kept lines,** under the words, one quiet 12.5px row each with its icon: "Noted about you: …" (Memory's
  mark), "Added tool Notes · needs key" with an Add button that reads Added once given (Sign in and Signed in
  for a sign-in), "Added skill branch-names" or "Changed skill …", a tool's or skill's name cut with an ellipsis. A click opens
  Memory or the tool's or skill's page in Plugins.
- **Steps:** all of a turn's tool runs and working notes are one quiet line with a chevron, "Read 4 files,
  ran 2 commands", "Ran 14 steps", or "5 steps so far" while it runs; a click lists them in order, a failed
  one in the danger colour.
- **The approval card** is the registry's, at the foot of the card that waits on it: "Allow Bash?" (or
  "Allow adding a tool?"), the agent's name under it, the command or the file it would write, Gathr's own
  reason when Gathr asks ("Writes outside the agent's folders. Allow covers files in … in this chat."),
  then Deny and Allow; Allowing… or Denying… while the answer is sent.
- **The row under a message:** its time ("9:21", the person's clock), Copy (a check for a moment once
  copied), then one thumbs-up with its count last, so a growing count moves nothing; text and icons 13px,
  muted. Under a card it starts at the left and shows once the turn ends; under the bubble it ends at the
  right. The thumb is filled blue once the person gave theirs. Who gave one shows beside the count while the
  pointer or keyboard is on it, before the time on the person's own row; there the thumb shows only once an
  agent gave one, and is no button. No hover text.
- **Replies** are folded under the message they answer, behind a 1.5px rule 18px in: one blue line, "3
  replies · Rev, Ann · 9:21", then Hide replies once open. Each reply is a 20px picture, the name and time on a
  13.5px line, the words at 14.5px and its own row starting with Copy. A reply that waits on an approval is
  never folded, and a search hit in one opens them.
- **A question to the person** is the same card, ringed in blue while it waits, its head saying Needs you,
  with Answer, which opens the Questions panel on it. Answered, a quiet block under it from You: the choice in
  bold, then the words. Closed when archived says so. The Questions panel (420px, from the top line's
  Questions button, whose dot is blue while one waits) lists each open question with its choices as outline
  buttons and an Answer box; with none, No questions.
- **A search hit** opened here is tinted blue for a moment.

Code blocks are one frame with no bar: no language row, no Download, no line numbers; Copy sits in the top
right corner and a long line wraps under it, never covered. Tables have no buttons and scroll sideways in their
frame when too wide.

There is no recipient picker (C24, 0028): the box's hint names who
gets a message that names nobody in a new chat, "Message Mira" (the assistant, or the project's lead), and
reads "Message the team" in a chat that has begun; with no agents at all it reads "No agents yet" and takes
nothing. Enter sends, Shift+Enter is a new line; the box grows to a third of the window, then scrolls; the caret is
blue. The paperclip, a paste or a drop on the chat ("Drop to add") adds files, up to 10, each up to 10 MB. While a
turn runs and nothing is typed, Send is Stop chat. Typing @ at a word's start, anywhere in the
message, opens the composer's one menu above the box's left edge: a 24px face, the name, its description
muted, and Lead muted at the right for the lead; in a chat with no
project, every other agent follows under "Bring in" (12px muted, a rule above). In a new chat with no project
the assistant comes first, marked "Assistant", and nobody is under "Bring in". It is at most 19rem tall, then
scrolls. Up and Down move the lit row, Enter or Tab picks, Escape
closes it, and a click picks; the lit row has no Enter mark. The name goes in as
"@Name ". Only a name picked from the list counts, as @ and / work in other apps'
boxes: a typed @Ann with nothing picked is plain text. The box keeps each picked
name as a range of its text with its agent, through typing before and after it,
pasting and a draft kept while another chat is open; an edit inside it makes it
plain. Picked names show in the attention blue at the text's own weight: the box's
text is drawn again in one piece under the see-through textarea and the names
coloured by a highlight (a span would move the letters after it off the
textarea's by up to 1/16px). Once something is typed, a quiet sign by Send says
who gets it: every picked agent in the order the names stand, else the hint's
agent (0028), in 12.5px muted with each name in the foreground at 550 and bare
20px pictures side by side, 2px apart (the mock-up's `#who` A): "To Ann", "To Ann and Bo", "To Ann, Bo and Cy";
agents picked from outside a chat with no project are said apart, "Bo joins", "Bo
and Cy join", "To Ann, Bo joins". The names end in "…" where the row runs out (the
360 column), the pictures stay. Each agent named gets the message.
There is no Everyone option. Sent, the message keeps its picked names, and its bubble draws each in the same attention blue; a typed
@Ann stays plain, as does every name in a message from before picks were kept.

Streamdown draws an agent's words (`Markdown.tsx`); the person's stay plain text. An agent's raw HTML shows
as the text it is. A link opens in the person's browser; a link to a file opens it in the chat's sidebar as a
tab. A file in a message is a square tile where it stands: a picture cropped to fill it opens large in a box, any
other file opens as a tab. An @Name of the chat's agents in an agent's words is blue.

The chat's sidebar (`room/Sidebar.tsx`) is tabs: Home, always first and never closed, with Find file at its
top, then Open (the Files and Designs tiles, and in a project's chat Changes with its count of changed files)
and the Board (To do, Doing in blue, In review in `review`, Done in `free`; No work yet). Files, Designs and
Changes, once opened, then a tab per open file or design, each with its close X; a tab is at most 180px, the
open one on the chat's own surface with a border. The file tabs scroll sideways, and a tab partly out of
sight isn't drawn.

### State marks

An agent's picture carries no state (the owner's call of 2026-11-21). A state shows only where it is said
beside words ([0071](docs/archive/decisions/0071-an-agents-state-is-one-mark.md)): Needs you (a blue dot), Can't
run (a red disc with !) and Working (a three-quarter blue ring). A card's head says "Working · 1m" or a question's
"Needs you" with its mark at 11px. A Working mark turns once in 1.6s
(the owner's call of 2026-11-19: a ring standing still read as stuck); under reduced motion it is still.

A chat's sidebar row shows its most pressing of Needs you, Can't run and Working as the
same mark at 9px, Working turning, 8px after its cut title.

### Navigation and workspace

New and Search sit above projects and their chats; Search shows no key words, and Ctrl K (Cmd K
on a Mac) still opens it. Settings and Update are on the rail. Rows are 30px; the word mark's block
36px, 8px under the frame's top. Each project is a small muted header like Chats, with no icon, its chats
under it; a click folds it, and folded it shows its count. A chat's row is one 7px-rounded box that greys as
a whole on hover; its More, and a project header's + and More, are 24px icons with no box of their own, shown
only on hover, keyboard focus or while the menu is open, the open chat's row too (C70, C71). A chat with
something going on carries one 9px mark 8px after its title, which is cut before it (0040, C65, 0071): the
Needs you, Can't run or Working mark, the most pressing of the three. The mark never moves
More. Its name is the mark's word. An agent that only waits, a queued turn and a paused chat with nothing
running carry no mark. Archived is a header like Chats, with no icon.
Search opens a contextual flyout, at most
340px wide, beside navigation. Escape, outside click, Close or the same trigger
dismisses it. Search matches titles and message text, supports arrow selection and
Enter, and retains its query. Archived chats are included and labelled Archived. Chats precede Messages; one
keyboard cursor spans both groups. A hit opens its chat. In-chat find is separate work.

Search uses [title first](designs/search/v1.html), selected 2026-11-10. Snippet first (v2) is retired. Search
has the icon-button close.
The portable reference demonstrates empty, loading, no matches and referenced errors.

Agents show their picture beside their name: Gathr's own drawing of a shape (twelve: Circle, Tile, Cloud,
Flower, Hexagon, Pebble, Shield, Drop, Star, Diamond, Bubble, Leaf), a pair of eyes (twelve: Lines, Glance, Side,
Squint, Dots, Rings, Ovals, Happy, Rest, Wink, Calm, Quirk) and a line colour (twelve, Red to Grey), lines only on
no background, the same everywhere: 56 on an agent's page and the new chat, 28 on a card, 24 in the @ list, the
Agents panel and a project's team, 20 on a reply, in the Questions panel, in the To line and in the Agents
screen's Assistant pick. Each colour has a light value for
the dark theme and a deep one for the light theme (OKLCH 0.80 and 0.50, the same hue). The line is 3 in the 48
box and never under 1.5px; at 20 and under the Cloud draws six bumps and the Hexagon sharper corners. An agent
with nothing picked has all three worked out from its id, so it always looks the same. The picture is hidden
from screen readers beside the name and carries the name where it stands alone.
Roles such as Lead and Member use the normal sans-serif.

### Archive (F11)

The selected archive location is a folded Archived group after the active chats
in the sidebar. It starts closed, shows a count, keeps its fold while the window
is open and shares the chat-list scroll region. Archived rows are not indented;
More appears on hover and keyboard focus. Restore returns a
chat to its original project or Chats without restarting cancelled work.
The separate Archived button and flyout are retired. See
[the system page](designs/system.html#archive-stale).

The top line's More has Pause (Resume, or Restart after a stop), Rename, Archive and Delete, each after a
separator; a chat's row in the sidebar has Rename, Archive and Delete, and a project's Settings and Delete, in the
same shared menu. Rename opens "Rename chat": one Name field (60 characters), Cancel and Rename. A new chat is
titled by its first message's first line; from a first message of 40 characters or more, the background model
(Settings > Background) names it a moment later. The person's name for it is never replaced. An archived chat offers Restore
and Delete; its composer becomes a quiet Archived line with Restore. Deleting
the current chat selects its next sidebar neighbour, then its previous one,
then New. Other chats retain their drafts and reading position.

Confirm is a shared Dialog variant: "Archive <title>?" or "Delete <title>?",
Cancel and the verb. The title wraps, Cancel has initial focus, Escape cancels
and focus returns to the trigger. Archive uses the neutral primary Button;
Delete uses the outlined danger Button. A refused action keeps the confirm
open; "Archive failed" or "Delete failed" and its Diagnostics reference show in a corner card.
A confirm that needs it has a short body under the question, in muted 14px: a
count and what happens to it, then what is kept ("1 project and 2 chats will be
deleted. Files are kept."). Every thing named carries its count, never its name;
no line per fact, no paths. A project's or an agent's Delete (0069) takes its
body from the service's counts when it opens; the verb waits for them, and
counts that fail show in a corner card, with the verb off. No Undo.
Restore asks nothing; its failure shows in a corner card.

### Settings, accounts and agents

Settings is the centred dialog (0023), laid out as the owner's pick A of 2026-11-21 ("flat rows"): at most 1024
by 760px, 32px horizontal and 48px vertical clearance, no header bar. A left column (220 to 240px, a shade off the
dialog) has the word Settings at its top, then the pages in three groups under quiet 12px headings, each with a
16px lucide icon: App (General, Engines, Background), Team (Teamwork, Projects) and Diagnostics (Turns, Friction).
The rail's cog opens Settings at Engines. A
28px X (the shared icon button, named Close Settings) floats 12px from the dialog's top and right. Each page is a
550px column centred in the content side: a 24px regular-weight title with one muted line under it, then each
setting as a row between hairlines, its name and one muted line at the left and one compact 28px control at the
right. There is no ? Help in these pages: what it said is the line under the setting. Escape/the X return focus
and preserve the chat's draft and position.

General has Your name, then Theme (System, the default, follows the system's setting; Light or Dark), applied at
once without a reload, and Open at login, applied immediately with Switch, both for this computer only. The window
keeps the theme and the navigation and workspace widths across restarts. General ends with the version, with
Update while a release is ready, else Check.

Background has one row, Model: the small model, on one of the person's accounts, for work nobody waits on (it
names new chats). Its first choice is Gathr's own pick, or None when no account has a small model.

Teamwork has Pause after, Auto-compact and the Agent folder: the current path and Change. It
belongs to the service host. In the desktop window, Change opens the system folder picker at the current folder.
Selecting a folder saves immediately; cancelling saves nothing. Browsers and failed pickers use the path editor
under the row: Save/Enter saves; Cancel/Escape returns focus to Change without closing Settings. A refused save
shows its reason and error reference; the path editor also preserves the typed input. Changes apply to new work.
Current work stays where it is.

Engines manages named accounts and one Gathr-owned program per engine. No custom
path. An account's row has Sign in or Sign out, and its More menu has Rename and Remove. A new Gathr has no
accounts and does not reuse external logins. Agents is a screen of its own, as Plugins is:
every agent in its sidebar, the list with the Assistant dropdown and Add, and one page per agent with Name,
Description, Account, Model, Effort and Approvals as rows, each with one line under its name and no ? Help. The
page's 56px picture sits in a 64px box with a pencil on its corner; the box opens Shuffle, which picks a new shape,
eyes and colour, then Shape, Eyes and Colour. Shape and Eyes are the twelve choices each,
drawn alone in 40px boxes in the agent's colour, the one shown ringed; Colour is twelve 26px swatches, names for
screen readers only, the one shown ringed. A click redraws the picture at once and is saved with Save; one never
picked shows what the id worked out. A new agent's form starts with three picked as Shuffle picks them, saved with
it. No upload and no colour of one's own. Account implies engine;
model/effort choices come from that installed engine. Family Latest follows new
releases; numbered versions stay pinned. Do not ship prototype fixture models,
temperature or extra toggles as product settings. Save affects future turns in
all chats using the agent; running turns retain their assignment. Roles are
chat-specific, with one lead for two to five members.

Approvals is Ask, Edits, Auto (new-agent default) or Full, per 0029. The line under
its name is the chosen mode's sentence. The window has no ? Help anywhere. Switch remains the existing
accessible immediate setting, with pending save and referenced failure states.

Projects lists the projects, each row its folder and its team's faces; a row opens the project's own page, with a
back link to the list, as a tool's page in Plugins. A project's page has its name, folder and default team in one
draft, Delete (the outlined danger Button) at the title's right, and Cancel and Save at its foot. A project's
More in the sidebar has Settings, which opens that project's page, then Delete after a separator. An
agent's page has Delete at the right of its header.

Turns (Diagnostics) is a Settings page the whole width of the content side: a 214px turn list beside the
independently scrolling timeline. Friction, what agents reported got in their way, is a page of its own. Failure links open its matching turn and reset conflicting
filters. Preserve category/filter state and make long turns usable. No normal
header Diagnostics action. The prototype's component gallery is a reference tool.

### Recovery, focus and motion

Every failure the window tells shows as a small card in its bottom-left corner, just right of the rail (the owner's
pick of 2026-11-21): its title in bold, its reason muted, its reference, which opens Diagnostics, an action such as
Retry beside it, and an X named Close. Cards stack upward, the newest nearest the corner; a new failure from the same
place replaces its card. A card stays until it is closed or what failed succeeds or stops mattering; it never takes
focus and is announced as an alert. Over a dialog, the cards show above it. What the app only loads or reloads by
itself says nothing when it fails: the live socket reloads it.

A turn that stopped, waits for a sign-in or failed says so in words at its card's head (Stopped, Waiting for
sign-in, Failed), neutral and muted; its words stay. A message that couldn't be sent stays where it would be, the
person's bubble dimmed with a quiet Retry under it; its card has Retry too.

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
- Keep screenshots and fixture data invented; never port personal content.
- Do not replace accepted architecture, membership, safety or lifecycle policy
  with a prototype callback. Conflicts go to the decision/plan, not into styling.
- Do not claim the current production screens already match this reference.

## Check

Run `node designs/check.cjs`. Open `designs/system.html` at 1440 and 1024px,
dark and light (`#light` or Switch theme). Exercise composer, tool disclosure,
Settings categories, Help, Switch, Escape and keyboard focus.
No engine, account sign-in, real command or Settings service connects to this page. Its Conversation and
Recovery parts show the chat that was replaced: the room itself is seen on a made-up chat with `npm run dev` in
`ui/`, then `/room.html` (`#light` for light).
