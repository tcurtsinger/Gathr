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
- Thread column is 44rem including 16px inner padding, leaving 672px at the
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

### Team messages: inline thread with Summary

The main conversation holds the person's messages, the Supervisor's hand-offs
and final words. A hand-off has one collapsible team thread below it. Replies
belong to the message that gave their task, never to the nearest previous message.
Messages nobody answers have no thread. A direct @member exchange stays in the
main conversation. See [0015](docs/decisions/0015-chat-design.md).

The disclosure names who replied, who is working and who is waiting. Open while
work runs; fold when the Supervisor finishes, unless the person has opened or
closed it themselves. Retain that manual choice. Opening grows the conversation
in the page: no inner scroll box, height cap or separately scrolling reply list.
Indent replies with one neutral hairline; keep names, initials and normal-font
roles on every reply. Tools fold where they happened. Preserve 0028's turn-start
order within each thread; concurrent text never interleaves. Wide code and tables
may scroll horizontally. Thread disclosure and tool disclosure are separate.

A late reply updates its original thread in place, including the collapsed
summary. The Supervisor's next word arrives at the current bottom of the chat,
after any intervening conversation, with a link back to the hand-off. That link
opens the thread and moves keyboard focus to its disclosure. Updates alone never
move the reader's viewport or reopen a manually folded thread. Keep the draft,
recipient and reading position when toggling a thread or Summary. A reviewer's
agreement is a named reaction on the final word; a question remains a reply.

Summary stays available from the chat header for a quick glance at every agent's
role, actual state, named wait and assignments. It is the prototype's floating
popover, not a new workspace tab or permanent team pane. Opening an agent reveals
its tasks. Escape/outside click dismisses it; Escape returns focus to Summary.

These are selected presentation rules, not existing service capabilities. F5 must
record each task's originating turn and each turn's task before F8b groups replies.
Never infer those links or completion from prose, @mentions or arrival time.

### Navigation and workspace

New, Search and Tasks sit above projects and their chats. Settings stays bottom
left. Search and the pending human Tasks list share a contextual flyout, at most
340px wide, beside navigation. Escape, outside click, Close or the same trigger
dismisses it. Search matches titles and message text, supports arrow selection and
Enter, and retains its query. A task routes to its exact chat and task.

Workspace has a permanent Home tab, then Tasks, Changes, Terminal, Files, Web and
canonical document tabs as opened. Plus returns to Home. Each chat retains its
tabs, selection and tool state; canonical paths reuse a tab, terminals may have
several instances. Close other tabs retains Home and Recents. Arrow keys,
Home/End and Delete operate tabs. Summary replaces 0015's earlier Team tab, as
recorded in its accepted inline-thread refinement. Other tab workflows remain
governed by their decisions.

Agents show their initials beside their name, replacing the shared robot icon.
Roles such as Supervisor, Member and Reviewer use the normal sans-serif.

Team status moves to Summary. Inline SubagentList rows show chat role and named
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

### Settings, accounts and agents

Keep Gathr's accepted centred Settings layout (0023), not the prototype's example:
at most 1024 by 760px, 32px horizontal and 48px vertical clearance, a fixed header,
a 200px category rail and Close at top right. Below 768px categories move above
content. Escape/Close return focus and preserve the chat's draft and position.
Apply the new shared tokens, type and controls within that layout.

General includes Open at login for This computer, applied immediately with Switch.
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

Stopped, waiting for sign-in and failed-send notices stay at their affected reply
per 0023. Preserve partial text and drafts. Neutral for stopped/waiting; destructive
for failure, with the reference, Diagnostics and Retry. Errors end in failed.
Sign in opens Engines in the same Settings panel.

No focus rings anywhere, including the composer. Keyboard focus uses the same
quiet highlight as hover, so the current item stays visible. Keep native keyboard
behavior. Icon buttons have accessible names. Dialogs trap focus; Escape closes
the top layer and returns focus. Reduced motion removes movement. Keep registry hover/press feedback and
purposeful transitions, never animate a simulated service result into truth.

## Do's and Don'ts

- Capitalize Gathr. Keep one component for each repeated pattern.
- Use the prototype's real components with Gathr adapters and real state.
- Keep copy terse, action labels as verbs and detail behind Help/disclosure.
- Keep screenshots and fixture data invented; never port personal content.
- Do not replace accepted architecture, membership, safety or lifecycle policy
  with a prototype callback. Conflicts go to the decision/plan, not into styling.
- Do not claim the current production screens already match this reference.

## Check

Run `node designs/check.cjs`. Open `designs/system.html` at 1440 and 1024px,
dark and light (`#light` or Switch theme). Exercise composer, tool disclosure,
tabs, Summary, Settings categories, Help, Switch, Escape and keyboard focus.
Run `node designs/check-team.cjs` after the UI dependencies are available. It
checks running, finished and late replies, manual folding, keyboard links and
Summary at both sizes and themes. An optional output folder saves screenshots;
keep evidence outside the repo.
The old selected pages are retired as layout authority; see designs/README.md.
No engine, account sign-in, real command or Settings service connects to this page.
