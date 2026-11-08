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
- Every turn records the provider's token totals as reported and the characters of each part Gathr added (briefing, catch-up, the ask), with any token estimate labelled as one ([0020](decisions/0020-context-per-turn.md)).
- The service idles under 1% CPU with nothing running.

## Test data

A seed command makes made-up chats: one with 500 messages, one with 50 tool calls in a turn, one with a 20,000-word answer. No real conversations, ever.

## How the person checks it by hand

Once, on a computer with the toolchain from [AGENTS.md](../AGENTS.md) ("Build and check"): clone the repo, then run `npm ci` in `ui/`. Every command below runs in PowerShell from the repo root. The test folder is new and only for this; Gathr never touches your own `~/.claude`.

```powershell
$data = "$env:USERPROFILE\gathr-check"
```

1. Start Gathr with the made-up chats: `cargo dev --port 47100 --data $data --seed --window`. The first build takes a few minutes; the window opens when it's done. Leave this terminal running.
2. Click **Seed: 500 messages** in the sidebar: it should open at once, at its latest message. Click **Seed: 50 tool calls in a turn**, then back.
3. Click **Settings** (the gear at the bottom left). **Engines** has no accounts yet. Click **Add**, type `Personal`, then click **Sign in** on Personal: Claude's sign-in page opens in your browser. Sign in there. If the browser didn't open, click **Open sign-in page**; if the page shows a code, paste it into **Code** and click **Sign in**. Personal reads **Claude Code / Signed in**. Chats run on the first account added.
4. Still in Settings: click **Add**, type `Work`, click **Sign in** and sign in the same way. Click **Rename** on Work, type `Second`, **Save**. Click **Sign out** on Second: only Second reads Signed out; Personal stays signed in, because each account has its own login folder ([0002](decisions/0002-login-folders.md)). Close Settings.
5. Click **New**, type `Write 2,000 words on rivers`, press Enter. Watch it stream. Click **Stop** partway: the reply shows Stopped, with what it had written.
6. Type `What were you writing?` and send. It should know: same session.
7. Quit: press Ctrl+C in the terminal (the window closes with it). Run step 1's command again without `--seed`: `cargo dev --port 47100 --data $data --window`. Open the chat and ask `What were you writing?` once more. Same session, history intact.
8. Open **Settings**, **Diagnostics**. Each turn of the chat shows its timeline, tokens and how it ended; the stopped one says stopped.
9. Quit (Ctrl+C). Start with an engine that doesn't exist (Settings has no path, [0026](decisions/0026-agent-settings.md)):
   ```powershell
   $env:GATHR_CLAUDE_PATH = "$data\no-such-claude.exe"
   cargo dev --port 47100 --data $data --window
   ```
   Send a message. The chat shows **Send failed** with a reference. In a second terminal, in the repo root: `cargo run -q --bin gathr -- diag trace <reference> --data $data --text` (set `$data` there too). It shows the message, the engine start that failed and why. Quit, run `Remove-Item Env:GATHR_CLAUDE_PATH`, and start again as in step 7.
10. Open **Settings** and click **Sign out** on Personal. Close Settings and send a message. Per [0002](decisions/0002-login-folders.md) it doesn't run: the chat says it's waiting for sign-in. In the second terminal, `cargo run -q --bin gathr -- diag health --data $data --text` shows Personal as `signed_out`, with the message under `held`. Sign Personal in again (step 3): the message sends.
11. Quit (Ctrl+C). Make a folder with a broken database, with no logins in it:
    ```powershell
    New-Item -ItemType Directory "$data-broken"
    Set-Content "$data-broken\gathr.db" 'not a database'
    cargo dev --port 47100 --data "$data-broken"
    ```
    Gathr fails to start. `cargo run -q --bin gathr -- diag health --data "$data-broken" --text` says the database didn't open, why (`file is not a database`), and shows the crash log's last lines, with nothing read by hand.

To clean up: in Settings, click **Remove** on each account, then **Remove** again to confirm: the row goes, and its login with it. Quit, and delete `$data` and `$data-broken`.

## Proof in the PR

Screenshots of the chat, a streaming answer, the stopped turn and Diagnostics, at desktop size, light and dark, next to the picked design. The timings above from the event record. One forced failure found from its reference with `gathr diag`, with the output. What couldn't be run, said plainly.

## Left out

Chats with several agents, tasks, waits, pause and plans (0010 and 0013 come next), a second engine, agents' tools beyond the engine's own, the MCP server, skills, memory, routines, voice, and every setting beyond signing in and the Diagnostics retention.
