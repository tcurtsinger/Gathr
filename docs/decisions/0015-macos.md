# 0015. Gathr runs on macOS as well as Windows

Status: proposed. Date: 2026-11-07. The owner works from a Windows PC and a Mac and wants Gathr on both.

## Choice

Windows and macOS are both first-class from the first slice: the same service, the same UI, the same checks. What differs per platform lives in one module of the service (`platform`), with a test for each side. Linux isn't a target yet, but nothing here rules it out.

What changes for the Mac, decision by decision:

- **Starting engines (0001).** Engines are started directly by path, never through a shell, as on Windows. A Mac app opened from the Dock or Finder doesn't get the person's shell `PATH`, so `claude`, `codex` and `node` are found by asking the person's login shell once at service start (`$SHELL -l -c 'command -v claude'`, and the same for the others), then the usual install places (`~/.local/bin`, `/opt/homebrew/bin`, `/usr/local/bin`, the npm global prefix). The path each engine was found at is recorded and shown on the Diagnostics page; a setting can override it.
- **Stopping engines (0001, 0004).** Nothing changes in how a stop works: the engine's own interrupt, then closing stdin and waiting. For the rare explicit kill (the person's action, or shutdown after the grace period), each engine is started as the leader of its own process group (`process_group(0)` in Rust's `std`), so the kill reaches the builds and tests it started: `SIGTERM` to the group, then `SIGKILL` after the grace period. Windows keeps its tree kill. Both are written to the event record with why, as now.
- **Claude Code's login (0002).** On macOS Claude Code keeps its login in the Keychain, not in `.credentials.json`, and setting `CLAUDE_CONFIG_DIR` "keys the macOS Keychain entry to that directory too, so a session with a different `CLAUDE_CONFIG_DIR` reads a different entry" ([authentication](https://code.claude.com/docs/en/authentication)). So one folder per account still means one login per account, and nothing in 0002 changes except how the service sees a renewal:
  - The login state probe is `claude auth status`, which 0002 already names for macOS.
  - The renewal gate's waiters, which on Windows re-read the file's expiry, read the Keychain item's modification date instead (its attributes only, never the secret). Gathr never asks the Keychain for the token, so the person is never shown a Keychain prompt on Gathr's behalf.
  - The login watcher watches the same item's modification date.
- **Codex's login (0002).** No change: the driver already starts app-server with `cli_auth_credentials_store="file"`, so the login stays in `auth.json` in the account's folder on both systems ([Codex auth](https://learn.chatgpt.com/docs/auth)).
- **The service and the window (0004, 0012).** `gathrd` and the lock work as on Windows. `service.json` is created with mode `0600`. Data lives in `~/Library/Application Support/Gathr` (Windows: `%APPDATA%\Gathr`). The window renders with the system's WKWebView instead of WebView2, and its origin (`tauri://localhost` on macOS, `http://tauri.localhost` on Windows) is the one the service accepts. The tray icon is a menu bar icon; "start with Windows" becomes "open at login" on both.
- **Sleep.** While any turn is running, the service asks the system not to idle-sleep (macOS: a power assertion, as `caffeinate -i` takes; Windows: `SetThreadExecutionState(ES_SYSTEM_REQUIRED)`), and lets go when the last turn ends. The screen may still turn off. This matters most for remote use (0016): a chat driven from the other computer stops if its host sleeps.
- **CI.** The CI workflow runs the same three checks on `macos-latest` next to `windows-latest`. Standard GitHub-hosted runners are free for public repositories ([Actions billing](https://docs.github.com/en/billing/concepts/product-billing/github-actions)).

## Why

- **One code base, one module for the differences.** Everything above that differs is small: where to find a binary, how to kill a group, where a login's expiry is read. Keeping it in one module, each side tested, keeps the rest of the service free of `cfg` branches.
- **The PATH problem is real on the Mac.** Apps launched from Finder get the system's minimal `PATH`, not the shell's, so `claude` installed in `~/.local/bin` or `codex` installed through Homebrew or npm isn't found by name. Asking the login shell once is what most Mac apps that start CLI tools do; recording the result makes a wrong find visible on the Diagnostics page instead of a silent "engine not found".
- **Process groups are the Unix tree.** macOS has no Job Objects. A process group catches everything the engine starts unless a grandchild starts its own session, which builds and test runners rarely do. Since kills are rare and always recorded, that gap is acceptable.
- **The Keychain keeps 0002's promise.** Gathr never reads a login's secret; reading only an item's attributes keeps that true on the Mac, and avoids a Keychain prompt that would name Gathr as asking for Claude's password.
- **Sleep would break a remote chat silently.** The previous app ran on one always-on PC; with two computers, the one running a chat is often not the one in front of the person.

## Turned down

- **Windows first, the Mac later.** The owner works from both now; every Windows-only choice made before the Mac runs would have to be found and undone later. CI on both from the start catches them as they're written.
- **Reading `PATH` from `launchctl getenv` or `/etc/paths` only.** It misses what the person's shell profile adds (`~/.local/bin`, nvm, Homebrew on Apple silicon), which is where these CLIs usually are.
- **Copying the Claude login out of the Keychain into a file** so both systems look the same. That copies a login, which 0002 forbids.
- **A launchd agent to keep `gathrd` running on the Mac.** As with a Windows service (0004), it adds an install step for nothing the window-started daemon doesn't already do. "Open at login" covers starting it.

## Not verified

No Mac was available for this decision; everything below is checked on a real Mac in the first slice, before any real sign-in.

- The Keychain item's name and how it's keyed. Third-party code reads it as `Claude Code-credentials`, with a suffix from a hash of the folder path when `CLAUDE_CONFIG_DIR` is set ([ai-usagebar](https://docs.rs/crate/ai-usagebar/1.20.2/source/src/anthropic/keychain.rs)); Anthropic documents only that the entry is keyed to the folder. Also whether reading the item's attributes (`security find-generic-password -s <name>` without `-g` or `-w`) shows any prompt.
- Whether Claude Code falls back to `.credentials.json` in the folder on this Mac (the docs say it does when the Keychain refuses the write), which would change where the gate reads.
- The window's origin on macOS (`tauri://localhost`).
- That the login shell lookup finds `claude` and `codex` as installed by their own installers, Homebrew and npm.
- That a running turn keeps a Mac awake with its lid open and on battery.
