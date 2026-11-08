---
name: Gathr
description: Desktop chat shell with a Team panel for multi-agent work.
tokens: designs/tokens.css
colors:
  canvas: "#191919"
  surface: "#222222"
  raised: "#303030"
  text: "#e8e8e6"
  muted: "#b0b0ad"
  border: "#383838"
  accent: "#a6c4df"
  focus: "#a6c4df"
  control: "#777774"
  danger: "#ffb4b4"
  danger-surface: "#382525"
  danger-border: "#885454"
  backdrop: "#00000088"
typography:
  body: { fontFamily: "Segoe UI Variable", fontSize: "15px", fontWeight: 400 }
rounded: { sm: "7px", md: "16px", composer: "22px" }
spacing: { sm: "8px", md: "16px", lg: "32px" }
components:
  composer: { backgroundColor: "{colors.raised}", rounded: "{rounded.composer}" }
---
## Overview
Operate mode for local desktop agent work. Variance 2, motion 1, density 4. The selected Team panel keeps activity beside a single conversation. No production UI exists yet.
## Colors
Neutral charcoal canvas, surface for sidebar and user messages, raised composer. Off-white text, secondary muted text. Borders separate regions; control is the stronger outline for small controls. Blue accent only for focus, links and selected reactions. Light theme: canvas #faf9f6, surface #f0efeb, raised #e6e5e0, text #292927, muted #63635e, border #dbdad4, accent/focus #365b7b, control #85857f.
The backdrop token is #00000088 (black at 53.3% opacity) in both themes. Dialogs and the narrow navigation drawer share it; content and controls remain opaque.
## Typography
Segoe UI Variable then Segoe UI. Prose 15/24.75, sidebar 13, controls and metadata 12, empty title 28. Weight 400 to 600.
## Layout
248 px project sidebar, conversation max 760 px, optional 330 px side panel. At 1024 the sidebar is 205 and panel 285. Below 768 px the sidebar folds into a native popover drawer opened by the topbar sidebar icon. Its matching Close control, Escape or outside click dismisses it and returns focus to the opener. The panel overlays. Desktop is the target; small-width reflow keeps controls reachable.
## Elevation
No shadows in the shell. Dialog is the sole blocking layer.
## Shapes
7 px controls, 18 px content-sized user messages (10 px vertical, 16 px horizontal padding), 22 px composer, circular agent monograms. Agent replies have no containing box.
## Components
Shared sidebar, transcript, tool disclosure, message actions, reaction, composer, context control, task row, agent row, side-job disclosure, panel tabs. All four screens use designs/components.css and designs/tokens.css. New chat and setup differ because their job is configuration. Multi-agent chats add a Team tab: named agents, roles, tasks, side jobs and waits. Single-agent chats omit it. New chat centres the composer; Choose agents configures membership and roles before opening the conversation.
## Do's and Don'ts
Use one chat model with optional project. Capitalize Gathr. Keep replies plain. Fold tool runs. Name waits and start times. No overview, separate room navigation, tinted canvas, agent portraits, or explanatory subtitles. Sample data only.
## Check
Run node designs/check.cjs. Open each HTML locally; #light selects light mode. All actions are local prototype interactions. No engine, filesystem folder picker, terminal process or settings service is connected.

## Patterns and states
Use + New and Search with matching 16 px outline icons, no shortcut hint. Icons use currentColor and a 1.5 px stroke. Keep the product name in the sidebar only. User messages size to their text; message actions appear below on hover or keyboard focus. Never reserve action-row height inside the message bubble.

Tools use native details disclosures. Panel tabs share one content region. Message actions, reactions, queue removal, send, pause and context summarisation provide local feedback. Role selection allows one to five agents; two or more require a Supervisor; Reviewer is optional and distinct. Disabled Create reflects invalid membership. Every setting must ultimately have a Settings control. The Phase 1 previews open the centered Settings panel.

Focus uses a 2 px accent outline with 2 px offset; in the composer it goes around the rounded box, not the text area inside. Native buttons, selects, checkboxes and details preserve keyboard operation. Escape closes the dialog or panel. Reduced motion removes transitions. At narrow widths, sidebar and panel controls reveal their regions; touch targets grow to 44 px. Error copy ends in failed, confirms use a short question, Cancel and the verb. No helper subtitles.

Help (approvals/v1, `ui/src/components/Help.tsx`): a setting's detail sits behind a plain ? in a 16 px circle, in a 28 px muted button beside its label (44 px at narrow widths), named "About <setting>". Click, Enter or Space opens a popover up to 350 px wide, below the ? or above it when there's no room, inside the window; it doesn't move the form. Escape or a click outside closes it before Settings, and focus stays on the ?. No hover-only content or title tooltips.

## Token ownership
The YAML above records dark values; light overrides live in designs/tokens.css. Use canvas for the conversation, surface for the sidebar and user bubbles, raised for the composer and hover, text for primary copy, muted for metadata, border for separators, control for input outlines, accent for links and reactions, focus for keyboard focus. Separators are not control boundaries. No independent per-screen palettes or duplicate shared components. The runtime frontend must implement these as shared components when it is built.

Shared styles enter through `designs/components.css`, which imports shell, conversation, panels and responsive rules in cascade order.

