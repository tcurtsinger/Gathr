# 0033: Codex, the second engine

Status: accepted (owner, 2026-11-09: Codex before the move-in). Slice 2's C1 to C4.

## The choice

Codex runs behind the same turn machinery as Claude Code. What differs is one driver per engine,
picked by the account's engine; everything above it (turns, tasks, waits, pauses, approvals, the
record) reads the same `Frame`s and doesn't know which engine runs.

- **The engine interface** is the `Frame` enum each driver's output is read into, plus what a turn
  writes (a message, Stop, an approval's answer, ending a background command) and how an engine
  starts. Two engines, so an `enum` per call site, not a trait.
- **One `codex app-server` per agent,** JSON-RPC over stdio, kept open across the turns of one chat
  like Claude Code (0001). At start: `initialize` (`clientInfo.name` `gathr`), `initialized`, then
  `thread/start` or `thread/resume` with the chat's saved thread id, its folder, approvals and
  Gathr's briefing as `developerInstructions` (a stored thread resumes by its id: "call
  `thread/resume` with the `thread.id` you recorded earlier", [app-server](https://learn.chatgpt.com/codex/app-server)).
  A message is `turn/start`, or `turn/steer` while a
  turn runs; Stop is `turn/interrupt`. Writes wait inside the driver until the thread is open.
- **How its output reads:**

  | Codex | Frame |
  |---|---|
  | `thread/start`/`thread/resume` result | `Init` (the thread id is the session) |
  | `item/agentMessage/delta` | `TextDelta` |
  | `item/completed` `agentMessage` | `Text` |
  | `item/started`/`item/completed` for a command, file change, MCP or web call | `ToolStart`/`ToolEnd` |
  | `item/started` `userMessage` | `Read` (a message sent into the turn was taken) |
  | `item/commandExecution/requestApproval`, `item/fileChange/requestApproval` | `ApprovalAsked` |
  | `error` with `willRetry: false`, a 401 | `EngineError` (`authentication_failed` for a 401) |
  | `turn/completed` | `TurnEnd` (`completed`, `interrupted`, `failed`), usage from the last `thread/tokenUsage/updated` |
  | `account/rateLimits/updated` at its limit | `Limit` |

  Retried errors (`willRetry: true`) are recorded only: Codex retries five times itself.
- **Approvals (0029):** `ask` is `read-only` with `on-request` (anything past reading asks, as
  Claude Code's Manual); `edits` is `workspace-write` with `on-request`; `auto` adds Codex's own
  reviewer (`approvalsReviewer: auto_review`), its counterpart of Claude Code's classifier; `full`
  is `danger-full-access` with `never`. Each ask comes to the person as Claude Code's do, answered
  `accept` or `decline`. Every push still waits in git (the worktree's `pre-push` hook),
  whichever engine runs it. `untrusted` isn't used: Codex retired it ("The retired setting can
  prevent either client from starting", [approvals and security](https://learn.chatgpt.com/codex/agent-approvals-security)).
- **Windows sandbox:** app-server gets `windows.sandbox="mxc"`, Codex's recommended backend, which
  "uses native process isolation without administrator-approved setup, additional Windows accounts,
  or local firewall rules" ([Windows sandbox](https://learn.chatgpt.com/codex/windows/windows-sandbox)).
  Without it this PC's readiness is `notConfigured` and `workspace-write` runs `readOnly`; with it
  readiness is `ready` and the thread runs `workspaceWrite`. The legacy backends (`elevated` needs
  an administrator prompt and its own Windows users) are never set up by Gathr.
- **Gathr's tools (0005):** `-c mcp_servers.gathr.url=…`, `bearer_token_env_var =
  "GATHR_AGENT_TOKEN"`, `default_tools_approval_mode = "approve"` (one of `auto`, `prompt`,
  `writes`, `approve`, [MCP](https://learn.chatgpt.com/codex/extend/mcp)), and `tool_timeout_sec`
  raised from its 60 s default, since `ask_person` and `wait` can hold a call. The token stays in
  the environment, never in arguments or `config.toml`.
- **Only what Gathr gives (0024):** each account's `CODEX_HOME` starts empty, and app-server gets
  `features.apps=false`, `features.plugins=false`, `features.hooks=false`,
  `skills.bundled.enabled=false`, `skills.include_instructions=false`, `notify=[]`,
  `check_for_update_on_startup=false`, `cli_auth_credentials_store="file"`. `config/read` proves
  each took; a test reads them from a recorded start.
- **Accounts (C2):** `<data>/accounts/<id>/codex` is the account's `CODEX_HOME`, with
  `OPENAI_API_KEY`, `CODEX_API_KEY` and `CODEX_ACCESS_TOKEN` removed from the engine's environment
  (0002). Sign-in is app-server's own `account/login/start`: `chatgpt` answers a page whose
  redirect is `127.0.0.1:1455` on the computer that runs Gathr ("the app-server hosts the local
  callback"), and `chatgptDeviceCode` a page and a code that work from any browser. Both end with
  `account/login/completed`, "emitted when a login attempt finishes (success or error)"
  ([app-server](https://learn.chatgpt.com/codex/app-server)); no code is pasted back. The browser
  page is the default: a device code works only once the person enables it in ChatGPT's security
  settings (or a workspace admin does), and is marked beta ([auth](https://learn.chatgpt.com/codex/auth)). Sign-out is `account/logout`. Signed in means `auth.json` exists in the folder,
  read for its keys only, never its tokens.
- **Gathr's copy (C3, 0025's Codex part):** npm's per-platform package
  (`@openai/codex@<version>-<platform>`), checked against its SHA-512 `integrity`, unpacked with the
  system's `tar` into `<data>/engines/codex/<version>`. The whole `vendor/<target>` folder is kept,
  not only `codex(.exe)`: it starts its own helpers from there (`codex-command-runner`, the sandbox
  setup, `codex-code-mode-host`). The same `current.json` switch and known-good copy as Claude's.
  `GATHR_CODEX_PATH` overrides it in dev and tests only.
- **Models (C4):** `model/list` on the account's app-server, all pages, hidden ones left out. Each
  model's `supportedReasoningEfforts` are its efforts and `defaultReasoningEffort` its default;
  efforts stay open strings. Codex has no family alias like `opus`, so it has no Latest until the
  owner picks how one is chosen (C4's open question).
- **The replay engine** plays a Codex recording too: it answers each request it reads with the
  recording's next frames, so the same tests run both engines without a login.

## Why

- **The protocol is the installed CLI's.** `codex app-server generate-json-schema` on 0.166.0 gave
  every method above; each mapping was then seen live in a Gathr account folder (below).
- **Kept open with a real interrupt,** not one process per turn (0001's reasons).
- **Both sign-in kinds,** because the owner uses Gathr from another computer (PC to Mac): the
  browser page's redirect only reaches the computer running the service, a device code works from
  anywhere.
- **Its own reviewer for `auto`:** the person picked auto for normal work; Codex's `auto_review`
  is the same idea as Claude Code's classifier, from the same vendor as the engine.

## Turned down

- **A trait per engine.** Two implementations, called from a handful of places: an `enum` is the
  smaller code (AGENTS.md: no abstraction with one user is the same rule at two).
- **ACP through `codex-acp`** (0001's verdict stands).
- **`codex exec` per turn.** No steer, no interrupt, a start per turn.
- **Only `codex(.exe)` in Gathr's copy.** Codex starts its helpers from beside itself.
- **Gathr's PreToolUse hook for Codex.** Codex's sandbox keeps writes in the folder, and the push
  hook is git's own, so the hook's two rules hold without one.

Docs read 2026-11-09 at learn.chatgpt.com (where developers.openai.com/codex now redirects); schema
from `codex app-server generate-json-schema` on 0.166.0. Where they differ, the schema wins.

## Checked live (codex-cli 0.166.0, Windows, a Gathr account folder, signed out)

- `initialize` answers `codexHome` as the account's folder; stdin closed, app-server exits in 10 to
  17 ms.
- `model/list` answers 7 models signed out, each with its efforts.
- `thread/start` answers the thread at once; a `turn/start` signed out retries 5 times over
  WebSocket, then 5 over HTTPS, and ends `failed` with `httpConnectionFailed` 401 after 13.9 s.
- `account/login/start` answers both kinds, and `account/login/cancel` cancels them.
- The `-c` switches above read back through `config/read`; Gathr's MCP server shows in
  `mcpServerStatus/list`, needing `GATHR_AGENT_TOKEN`.

- With `windows.sandbox="mxc"`, `command/exec` under `workspaceWrite` wrote a file in its folder,
  was refused one outside it ("Access is denied."), and saw `GATHR_AGENT_TOKEN` in its environment.

## Not verified

- A signed-in turn, approvals, Stop and Gathr's tools from Codex: they need a ChatGPT sign-in.
- That the agent's own shell tool passes `GATHR_AGENT_TOKEN` the way `command/exec` did (the docs'
  `shell_environment_policy` wording on names with `TOKEN` is unclear).
- macOS.
