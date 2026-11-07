# Gathr design previews

Selected direction: one centred conversation, Team in the side panel for multi-agent chats.

- `chat/v13.html`: multi-agent chat
- `single-chat/v11.html`: one-agent chat
- `new-chat/v11.html`: new chat
- `start-chat/v12.html`: choose agents
- `system.html`: tokens and components

The picked direction continues in the polished versions listed above. Earlier versions and directions are in git history. All names and messages are invented fixtures. Open files locally, append `#light` for light mode, or use Switch theme. Run `node designs/check.cjs`. Actions are local simulations; no providers, terminal or persistence.

`components.css` imports shared shell, conversation, panels and responsive styles in that order.

Phase 1 proposals: `diagnostics/v1.html` and `v2.html`, `engines/v1.html` and `v2.html`,
`chat-states/v1.html` and `v2.html`. These are unselected options. See `phase1-options.md`
for comparisons, recommendations and prototype limits. Their shared styles and fixtures
live in `phase1-prototype.css` and `phase1-prototype.js`.

`engines/v3.html` revises the single form into a centered Settings dialog over the chat,
using `settings-overlay.css` and `settings-overlay.js`. Earlier options stay intact.
