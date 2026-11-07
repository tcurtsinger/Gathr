# 0011. One event record, named after OpenTelemetry's GenAI conventions

Status: proposed. Date: 2026-11-06.

## Choice

Every turn, model call, tool call, task change, hand-off, wait, pause, side job and failure is a row in one append-only `events` table in the service's SQLite database. The row is written in the same transaction as the state change it records (0010), so the record and the state never disagree.

Names and attributes follow OpenTelemetry's GenAI semantic conventions where they cover the thing, and a `gathr.` namespace where they don't. Export over OTLP is possible, off by default.

A Diagnostics page in the app reads the record.

## Why

- The previous app's logs were spread over six places: a normalised event log per thread, each engine's raw messages, command counts, token totals, a per-turn engine report, and a text server log. Answering "why did this room stall for 20 minutes" meant reading several of them by hand.
- A flaky failure arrived as "fetch failed" with the cause already gone. Recording the failure with its cause, where it happened, is the fix.
- Using OpenTelemetry's names means the record exports to any OTel backend without a translation layer, and the names are ones other tools already understand.

## The table

| Column | What |
|---|---|
| `id` | ULID, so ids sort by time |
| `ts` | UTC, microseconds |
| `name` | the span or event name, below |
| `phase` | `start`, `end` or `point` |
| `trace_id`, `span_id`, `parent_span_id` | OpenTelemetry ids; one trace per ask (the message or task that started the work), so a room's whole chain is one trace |
| `room_id`, `conversation_id`, `agent_id`, `task_id` | nullable; indexed for the Diagnostics page |
| `duration_us` | on `end` rows |
| `status` | `ok` or `error` |
| `error_type` | OTel's `error.type`: a short class of failure |
| `attrs` | JSON: the attributes below |

