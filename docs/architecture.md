# Architecture

This describes the built system and distinguishes accepted work that is not built yet.

## The local service

One Rust `gathrd` per data folder owns SQLite, engine processes, queues and the local API.
The window and browser are clients. The data-folder lock prevents a second service on the same folder.
HTTP and WebSocket clients authenticate through a Bearer token, a cookie, or the live socket's token query.
Host/Origin checks apply. Remote device authentication is not built yet.
[0003](archive/decisions/0003-storage.md), [0004](archive/decisions/0004-service-process.md).

SQLite uses WAL, numbered migrations and one writer thread. API reads currently use that thread too;
the planned read-only pool is not built yet. The CLI's diagnostics open a read-only connection.

## Engines and accounts

Claude Code uses a long-lived stream-json process; Codex uses a long-lived app-server.
Each agent has at most one engine on one conversation at a time: its latest in the chat. Each turn's opening carries
the chat's lines the agent hasn't heard, and `@Name` in an agent's words wakes that member when its turn ends, or in a chat with no project brings that agent in
([0081](archive/decisions/0081-agents-hear-the-room-and-name-each-other.md)). Switching closes the old process gently;
after compaction, the conversation's next turn hears the chat's last lines.
Stop uses the provider's interrupt. The turn boundary uses a `Driver` enum and shared `Frame` values,
not the originally planned shared trait. Shared process cleanup follows 0031.
[0001](archive/decisions/0001-engine-interface.md), [0031](archive/decisions/0031-short-programs-end-together.md),
[0033](archive/decisions/0033-codex-driver.md).

