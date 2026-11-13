# Decisions

One file per decision: the choice, why, and what was turned down. Each starts with two lines.

- `Status:` is `accepted`, or `replaced by` the decision that took its place. A decision replaced in
  part says which part under its status.
- `Built:` is `yes`, `partly` or `no`: whether the code does what the decision says today. `yes` and
  `partly` name the test that fails if it stops (or, where no test can, the file that is the proof), and
  `partly` says what is missing. `no` means it is written down only. None of this says the decision has
  held in real use: that is Kept, in [the charter](../charter.md)'s words, and it is counted per promise.

This list is written by `node scripts/docs.check.mjs --write` from those lines, and the same script
fails in CI when the two differ.

| # | Decision | Status | Built |
|---|---|---|---|
| 0001 | [One engine interface, each provider's own harness behind it, ACP later as one more driver](0001-engine-interface.md) | accepted | partly |
| 0002 | [One login folder per account, shared by its agents through a renewal gate, never copied, never killed mid-renewal](0002-login-folders.md) | accepted | partly |
| 0003 | [SQLite through rusqlite, one writer thread, migrations as numbered SQL files](0003-storage.md) | accepted | partly |
| 0004 | [One service process, `gathrd`, with an HTTP and WebSocket API on localhost](0004-service-process.md) | accepted | partly |
| 0005 | [One MCP tool server inside the service, over streamable HTTP, with rmcp](0005-mcp-server.md) | accepted | partly |
| 0010 | [Coordination is state in the service](0010-coordination-as-state.md) | accepted | partly |
| 0011 | [One event record, named after OpenTelemetry's GenAI conventions](0011-event-record.md) | accepted | partly |
| 0012 | [React and TypeScript, in a Tauri window, also served to a browser](0012-ui-shell.md) | accepted | partly |
| 0013 | [One chat model](0013-one-chat-model.md) | accepted | partly |
| 0014 | [Decisions: fast yes/no answers, optional from day one](0014-decisions-provider.md) | accepted | no |
| 0015 | [Desktop chat with split team conversation and Summary](0015-chat-design.md) | accepted | partly |
| 0016 | [Gathr runs on macOS as well as Windows](0016-macos.md) | accepted | partly |
| 0017 | [Using one computer's chats from another, over Tailscale](0017-remote-use.md) | accepted | no |
| 0018 | [The agents' browser: a browser the service runs, shown as a live stream](0018-agent-browser.md) | accepted | no |
| 0019 | [Troubleshooting is built in, for agents as much as for the person](0019-troubleshooting.md) | accepted | partly |
| 0020 | [Context per turn: lean by default, measured, never trimmed blind](0020-context-per-turn.md) | accepted | partly |
| 0021 | [Gathr works without a good prompt](0021-no-prompting-needed.md) | accepted | partly |
| 0022 | [UI tooling: Vite, Biome, TypeScript 7 and Vitest in the browser, on npm](0022-ui-tooling.md) | accepted | yes |
| 0023 | [Settings and reply states](0023-settings-and-reply-states.md) | accepted | yes |
| 0024 | [Agents get only what Gathr gives them](0024-only-what-gathr-gives.md) | accepted | yes |
| 0025 | [Gathr keeps its own engine copies, and updates them](0025-engine-copies.md) | accepted | yes |
| 0026 | [Agent settings](0026-agent-settings.md) | accepted | yes |
| 0027 | [Reply formatting: react-markdown with remark-gfm, re-parsing only the end while streaming](0027-reply-formatting.md) | accepted | partly |
| 0028 | [Chats for a team: routing, order, worktrees and PATH](0028-team-chats.md) | accepted | yes |
| 0029 | [Approvals: what an agent may do without asking](0029-approvals.md) | accepted | yes |
| 0030 | [The chat on assistant-ui, Streamdown and Tailwind, with the registry files kept as published](0030-ui-foundation.md) | accepted | yes |
| 0031 | [Short programs end with everything they started](0031-short-programs-end-together.md) | accepted | yes |
| 0032 | [Gathr updates itself when the person says](0032-service-updates.md) | accepted | yes |
| 0033 | [Codex, the second engine](0033-codex-driver.md) | accepted | yes |
| 0034 | [The agent folder: where agents' work lives, and who cleans it up](0034-agent-folder.md) | accepted | yes |
| 0035 | [Search: an FTS5 index of message text, word prefixes, newest first](0035-search.md) | accepted | yes |
| 0036 | [Colours: surfaces in three steps, in both themes](0036-surfaces-in-three-steps.md) | accepted | yes |
| 0037 | [The chat is one timeline](0037-one-timeline.md) | accepted | yes |
| 0038 | [Summary holds the plan and the team](0038-summary-holds-the-plan.md) | accepted | yes |
| 0039 | [A conversation map on the chat's left edge](0039-conversation-map.md) | accepted | yes |
| 0040 | [A chat's state in the chat list](0040-chat-state-in-the-list.md) | accepted | yes |
| 0041 | [A step shows what it ran](0041-what-a-step-ran.md) | accepted | yes |
| 0042 | [A turn shows that its agent thought, as a time](0042-thinking.md) | accepted | yes |
| 0043 | [Gathr runs the checks and does the merge](0043-checks-and-merge.md) | accepted | partly |
| 0044 | [A step keeps what it returned, and a command whole](0044-what-a-step-returned.md) | accepted | yes |
| 0045 | [Gathr's own repo: review and local tests are the gate, not GitHub CI](0045-review-gates-gathrs-own-repo.md) | accepted | yes |
| 0046 | [Gathr reaches GitHub for agents, with its own sign-in](0046-github-tools.md) | accepted | yes |
| 0047 | [A task's report is a record, and Gathr works out where the task stands](0047-a-report-is-a-record.md) | accepted | yes |
