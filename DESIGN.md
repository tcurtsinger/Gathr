---
name: Gathr
description: A desktop conversation workspace for a team of agents.
tokens: designs/tokens.css
colors:
  background: "#181818"
  foreground: "oklch(.985 .002 106)"
  card: "#202020"
  popover: "#202020"
  primary: "oklch(.922 .002 106)"
  primary-foreground: "oklch(.215 .003 106)"
  secondary: "#202020"
  muted-surface: "#202020"
  muted-foreground: "oklch(.708 .004 106)"
  accent-surface: "#202020"
  destructive: "lab(63.7053% 60.745 31.3109)"
  border: "oklch(.985 .002 106 / 10%)"
  input: "oklch(.985 .002 106 / 15%)"
  ring: "transparent"
  attention: "#60a5fa"
typography:
  body: { fontFamily: "Public Sans Variable", fontSize: "14px", fontWeight: 400 }
  composer: { fontFamily: "Public Sans Variable", fontSize: "15px", fontWeight: 400 }
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

Dark canvas is #181818; sidebar, workspace, composer and floating surfaces are
#202020. Light uses the prototype's warm near-white background and neutral muted
surface. `tokens.css` records both themes. Foreground carries text and primary
actions. `muted-foreground` is metadata; `muted-surface` is a background.
`border` separates regions; `input` bounds inputs; `ring` is transparent.
Blue is attention, links and active work, not an agent palette.
Errors combine destructive text, a tinted surface, an icon and a reference.

The prototype calls its neutral surface tokens `muted` and `accent`. Existing
Gathr code uses those names for text. Here they are `muted-surface` and
`accent-surface`; the old names remain compatibility aliases to readable text.
The runtime adapter maps these two names back for assistant-ui. Do not paste the
prototype's conflicting names into the existing UI. Other aliases: canvas to
background, surface to card, text to foreground, danger to destructive.

## Typography

Public Sans Variable, self-hosted under the SIL Open Font License, then Public
Sans and the platform sans-serif. The preview includes the Latin variable file;
other scripts use fallback fonts. No remote font requests. Body 14px; composer
15px; navigation and agent names 13px; text actions 12px; metadata 11 to 12px
monospace, except agent roles: those use the normal sans-serif. Headings 26px for
a new chat and 20px for a panel. Weights 400 and 500,
600 where emphasis needs it. Prose has relaxed line spacing; code stays monospace.

## Layout

- Navigation starts at 246px, resizes from 190 to 400px, and uses 190px below 760px.
- Chat header and workspace tabstrip align at 46px. The prototype's 40px preview
  strip is fixture chrome, not a Gathr requirement.
- Thread column is 52rem including 16px inner padding, leaving 800px at the
  normal 16px root size. Composer shares that content width. New chat centres
  its heading, optional project notch and composer.
- Workspace starts at 40% of available width, initially clamped to 480 to 760px.
  Resize minimum is 360px; desktop sizing preserves at least 360px for chat.
  At 1200px and below it overlays chat. Close stays at its far right.
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
Summary anchors 8px below and 8px inside the chat header corner, independently of
workspace width. Its width is 320px, padding 12px, radius 16px and maximum height
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
Six recorded adaptations are FileTree, SpeakerIdentity, Button, ElicitationForm,
OptionList and SubagentList. Retain source notices and original/adapted hashes.
Dependency approval, pinned versions and file splitting belong to the integration
decision; this design update adds no JavaScript dependency.

### Conversation and composer

Compose ThreadPrimitive, MessagePrimitive and the shared ChatComposer. The full
upstream Thread has no Composer override. ChatComposer uses the modular Composer,
attachments, mentions, slash actions, selection quoting and Send/Stop through the
runtime adapter. The recipient picker selects an agent, never its model or effort.
The recipient picker is decided: default to the Supervisor, shown by name first,
then the other members. Choosing another agent addresses it just like @Name.
There is no Everyone option. A chat without a project starts with one agent, its
lead. Choosing another agent adds it to that chat, up to five. This rule is to be
recorded in 0028 with F8; it does not introduce a fan-out callback.

Streamdown is the rendering reference, pending integration approval. Keep user
messages plain text; render agent Markdown, lists, headings, quotes, code, tables,
sources, files and tool results. Wide code and tables scroll inside the reply.
Raw HTML policy, link behavior and Diagnostics references stay governed by Gathr.
Tools are collapsed where they happened. MessageActions serve hover and keyboard
focus. Scroll-aware canvas fades appear only beyond the visible top/bottom edge,
pass pointer input through and leave the scrollbar clear.

