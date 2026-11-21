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
sidebar, a composer and a resizable sidebar beside the chat. No Rooms or Overview screen.

The source record and component map are in [prototype-reference.md](designs/prototype-reference.md).
The portable reference is [system.html](designs/system.html). It demonstrates
tokens and static component anatomy, not replacement runtime components.

## Colors

Surfaces take four steps. `shell` is the window's own surface, under the rail and the top line and
around the frame ([0070](docs/archive/decisions/0070-the-windows-shell.md)): #2c2c2c in dark, a grey darker than the
frame in light, with `shell-edge` (7% foreground) as the frame's 1px ring. `frame` is navigation and the
workspace. `background` is the chat. `card` is what stands on the chat: an approval, a
notice. A message is no card: it stands in a bubble, the foreground at 8% over the chat
([0072](docs/archive/decisions/0072-the-chat-reads-like-a-team-chat.md)). In dark the chat is darkest and the frame lighter ([0068](docs/archive/decisions/0068-the-windows-colours.md)):
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
light `oklch(0.54 0.15 148)`) is the Free mark's green
([0071](docs/archive/decisions/0071-an-agents-state-is-one-mark.md)) and Done on the board; `review` (amber) is
the board's In review.
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
  More stays visible. The right holds the questions button, then the sidebar's button, open or
  closed, 8px from the frame's end (from the window's buttons on Windows). These share one icon-button style: 28px, 8px radius, 4px apart,
  17px Lucide icons at 1.75 stroke (More's dots radius 1.4). Pressed (the sidebar's button
  while it is open): foreground 9% background, foreground icon, 12% on hover. The chat has no header of
  its own: the thread starts at the frame's top.
- Thread column is 42rem including 16px inner padding, leaving 640px (40rem) of text at
  the normal 16px root size (0037). A new chat's box
  keeps the column's content width. A new chat centres the face of who its message goes to (the assistant, or
  the picked project's lead; none without one) at 56px over a greeting by the computer's clock, "Good morning,
  Sam" (05:00), "Good afternoon" (12:00), "Good evening" (17:00), "Still up, Sam?" (00:00), the person's name
  from Settings > General; while the @ list is open they sit at the column's top.
- Workspace starts at 40% of available width, initially clamped to 360 to 760px.
  Resize minimum is 360px. It always sits beside the chat, never over it, and leaves the
  chat at least 360px: when the window narrows, the workspace gives way first, down to
  360px, then navigation. Only the drawn widths give way; widths the person dragged come
  back when the window widens. Its tabs sit in a 38px row of their own at its top, tabs 28px high.
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
floating surfaces 16px; ComposerBar 24px with 10px padding. Every message, the person's and
an agent's, is a bubble with 18px corners and 8px 13px padding that fits its words (0072).

## Components

Use actual assistant-ui primitives and the recorded official registry components
after the integration decision is approved. Do not build lookalikes in `ui/`.
The prototype gallery contains 64 examples, not 64 new Gathr service capabilities.
`ui/registry.json` records each adaptation with its original and adapted hash and why:
Button today. Retain source
notices and both hashes.
Dependency approval, pinned versions and file splitting belong to the integration
decision; this design update adds no JavaScript dependency.

### Conversation and composer

