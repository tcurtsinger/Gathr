# Architecture

Status: draft. Each choice below has its decision file in [`decisions/`](decisions/): what was chosen, why, and what was turned down.

## The picture

```
 Tauri window (tray, notifications)      a browser on this machine
            \                                   /
             React + TypeScript UI, served by the service
                              |
                 HTTP + WebSocket API (localhost)
                              |
 +----------------------- Gathr service (Rust) -----------------------+
 |  coordination: tasks, turns, waits, pause, plans, hand-offs, asks  |
 |  scheduler gate: every automatic start passes one check            |
 |  MCP tool server: agents act on coordination through it            |
 |  engine adapters: each provider's own harness, one interface       |
 |  event record: every turn, tool call, change and failure           |
 |  SQLite: all state and events, one database                        |
 +--------------------------------------------------------------------+
        |                         |
  Claude Code                    Codex
  one `claude -p` per agent,     one `codex app-server`
  resumed per conversation       per agent
  (agent's own login folder)     (agent's own login folder)
```

## The service

One Rust process owns the data, the engines, scheduling and the API. The window and the browser are clients of it.

- **Process.** `gathrd`, one daemon per user, serving HTTP and a WebSocket (axum) on 127.0.0.1. Every request carries a token; a wrong Host or Origin gets 403. The window starts it as a plain child, so closing the window doesn't kill engines mid-turn. Engines run at below-normal priority; nothing uses the GPU. [0004](decisions/0004-service-process.md).
- **Engines.** One Rust `Engine` interface, a driver per provider, each driving the provider's own harness: Claude Code as a long-lived `claude -p` stream-json process, Codex as a long-lived `codex app-server` per agent. An agent runs one turn at a time; switching to another conversation closes Claude's process gently and resumes the other session ([0010](decisions/0010-coordination-as-state.md#sessions)). ACP isn't used for these two; the interface follows its concepts so an ACP driver can come later. Claude Code first. [0001](decisions/0001-engine-interface.md).
- **Logins.** Each agent gets its own `CLAUDE_CONFIG_DIR` or `CODEX_HOME` under Gathr's data folder, one process per folder. Login files are never copied; the person's own `~/.claude` and `~/.codex` are never touched; API-key variables are removed from the engine's environment. [0002](decisions/0002-login-folders.md).
- **Storage.** SQLite through rusqlite, bundled, one writer thread, WAL, numbered SQL migrations. [0003](decisions/0003-storage.md).
- **MCP.** One streamable-HTTP MCP server at `/mcp` on the same port, a token per agent so the service knows who is calling. [0005](decisions/0005-mcp-server.md).

## Chats

One place to talk to agents: a chat with 1 to 5 agents, optionally tied to a project folder. With two or more, a Supervisor is required and a Reviewer is optional. No separate rooms or Quick ask. [0013](decisions/0013-one-chat-model.md).

## Coordination

Tasks, turns, waits, pause, plans and plan checks, hand-offs, side jobs and usage holds are records with named states and fixed transitions in the service. Agents change them only through tools; the service checks who may. Nothing is read from what an agent says. [0010](decisions/0010-coordination-as-state.md).

## Events and diagnostics

One append-only event table, written with each state change, named after OpenTelemetry's GenAI conventions (`invoke_agent`, `chat`, `execute_tool`, `gen_ai.*`) plus `gathr.*` for coordination. Optional OTLP export, off by default. A Diagnostics page answers what happened, where work stalled, what waited on whom, and what fails. [0011](decisions/0011-event-record.md).

## The UI

React and TypeScript, served by the service, shown in a Tauri 2 window over WebView2 and usable from a browser on this machine. The window holds no state and owns no engines. [0012](decisions/0012-ui-shell.md).

## Rules that hold everywhere

- Each provider's own agent harness, never a homemade loop over raw model APIs.
- Each agent's login in its own folder, never shared, copied or force-killed while it might be renewing.
- Background work runs on the CPU at low priority, never the GPU.
- No paid service required; anything that costs money is opt-in and off by default.

## Building it

The first slice, one chat with one engine end to end with its Diagnostics page, is in [first-slice.md](first-slice.md).
