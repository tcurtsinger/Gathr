# 0019. Troubleshooting is built in, for agents as much as for the person

Status: accepted (the owner's requirement). Date: 2026-11-07.

Built: partly. Proof: `crates/gathr/tests/diag.rs`, `crates/gathr/tests/window.rs`, `ui/src/chat/Thread.test.tsx`. Not built: `diag stalls` and `diag usage`; the diagnostics tools for agents; the detailed-logs file; `bundle --hours`; `diag`'s own errors as events; engines, logins and Tailscale in the health event.

Out of date below: event names are [0011](0011-event-record.md)'s (`gathr.settings.change`; `gathr.ui.chat_open`, `chat_switch` and `first_token`, not `gathr.ui.timing`); warnings and errors become events, not every span; a reference opens Diagnostics, which is a Settings category ([0023](0023-settings-and-reply-states.md)); `diag turn` takes a reference.

## Implementation status

Health, trace, failures, read-only query, turn/raw frames, support bundles and the UI timeline are built. `diag stalls`, `diag usage`, diagnostics MCP tools and the remaining settings described below are not built yet.

The installer/app and service update archive ship `gathr` beside `gathrd`, built at the same version.
Use its full installed path ([running](../running.md)); no PATH change or separate CLI installer.

## Choice

Any problem in Gathr can be diagnosed from Gathr's own data, by an agent or the person, without reading files by hand, guessing, or reproducing it live. The event record ([0011](0011-event-record.md)) is the one place; this decision makes sure everything reaches it, that every error can be found from what the person sees, and that agents can query it directly.

### Everything lands in the record

| Source | How | Event |
|---|---|---|
| The service's own logs | A `tracing` layer writes every span and every `warn` and `error` to the event record, with the trace it belongs to. `debug` goes to a rotating file only while Settings, Diagnostics, "Detailed logs" is on. | `gathr.log` |
| Panics | A panic hook records the message, the thread and a backtrace, and the service carries on or restarts cleanly. | `gathr.service.panic` |
| Before the database opens | A plain crash log in the data folder; on the next start its lines are copied into the record. | `gathr.service.early_log` |
| Engines' stderr | Each line, redacted like raw frames, linked to the turn. A crashed engine's last 200 lines, never only the last two. | `gathr.engine.stderr` |
| The UI | Uncaught errors and failed API calls, with the screen, the component and the trace id; screen opens and timings (0011). | `gathr.ui.error`, `gathr.ui.timing` |
| Start-up health | What was found and what's wrong: engines and their versions, each account's login state, the data folder, the database size and migrations, the port, Tailscale when on. | `gathr.health` |
| Settings | Every change (0011). | `gathr.settings.changed` |

Nothing here ever holds a secret; the redaction rules of 0011 apply to every row.

### Every error can be found from what the person sees

- Each request from the UI carries a trace id, and every turn, task and wait belongs to a trace (0011).
- Every error shown in the app reads "… failed" with a short reference (the first 8 characters of its trace id), which copies on click. Given the reference, an agent finds the whole trace: what the person did, what the service did, what the engine said.

### Agents can query it

Read-only diagnostics, the same in two places:
- **`gathr diag`**, a command-line tool for agents building Gathr and for the person. It reads from the running service; when the service is down, from the database opened read-only; and when the database won't open either (a failed migration, a corrupt or locked file), from the plain crash log directly, already redacted when written. In that last case `health` reports why the database didn't open and the crash log's last lines, and `bundle` includes them, so a service that can't start is still diagnosed with one command;
- **diagnostics tools in Gathr's MCP server** ([0005](0005-mcp-server.md)), for agents working in Gathr, each allowed by the person per agent in Settings.

| Command | Answers |
|---|---|
| `trace <ref>` | Everything in one trace, in order, with timings |
| `failures [--since]` | Recent failures, grouped by cause, with their references |
| `stalls [--chat]` | Stretches where work was held and nothing ran, with the gate's reason and who it waited on |
| `turn <id> [--raw]` | A turn's timeline; with `--raw`, its redacted engine frames |
| `health` | The last start-up health check, and now |
| `usage [--by agent\|engine\|tool\|screen]` | Counts and tokens over a period, including the share of tool work by kind (0011) |
| `query "<sql>"` | Read-only SQL over the event tables, capped at 1,000 rows |
| `bundle [--hours]` | A redacted file of the last hours (events, health, versions, settings with secrets masked), for an issue report |

Output is JSON by default and readable text with `--text`. Errors from `diag` itself are events too.

### A driver bug becomes a test in one step

A turn's raw frames (0011) can be replayed through its engine's driver in a test, with no engine and no login: `gathr diag turn <id> --raw` saves them as a test fixture. A bug seen once in use becomes a failing test before it's fixed.

### Every feature is diagnosable before it's done

A feature PR shows its own events in the Diagnostics page and in `gathr diag`, and its proof includes one forced failure found from its reference. The reviewer checks: can its failure be diagnosed from the record alone?

## Why

- The owner's requirement: Gathr is built and run by agents, so troubleshooting has to be as fast for an agent as for a person.
- Lessons from the previous app: its logs were spread over six places, and answering "why did this stall" meant reading several by hand. An engine crash showed only its last two lines, with the real error cut off. A test runner's `fetch failed` arrived with no cause. A wait's check failed silently for two hours. Agents couldn't read usage counts through the API at all, so they read the raw files.
- A reference on every error turns "it broke" into one command.

## Turned down

- **Text log files as the main record.** Not queryable, not linked to turns or traces, and the reason the previous app's troubleshooting was slow. A plain log remains only for before the database opens and for "Detailed logs".
- **A hosted error tracker** (Sentry and the like). A paid or third-party service, and it would send data off the machine. OTLP export (0011) stays the opt-in way out.
- **Diagnostics only in the UI.** Agents can't read a page as fast as they can run a query, and the UI may be what's broken.
