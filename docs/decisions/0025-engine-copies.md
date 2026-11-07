# 0025: Gathr keeps its own engine copies, and updates them

Status: accepted (owner, 2026-11-07). Claude Code is built; Codex follows when its driver lands.

## The choice

Gathr downloads each engine into its data folder, checks it, and keeps it current by itself.
The person never installs Claude Code or Codex, and their own installs are never touched.

- **Where:** `<data>/engines/claude/<version>/claude(.exe)`. `current.json` names the copy in use
  and the one before it.
- **Which engine a chat runs:** the path set in Settings > Engines (an override), else Gathr's
  copy, else `claude` on the PATH.
- **When:** at service start, then every 24 hours, and on `POST /api/engines/claude/update`.
- **From where:** Claude Code's own release feed, the one its installer reads:
  `https://downloads.claude.ai/claude-code-releases/latest` (a bare version) and
  `<version>/manifest.json` (each platform's file name, SHA-256 and size).
- **Checks before a copy is used,** each failure recorded with its reference, and the copy in use
  kept:
  1. The manifest names this platform's file, a 64-hex SHA-256 and a size up to 512 MB.
  2. The download is that size and that SHA-256.
  3. Started the way chats start it, in an empty scratch folder with no login: `--version` names
     the feed's version and passes Gathr's minimum (0024), and an `initialize` request with
     Gathr's own switches answers `success`. A release that drops a switch Gathr passes fails here,
     not in a chat.
- **Switching:** writing `current.json` is the switch (a rename, so a start reads the old or the
  new, never half of one). A running engine keeps its own file: a chat moves to the new copy at its
  next turn, after its engine closes the gentle way between turns, never during one. A login
  renewal happens inside a turn, behind the gate (0002), so it's never cut short by a switch.
- **Known good:** the copy before is kept beside the current one. If the current one's file goes
  missing, chats use it. Older copies are deleted; one still running on Windows can't be, and is
  tried again at the next check.
- **Claude Code's own updater is off** in every engine Gathr starts (`DISABLE_AUTOUPDATER=1`), so
  Gathr's copy only changes through these checks. `claude update` still works by hand.
- **Diagnostics:** each check is one `gathr.engine.update` event: what was found, what was
  installed, how long it took, or what failed (`feed`, `manifest`, `download`, `checksum`,
  `probe`, `disk`) with its reference.
- **Downloads use the system's own curl** (`%SystemRoot%\System32\curl.exe`, `/usr/bin/curl`),
  with time limits on every request and no redirects. SHA-256 is the `sha2` crate.
- **Dev and tests download nothing** unless `GATHR_CLAUDE_RELEASES` names a feed: debug builds have
  no default feed, so a dev service never pulls 345 MB unasked. Release builds use Claude's.

## Codex, when its driver lands

The same layout under `<data>/engines/codex`, the same checks and switching.
- **Feed:** npm's per-platform tag, `registry.npmjs.org/@openai%2fcodex/<platform>` (for example
  `win32-x64`), whose tarball carries a SHA-512 `integrity`. The binary is
  `package/vendor/<rust target>/bin/codex(.exe)`, unpacked with the system's `tar`.
- **Probe:** `--version`, then the stdio `app-server`'s `initialize`.
- **Its own updater:** the stdio `app-server` never checks for updates (its source has no update
  code outside the interactive screen and the experimental daemon), so nothing needs turning off.

## Models and effort

They come from the engine, never a list in Gathr: Claude Code's `initialize` answers `models`, each
with `supportedEffortLevels`, and family names (`opus`, `sonnet`) that always mean the newest, which
gives "Opus latest". Codex's `model/list` answers its models with `supportedReasoningEfforts`. The
list depends on the account (13 models signed in, 5 signed out on 2.1.318), so it's read per
account: `GET /api/accounts/<account>/models` starts the account's engine through its renewal gate,
asks `initialize`, and keeps the list until the engine copy or the sign-in state changes. Measured on
Windows with 2.1.318 signed in: 0.9 s the first time, 2 to 6 ms after. Turned down: asking on every
open (0.9 s each time a picker opens) and reading it from a chat's own start (no list until a chat
has run, and a chat's engine never sends `initialize` today).

## Turned down

- **Using the person's own Claude Code (as before).** It can be missing, too old, or updated by its
  own updater in the middle of the person's day. Kept as the override.
- **Copies built into Gathr's installer.** Each Gathr release would carry 2 × 345 MB per platform
  and go stale between releases.
- **Letting Claude Code's own updater manage Gathr's copy.** It writes under the person's home
  (`~/.local`), the install it would update is theirs, and whether it runs in `-p` mode isn't
  documented.
- **Built-in pinned checksums as the known-good copy.** Each pin is a version Gathr must ship and
  bump; the copy that last passed the checks here is a better known-good.
- **Checking the manifest's signature (`manifest.json.sig`).** It needs a GPG verifier for one
  file. The checksum comes over HTTPS from the same origin the official installer trusts, as the
  installer itself does.
- **An HTTP client crate (with TLS) in the service.** curl ships with Windows 10 and later and
  with macOS, uses the system's certificates, and keeps a TLS stack out of the service.
- **Re-hashing the copy before every start.** Hashing 345 MB takes about a second per engine
  start; the copy is hashed once, before it's used, inside the person's own data folder.
- **Restarting engines the moment an update lands.** A chat mid-turn would lose its turn; moving
  at the next turn costs at most one turn on the older copy.

## Not verified

- A real update from one Claude Code release to the next on macOS (the Windows one was run).
- Whether separate Claude Code config folders on macOS share one Keychain entry.
- Codex: none of its part is built or run.
