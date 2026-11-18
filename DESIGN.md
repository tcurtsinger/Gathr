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
notice. In dark the chat is darkest and the frame lighter ([0068](docs/decisions/0068-the-windows-colours.md)):
chat #1b1b1b, frame #232323, card #2c2c2c, with floating surfaces
(the composer, popovers, the dialog, Help) at #2f2f2f. `raised` and
`danger-surface` are tints, so they lift whatever they lie on, and
`accent-surface`, a menu's highlight, is lighter than the popover. Light is a white chat with a
light-grey frame and cards and a border dark enough to see. Each step differs
from the next in both themes. `tokens.css` records both themes. Foreground carries text and primary
actions. `muted-foreground` is metadata; `muted-surface` is a background.
`border` separates regions; `input` bounds inputs; `ring` is transparent.
Blue is attention, links and active work, not an agent palette. `free` (dark `oklch(0.74 0.17 148)`,
light `oklch(0.54 0.15 148)`) is the Free mark's green alone: green means Free and nothing else
([0071](docs/decisions/0071-an-agents-state-is-one-mark.md)).
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
  every corner, ringed 1px in `shell-edge`, with 6px of shell on its right and below. The system's
  own title bar still sits above the top line; nothing in the page draws window buttons or leaves room
  for them.
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
  closed, 8px from the frame's end. These share one icon-button style: 28px, 8px radius, 4px apart,
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
floating surfaces 16px; ComposerBar 24px with 10px padding. User messages have
16px corners and fit their content. Agent replies remain unboxed.

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
agent, else a new chat's first agent), and @Name sends to that agent. Typing @ at a
word's start opens the composer's one menu above the box's left edge: a 24px face
with its mark, the name, and the role muted in a chat of two or more; in a chat
with no project, every other agent follows under "Bring in" (12px muted, a rule
above), up to five in the chat. Up and Down move the lit row, Enter or Tab picks,
Escape closes it, and a click picks; the lit row has no Enter mark. The name goes
in as "@Name ". Once something is typed, a quiet sign by Send says who gets it, in
12.5px muted with the name in the foreground at 550 and a bare 20px face: "To Ann",
or "Bo joins" for an agent naming brings in. There is no Everyone option.

Streamdown is the rendering reference, pending integration approval. Keep user
messages plain text; render agent Markdown, lists, headings, quotes, code, tables,
sources, files and tool results. Wide code and tables scroll inside the reply.
Raw HTML policy, link behavior and Diagnostics references stay governed by Gathr.
Tools are collapsed where they happened. MessageActions serve hover and keyboard
focus. Scroll-aware canvas fades appear only beyond the visible top/bottom edge,
pass pointer input through and leave the scrollbar clear.

### Team messages: one timeline

The owner retired the split chat on 2026-11-11
([0037](docs/decisions/0037-one-timeline.md)): the chat is one timeline in order, for one
agent or five. There is no team column, no header toggle and no split state. The approved
design (Shared/designs/chat-redesign) is a reference only: compose the existing assistant-ui
registry components and Gathr state in React; never port its HTML.

A turn is one block where it began, in 0028's turn-start order between turns, so replies
streaming at once never interleave and a later reply lands in its own turn's block. Only a
person's message sent into a running turn splits that turn: one block before it, one after.