Compose ThreadPrimitive, MessagePrimitive and the shared ChatComposer. The full
upstream Thread has no Composer override. ChatComposer uses the modular Composer,
attachments, mentions, slash actions, selection quoting and Send/Stop through the
runtime adapter. There is no recipient picker (C24, 0028): the box's hint names who
gets a message that names nobody in a new chat, "Message Mira" (the assistant, or the project's lead), and
reads "Message the team" in a chat that has begun. Typing @ at a word's start, anywhere in the
message, opens the composer's one menu above the box's left edge: a 24px face, the name, its description
muted, and the role muted at the right in a chat of two or more; in a chat with no
project, every other agent follows under "Bring in" (12px muted, a rule above). In a new chat with no project
the assistant comes first, marked "Assistant", and nobody is under "Bring in". It is seven rows tall, then scrolls. Up and Down move the lit row, Enter or Tab picks, Escape
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
20px faces, overlapping by 5px, each after the first ringed 1.5px in the box's own
colour (the mock-up's `#who` A): "To Ann", "To Ann and Bo", "To Ann, Bo and Cy";
agents picked from outside a chat with no project are said apart, "Bo joins", "Bo
and Cy join", "To Ann, Bo joins". The names end in "…" where the row runs out (the
360 column), the faces stay. Each agent named gets the message.
There is no Everyone option. Sent, the message keeps its picked names, and its bubble draws each in the same attention blue; a typed
@Ann stays plain, as does every name in a message from before picks were kept.

Streamdown is the rendering reference, pending integration approval. Keep user
messages plain text; render agent Markdown, lists, headings, quotes, code, tables,
sources, files and tool results. Wide code and tables scroll inside the reply.
Raw HTML policy, link behavior and Diagnostics references stay governed by Gathr.

### State marks

An agent's state is one mark on its face ([0071](docs/archive/decisions/0071-an-agents-state-is-one-mark.md)), the
same on every face of it in a chat: the most pressing that holds of Needs you (a blue dot), Can't run (a red
disc with !), Working (a three-quarter blue ring), Stopped (a grey disc with a square; a plain square at 24 and
under), Blocked (a grey disc with a bar), Waiting (a grey disc with a clock's hand) and Free (a plain green
dot). It sits at the face's top right, 1px over its edge: 12px at 36, 10px at 28, 9px at 24, 8px at 20, cut
out of the surface behind it by a ring of that surface's colour (2px at 36 and 28, 1.5px below), so it reads on
any background a face is drawn on. Faces of 20 to 36px carry it; Settings' faces don't. A Working mark turns once in 1.6s on every face of an agent at work,
its older messages too (the owner's call of 2026-11-19: a ring standing still read as stuck); under reduced
motion it is still.

A chat's sidebar row shows its most pressing of Needs you, Can't run and Working as the
same mark at 9px, Working turning as on a face, 8px after its cut title.

### Navigation and workspace

New and Search sit above projects and their chats; Search shows no key words, and Ctrl K (Cmd K
on a Mac) still opens it. Settings and Update are on the rail. Rows are 30px; the word mark's block
36px, 8px under the frame's top. Each project is a small muted header like Chats, with no icon, its chats
under it; a click folds it, and folded it shows its count. A chat's row is one 7px-rounded box that greys as
a whole on hover; its More, and a project header's + and More, are 24px icons with no box of their own, shown
only on hover, keyboard focus or while the menu is open, the open chat's row too (C70, C71). A chat with
something going on carries one 9px mark 8px after its title, which is cut before it (0040, C65, 0071): the
faces' own Needs you, Can't run or Working mark, the most pressing of the three, still. The mark never moves
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

Agents show their face beside their name, replacing the shared robot icon (0074): a drawn face on the
background the person picked, the same everywhere, at 28 on a card, 24 in the @ list and a project's team,
and 20 on a reply and in the To line. The face is hidden from screen readers
beside the name and carries the name where it stands alone. The background shows as picked, the same in light
and dark; until one is picked it is a grey, #CECECE.
Roles such as Supervisor, Member and Reviewer use the normal sans-serif.

### Archive (F11)

The selected archive location is a folded Archived group after the active chats
in the sidebar. It starts closed, shows a count, keeps its fold while the window
is open and shares the chat-list scroll region. Archived rows are not indented;
More appears on hover and keyboard focus. Restore returns a
chat to its original project or Chats without restarting cancelled work.
The separate Archived button and flyout are retired. See
[the system page](designs/system.html#archive-stale).

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
credit: "Faces", then "Dylan! The Avatar Generator by Natalia Spivak, CC BY 4.0, changed", the work
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
one shared editor: Face, Skin tone, Background, Account, Model, Effort, Approvals. Face is the agent's
56px face (DiceBear's Dylan; the seed picks hair, hair colour, mood and facial hair) and Shuffle, which draws it
from a new seed and keeps the tone shown, Upload, which shows a picture from the computer in its place, cut to its
middle square, and Remove, which brings the drawn face back; Skin tone is six swatches,
light to deep, named Skin tone 1 to 6; Background is twelve 26px swatches (Rose to Orchid, round the wheel, names
for screen readers only) and a last + swatch, dashed with a plus, that opens the shared colour picker under the
row: a saturation and brightness square, a hue bar, the colour now and its hex, on the popover surface. A colour
not among the twelve shows on the +, pressed, and stays on it, not pressed, while a ready colour is picked. Escape or a click outside closes the picker, not Settings. Each
redraws the face at once and is saved with Save. A tone never set shows what the face's seed picked;
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
its name, folder and default team in one draft with one Save. A project's More in the sidebar has Project settings, which
opens Projects at that project, then Delete after a separator. Projects and Agents
have Delete, the outlined danger Button, at the left of the form's foot; Agents'
Delete replaced Remove and its inline question.

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
- Keep screenshots and fixture data invented; never port personal content.
- Do not replace accepted architecture, membership, safety or lifecycle policy
  with a prototype callback. Conflicts go to the decision/plan, not into styling.
- Do not claim the current production screens already match this reference.

## Check

Run `node designs/check.cjs`. Open `designs/system.html` at 1440 and 1024px,
dark and light (`#light` or Switch theme). Exercise composer, tool disclosure,
Settings categories, Help, Switch, Escape and keyboard focus.
No engine, account sign-in, real command or Settings service connects to this page.
