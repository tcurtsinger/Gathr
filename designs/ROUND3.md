# Round 3: one desktop shell

Six local working pages, one neutral desktop shell. No production interface changed.

- chat/v7.html: live agent strip, details and tasks in Team panel.
- chat/v8.html: Team panel open, unchanged conversation and composer. Recommended for five-agent scanning. At 1024 px it trades conversation width for readable work and waits.
- chat/v9.html: active turns pinned above the composer; completed turns remain in history.
- new-chat/v7.html: empty chat.
- single-chat/v7.html: one agent, queued message and diff.
- start-chat/v7.html: project, one to five agents, required Supervisor for several agents, optional distinct Reviewer.

Open each file locally. Dark is default; the theme button switches, and #light opens light. New chat links to the empty page; Agents opens setup. A fresh setup requires choosing a Supervisor. Conversation history contains 42 made-up turns and opens at the end. Sidebar contains 36 project chats and 6 recent chats. Expand project groups or use Search.

Check: node designs/check-round3.cjs

Verified in installed Edge: six pages at 1440, 1024 and 390 px in both themes, 36 cases. No document overflow, duplicate design IDs or script errors. Theme readback and conversation end position checked. Computed visible-text contrast minimum 4.79:1. Serenity design check found zero issues on six pages plus the provisional component sheet.

Exercised local send, Summarise, reactions, Review dialog, Pause/Resume, Team/Diff/Terminal/Files tabs, terminal text entry, queue removal, Supervisor required state and Reviewer role collision. These controls demonstrate local state only. No engines, actual terminal commands, folder picker, Settings service or real summarisation are connected. Agent timings are sample labels; no production latency is claimed.

Compared dark and light shots at both desktop widths against the supplied references outside the public repo. Main difference: named agent identities, waits and tasks occupy a dedicated region; base shell and reading treatment stay familiar.

DESIGN.md and designs/system.html record the fixed shell provisionally. The selected coordination layout still needs approval. v1-v6 are preserved as historical candidates. No separate Room or Overview surface is introduced by this round.