One row above a turn's message holds everything it did first. Finished, it reads "Worked for
1m 49s · 6 steps · gave 2 tasks": 14px, muted, one weight, a part omitted when it is zero.
Running, its label is what the agent is doing now (Running a command, Reading a file), with
the registry's shimmer and no spinner, then the steps and the elapsed time, muted. Click or
Enter opens it: each tool call as a step in plain words from the tool's name, its duration at
the right, Failed in the danger colour on a failed one; a task it gave reads "Gave Cass a
task" with a "#119 Open the settings tools PR" chip that opens the task (a look-into task's chip reads
"#119 Look into: Survey tag libraries", 0063); what it wrote between
steps shows as muted lines. A step that kept what it ran
([0041](docs/decisions/0041-what-a-step-ran.md)) shows it in the same chip, in monospace: a
status icon, a short label, the chip, the duration ("Ran" then "git merge origin/main"; also
Read, Edited, Searched, Fetched, Searched the web, Ran a sub-agent). When the engine gave the
call its own words, they are the label ("Show what changed" then "git status --short"). The
chip is one line, cut with an ellipsis to the column it is in, never widening it, before the
label is cut; pointing at a cut chip, or focusing it (a cut chip is a tab stop), shows it whole in
the shared tooltip. A step that kept
nothing reads as before ("Ran a command"), and the row's own label while it runs does not
change. A thought ([0042](docs/decisions/0042-thinking.md)) is a step of its own, in order with
the others: "Thought for 9s", with no chip, no duration at the right and nothing to open; one
under a second is left out, and thoughts are not counted in "6 steps". Thoughts with nothing
between them are one step, with their time together: an engine thinks in pieces, and four steps
of "Thought for" read as a list of nothing. While a thought runs the
row's label is "Thinking"; when an engine says it thought only afterwards (Codex, on a model
that gives no reasoning summary), the row keeps its working label until the step arrives. The turn's last text is the message. A turn that only
wrote once has no row, nor has one whose only work was a thought under a second.

The lead's words read at 16px/400/1.6. A member's or reviewer's turn reads at 15px. A header
names the speaker when it changes from the block before: the avatar, the name, then for a
member or reviewer the task the turn works on ("on Open the settings tools PR", cut with an
ellipsis; its role when it has no task) and for the lead always its role, and the time at the right. Bodies, rows and cards indent 34px to align with the speaker name
(24px avatar plus 10px gap), with or without the header above them.

Messages agents write to each other are their own block: the card surface, a 6% foreground
hairline, 16px radius, 14px 16px padding, 14px between messages, text at 15px. Each is headed
"Juno to Orin" with its time. Messages that follow each other share one block, and an answer
sits beside the message it answers. Gathr's copy of a message in the receiver's turn is never
shown. The reviewer's agreement stays a named reaction; a question stays a reply. A reaction is the thumbs-up icon and a count of the agents who gave it, under the words it was given to, at their indent, in the type of an event line: 13px, muted. Hovering or focusing it shows who gave it, in the shared tooltip. The line has no label.

