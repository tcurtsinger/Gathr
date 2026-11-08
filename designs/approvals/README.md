# Approvals help
Task 1083. Refinement of Settings > Agents at e50dace.
Audience: the owner configuring a reusable agent. Job: choose how freely it acts.
Operate: variance 2, motion 1, density 4. Existing tokens, type and form layout.

Now reproduces the current form structure with sample data.
v1 (recommended): click-open question-mark popover, compare four modes without moving the form.
v2: question-mark disclosure below the select, reflows the form in the reading order.
Approvals goes below Effort. Ask, Edits, Auto, Full. Auto is the initial selection.
No service connected. Save feedback is local. Twelve generic agents and model choices are illustrative fixtures.
Add and Remove are disabled in this focused preview.

## Proposed shared pattern
Plain ? in a 16 px circle, muted, in a 28 px button beside the label (44 px on narrow layouts).
Hover uses raised; focus uses the existing 2 px focus ring. Accessible name: About approvals.
Click, Enter or Space toggles; no hover-only content or title tooltip.
Content is a named region, not a tooltip.
v1: native auto popover, Escape, outside click or opener dismiss it before Settings closes.
Keep it within viewport, maximum 350 px wide. Escape restores focus to opener.
v2: aria-controls and aria-expanded. Escape collapses help and focuses opener.
Opening help changes no setting. Reuse one Help component after the owner's pick.
Pia updates DESIGN.md and designs/system.html with the selected shared pattern when building it.
Neither option is approved yet.

## Copy
 Approvals 
 Ask Ask before actions that need permission. 
 Edits Allow file edits in the worktree. Ask before other actions that need permission. 
 Auto The engine checks actions. Pushes and file-tool writes outside the worktree ask. 
 Full Run actions without asking. 
 Auto does not confine shell commands to the worktree. 
Copy follows approvals-options.md and its live shell-write probe.
Cass must confirm final Ask and Edits semantics and how Auto refusals surface before shipping.
Do not promise every write outside the worktree asks. That claim covers file tools only.
Whether Auto blocks become approvals is still unverified.

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
Desktop proof images are in X:/code/gathr/designs/approvals-*.png.
