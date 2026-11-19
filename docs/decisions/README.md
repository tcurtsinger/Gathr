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
| 0005 | [One MCP tool server inside the service, over streamable HTTP, with rmcp](0005-mcp-server.md) | accepted | yes |
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
| 0048 | [Gathr records friction, starting with what agents report](0048-gathr-records-friction.md) | accepted | partly |
| 0049 | [Gathr lands a pull request](0049-gathr-lands-pull-requests.md) | accepted | yes |
| 0050 | [One answer to what a chat waits on](0050-what-a-chat-waits-on.md) | accepted | yes |
| 0051 | [The giver hears when all is in, or at once on trouble](0051-the-giver-hears-when-all-is-in.md) | accepted | yes |
| 0052 | [A quiet chat is noticed, and its dead time has a number](0052-a-quiet-chat-is-noticed.md) | accepted | yes |
| 0053 | [A review asks both questions, and a left review gets its fix](0053-a-review-asks-both-questions.md) | accepted | yes |
| 0054 | [A change past the far end waits for the person, and every landed change is recorded](0054-a-change-past-the-far-end-waits-for-the-person.md) | accepted | yes |
| 0055 | [A member's task starts in a new session](0055-a-members-task-starts-in-a-new-session.md) | accepted | yes |
| 0056 | [Gathr records which words opened each turn](0056-gathr-records-which-words-opened-each-turn.md) | accepted | yes |
| 0057 | [Agreeing is a thumbs-up, not a message](0057-agreeing-is-a-reaction.md) | accepted | yes |
| 0058 | [When a chat needs you, Gathr tells you outside the window](0058-gathr-tells-you-outside-the-window.md) | accepted | yes |
| 0059 | [A work task carries a complete brief, and its giver can edit it until it starts](0059-a-work-task-carries-a-complete-brief.md) | accepted | yes |
| 0060 | [Gathr gives the review task](0060-gathr-gives-the-review-task.md) | accepted | yes |
| 0061 | [Every ask in a team chat has a size](0061-every-ask-has-a-size.md) | accepted | yes |
| 0062 | [A plan is a checked record, and Approve hands it out](0062-a-plan-is-checked-and-approve-hands-it-out.md) | accepted | yes |
| 0063 | [Building waits until a new ask's plan is approved; a repair goes](0063-building-waits-for-a-new-asks-plan.md) | accepted | yes |
| 0064 | [A change far past its Fix or its part stops and asks](0064-a-change-far-past-its-fix-or-its-part-stops-and-asks.md) | accepted | yes |
| 0065 | [Questions are in the lead's words, and an answer reaches the member at once](0065-questions-in-the-leads-words.md) | accepted | yes |
| 0066 | [Role text is two lines, each shipped only after a trial on real engines](0066-role-text-tried-on-real-engines.md) | accepted | yes |
| 0067 | [A merge brings the project folder along, or says why not](0067-a-merge-brings-the-project-folder-along.md) | accepted | yes |
| 0068 | [The window's colours](0068-the-windows-colours.md) | accepted | yes |
| 0069 | [Deleting a project or an agent](0069-deleting-a-project-or-an-agent.md) | accepted | yes |
| 0070 | [The window's shell](0070-the-windows-shell.md) | accepted | yes |
| 0071 | [An agent's state is one mark](0071-an-agents-state-is-one-mark.md) | accepted | yes |
| 0072 | [The chat reads like a team chat](0072-the-chat-reads-like-a-team-chat.md) | accepted | yes |
| 0073 | [Gathr speaks as a speaker](0073-gathr-speaks-as-a-speaker.md) | accepted | yes |
| 0074 | [An agent's face](0074-an-agents-face.md) | accepted | yes |
| 0075 | [A team chat is main messages, each with its thread](0075-a-team-chat-is-main-messages-each-with-its-thread.md) | accepted | yes |
| 0076 | [Replying in a thread](0076-replying-in-a-thread.md) | accepted | yes |
| 0077 | [Where you left off](0077-where-you-left-off.md) | accepted | yes |
| 0078 | [Said to you is main, one bubble per turn](0078-said-to-you-is-main-one-bubble-per-turn.md) | accepted | yes |
| 0079 | [Agents choose where their words show](0079-agents-choose-where-their-words-show.md) | accepted | yes |
| 0080 | [The team checks the lead's answer in the open](0080-the-team-checks-the-leads-answer-in-the-open.md) | accepted | yes |
| 0081 | [Agents hear the room, and name each other with @Name](0081-agents-hear-the-room-and-name-each-other.md) | accepted | yes |