## Settings and Diagnostics
Selected previews: engines/v4.html, diagnostics/v3.html and chat-states/v3.html.
Settings opens over the current screen as a native modal dialog, at most 1024 by 760 px,
with 32 px horizontal and 48 px vertical clearance. The header stays fixed; categories
sit in a 200 px left rail. Close and Escape restore focus to the opener and preserve
chat position, draft and form fields. Below 768 px categories move above content.
Shared rules: settings-overlay.css and phase1-selected.css; shared interactions:
phase1-selected.js. Preview selectors and simulated completion never ship.

Settings entry: Lucide settings cog plus Settings at the bottom left, one click (settings-entry/v5).
Use the app's 18 px icon, 24 px viewBox, 1.5 px stroke and rounded caps and joins; retain its ISC notice.
Engines lists named accounts. Each row shows name, engine, sign-in state, Rename
and Sign in or Sign out. Add asks for engine and name, then starts sign-in.
Gathr owns one program per engine; version and updates appear once per engine
below the accounts. There is no custom path or engine source control.
Each account keeps its own login folder, per 0002. Updating a program leaves
account assignments and logins intact. The preview simulates login completion.
The accounts layout, entry and account model are approved.
Diagnostics is another category,
with a 210 px independently scrolling turn list beside a scrolling timeline. Filters
stay above both. Forty-eight turns and a fifty-tool-call turn are the size fixtures.
Below 768 px the turn list sits above the timeline. Only an error link in the
chat and Settings > Diagnostics open Diagnostics; no header or chat menu entry.
An error link opens Diagnostics filtered to the affected turn.
Clearing Find restores all turns. Switching categories preserves the filtered view.
Opening a failure reference resets the status filter so the affected turn is visible.
References in the timeline simulate Copy, without using the system clipboard.

## Rich cards and scrolling
Design, image and file cards show a still preview until opened. Nothing off screen
keeps animating. Chats must scroll smoothly with 500 messages and dozens of cards;
the app window PR includes scroll timings as proof.

## Reply notices
The shared notice occupies the reply width below the related user message. Stopped
sits after any retained partial reply; Continue fills the composer. Waiting for sign-in
uses the same neutral surface, with Sign in opening Engines. A failed send uses
Send failed, its reference, Diagnostics and Retry at the right. Retrying disables
Retry and changes its label to Retrying in this simulation. A new preview selection
resets it. Production completion replaces the notice with the reply or a new failure.
The draft remains intact when opening Settings or retrying.

Failures alone use danger, danger-surface and danger-border. Dark values are in the
YAML; light values are #a62626, #fff3f2 and #d49a98. Neutral notices retain surface,
text and control tokens. This is a semantic error role, separate from the blue action
accent. Normal replies remain unboxed. No service integration is claimed.

## Reply formatting
An agent's Markdown is formatted (0027) with the shared `Markdown` component: body
type for paragraphs and lists, 20, 17 and 15 px semibold headings, accent links that
open in the person's browser. Code, code blocks and tables sit on surface with
border lines; code is 13 px monospace, 12 px inside blocks. Quotes take a 3 px border
and muted text. Wide code blocks and tables scroll sideways inside the reply width.
Raw HTML shows as text. User messages stay plain text.

## Agent selection and editing
The composer selects an agent. It has no model or effort control. Each agent owns
its engine, account, model and effort. Selecting another recipient never changes
those assignments. Selected shell previews: chat/v14, single-chat/v12,
new-chat/v12 and start-chat/v13. Approval and context controls keep their own jobs.

Selected: Settings > Agents (agents/v1), in the centred Settings panel with its
category list at the left. Edit agent in the chat's agent picker opens this same
editor at the selected agent. It is unavailable while Everyone is selected.
The inline Team editor (agents/v2) and separate chat menu editor (agents/v3)
were not chosen. There is one editor for single and team chats.

Save updates the reusable agent everywhere it is used. There is no per-save Scope
control. Future turns use the new assignment; running turns retain theirs.
Roles remain chat-specific. Decision 0026 records the selected design; engine
integration still needs its approved implementation plan.
The editor has one Account picker, then Model and Effort. The account implies
the engine; there is no separate Engine picker. Account changes refresh models
and effort and reset dependent selections to the engine default. Signed-out
accounts stay labelled and selectable; sending asks for sign-in.
Models are searchable and independently scroll;
the chosen model stays visible above the list. No free-text model identifier.
Production choices must come from the installed engine, including supported
effort values and unavailable-model states, pending Cass's capability research.
Family Latest follows new releases in that family without editing each agent; a
numbered version stays pinned. Engine default follows the engine's own default,
which may differ from the selected family. Latest is the default for new
agents when supported. The sample Opus Latest and Opus 4.6 choices illustrate
these assignments only; actual discovery and resolution await Cass's research.
Approvals goes below Effort: Ask, Edits, Auto or Full (0029), Auto for a new agent, each mode's detail behind its Help.
Cancel and Escape discard form changes and restore focus. Save gives local feedback.
Preview changes last only while the page remains open. The form and native dialog
reuse Gathr's tokens, focus, button and Settings shell patterns. No new palette.
