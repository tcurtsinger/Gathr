# Approvals help
Selected: **v1, pop-up**. Refinement of Settings > Agents at e50dace.
Audience: a person configuring a reusable agent. Job: choose how freely it acts.
Operate: variance 2, motion 1, density 4. Existing tokens, type and form layout.

Now reproduces the current form structure with sample data.
v1 (selected): click-open question-mark popover, compare four modes without moving the form.
v2: question-mark disclosure below the select, reflows the form in the reading order.
Approvals goes below Effort. Ask, Edits, Auto, Full. Auto is the initial selection.
No service connected. Save feedback is local. Twelve generic agents and model choices are illustrative fixtures.
Add and Remove are disabled in this focused preview.

## Selected shared pattern
Plain ? in a 16 px circle, muted, in a 28 px button beside the label (44 px on narrow layouts).
Hover uses raised; focus uses the existing 2 px focus ring. Accessible name: About approvals.
Click, Enter or Space toggles; no hover-only content or title tooltip.
Content is a named region, not a tooltip.
v1: native auto popover, Escape, outside click or opener dismiss it before Settings closes.
Keep it within viewport, maximum 350 px wide. Escape restores focus to opener.
v2: aria-controls and aria-expanded. Escape collapses help and focuses opener.
Opening help changes no setting. Reuse `ui/src/components/Help.tsx` and its `help.css`.
DESIGN.md describes it; designs/system.html links the runtime stylesheet.
v2 is retained as the unselected alternative. Shown versions remain unchanged.

## Copy
 Approvals 
 Ask Ask before actions that need permission. 
 Edits Allow file edits in the worktree. Ask before other actions that need permission. 
 Auto The engine checks actions. Pushes and file-tool writes outside the worktree ask. 
 Full Run actions without asking. 
 Auto does not confine shell commands to the worktree. 
Copy matches `ui/src/settings/Agents.tsx`. Decision 0029 records the service semantics
and limits. Auto blocks are refusals reported to the agent and logged, not new approvals.
The git hook asks for scripted pushes too; bypass routes are listed in 0029.
The folder guard covers file tools, not every shell command.

## Check
Open Now and both options at 1440 and 1024, dark and #light.
Open help, change modes, Escape, reopen, outside-dismiss, switch agents and Save.
Check 390 for reflow too. Query ?help opens details for screenshots.

## Results, 2026-11-08
Rendered Now, v1 and v2 at 1440 and 1024 in light and dark, with twelve agents.
Checked local select changes, Save feedback, reopening and Escape handler focus restoration.
Escape handler checks used dispatched DOM events. Browser key tools did not dismiss help,
so native keyboard delivery is unverified. No engine or service integration was run.
Static design checks found no colour issues; node designs/check.cjs passed.
Screenshots are kept outside the public repository. Runtime Help keyboard checks
and browser evidence belong to implementation PR #50.

Selection check, 2026-11-08: Now, v1 and v2 rendered again at 1440 and 1024 in
dark and light with twelve agents. V1 passed real Edge keyboard Escape and Enter,
focus return, viewport bounds and local Save at 1440, 1024 and 390. All three
pages passed the design colour check; `node designs/check.cjs` passed.
This recheck covers the prototypes only, with no service or engine connected.
