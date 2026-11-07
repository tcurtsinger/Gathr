# 0001. One engine interface, each provider's own harness behind it, ACP later as one more driver

Status: proposed. Date: 2026-11-06.

## Choice

- The service defines one Rust `Engine` interface. Every provider sits behind it as a driver that runs that provider's own agent harness as a child process and speaks its own machine protocol. Gathr never runs a model loop of its own over raw APIs.
- **Claude Code driver:** at most one long-lived `claude -p --input-format stream-json --output-format stream-json --verbose --include-partial-messages` process per agent, on one session at a time, with stdin kept open across that session's turns. An agent runs one turn at a time across all its conversations ([0010, Sessions](0010-coordination-as-state.md#sessions)); when its next turn is in another session, the driver closes the process the gentle way (below) and starts a new one with `--resume <id>`. Tool approvals come back over the same pipe with `--permission-prompt-tool stdio`. Stop is the `interrupt` control request.
- **Codex driver:** one long-lived `codex app-server` process per agent, JSON-RPC over stdio: `initialize`, then `thread/start` or `thread/resume`, `turn/start`, `turn/steer`, and `turn/interrupt` for Stop. Approvals arrive as server requests (`item/commandExecution/requestApproval`, `item/fileChange/requestApproval`).
- **ACP** is not the path to Claude Code or Codex. The interface is shaped after ACP's concepts (session new, load and resume; prompt; cancel; permission request; plan; usage), so an ACP driver can be added later for agents that speak ACP natively (Gemini CLI, Copilot, Goose and others).
- **First slice engine:** Claude Code.

The interface, in outline:

| Gathr call | Claude Code | Codex app-server |
|---|---|---|
| start / resume session | spawn with `--session-id <uuid>` / `--resume <id>` | `thread/start` / `thread/resume` |
| send | `{"type":"user",...}` line on stdin | `turn/start`, or `turn/steer` mid-turn |
| stop | `control_request` `interrupt` | `turn/interrupt {threadId, turnId}` |
| approval asked | `can_use_tool` control request | `item/*/requestApproval` server request |
| plan | `TaskCreate` / `TaskUpdate` tool calls (legacy `TodoWrite` also read) | `turn/plan/updated` |
| usage | `result.usage`, `modelUsage`, `total_cost_usd` | `thread/tokenUsage/updated` (`last`, `total`) |
| turn ended | `result` message | `turn/completed` |

