# Architecture

This describes the built system and distinguishes accepted work that is not built yet.

## The local service

One Rust `gathrd` per data folder owns SQLite, engine processes, queues and the local API.
The window and browser are clients. The data-folder lock prevents a second service on the same folder.
HTTP and WebSocket clients authenticate through a Bearer token, a cookie, or the live socket's token query.
Host/Origin checks apply. Remote device authentication is not built yet.
[0003](decisions/0003-storage.md), [0004](decisions/0004-service-process.md).

SQLite uses WAL, numbered migrations and one writer thread. API reads currently use that thread too;
the planned read-only pool is not built yet. The CLI's diagnostics open a read-only connection.

## Engines and accounts

Claude Code uses a long-lived stream-json process; Codex uses a long-lived app-server.
Each agent has at most one engine on one chat session at a time. Switching closes the old process gently;
Stop uses the provider's interrupt. The turn boundary uses a `Driver` enum and shared `Frame` values,
not the originally planned shared trait. Shared process cleanup follows 0031.
[0001](decisions/0001-engine-interface.md), [0031](decisions/0031-short-programs-end-together.md),
[0033](decisions/0033-codex-driver.md).

Gathr owns and updates both engine copies; an installed app waits for its checked copy.
Each account has its own `CLAUDE_CONFIG_DIR` or `CODEX_HOME`, never a copied personal login.
Agents select an account, live model/effort options and an approval mode. Account isolation and gates are built;
a background login watcher is not built yet. External account plugins and tools are disabled by default.
[0002](decisions/0002-login-folders.md), [0024](decisions/0024-only-what-gathr-gives.md),
[0025](decisions/0025-engine-copies.md), [0026](decisions/0026-agent-settings.md),
[0029](decisions/0029-approvals.md).

## Chats, projects and coordination

A chat has one to five agents; team chats have a Supervisor and an optional Reviewer.
The service persists tasks, person requests, waits, pause/Stop, usage holds and archive/stale handling.
Agents use Gathr's in-process MCP server to read chat history and change coordination state.
Only the Supervisor files person tasks. A task settles by its owner's report, never by its turn ending ([0047](decisions/0047-a-report-is-a-record.md)).
One answer says what a chat waits on, and a chat where nothing moves wakes its lead, then tells the person, each
only of what is new to them
([0050](decisions/0050-what-a-chat-waits-on.md), [0052](decisions/0052-a-quiet-chat-is-noticed.md)).
Plan mode/checks, hand-off chains, side jobs and fresh sessions per task are not built yet.
[0005](decisions/0005-mcp-server.md), [0010](decisions/0010-coordination-as-state.md),
[0013](decisions/0013-one-chat-model.md), [0028](decisions/0028-team-chats.md).

The Agent folder contains each project's Shared folder and per-agent working folders.
The service owns worktrees, engine-lifetime temp folders and cleanup, preserving work still in use.
It supplies allowed folders to the engines and file-write guard; the limits in 0029 still apply.
[0034](decisions/0034-agent-folder.md).

## Events and diagnostics

The local event record stores turn, tool, lifecycle and coordination events, timings and failure references.
Engine frames are recorded for replay and troubleshooting. Settings > Diagnostics shows turn timelines;
`gathr diag` supports `health`, `trace`, `failures`, `query`, `turn --raw` and `bundle`.
OTLP export, retention/pruning, daily rollups, diagnostics MCP tools, `diag stalls` and `diag usage`
are not built yet.
[0011](decisions/0011-event-record.md), [0019](decisions/0019-troubleshooting.md).

## The UI and updates

React and TypeScript run in a Tauri 2 window or a local browser. Windows uses WebView2; macOS uses WKWebView.
The desktop window closes to the tray/menu bar and Quit ends it and its service.
Local Theme and panel widths persist in the client. Vite builds the UI; Biome, TypeScript and Vitest check it.
[0012](decisions/0012-ui-shell.md), [0016](decisions/0016-macos.md), [0022](decisions/0022-ui-tooling.md).

The accepted chat layout, Settings panel, reply states and registry controls are documented in
[0015](decisions/0015-chat-design.md), [0023](decisions/0023-settings-and-reply-states.md) and
[0030](decisions/0030-ui-foundation.md). 0030 replaces 0027's renderer.
The signed window updater and service handover are built.
[0032](decisions/0032-service-updates.md).

## Accepted work not built yet

- Remote use across the person's computers: [0017](decisions/0017-remote-use.md).
- Agents' browser, profiles, live stream and handover: [0018](decisions/0018-agent-browser.md).
- Optional Decisions provider and settings: [0014](decisions/0014-decisions-provider.md).
- Fresh-session context digests, stored binding decisions and quality comparisons:
  [0020](decisions/0020-context-per-turn.md).
- Plan mode and further role/state defaults toward the no-prompting requirement:
  [0021](decisions/0021-no-prompting-needed.md).

The original first-slice scope and check procedure are in [first-slice.md](first-slice.md).
