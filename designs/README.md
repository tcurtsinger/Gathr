# Gathr design previews

Selected direction: one centred conversation, Team in the side panel for multi-agent chats.

- `chat/v14.html`: multi-agent chat
- `single-chat/v12.html`: one-agent chat
- `new-chat/v12.html`: new chat
- `start-chat/v13.html`: choose agents
- `system.html`: tokens and components

The picked direction continues in the polished versions listed above. Earlier versions and directions are in git history. All names and messages are invented fixtures. Open files locally, append `#light` for light mode, or use Switch theme. Run `node designs/check.cjs`. Actions are local simulations; no providers, terminal or persistence.

`components.css` imports shared shell, conversation, panels and responsive styles in that order.

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

Named accounts revision: `settings-entry/v4.html`, layout pending approval.
Settings entry option 1 is selected. Earlier engine and agent pages are historical
for account/source controls; v4 shows the account-only assignment form.