Everything Gathr tells an agent is a one-line event, with two exceptions that are never
drawn: the brief of a member's own task (Gathr's message that opens with "Task #<number>
from", in a turn on that task), because the member's header says the task and the giver's row
lists it; and Gathr's copy of an agent's message. A hear-back draws one line for each task it
reports, in the state that hear-back gave, not the task's state now: "Vale completed: Review
PR #138", "Cass is blocked: Fix the build", "Juno finished, waiting to merge: Bring the fix
up to date", also failed, stopped and cancelled. A task that ends done is such a line when it
ends, said by Gathr in no turn, and a hear-back after it doesn't draw it again (0051). The person's answer reads "You answered
Orin: Hold the button", a message sent with Discuss "You to Orin: Does it retry by itself?", a question "Orin asked you: Ship it?", or "Gathr asked you: …" for Gathr's own. The size of an ask is one too,
after the lead's turn that sized it: "Mira: Fix" or "Mira: Plan" (the lead's name), "You: Fix"
or "You: Plan" once the person changed it, "Plan" alone for an ask the lead hasn't sized, drawn
only once the ask has a task or Gathr held something of it (a refused give, write or push, or a caught change: 0063), with a wrench for Fix and a clipboard for Plan. Opened, it shows
the two sizes, "Just fix it." and "Plan it with me first.", the current one marked, and nothing
that explains them (0061). A change Gathr caught in a held turn is one too, of a kind apart (`changed`, the `FilePen` icon): "Mira changed 3 files before the plan was approved" ("on look-into task #12" for a look-into task; "1 file" for one), opening to one file per line, at most 20, then "and 2 more", then "Committed: 1" when some came from commits (0063). It reads after the turn's last block, not as a note. When a repair's work is in, Gathr says "Fixed: <the ask>", a check
icon, which opens to each finished task with its PR and its proof. The plan's lines are one
too, of a kind apart from the size line, with a clipboard: the lead-in "Plan v1:" and "Bo agrees",
"3 findings" or "Bo didn't finish" (opening to the plan's title), "Sent back" (opening to the
person's words), and "Plan v2:" "Approved" or "Dropped"; each opens to the whole line and Open plan
(0062). Anything else (a wait's
result, a wake-up, a stale-task nudge, a usage interruption) is drawn in Gathr's own words.
13px, muted, the lead-in in the foreground colour at 500, an icon first, no Gathr speaker.
Closed, a line stays on one line, with an ellipsis (…) for overflow. Click or Enter opens it
in place: the whole line, the reason or the question, and Open task. Click or Enter again
closes it. Keyboard focus uses the hover highlight, without an outline or ring.

An approval keeps its card where the agent asked. Waiting approvals also dock above the
composer, and a plan waiting on you after them: an attention border at 50%, 16px radius, the popover surface; the agent's face,
"Needs you · 1 of 3" in the attention colour, "Vale wants to run" and the command or tool in
a code chip, then Show, Deny and Allow. It shows the oldest. Show scrolls to the card and
focuses it. An answer being sent leaves the dock; a refused answer brings it back. With no approval
waiting, the dock shows the oldest plan: the lead's face, "Needs you · 1 of 1", "Mira's plan:
Tags on notes · 9 parts" and one button, Open, which opens the plan's tab at its newest version.
The count is approvals and plans waiting on you; a plan still being checked isn't one.

Working appears after the latest message while any turn runs: an attention spinner, Working,
overlapping 20px faces for agents with running turns, and the time since the longest-running
of those turns began, in whole minutes (12m), redrawn once a minute. The time is hidden
from screen readers, so the status reads only Working and who. Never show a queued or pending task as a working agent.
In a chat with more than one agent the line is a button that opens Summary, and closes it when pressed again;
it says it opens a dialog and whether it is open, and who is working is announced from beside it, not by a live
region around the button.

### Conversation map

The registry's ConversationMap as published ([0039](docs/decisions/0039-conversation-map.md)): a 24px rail
14px inside the chat pane's left edge. Its length is at most half the pane's height below the header and
never more than 616px (44 ticks at 14px); it is centred in that height, with the same space above it as
below it down to the pane's bottom edge. It shows from three ticks up and only with 44px left of the
column, so it never lies over the text, the composer, the approval dock or the scroll-to-bottom button.
Only its ticks take the pointer.

- Ticks: 12px long and 2px thick for a message from the person (foreground 22%), 6px for the lead's reply to
  them (15%), 12px long and 4px thick for a folded day (22%). On screen 50%; current 3px thick at 90%.
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

The button, and the Working line after the latest message, open a 348px popover under the top line.
Escape closes it and returns focus to whichever of the two was last used to open or close it; a press
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
Summary, and from an agent's face in the timeline. 20px 24px 32px padding, 24px between sections, each under a 13px muted
heading; a section with nothing in it is left out, and an agent with nothing at all reads "Nothing in
this chat yet".

Its face in the timeline is a button the face's size, which takes the hover tint. An agent that has
left the chat keeps its words, and its steps in Plan, but has nothing to open.

- **Header:** the 36px face with its mark, still, the name at 20px/600, and under it the role
  and state in 13px muted: "Member · Working since 16:06" with Working in the attention colour, "Reviewer
  · Needs your approval", "Supervisor · Waiting for Juno since 09:42", "Member · Free". Start times
  live here. At the right, the context bar once the engine has reported it, and "Message Juno", which
  puts "@Juno " first in the composer, unless its text already goes to Juno, and focuses it. An archived chat, or an agent that can't be sent to, has no Message.
- **Task:** the task its running turn works on, else its first open one: the title, "From Orin" and
  Open task, which opens it in the Tasks tab.
- **Now:** its running turn's row, the timeline's own, with its steps showing. A turn that has done
  nothing yet reads Working; one that only waits on the person has no Now.
- **Earlier in this chat:** its earlier blocks, newest first, drawn by the timeline's own components
  at 15px, each headed by its name and time: its turns, and what it wrote to other agents. The latest
  10; older ones stay in the chat.
- **Tasks in this chat:** the official TodoList, open tasks first, five before Show more: checked and
  struck-through done items, active spinners, pending empty squares. A title opens the task.

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
rows, the dock and a project's team, 20 in a stack and 16 on its tab. The face is hidden from screen readers
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
"Plan v2" with a close button. The tab reads top to bottom as the page of a task does, in the same
classes: a refusal in the shared failure notice, "Plan · version 2" (and the versions as links
`v1 · v2`, the one shown as text), the title, the lead's name, the time and one state word when it
isn't waiting on you (Checking, Revising, Approved, Dropped, or Replaced on an older version), a
quote of what you asked ("You asked", muted), the plan's text, one field, Check ("Bo agrees", "3
findings from Bo", "Bo is checking", "Bo didn't finish"), then "9 parts": an ordered list, each
part whole, its number and what, the owner and what it comes after muted, and Why, Done when,
Proof, Where and Size as text lines. After Send back, the shared inline form with one required
multiline field, "What to change". The foot has three cells: Drop (ghost) at the left on the newest
version while the plan is open, "Saving" in the middle while a request runs, and at the right, on
the newest version once it has reached you, Send back (outline) and Approve; after Send back,
Cancel (ghost) and Send back, disabled until the field has words. No sentence explains a button.
The foot's buttons fit at the workspace's narrowest, 360px, in both states. The Check field also lists the version's Findings (a suggestion is left out) in a task's
note format, "Wrong · src/notes.rs: ...", the lead's Answers under the findings they answer
("Changed: ..." or "Kept: ..."), the ask's Questions, each with its answer or "Open", and Changed, one row for each turn of the ask in which Gathr caught a change, "Ann: src/notes.rs, src/tags.rs", shown only when there is one. Each
part is a line that opens to its brief, one at a time and none at first: Why, Done when, Proof,
Where, Size and After, in the classes of a task's fields. Against the version before, a part
carries a mark in the attention colour at the end of its line, Changed or New, matched by what it
says, so a part moved in order is not changed; a Changed part's open brief shows Was under each
field that differs, in muted struck-through type; a part that went is a muted line after the parts,
Removed, its words struck through. Version 1 has no marks. The tab carries the attention mark
while the plan waits on you, as Tasks does.

### Human tasks

The navigation count shows pending human tasks across chats, in the attention
colour. The workspace Tasks
tab shows the current chat's questions, one at a time, with title and source.
Only the Supervisor asks the person (the sole agent in a one-agent chat).
Inline OptionList stages single/multiple choices. ElicitationForm supplies the
optional Notes field or the open-ended answer. There is no duplicate Confirm or
automatic Next. Back/Next retain drafts; final Send submits ready answers only.
Notes alone make any question ready and go as the answer in words, so "none of these" can be said. Unanswered tasks remain open.
The first choice of an agent's choice question reads "<lead>'s pick", then " · " and its detail
("Mira's pick · it comes back by itself"); Gathr's own question has no pick.
Under the answer of a question an agent gave, a row holds the label "Discuss" and the shared
Switch. On, the choices hide and the one field is "Your message", required, for every form; Send
sends it with the other answers. Off again, what was picked is still picked. Gathr's own question
has no switch. After Send the question leaves the list, the count and the chat's dot until the
lead's turn that got the message ends, or is replaced by a new question (0065).
After an accepted answer, show the shared answer line at the asker's wake-up turn
in that agent's column. It stays after reload. Main-column team completion reports
stay hidden.
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