### Team messages: split chat with Summary

The owner's split chat handoff (2026-11-10) supersedes the inline threads in
[0015](docs/decisions/0015-chat-design.md). This is the accepted target, not a claim
that every screen has shipped. The handoff HTML is a reference only: compose the
existing assistant-ui registry components and Gathr state in React; never port
its HTML or third-party runtime.

The main column holds the person's messages, the Supervisor's hand-offs and final
words, and direct @member exchanges. A header Columns2 toggle opens one running
team conversation on the right. Both columns get half the chat width, with
min-width:0 and independent scrolling. No per-hand-off disclosures or return
links remain. Every hand-off reply goes in the shared team column, in 0028's
turn-start order, with each turn's text and tools kept together. Keep the
assignment line above each reply. A late reply appears in that same chronology;
the Supervisor's next word stays at the bottom of the main column.

While split, sticky overlay labels name the Supervisor and Team. Each has a
44px height, a negative 44px bottom margin, 12px 24px top/side padding and a
canvas-to-transparent fade. They have no border or banner and pass pointer input
through. Content below a visible label starts at 48px, leaving 4px clear of the
label; the first assignment line never sits under it. This corrects the overlap
in the owner's prototype. The workspace still opens to the right of both columns,
with its existing sizing and overlay rules. Team content uses the same centred 52rem
maximum as the conversation. The composer stays in the Supervisor column.

The split toggle is a 24px TooltipIconButton, after Summary and before Open
sidebar. Tooltips are Show team chat and Hide team chat. Pressed uses a 9%
foreground tint. A new team message while closed adds an attention dot to the
toggle and beside that message's time until seen. Opening clears the header dot.
Retain split state per chat, both scroll positions, draft and recipient. Incoming
work never moves the reader. The reviewer's agreement stays a named reaction;
a question stays a reply. Agent bodies, tools and reactions indent 34px to align
with the speaker name (24px avatar plus 10px gap).

Waiting on team appears after the main column's latest message while the current
round has an open task and no Supervisor turn is running. Working appears at the
bottom of the team column for members with running turns. Both use an attention spinner
and overlapping 18px initials for members with running turns. Waiting on team
opens the split. Its time and tooltip use the earliest created_us of the round's
open tasks given by the Supervisor, plus created_ts of any own open wait, as
Summary does. A Supervisor cannot wait on its own team; no wait record is invented.
Never show a queued or pending task as a working agent.

### Agent plan and Summary

The official agent-plan registry element docks above the composer as a collapsible
bar, max-width 44rem, bottom margin 8px, radius 16px, popover surface and 6%
foreground border. Its 40px row shows state, title, segmented progress, done/total
and chevron. Expansion opens above the row, capped at 40vh. Rows show task state,
label and owner. Working detail is the latest tool call on that task; held or
blocked detail is the stored reason, omitted when absent. Agents have no free-text
progress record. A round follows given_in/task/answers links to one main-line
Supervisor turn, including nested tasks. Exclude person questions and cancelled
tasks; order by task number. Only done tasks count toward the fraction. Never invent a step
from prose. Until plan records exist, the title is Plan; afterwards it is the
recorded plan title. Do not infer titles, progress or review completion from text. The bar says Done
only after every task is settled and the Supervisor's hear-back has ended;
settled failures can leave the fraction below total, so the outcome stays visible.

Blocked steps use a static square-minus, Blocked and the stored reason. Failed
uses a destructive circle-exclamation and Failed. Stopped uses a muted square
and Stopped. Keep these outcomes separate from pending; none counts done or spins.
Their segments stay unfilled, with destructive color for failed. A mixed round
can still show running agents beside a terse outcome count (Lane working, 1 blocked).
Use stored task states only; never infer success from a finished turn.

Summary remains a 320px floating popover in the chat header, alongside the optional
team column. It is not a workspace tab. Use official SubagentList rows and
expandable TodoList assignments, without card borders. Each row shows initials,
name, role, actual state and More. States are Working, Ready or a named wait with
its recorded start. Indent the 3px progress bar and done/total count 34px; count
done tasks over that agent's tasks in this chat, excluding cancelled tasks.
The TodoList shows checked/struck-through done items, active spinners and pending
empty squares. No tasks says No assigned tasks, without a fabricated fraction.
Escape/outside click dismisses Summary; Escape returns focus to its trigger.

All plan steps, counts, working agents, assignments and timing come from
service tasks, turns and waits. Missing records never become inferred state.
The portable reference uses explicitly invented records only. Reduced motion
stops spinners and removes chevron and archive-lid transitions.

