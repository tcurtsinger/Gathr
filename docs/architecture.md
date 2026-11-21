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
a background login watcher is not built yet. External account plugins and tools are disabled by default.
[0002](archive/decisions/0002-login-folders.md), [0024](archive/decisions/0024-only-what-gathr-gives.md),
[0025](archive/decisions/0025-engine-copies.md), [0026](archive/decisions/0026-agent-settings.md),
[0029](archive/decisions/0029-approvals.md).

## Chats, projects and the room

A chat has one to five agents; team chats have a Supervisor and an optional Reviewer.
The service persists messages, replies, reactions, a board, questions to the person, pause/Stop, usage holds and archives.
Agents use Gathr's in-process MCP server to read chat history, react, set a board line and ask the person.
The team checks the lead's answer in the open ([0080](archive/decisions/0080-the-team-checks-the-leads-answer-in-the-open.md)).
[0005](archive/decisions/0005-mcp-server.md), [0013](archive/decisions/0013-one-chat-model.md), [0028](archive/decisions/0028-team-chats.md).

The Agent folder contains each project's Shared folder and per-agent working folders, and the Skills folder every agent reads.
The service owns worktrees, engine-lifetime temp folders and cleanup, preserving work still in use.
It supplies allowed folders to the engines and file-write guard; the limits in 0029 still apply.
[0034](archive/decisions/0034-agent-folder.md).
Deleting a project removes its chats and every folder Gathr made for it but Shared; deleting an agent
passes its lead on and keeps its row as removed, so its old messages keep its name
([0069](archive/decisions/0069-deleting-a-project-or-an-agent.md)).

## Events and diagnostics

The local event record stores turn, tool and lifecycle events, timings and failure references.
Engine frames are recorded for replay and troubleshooting. Settings > Turns shows turn timelines;
`gathr diag` supports `health`, `trace`, `failures`, `friction`, `query`, `turn --raw`, `words` and `bundle`.
OTLP export, retention/pruning, daily rollups, `diag stalls` and `diag usage` are not built yet.
[0011](archive/decisions/0011-event-record.md), [0019](archive/decisions/0019-troubleshooting.md).

## The UI and updates

React and TypeScript run in a Tauri 2 window or a local browser. Windows uses WebView2; macOS uses WKWebView.
The desktop window closes to the tray/menu bar and Quit ends it and its service.
When a chat starts needing the person and the window isn't in front, the desktop app shows a system notification (`gathr::notice` decides, `crates/gathr-app/src/notify.rs` shows; [0058](archive/decisions/0058-gathr-tells-you-outside-the-window.md)).
Local Theme and panel widths persist in the client. Vite builds the UI; Biome, TypeScript and Vitest check it.
[0012](archive/decisions/0012-ui-shell.md), [0016](archive/decisions/0016-macos.md), [0022](archive/decisions/0022-ui-tooling.md).

The accepted chat layout, Settings panel, reply states and registry controls are documented in
[0015](archive/decisions/0015-chat-design.md), [0023](archive/decisions/0023-settings-and-reply-states.md) and
[0030](archive/decisions/0030-ui-foundation.md). 0030 replaces 0027's renderer.
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
