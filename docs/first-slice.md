# The first slice: one chat, one engine, end to end

Status: draft. The first thing Gathr builds, once the architecture is approved. It proves the spine every later feature hangs on: the service, the engine adapter, storage, the event record, the API and the UI, together.

## What it does

1. **Sign in.** The person signs the engine in, in Gathr's own login folder for that account, through the engine's own login flow. Gathr never reads or copies the login. Claude Code goes first ([0001](decisions/0001-engine-interface.md)); the folder layout is [0002](decisions/0002-login-folders.md).
2. **Send.** The person types a message in a chat and sends it.
3. **Stream.** The answer streams in as the engine produces it: text, and each tool call as a line with its name, status and duration.
4. **Stop.** Stop interrupts the turn through the engine's own interrupt, never a process kill. The chat shows the turn as stopped, with what it had produced.
5. **Resume after restart.** Quit Gathr (window and service) and start it again: the chat is there with every message, and the next message continues the same engine session.
6. **Diagnostics.** A Diagnostics page for the chat shows each turn's timeline: model calls, tool calls with durations, tokens, how it ended, and any failure with its cause.
7. **Troubleshooting for agents** ([0019](decisions/0019-troubleshooting.md)): the service's logs, panics, engine stderr, UI errors and the start-up health check all land in the event record; every error in the UI shows a reference; `gathr diag` answers `trace`, `failures`, `turn --raw`, `health`, `query` and `bundle`; a turn's raw frames replay through the driver as a test. The diagnostics tools in the MCP server come with the MCP server.

Engine-specific parts follow [0001](decisions/0001-engine-interface.md) (the stream-json driver, resume, tool approvals over the same pipe, Stop by interrupt) and [0002](decisions/0002-login-folders.md). The slice also proves each item on those files' "Not verified" lists.

## What it must meet

Measured on this PC, with the event record's own timings (0011):
- First token shown within **300 ms** of the engine emitting it.
- A chat with **500 messages** (tool runs and long answers among them) opens within **200 ms**.
- Switching between two such chats within **200 ms**.
- Stop: the chat shows "stopped" within **200 ms** of the click; the engine's turn has ended within 5 s.
- The service idles under 1% CPU with nothing running.

## Test data

A seed command makes made-up chats: one with 500 messages, one with 50 tool calls in a turn, one with a 20,000-word answer. No real conversations, ever.

## How the person checks it by hand

To be filled in with exact commands when it's built; the shape:
1. Run the dev build (one command) and the seed command.
2. Open the app. Open the 500-message chat; it should open at once. Switch to another and back.
3. Sign the engine in from Settings, Engines.
4. In a new chat, ask for something long ("write 2,000 words on rivers"). Watch it stream. Press Stop partway.
5. Ask "what were you writing?" It should know: same session.
6. Quit Gathr completely, start it again, open the chat, and ask once more. Same session, history intact.
7. Open Diagnostics from the chat. Each turn shows its timeline, tokens and how it ended; the stopped one says stopped.
8. Sign the engine out in its folder and send a message. The chat shows "Send failed" with a reference. In a terminal, `gathr diag trace <reference>` shows the message, the turn, the engine's error and the login state.

## Proof in the PR

Screenshots of the chat, a streaming answer, the stopped turn and Diagnostics, at desktop size, light and dark, next to the picked design. The timings above from the event record. One forced failure found from its reference with `gathr diag`, with the output. What couldn't be run, said plainly.

## Left out

Chats with several agents, tasks, waits, pause and plans (0010 and 0013 come next), a second engine, agents' tools beyond the engine's own, the MCP server, skills, memory, routines, voice, and every setting beyond signing in and the Diagnostics retention.
