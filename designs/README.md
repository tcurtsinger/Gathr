# Gathr design previews

Current visual reference: the assistant-ui React prototype, adapted to Gathr's
accepted decisions. See [system.html](system.html), [DESIGN.md](../DESIGN.md) and
[prototype-reference.md](prototype-reference.md). Open system.html locally; append
`#light` for light mode or use Switch theme. Run `node designs/check.cjs`.
After `npm ci` in `ui/`, run `node designs/check-help.cjs` for Help placement,
Escape and focus return in both themes at 1440 and 1024px (Edge on Windows,
WebKit on macOS).

Settings retains the centred Gathr layout. The accepted split chat (0015,
2026-11-10) replaces inline team threads: a header toggle opens one chronological
team conversation beside the Supervisor, with independent scrolling. Direct
@member exchanges stay in the main column. Summary remains a popover. The
recipient defaults to the Supervisor by name and has no Everyone option.

`system.html#team-message` demonstrates the split, 52rem maximum, 34px reply
indent, docked Agent plan, working initials and Summary progress with invented
records. Finish and Late reply simulate outcomes; Reset returns to running.
These are fixture controls, not product actions. The portable page has no service.
`team-reference.css` and `team-reference.js` are fixture code only. Production
uses assistant-ui registry components and Gathr state; never port the handoff
HTML or its support.js runtime. The owner's reference stays in Shared/designs/split_chat.

Run `node designs/check-team.cjs` after installing UI dependencies. It checks split
state, independent scrolling, late/unread work, plan and Summary counts, keyboard
controls and draft retention at 1440 and 1024px in both themes. An optional
absolute folder outside the repo saves PNGs and JSON evidence.

## Retired selected shell pages

These pages are retained as historical fixtures, not selected designs:

- `chat/v14.html`: multi-agent chat
- `single-chat/v12.html`: one-agent chat
- `new-chat/v12.html`: new chat
- `start-chat/v13.html`: choose agents

The prototype supersedes their shell, typography, palette, spacing, composer and
workspace presentation. Do not build from them. Their older versions are also
historical. All fixtures are invented. Actions are local simulations.

`components.css` retains the four historical imports so older evidence stays
viewable, then imports `system-reference.css` for the current portable reference.
That page demonstrates static anatomy, not substitute React implementations.

Phase 1 proposals: `diagnostics/v1.html` and `v2.html`, `engines/v1.html` and `v2.html`,
`chat-states/v1.html` and `v2.html`. These are unselected options. See `phase1-options.md`
for comparisons, recommendations and prototype limits. Their shared styles and fixtures
live in `phase1-prototype.css` and `phase1-prototype.js`.

`engines/v3.html` revises the single form into a centered Settings dialog over the chat,
using `settings-overlay.css` and `settings-overlay.js`. Earlier options stay intact.

## Selected Phase 1 patterns
- `engines/v4.html`: centered Settings, single engine form.
- `diagnostics/v3.html`: Settings category, turn list and timeline.
- `chat-states/v3.html`: full-width reply notices, contextual Settings links.

These supersede the unselected Phase 1 proposals above. All three share the same
Settings shell and retain the current chat underneath. No service is connected.

Settings regression check (requires an already installed `playwright-core` and browser):
set `NODE_PATH` to the installed module directory if needed and `BROWSER_PATH` to the
browser executable, then run `node designs/check-settings.cjs`. It checks contextual
references after a Stopped filter, clearing Find, and focus return after category
changes with Close and Escape at 1440 and 1024 px in both themes. No downloads.


Selected agent editor: `agents/v1.html`, Settings > Agents, also opened by Edit
agent in the chat picker. Save applies everywhere. Not chosen: `agents/v2.html`
Team panel and `agents/v3.html` separate chat menu editor. Shared form: agent-editor.css and
agent-editor.js. Five invented agents, two engines, two accounts and 24 sample
models per engine. Model names and effort values are layout fixtures, not a
provider capability claim. No engine, login, service or durable storage connects.

Named accounts revision: `settings-entry/v5.html`, layout pending approval.
Settings entry option 1 is selected. Earlier engine and agent pages are historical
for account/source controls; v5 shows the account-only assignment form.

Selected setting help: `approvals/v1.html`, a pop-up from the ? beside a label.
`approvals/now.html` captures the earlier form; `approvals/v2.html` is the
unselected expanding option. See `approvals/README.md` for copy and checks.
The shared runtime component is `ui/src/components/Help.tsx`, demonstrated in
`system.html` with its runtime stylesheet.

## Selected archive and stale patterns

The folded Archived group from archive-stale/v1 is selected. The current
reference is [archive-stale/v3.html](archive-stale/v3.html): the same layout with
Idle age labels and the service's 1-to-365 day range. v1 is retained as the chosen
proposal; v2's Archived-button flyout is retired and must not be built. The
selected anatomy also appears in system.html#archive-stale. No service connects
to these fixtures. Runtime Archive, Delete and stale rules belong to 0010.

## Selected Search

[Title first](search/v1.html), selected 2026-11-10, extends the existing Tasks
flyout shell. Snippet first (v2) is retired. system.html#search-preview shows the
selected rows and fixture states. See [Search checks](search/README.md).
