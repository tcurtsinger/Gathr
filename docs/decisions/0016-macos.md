# 0016. Gathr runs on macOS as well as Windows

Status: accepted. Date: 2026-11-07. The owner works from a Windows PC and a Mac and wants Gathr on both.

## Implementation status

The macOS service, desktop window, menu bar, startup setting and isolated accounts are built. Preventing idle sleep, the login watcher and reporting a locked Keychain distinctly are not built yet. Engine discovery is superseded by 0025, as clarified below.

## Choice

Windows and macOS are both first-class from the first slice: the same service, the same UI, the same checks. What differs per platform lives in one module of the service (`platform`), with a test for each side. Linux isn't a target yet, but nothing here rules it out.

What changes for the Mac, decision by decision:

- **Starting engines (0001, amended by 0025).** Gathr starts only its own checked Claude Code/Codex copy by path. Installed builds wait for that copy; there is no custom path setting or fallback to a personal install. Dev/test overrides are explicit. The login shell supplies the environment for engine tools on macOS, not the engine binary itself.
- **Stopping engines (0001, 0004).** Nothing changes in how a stop works: the engine's own interrupt, then closing stdin and waiting. For the rare explicit kill (the person's action, or shutdown after the grace period), each engine is started as the leader of its own process group (`process_group(0)` in Rust's `std`), so the kill reaches the builds and tests it started: `SIGTERM` to the group, then `SIGKILL` after the grace period. Windows keeps its tree kill. Both are written to the event record with why, as now.
- **Claude Code's login (0002).** On macOS Claude Code keeps its login in the Keychain, not in `.credentials.json`, and setting `CLAUDE_CONFIG_DIR` "keys the macOS Keychain entry to that directory too, so a session with a different `CLAUDE_CONFIG_DIR` reads a different entry" ([authentication](https://code.claude.com/docs/en/authentication)). So one folder per account still means one login per account, and nothing in 0002 changes except how the service sees a renewal:
  - The login state probe is `claude auth status`, which 0002 already names for macOS.
  - The renewal gate can't see the token's expiry on macOS: it's inside the secret, and the item's modification date says only that something wrote it, not when the token expires or that a write renewed it. So on macOS the gate doesn't wait for an expiry. Every start and turn on an account goes through it one at a time, each waiting until the one ahead of it gets its first successful reply from the provider, or fails, or 30 s pass. A successful reply is, for this turn, the first `stream_event` whose event is `message_start`, or else the first `assistant` message, or a `result` that isn't an error ([Agent SDK message types](https://code.claude.com/docs/en/agent-sdk/streaming-output)). Nothing local counts: not `system` messages (`init`, retries, status), not hook events, not echoed tool results. A failure is an error `result` or the engine exiting; it releases the next waiter, which then tries the login itself. The first request after an expiry renews the login and the rest go after it has worked, whether the token was near expiry or not, so the idle and unchanged-item cases need nothing extra. Renewal is shown by that reply, never by the Keychain. The cost is a stagger of one first reply between starts on the same account; the first slice measures it.
  - The login watcher uses the item's modification date only as a sign that something changed (its attributes only, never the secret), then reads state with `claude auth status` as 0002 does. Gathr never asks the Keychain for the token, so the person is never shown a Keychain prompt on Gathr's behalf.
  - `gathrd` runs in the signed-in person's login session. Started from an SSH shell, the Keychain is locked to it, so `claude auth status` answers signed out for every account, signed in or not. When it says signed out and `security show-keychain-info` on the login keychain fails, Gathr reports the Keychain as locked to the service, not the account as signed out. (A launchd job in the person's session, `launchctl bootstrap gui/<uid>`, reaches the Keychain from SSH without a password; that's how the room runs its Mac checks.)
- **Codex's login (0002).** No change: the driver already starts app-server with `cli_auth_credentials_store="file"`, so the login stays in `auth.json` in the account's folder on both systems ([Codex auth](https://learn.chatgpt.com/docs/auth)).
- **The service and the window (0004, 0012).** `gathrd` and the lock work as on Windows. `service.json` is created with mode `0600`. Data lives in `~/Library/Application Support/Gathr` (Windows: `%APPDATA%\Gathr`). The window renders with the system's WKWebView instead of WebView2, and loads the service's loopback HTTP origin. The guard also accepts `tauri://localhost` and `http://tauri.localhost` for Tauri clients. The tray icon is a menu bar icon; "start with Windows" becomes "open at login" on both.
- **Sleep.** While any turn is running, the service asks the system not to idle-sleep (macOS: a power assertion, as `caffeinate -i` takes; Windows: `SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED)`, which holds until cleared, where `ES_SYSTEM_REQUIRED` alone only resets the idle timer once), and lets go when the last turn ends. On Windows the state belongs to the thread that set it, so one dedicated thread sets it when the first turn starts and clears it with `ES_CONTINUOUS` when the last ends; async tasks only signal that thread, since they can move between threads across an await. The screen may still turn off. This matters most for remote use (0017): a chat driven from the other computer stops if its host sleeps.
- **CI.** The CI workflow runs the same three checks on `macos-latest` next to `windows-latest`. Standard GitHub-hosted runners are free for public repositories ([Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)).

## Why

- **One code base, one module for the differences.** Everything above that differs is small: where to find a binary, how to kill a group, where a login's expiry is read. Keeping it in one module, each side tested, keeps the rest of the service free of `cfg` branches.
- **The login-shell environment still matters on the Mac.** Finder does not supply the person's full tool environment. Gathr reads it for commands the engine runs and records its source. Engine selection itself follows 0025.
- **Process groups are the Unix tree.** macOS has no Job Objects. A process group catches everything the engine starts unless a grandchild starts its own session, which builds and test runners rarely do. Since kills are rare and always recorded, that gap is acceptable.
- **The Keychain keeps 0002's promise.** Gathr never reads a login's secret; reading only an item's attributes keeps that true on the Mac, and avoids a Keychain prompt that would name Gathr as asking for Claude's password.
- **Sleep would break a remote chat silently.** The previous app ran on one always-on PC; with two computers, the one running a chat is often not the one in front of the person.

## Turned down

- **Windows first, the Mac later.** The owner works from both now; every Windows-only choice made before the Mac runs would have to be found and undone later. CI on both from the start catches them as they're written.
- **Reading `PATH` from `launchctl getenv` or `/etc/paths` only.** It misses what the person's shell profile adds (`~/.local/bin`, nvm, Homebrew on Apple silicon), which is where these CLIs usually are.
- **The Keychain item's modification date as the gate's expiry signal.** It can't tell when the token expires, an idle account's date is the same before and after expiry, and any other write to the item would release the waiters.
- **Copying the Claude login out of the Keychain into a file** so both systems look the same. That copies a login, which 0002 forbids.
- **A launchd agent to keep `gathrd` running on the Mac.** As with a Windows service (0004), it adds an install step for nothing the window-started daemon doesn't already do. "Open at login" covers starting it.

## Not verified

No Mac was available for this decision. Checked since on the owner's Mac (Apple silicon, macOS 27.2, Claude Code 2.1.319, CI's build of Gathr, 2026-11-07):

- **Keychain items:** one `Claude Code-credentials` item, plus one `Claude Code-credentials-<8 hex characters>` per other config folder signed in. Reading their attributes without `-g` or `-w` over SSH showed no prompt.
- **No file fallback:** after sign-in, the account's folder held no `.credentials.json`; the login was in the Keychain.
- **Over SSH**, `claude auth status` answered signed out for the signed-in folder, and `security show-keychain-info` exited 36 ("User interaction is not allowed"). In the login session (a launchd job) it answered signed in, and `show-keychain-info` exited 0.
- **Part B on the Mac:** Settings and `gathr diag health` said signed in; a send answered in 162 ms, its first text came at 2.4 s; Stop ended the turn and the next message ran at once; two chats on one login started one after the other, the second 1 ms after the first one's first reply; after a restart the chat resumed its Claude Code session and remembered it.

Still not verified:

- A real renewal through the gate on the Mac: the token wasn't near expiry, so the two-chat run shows the ordering, not a renewal.
- Reporting a locked Keychain as such (above): decided here, not built yet.
- The installed window loads the service's loopback HTTP page on both platforms; remote origins are not built yet.
- That engine tools receive the current login-shell environment. Personal engine discovery was superseded by 0025.
- Idle-sleep prevention is not built yet; it has no live check to claim.
- How long the macOS gate's stagger is: the time from a Claude start to its first successful reply (`message_start`, or else the first `assistant` message or a non-error `result`), with several agents on one account starting together and the login expired. That `system/init` arrives before that reply and doesn't release the gate. And that Claude has renewed the login before that reply, so the next waiter starts on the fresh token.
- Remote use and idle-sleep prevention are not built yet.
