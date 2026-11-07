---
name: Gathr
description: Round 3 fixed shell, provisional until selection.
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
typography:
  body: { fontFamily: "Segoe UI Variable", fontSize: "15px", fontWeight: 400 }
rounded: { sm: "7px", md: "16px", composer: "22px" }
spacing: { sm: "8px", md: "16px", lg: "32px" }
components:
  composer: { backgroundColor: "{colors.raised}", rounded: "{rounded.composer}" }
---
## Overview
Operate mode for local desktop agent work. Variance 2, motion 1, density 4. This records the owner's fixed shell, not approval of a multi-agent option. No production UI exists yet.
## Colors
Neutral charcoal canvas, surface for sidebar and user messages, raised composer. Off-white text, secondary muted text. Borders separate regions; control is the stronger outline for small controls. Blue accent only for focus, links and selected reactions. Light theme: canvas #faf9f6, surface #f0efeb, raised #e6e5e0, text #292927, muted #63635e, border #dbdad4, accent/focus #365b7b, control #85857f.
## Typography
Segoe UI Variable then Segoe UI. Prose 15/24.75, sidebar 13, controls and metadata 12, empty title 28. Weight 400 to 600.
## Layout
248 px project sidebar, conversation max 760 px, optional 330 px side panel. At 1024 the sidebar is 205 and panel 285. At small widths the sidebar folds away and panel overlays. Desktop is the target; small-width reflow keeps controls reachable.
## Elevation
No shadows in the shell. Dialog is the sole blocking layer.
## Shapes
7 px controls, 18 px user messages, 22 px composer, circular agent monograms. Agent replies have no containing box.
## Components
Shared sidebar, transcript, tool disclosure, message actions, reaction, composer, context control, task row, agent row, side-job disclosure, panel tabs. All candidates share these. New chat and setup differ because their job is configuration. Only multi-agent activity placement varies: v7 strip, v8 Team panel, v9 pinned turns.
## Do's and Don'ts
Use one chat model with optional project. Capitalize Gathr. Keep replies plain. Fold tool runs. Name waits and start times. No overview, separate room navigation, tinted canvas, agent portraits, or explanatory subtitles. Sample data only.
## Check
Run node designs/check-round3.cjs. Open each HTML locally; #light selects light mode. All actions are local prototype interactions. No engine, filesystem folder picker, terminal process or settings service is connected.