### Navigation and workspace

New, Search and Tasks sit above projects and their chats. Settings stays bottom
left. Archived uses Lucide Archive when collapsed and the same icon with its lid
lifted when expanded, never ArchiveX. Rotate the lid -14 degrees about its left
hinge (2px 8px), translate it up 1.5px, over 160ms ease-out.
Search and the pending human Tasks list share a contextual flyout, at most
340px wide, beside navigation. Escape, outside click, Close or the same trigger
dismisses it. Search matches titles and message text, supports arrow selection and
Enter, and retains its query. A task routes to its exact chat and task.

Workspace has a permanent Home tab, then Tasks, Changes, Terminal, Files, Web and
canonical document tabs as opened. Plus returns to Home. Each chat retains its
tabs, selection and tool state; canonical paths reuse a tab, terminals may have
several instances. Close other tabs retains Home and Recents. Arrow keys,
Home/End and Delete operate tabs. Summary and the optional team column replace
0015's earlier Team tab and inline thread. Other tab workflows remain
governed by their decisions.

Agents show their initials beside their name, replacing the shared robot icon.
Roles such as Supervisor, Member and Reviewer use the normal sans-serif.

Team status appears in Summary. SubagentList rows show chat role and named
state, with a TodoList disclosure for that agent's assignments in that chat.
No card borders around the inline rows. Progress must derive from real task
completion, not a decorative timer. States include Working, Ready and named waits
with their start times. No assignment says No assigned tasks. Plans have their own
identity and tab. A typed note is never approval.

### Human tasks

The navigation badge counts pending human tasks across chats. The workspace Tasks
tab shows the current chat's questions, one at a time, with title and source.
Only the Supervisor asks the person (the sole agent in a one-agent chat).
Inline OptionList stages single/multiple choices. ElicitationForm supplies the
optional Notes field or the open-ended answer. There is no duplicate Confirm or
automatic Next. Back/Next retain drafts; final Send submits ready answers only.
Notes alone do not make a structured answer ready. Unanswered tasks remain open.
Lifecycle/history policy and persistence are service work, not prototype rules.

### Archive and stale (F11)

The selected archive location is a folded Archived group after the active chats
in the sidebar. It starts closed, shows a count, keeps its fold while the window
is open and shares the chat-list scroll region. Archived rows use 34px left padding;
More appears on hover, keyboard focus and the current chat. Restore returns a
chat to its original project or Chats without restarting cancelled work.
The separate Archived button and flyout are retired. See
[the selected reference](designs/archive-stale/v3.html), a refinement of v1;
v1 and v2 remain unchanged as the compared proposals.

Header More keeps Pause/Resume/Restart first, then Archive, then Delete after a
separator. Row menus use the same shared menu. An archived chat offers Restore
and Delete; its composer becomes a quiet Archived line with Restore. Deleting
the current chat selects its next sidebar neighbour, then its previous one,
then New. Other chats retain their drafts and reading position.

Confirm is a shared Dialog variant: "Archive <title>?" or "Delete <title>?",
Cancel and the verb. The title wraps, Cancel has initial focus, Escape cancels
and focus returns to the trigger. Archive uses the neutral primary Button;
Delete uses the outlined danger Button. A refused action keeps the confirm
open with "Archive failed" or "Delete failed" and its Diagnostics reference.
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
and right, with a Close Settings tooltip; 0023). Below 768px categories move above
content. Escape/the X return focus and preserve the chat's draft and position.
Apply the new shared tokens, type and controls within that layout.

General starts with Theme for This computer: System (default, follows the system's
setting), Light or Dark, applied at once without a reload. Then Open at login for
This computer, applied immediately with Switch. The window keeps the theme and the
navigation and workspace widths across restarts.

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
one shared editor: Account, Model, Effort, Approvals. Account implies engine;
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

Diagnostics stays a Settings category: a 210px turn list beside the independently
scrolling timeline. Failure links open its matching turn and reset conflicting
filters. Preserve category/filter state and make long turns usable. No normal
header Diagnostics action. The prototype's component gallery is a reference tool.

### Recovery, focus and motion

App-level failures appear under the chat header in the shared Notice, with the reason, reference and Dismiss.

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
checks running, finished and late replies, split retention, independent scrolling,
plan progress, Summary and keyboard controls at both sizes and themes.
An optional output folder saves screenshots;
keep evidence outside the repo.
The old selected pages are retired as layout authority; see designs/README.md.
No engine, account sign-in, real command or Settings service connects to this page.