Gathr owns and updates both engine copies; an installed app waits for its checked copy.
Each account has its own `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, never a copied personal login.
Agents select an account, live model/effort options and an approval mode. Account isolation and gates are built;
a background login watcher is not built yet. The account's own connectors and MCP tools stay off, and
Codex's plugins and apps: an agent gets the tools and skills added in Gathr.
[0002](archive/decisions/0002-login-folders.md), [0024](archive/decisions/0024-only-what-gathr-gives.md),
[0025](archive/decisions/0025-engine-copies.md), [0026](archive/decisions/0026-agent-settings.md),
[0029](archive/decisions/0029-approvals.md).

Tools and skills, on the Plugins screen, are added once for every agent. A tool on the web is a remote MCP server,
with a key or a browser sign-in sealed for the computer's user (DPAPI, or the Keychain on a Mac) (`web_tools.rs`,
`tool_sign_in.rs`, `seal.rs`). A tool on this PC is a program Gathr starts and talks to over its stdin and stdout,
one running copy for every agent (`pc_tools.rs`). `gathrd` relays both, so an engine only ever talks to Gathr.
A skill is a folder with a `SKILL.md` in the agent folder's Skills (`skills.rs`). An engine gets its tools and
skills as it starts, so an agent's next turn after a change starts its engine again (`gathrd/turns/mod.rs`).

The background model (`gathrd/naming.rs`, `src/titles.rs`) is a small, quick model on one of the person's signed-in
accounts, for work nobody waits on. Its one job: a moment after a new chat's first message of 40 characters or
more, a name of 2 to 5 words. Settings > Background picks it; left unset, Gathr takes the first Claude Code
account's model whose id holds `haiku` (Codex names no small model, so none is picked there by itself). Each run is
the engine's one-shot mode, never a chat session: `claude -p` (no tools, `--safe-mode`, no session kept) or
`codex exec` (`--ephemeral`, no `config.toml`, read-only), on the account's own folder, in the empty
`<data>/background`, with the question in its arguments and the message on its input. It passes the account's
renewal gate as every start does, and past 30 s its answer is let go but it is never killed. One
`gathr.background.name` event says `named`, `skipped` or `failed` and why, never the words. A rename by the person
(Rename, in a chat's More menu) is final: the name lands only while the title is still the one the first line gave.

## Chats, projects and the room

A chat has one to five agents; team chats have a lead.
The service persists messages, replies, reactions, the person's likes, where the person left off, chat names, a board,
questions to the person, pause/Stop, usage holds and archives.
Agents use Gathr's MCP server at `/mcp`, each engine with a token of its own, to read the chat (`read_chat`), react,
set a board line, ask the person, keep a note in Memory (`note`), put a table or a chart in the chat (`show`), add a
tool or a skill when the person asks (`add_tool`, `add_skill`), schedule work in the chat (`schedule`,
`unschedule`) and read or update a pull request (`pr`, `ci_failures`, `update_pr`).
The team checks the lead's answer in the open ([0080](archive/decisions/0080-the-team-checks-the-leads-answer-in-the-open.md)).
Scheduled work (`schedules.rs`, `gathrd/scheduled.rs`): what an agent scheduled comes into its chat at its time, on this
PC's clock (read through SQLite), as a message to that agent drawn as the person's, and starts its turn as the person's
message would; the service looks as it starts and every 30 seconds, and writes each run with its turn in one
transaction. A missed run runs late when it was due today, else one skipped run is kept.
[0005](archive/decisions/0005-mcp-server.md), [0013](archive/decisions/0013-one-chat-model.md), [0028](archive/decisions/0028-team-chats.md).

The Agent folder contains each project's Shared folder and per-agent working folders, the Skills folder every agent reads, the Memory folder of notes agents keep with the `note` tool (about the person, a project, their own work or a tool added in Plugins) and the person edits on the Memory screen, and the Designs folder with a part per project, which that project's agents write in.
The service owns worktrees, engine-lifetime temp folders and cleanup, preserving work still in use.
It supplies allowed folders to the engines and file-write guard; the limits in 0029 still apply.
Gathr asks the person before a risky push (to the default branch, rewriting or deleting a branch, or to anywhere
but the worktree's origin) and before a write outside the agent's folders; one Allow of such a write covers
its folder for that agent in that chat (`approvals.rs`).
[0034](archive/decisions/0034-agent-folder.md).
Deleting a project removes its chats and every folder Gathr made for it but Shared and its Designs; deleting an agent
passes its lead on and keeps its row as removed, so its old messages keep its name
([0069](archive/decisions/0069-deleting-a-project-or-an-agent.md)).

## Events and diagnostics

The local event record stores turn, tool and lifecycle events, timings and failure references.
Engine frames are recorded for replay and troubleshooting. Settings > Turns shows turn timelines, and
Settings > Friction the reports agents made before their friction tool was removed;
`gathr diag` supports `health`, `trace`, `failures`, `friction`, `query`, `turn --raw`, `words` and `bundle`.
OTLP export, retention/pruning, daily rollups, `diag stalls` and `diag usage` are not built yet.
[0011](archive/decisions/0011-event-record.md), [0019](archive/decisions/0019-troubleshooting.md).

## The UI and updates

React and TypeScript run in a Tauri 2 window or a local browser. Windows uses WebView2; macOS uses WKWebView.
The desktop window closes to the tray/menu bar and Quit ends it and its service.
When a chat starts needing the person, or a chat's agents have all stopped, and the window isn't in front, the desktop app shows a system notification (`gathr::notice` decides, `crates/gathr-app/src/notify.rs` shows; [0058](archive/decisions/0058-gathr-tells-you-outside-the-window.md)).
Local Theme and panel widths persist in the client. Vite builds the UI; Biome, TypeScript and Vitest check it.
[0012](archive/decisions/0012-ui-shell.md), [0016](archive/decisions/0016-macos.md), [0022](archive/decisions/0022-ui-tooling.md).

The window has a rail (Home, Agents, Memory, Plugins, Designs, Scheduled, Settings), the chat list and the chat.
A chat's right panel is tabs: Home (Find file, the Files, Designs and Changes tiles, and the board), and a tab for
each file or design opened. A file tab shows Markdown and CSV as they look (with Source), code with its colours, a picture, a
PDF or a workbook; an HTML page shows as it looks in a sealed frame (`view.rs`). The window itself sets no content security policy today
(`"csp": null` in `crates/gathr-app/tauri.conf.json`). Changes shows what each agent changed in its
worktree that isn't on the project's main line yet, and what it wrote in other checkouts (`changes.rs`, through git);
a file in no checkout shows from the lines its steps kept (a file step's stored body keeps its unified diff, which the
window's messages and live events leave out). `ui/src/registry` keeps the assistant-ui and
shadcn files as published ([0030](archive/decisions/0030-ui-foundation.md)).
The signed window updater and service handover are built.
[0032](archive/decisions/0032-service-updates.md).

## Accepted work not built yet

- Remote use across the person's computers: [0017](archive/decisions/0017-remote-use.md).
- Agents' browser, profiles, live stream and handover: [0018](archive/decisions/0018-agent-browser.md).
- Optional Decisions provider and settings: [0014](archive/decisions/0014-decisions-provider.md).
- Catch-up digests, stored binding decisions and quality comparisons:
  [0020](archive/decisions/0020-context-per-turn.md).
- Plan mode and further role/state defaults toward the no-prompting requirement:
  [0021](archive/decisions/0021-no-prompting-needed.md).

The original first-slice scope and check procedure are in [first-slice.md](archive/first-slice.md).