Indexes on `(conversation_id, ts)`, `(room_id, ts)`, `(trace_id)` and `(name, ts)`. SQLite runs in WAL mode so the page reads while the service writes ([SQLite WAL](https://www.sqlite.org/wal.html)).

## Names from OpenTelemetry

The GenAI conventions now live in their own repository, [open-telemetry/semantic-conventions-genai](https://github.com/open-telemetry/semantic-conventions-genai); the pages in the main semantic-conventions repo say "Moved". **Every GenAI name below has status Development, not Stable** ([docs/gen-ai/README.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/README.md)). Names can still change, so the service keeps the mapping in one module, pinned to a commit of that repository, and a rename is one change there.

| Gathr thing | Span | Source |
|---|---|---|
| A turn (one run of an agent's engine) | `invoke_agent {gen_ai.agent.name}`, `gen_ai.operation.name = invoke_agent`. Span kind `INTERNAL`: the engine runs on this machine | [gen-ai-agent-spans.md, invoke agent internal span](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/gen-ai-agent-spans.md) |
| One model call inside a turn, when the engine reports it | `{gen_ai.operation.name} {gen_ai.request.model}`, usually `chat` | [client-inference.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/client-inference.md) |
| A tool call | `execute_tool {gen_ai.tool.name}`, kind `INTERNAL` | [execute-tool.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/execute-tool.md) |
| A call to Gathr's own MCP server | `{mcp.method.name} {target}`, e.g. `tools/call task` | [mcp.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/mcp.md) |

Attributes used ([registry](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/registry/attributes/gen-ai.md)):
- turns: `gen_ai.operation.name`, `gen_ai.provider.name` (`anthropic`, `openai`), `gen_ai.agent.id`, `gen_ai.agent.name`, `gen_ai.conversation.id` (the engine's session or thread id), `gen_ai.request.model`, `gen_ai.response.finish_reasons`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_write.input_tokens`, `error.type`;
- tool calls: `gen_ai.tool.name`, `gen_ai.tool.call.id`, `gen_ai.tool.type`, `gen_ai.tool.description`, and for commands `process.executable.name`, `process.exit.code`;
- MCP: `mcp.method.name`, `mcp.session.id`, `mcp.protocol.version`.

OpenTelemetry marks message content and tool arguments and results as Opt-In (`gen_ai.input.messages`, `gen_ai.output.messages`, `gen_ai.tool.call.arguments`, `gen_ai.tool.call.result`). Gathr keeps conversation content in its own message tables (it has to show the chat); the event record links to them by id and never copies content.

## Gathr's own names

Where OpenTelemetry has nothing, `gathr.` events, each a `point` unless it has a duration:
- `gathr.task.given`, `gathr.task.state` (from, to, by, reason), `gathr.task.restarted`;
- `gathr.wait.left`, `gathr.wait.check` (exit code, duration), `gathr.wait.fired`, `gathr.wait.ended`;
- `gathr.pause.set`, `gathr.pause.quiet`, `gathr.pause.resumed`, `gathr.pause.stop_now` (what was ended, by pid);
- `gathr.gate.held` (what start was held, and why) and `gathr.gate.released`;
- `gathr.plan.proposed`, `gathr.plan.check` (checker, agree or objection), `gathr.plan.state`;
- `gathr.handoff` (from, to, chain length);
- `gathr.ask` span (asker, helper, outcome `answered`, `passed`, `failed`);
- `gathr.usage_hold` span (account, until);
- `gathr.engine.process` span (adapter, pid, exit code, how it ended), `gathr.engine.interrupt`, `gathr.engine.login` (status checks and renewals, never the token);
- `gathr.stream.first_token` (time from the engine emitting to the UI receiving), and `gathr.ui.timing` points the UI sends for screen opens and switches;
- `gathr.ui.screen_open` (which screen, never its content), so usage of each screen is counted per day;
- `gathr.settings.changed` (which setting, by whom, old and new value; secret values masked);
- `gathr.cli.command` (which command an agent ran, never its arguments).

Usage counts (screens, commands, engines, settings changes) are read through the service's API by the person, and by an agent the person allows. The previous app's usage report had gaps on exactly these: no screen counts, command counts only for the last few days, a turn log capped at 3,000 turns, and no record of settings changes.

## What is never recorded

- Secrets of any kind: tokens, API keys, login files or their contents, cookies, environment variable values.
- Command lines are recorded with environment-style values (`KEY=value`, `--token value`) masked.
- Login folders are recorded by kind ("claude login folder"), never their paths' contents.
- Conversation text and tool arguments, as above, unless the person turns on content capture for OTLP export, separately.

## Raw engine messages

Each engine adapter keeps the engine's own messages, as received, in an `engine_raw` table linked to the turn's span, so an adapter bug can be traced to what the engine actually sent. Same never-recorded rules apply.

## Retention

- `events`: 90 days by default; a setting in Settings, Diagnostics.
- `engine_raw`: 7 days by default; same place.
- Daily rollups per agent, room and engine (turns, tokens, tool calls, failures, time waiting) are kept until deleted, so trends outlive the detail.
- Pruning runs in the background at low priority, in small batches.

## OTLP export

Off by default. A setting turns it on with an endpoint (usually a collector on this machine). The service exports spans and events built from the same rows. Content capture is a second, separate switch. No paid service is needed for anything here.

## The Diagnostics page

Opens from any turn, task, room or chat, and from Settings. It answers:
- **What happened in this turn?** A timeline: the turn's start, each model call and tool call with its duration, side jobs, waits it left, and how it ended, with the error and its cause when it failed.
- **Where did the room stall?** Stretches where work was queued and nothing ran, each with its reason from the gate: paused, usage hold, waiting on an `after` task, a plan check, a wait.
- **What waited on whom, and for how long?** Waits and held tasks as a chain, each with its duration.
- **What fails, and how often?** Engine exits, tool errors, login problems and check errors, by agent, engine and day.
- **Where can the workflow improve?** Tasks that went `left` or `blocked`, plan-check objections per plan, asks and how many were passed or used, review notes per PR, time from a task given to done, tokens per task.
- **Is it fast?** First-token latency, turn duration, tool durations, screen open and switch times.

Every row links to where it happened in the conversation.

## Turned down

- **An OpenTelemetry SDK and backend as the store.** It would need a collector and a trace store running beside the app to see anything. A local table works offline, with nothing installed, and exports when wanted.
- **Separate files per kind of record.** That's the previous app's six files.
- **Text logs as the record.** Fine for a crash before the database opens (the service still writes a plain log for that), but not queryable.
- **Copying message content into events.** It doubles storage and spreads private text into the place most likely to be exported or attached to a bug report.
