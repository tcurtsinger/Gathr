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
  Claude Code              Codex            (one process per agent session,
  (own login folder)       (own login folder)   each in its own login folder)
```

## The service

One Rust process owns the data, the engines, scheduling and the API. The window and the browser are clients of it.

> **Slot for Cass's decisions (0001-0009):** the service process and how the window starts it; the engine interface and whether ACP fits; per-provider login folders; storage (SQLite crate, migrations); the MCP tool server. Linked here once they merge.

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
