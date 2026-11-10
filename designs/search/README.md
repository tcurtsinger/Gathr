# Search reference

v1: Title first, selected by the owner on 2026-11-10. Matches Tasks rows and keeps chat context above the snippet.
v2: Snippet first, retired. Kept as an unselected comparison.

Both use the same 340px navigation flyout, Chats then Messages, Archived labels, prefix-marked snippets and one keyboard cursor. The system page demonstrates the selected v1. Close uses the shared X icon button. No service, fetch, real history or durable state connects.

Open Search (Ctrl K or Cmd K), type reconnect, use arrows and Enter. Escape, outside click, Close or the trigger dismisses. Query stays on reopening. Tasks uses the same popover shell. State selects empty, no matches, loading and error; Retry returns to results. Loading timing and stale-response protection belong to the runtime, not these fixtures.

A message opens centred, with a 1.6-second attention tint (static for reduced motion). It stays at that message until Latest or scrolling to the bottom; live-tail behaviour is implemented and verified by Pia. Archived opens read only. Chat-title hits open at the latest message. The preview opens local invented records only.

Run node designs/check-search.cjs <evidence folder outside the repo> with UI dependencies available. Checks both options at 1440, 1024 and 390 in both themes.