Every frame in and out follows [0011's raw-frame policy](0011-event-record.md#raw-engine-frames): read it, redact secrets, write it to `engine_raw`, then interpret it. Nothing reaches the database unredacted, malformed frames included (pattern-redacted, cut to 64 KB, `malformed = 1`). With "Keep raw engine frames" off, only the metadata columns are written. So a misread frame can be diagnosed from the record without the record holding a secret.

## Why

- **The harnesses already expose what we need.** Claude Code's streaming input mode is "a long lived process that takes in user input, handles interruptions, surfaces permission requests, and handles session management" ([streaming vs single mode](https://code.claude.com/docs/en/agent-sdk/streaming-vs-single-mode)). The control protocol it uses (`control_request`, `control_response`, subtypes `interrupt`, `can_use_tool`, `set_permission_mode`, `set_model`) is defined in the official Agent SDK's sources ([types.py](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/types.py), [query.py](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/query.py)). Codex's app-server is OpenAI's interface for rich clients ([app-server docs](https://learn.chatgpt.com/docs/app-server)), and its schema can be generated from the installed CLI with `codex app-server generate-json-schema --out <dir>`; the method names above come from that schema (codex-cli 0.166.0).
- **No SDK in between.** The Agent SDK is Python and TypeScript only and itself spawns the `claude` binary ([overview](https://code.claude.com/docs/en/agent-sdk/overview)). Speaking the same stream-json the SDK speaks keeps Node and Python out of the service.
- **Stop never kills.** A refresh token is single-use: a process killed between renewing and saving leaves a spent token, and the next renewal is refused. Codex refreshes 5 minutes before expiry and fails with `refresh_token_reused` on a spent one (`codex-rs/login/src/auth/manager.rs` in [openai/codex](https://github.com/openai/codex)). The previous app lost logins this way and moved Stop to the engine's interrupt for that reason. Claude Code's docs say the same about signals: SIGTERM "leaves the turn unfinished", send the interrupt first ([headless: stop a run](https://code.claude.com/docs/en/headless#stop-a-run-with-sigterm)).
- **Codex kept open, unlike the previous app.** The previous app started one app-server per turn and tree-killed it to stop, because it had no interrupt call. `turn/interrupt` exists in the current stable schema, so one process per agent and a real interrupt replace both.
- **Claude Code first.** It is the engine the owner uses most (about 2,470 Claude turns to 720 Codex turns in the previous app's last 30 days), and its interface is the one the official SDK drives. Codex follows in the next slice; nothing in the interface is Claude-specific.

### ACP verdict

ACP ([agentclientprotocol.com](https://agentclientprotocol.com)) is JSON-RPC over stdio with stable `session/new`, `load`, `resume`, `list`, `prompt`, `cancel`, `request_permission` and plan updates ([schema](https://agentclientprotocol.com/protocol/schema)). It is well adopted (55 agents, Zed, JetBrains and about 20 multi-agent apps listed on [clients](https://agentclientprotocol.com/get-started/clients)). For Claude Code and Codex it costs more than it gives:

| | Native protocols | Through ACP adapters |
|---|---|---|
| Extra runtime | none | Node: both adapters are TypeScript ([claude-agent-acp](https://github.com/agentclientprotocol/claude-agent-acp), [codex-acp](https://github.com/agentclientprotocol/codex-acp)) |
| Token usage per turn | yes, both | stable `usage_update` has context size and cost only; token counts are unstable ([prompt turn](https://agentclientprotocol.com/protocol/prompt-turn)) |
| Plan updates | yes, both | Claude adapter sends none on current default models ([#1380](https://github.com/agentclientprotocol/claude-agent-acp/issues/1380)) |
| Resume | full | Claude adapter replays only from the last compaction ([#1297](https://github.com/agentclientprotocol/claude-agent-acp/issues/1297)) |
| Steering mid-turn | `priority` on Claude, `turn/steer` on Codex | not served by the Claude adapter in v2 ([#1498](https://github.com/agentclientprotocol/claude-agent-acp/issues/1498)); an extension in codex-acp |
| Codex diffs, approvals resolved | `turn/diff/updated`, `serverRequest/resolved` | dropped by codex-acp |
| Version churn | engine CLIs | engine CLIs, plus the adapters, plus the Rust crate: `agent-client-protocol` went 1.0 to 5.0 between June and November 2026 ([versions](https://crates.io/crates/agent-client-protocol/versions)) |
| Login | engine's own | ACP leaves where logins live to the agent ([authentication](https://agentclientprotocol.com/protocol/authentication)), so 0002 is needed either way |

What ACP would give: one driver for both engines and the long tail of other agents for free. That is the reason the interface mirrors its concepts and an ACP driver stays on the table for engines that speak it natively.

## Rules that follow

- A driver reports what its engine supports (resume, steer, plan, per-turn tokens, cost). The UI shows a feature only when the engine has it, never a stand-in.
- Usage is normalised per turn in the driver: Claude reports per message and a running session cost, Codex reports running totals with a `last` figure. The previous app once logged 781,768 input tokens for one line by adding them naively.
- Stop: the engine's interrupt, then wait. Closing a process (to switch sessions, or after 10 idle minutes): close stdin and wait for the engine's own exit. Switching never kills: a process that hasn't exited after 30 seconds holds the switch, shows as a hold on the agent and is recorded ([0010, Sessions](0010-coordination-as-state.md#sessions)). A kill happens only on the person's explicit action or at service shutdown after the grace period, written to the event record with why.
- Child processes on Windows: start `.exe` directly; resolve npm `.cmd` shims through PATHEXT and run their script under `node` rather than `cmd.exe` (cmd.exe mangles JSON arguments); `CREATE_NO_WINDOW` on every spawn; never `DETACHED_PROCESS`. On this PC `claude` is a native `claude.exe` and `codex` is an npm shim. On macOS: found through the login shell's `PATH`, each engine its own process group ([0015](0015-macos.md)).
- `--bare` is not used for Claude Code: it never reads a subscription login ([headless: bare mode](https://code.claude.com/docs/en/headless#start-faster-with-bare-mode)). The driver passes `--strict-mcp-config` and its own settings sources instead.
- **The engine's own tools stay on, with no per-agent switch.** The previous app added a Native tools switch, off by default, because each engine loaded the person's own setup: Claude Code had 79 skills and 18 connectors and Codex 132 skills and 6 connectors, none of them visible in that app, and plugin hooks (adhd-mode, yagni, ponytail) wrote their rules into every turn (the previous app's `planning/CLEAN-APP-PLAN.md`, 2026-10-08). In Gathr, each agent's own folder (0002) leaves out everything kept in `~/.claude` and `~/.codex`: user settings, plugins and their hooks, skills, memory and `config.toml` servers. What comes with the account itself still has to be turned off, the same way for every agent:
  - Claude Code: `--strict-mcp-config` (connectors added on claude.ai reach any folder signed in to that account);
  - Codex: `-c features.apps=false` (ChatGPT apps);
  - Claude Code's scheduling tools, through `--disallowedTools`: `CronCreate`, `CronDelete`, `CronList` and `ScheduleWakeup`. A wake-up set with them lives only as long as the process, and the driver closes idle processes, so the agent would never wake and nothing would say so. Coming back later is Gathr's job (0010).
  The deny list is one constant in the driver, each entry with its reason. A test reads the engine's `system/init` tool list and fails when a denied tool is still offered, or when the engine adds a tool the list hasn't looked at.
- **Plan from Claude Code's task tools.** Newer models get no task-tracking tools by default; the driver sets `CLAUDE_CODE_ENABLE_TODO_TOOLS=1` to turn them on. That only makes them available: which tools appear is chosen by `CLAUDE_CODE_ENABLE_TASKS`, which defaults to `TaskCreate`, `TaskUpdate`, `TaskGet` and `TaskList`, and gives the legacy `TodoWrite` only when set to `0` ([env vars](https://code.claude.com/docs/en/env-vars), [todo tracking](https://code.claude.com/docs/en/agent-sdk/todo-tracking)). The driver uses the default and builds the plan from the Task tools: `TaskCreate` adds a step (`subject`, its id from the tool result's `task.id`), `TaskUpdate` changes one by `taskId` (also sent as `id` or `task_id`) to `pending`, `in_progress` or `completed`, and `deleted` removes it. It also reads `TodoWrite` (a whole-list replace), because a person's own settings can set `CLAUDE_CODE_ENABLE_TASKS=0`.
- `--permission-prompt-tool stdio` appears in the SDK source, not the CLI docs ([subprocess_cli.py](https://github.com/anthropics/claude-agent-sdk-python/blob/main/src/claude_agent_sdk/_internal/transport/subprocess_cli.py)). The first slice's tests cover it so a CLI update that breaks it fails CI, not a user's turn.

## Turned down

- **ACP for Claude Code and Codex now.** See the verdict above: a Node dependency, lost usage, plan and resume detail, and three moving versions instead of one.
- **The Claude Agent SDK (TypeScript or Python) as a sidecar.** It drives the same CLI over the same protocol, and adds a second runtime to install and keep alive.
- **Linking Codex's Rust crates directly.** The archived Rust codex-acp did this ([zed-industries/codex-acp](https://github.com/zed-industries/codex-acp)) and pinned one Codex version per build; app-server lets the user's installed Codex update on its own.
- **One Codex process per turn** (the previous app): it costs a start per turn and needs a kill to stop.
- **A model loop over provider APIs.** Ruled out by the owner: each provider's own harness, never a homemade loop.

## Not verified

No live turn was run (Phase 0 rule). The stream-json frames, `--permission-prompt-tool stdio` and `turn/interrupt` are taken from docs, SDK sources and the generated schema; the first slice proves them against a signed-in engine. Also for the first slice: that Claude Code exits on its own when its stdin closes (the gentle close depends on it), and the exact Task tool input and result shapes, which the docs show only by example. Also whether skills synced from claude.ai reach a fresh signed-in folder; if they do, Gathr's skills design decides whether they stay.
