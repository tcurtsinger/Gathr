# 0011. One event record, named after OpenTelemetry's GenAI conventions

Status: accepted. Date: 2026-11-06.

## Implementation status

The event table, raw engine records, turn timeline and core diagnostics are built. Retention controls, pruning, daily rollups, content-capture controls and OTLP export are not built yet. Planned names are separated from emitted names below.

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
| `trace_id`, `span_id`, `parent_span_id` | OpenTelemetry ids; one trace per ask (the message or task that started the work), so a chat's whole chain is one trace |
| `room_id`, `conversation_id`, `agent_id`, `task_id` | nullable; indexed for the Diagnostics page |
| `duration_us` | on `end` rows |
| `status` | `ok` or `error` |
| `error_type` | OTel's `error.type`: a short class of failure |
| `attrs` | JSON: the attributes below |

Indexes on `(conversation_id, ts)`, `(room_id, ts)`, `(trace_id)` and `(name, ts)`. SQLite runs in WAL mode so the page reads while the service writes ([SQLite WAL](https://www.sqlite.org/wal.html)).

## Names from OpenTelemetry

The table is the naming target for model/MCP instrumentation. Current turn spans are
`invoke_agent {engine}`, tool spans are `execute_tool {name}`, and service MCP calls use
`gathr.tool.call`. The implemented Gathr names are listed below; OTLP export is not built yet.

The GenAI conventions now live in their own repository, [open-telemetry/semantic-conventions-genai](https://github.com/open-telemetry/semantic-conventions-genai); the pages in the main semantic-conventions repo say "Moved". **Every GenAI name below has status Development, not Stable** ([docs/gen-ai/README.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/README.md)). Names can still change, so the service keeps the mapping in one module, pinned to a commit of that repository, and a rename is one change there.

| Gathr thing | Span | Source |
|---|---|---|
| A turn (one run of an agent's engine) | `invoke_agent {engine}`, `gen_ai.operation.name = invoke_agent`. Span kind `CLIENT`: the engine is a separate process, and the conventions keep `INTERNAL` for an agent running inside the instrumented process. The service's own scheduling around a turn (the gate, holds) is `gathr.*` below, not a second agent span | [gen-ai-agent-spans.md, invoke agent client span](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/gen-ai-agent-spans.md) |
| One model call inside a turn, when the engine reports it | `{gen_ai.operation.name} {gen_ai.request.model}`, usually `chat` | [client-inference.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/client-inference.md) |
| A tool call | `execute_tool {gen_ai.tool.name}`, kind `INTERNAL` | [execute-tool.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/execute-tool.md) |
| A call to Gathr's own MCP server | `{mcp.method.name} {target}`, e.g. `tools/call task` | [mcp.md](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/gen-ai/mcp.md) |

Attributes used ([registry](https://github.com/open-telemetry/semantic-conventions-genai/blob/main/docs/registry/attributes/gen-ai.md)):
- turns: `gen_ai.operation.name`, `gen_ai.provider.name` (`anthropic`, `openai`), `gen_ai.agent.id`, `gen_ai.agent.name`, `gen_ai.conversation.id` (the engine's session or thread id), `gen_ai.request.model`, `gen_ai.response.finish_reasons`, `gen_ai.usage.input_tokens`, `gen_ai.usage.output_tokens`, `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_write.input_tokens`, `error.type`;
- tool calls: `gen_ai.tool.name`, `gen_ai.tool.call.id`, `gen_ai.tool.type`, `gen_ai.tool.description`, and for commands `process.executable.name`, `process.exit.code`; plus Gathr's own `gathr.tool.kind` (`find`, `read`, `review`, `edit` or `other`, from the tool's name and, for shell commands, the program) and `gathr.tool.result_bytes` (the full size of the result before anything is cut), so the share of work and context that goes to reading code is measured from day one. In the previous app, reading code was 41% of tool calls and most of the tool output that filled the context, but results were stored cut at 2,000 characters, so its true share couldn't be known. A code index (find code by function) is weighed against these numbers right after the first slice;
- MCP: `mcp.method.name`, `mcp.session.id`, `mcp.protocol.version`.

OpenTelemetry marks message content and tool arguments and results as Opt-In (`gen_ai.input.messages`, `gen_ai.output.messages`, `gen_ai.tool.call.arguments`, `gen_ai.tool.call.result`). Gathr keeps conversation content in its own message tables (it has to show the chat); the event record links to them by id and never copies content.

## Gathr's own names

Names currently emitted by the service and UI (points unless the call records a duration):
- tasks: `gathr.task.give`, `gathr.task.state`, `gathr.task.stale`,
  `gathr.task.hear_back`, `gathr.task.recover`, `gathr.task.answer` and `gathr.task.answered`;
- waits: `gathr.wait.leave`, `gathr.wait.fire`, `gathr.wait.end`, `gathr.wait.expire`,
  `gathr.wait.agent` and `gathr.wait.failing`; check commands also use the shared command record;
- pause: `gathr.pause.start`, `gathr.pause.quiet`, `gathr.pause.end` and `gathr.pause.stop_now`;
- usage holds: `gathr.usage.hold`;
- engine lifecycle: `gathr.engine.start`, `gathr.engine.exit`, `gathr.engine.close`, `gathr.engine.hold`,
  `gathr.engine.stderr`, `gathr.engine.sign_in` and `gathr.engine.sign_out`;
- Gathr MCP calls: `gathr.tool.call`;
- timings: `gathr.turn.first_token` at the service, `gathr.ui.first_token` when painted,
  `gathr.ui.chat_open` and `gathr.ui.chat_switch`;
- UI failures and settings: `gathr.ui.error` and `gathr.settings.change`.

These names match `tasks.rs`, `waits.rs`, `pauses.rs`, `bin/gathrd/coord`, the turn driver and UI reports.
Use `event_log` for readable JSON attributes and a failure's reference.

Not built yet: dedicated `gathr.gate.held`/`released`, `gathr.plan.*`, `gathr.handoff`,
`gathr.ask`, generic screen-open/CLI usage counts and their API. These remain planned instrumentation,
not names to query in a current record.

## What is never recorded

- Secrets of any kind: tokens, API keys, login files or their contents, cookies, environment variable values.
- Command lines are recorded with environment-style values (`KEY=value`, `--token value`) masked.
- Login folders are recorded by kind ("claude login folder"), never their paths' contents.
- Conversation text and tool arguments in `events`, as above. They're exported over OTLP only if the person turns on content capture, separately.

## Raw engine frames

One policy for every driver; [0001](0001-engine-interface.md) follows it.

- **What it's for:** tracing a driver bug to what the engine actually sent or was sent. It's local only and never exported, even with content capture on.
- **What's kept:** every frame in both directions, in an `engine_raw` table linked to the turn's span: direction, time, the frame's type, its size, and its body. Bodies may hold conversation text and tool arguments: the same content Gathr already keeps in its message tables on this machine, so keeping it here adds no new exposure. They never hold secrets.
- **Order:** read the frame, redact it, write it, then interpret it. Nothing reaches the database before redaction, so a crash between write and interpret leaves only redacted rows.
- **Redaction:** the driver removes secrets by field (authorization headers, tokens, keys, cookies, environment values in any frame that carries them) and then by pattern over the whole body (`sk-…`, `Bearer …`, JWT shapes, `KEY=value`). Each removal leaves `[redacted:<kind>]`. A frame that parses is redacted value by value, never as its JSON text: inside quotes and escapes a secret isn't a word on its own (`{"content":"sk-…"}`, a key split across streamed tool input). A string under a key naming a secret (`token`, `secret`, `password`, `api_key`, `authorization`, `credential`, `cookie`) is hidden whole, and the patterns end a word at whitespace and at JSON and shell punctuation. A flag whose name holds `token`, `key`, `secret` or `password` hides its value after a space or an `=` (`--token=[redacted:flag]`; `--port=47101` stays), and a `[redacted:<kind>]` already in the text is passed over whole, so text redacted twice reads the same ([0041](0041-what-a-step-ran.md)). Event attributes go through the same code. Stream fragments (Claude Code's `content_block_delta`, Codex's `…/delta`) and every line after them, stderr included, are held until the block or item ends (or the engine closes), then written in their order with the fragments redacted joined, so a key split across two frames is never written in either; when that changes them, the first fragment carries the redacted whole and the rest are left empty.
- **Malformed frames:** a frame that isn't valid JSON is pattern-redacted as text, cut to 64 KB, and written with `malformed = 1`; the driver then reports a parse failure event. It's never written whole and unredacted.
- **Off switch:** Settings, Diagnostics, "Keep raw engine frames", on by default with a 7-day retention. Off writes only the metadata columns.

## Retention

- `events`: 90 days by default; a setting in Settings, Diagnostics.
- `engine_raw`: 7 days by default; same place.
- Daily rollups per agent, chat and engine (turns, tokens, tool calls, failures, time waiting) are kept until deleted, so trends outlive the detail.
- Pruning runs in the background in small batches.

## OTLP export

Off by default. A setting turns it on with an endpoint (usually a collector on this machine). The service exports spans and events built from the same rows. Content capture is a second, separate switch. No paid service is needed for anything here.

## The Diagnostics page

Opens from any turn, task or chat, and from Settings. It answers:
- **What happened in this turn?** A timeline: the turn's start, each model call and tool call with its duration, side jobs, waits it left, and how it ended, with the error and its cause when it failed.
- **Where did the chat stall?** Stretches where work was queued and nothing ran, each with its reason from the gate: paused, usage hold, waiting on an `after` task, a plan check, a wait.
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
