# 0032. Gathr updates itself when the person says

Status: accepted (slice 2's F7, approved by the owner 2026-11-07; the window's updater, its key and its restart approved
2026-11-09). Amended 2026-11-12 (the owner): the service updates when the person says, by the Update button, no
longer between turns by itself; the window updates itself as before. Date: 2026-11-09. Follows
[0025](0025-engine-copies.md)'s way of keeping engine copies.

## Why

The service outlives the window and active engine turns. Separate checked copies let the window update independently
and let the service switch when the person says, with rollback for a failed trial. Switching by itself between turns
held the room's new turns while running ones finished, once for 30 minutes behind a background command that never
ended (G194); the person would rather choose when.

## The service

- **Where it runs from:** `<data>/app/<version>/gathrd`, with the built UI beside it in `ui/`, never the install folder,
  so an installer or the window's update never touches the program that runs. `<data>/app/current.json` names the copy
  to start, the one before it, whether the current one is still on trial, and the releases refused here.
- **The first copy:** the window (and `gathr open`) copies the installed `gathrd` and its UI in when the folder has no
  usable copy, when the installed one is newer (the window updated), or when it's the same version built again (a
  check on a fresh build, or macOS signing the bundle's again). A newer one starts on trial. The same version copied
  in again keeps its trial as it was, so a copy that died on trial still rolls back; clearing it was turned down, as a
  rebuild during a trial would hide that crash. A newer one keeps the database as it was, as an update does, so a crash
  after moving the schema on rolls both back. The UI is beside `gathrd`, or in a macOS bundle's `Resources`.
- **Starts race:** a start reads and changes the trial only while it holds the folder's lock. One that finds the lock
  held takes the service holding it, so a late hand-over never takes a copy another start just began for a dead one.
- **The feed:** each GitHub Release carries `gathrd.json`: `{"version": "x.y.z", "platforms": {"<target>": {"file",
  "sha256", "size"}}}`, where target is `win32-x64`, `darwin-arm64` (as Claude Code's feed names them), and the file
  a `.tar.gz` holding `gathrd(.exe)`, its matching `gathr(.exe)` diagnostics CLI and `ui/`.
  Installed-copy adoption keeps that CLI too; older installs/archives without it remain compatible.
  Read from `releases/latest/download`, following GitHub's redirects
  over HTTPS only. `GATHR_APP_RELEASES` names another feed at run time, and `GATHR_APP_FEED` when building builds
  one in (a test release, so an installed copy updates as a real one does); a debug build has none unless one is named.
- **When it checks:** at start, every 10 minutes, and on `POST /api/app/update`. The newest checked copy is shown
  ready (`state: "ready"`, `update`) until the person updates; a newer release replaces it.
- **Checks before a copy is used,** each failure one `gathr.app.update` event with its reference (`feed`,
  `manifest`, `download`, `checksum`, `unpack`, `probe`, `disk`, `platform`): the archive's size and SHA-256, the
  system's own `tar` unpacks it, and its `gathrd --version` names the release's version.
- **Update** (`POST /api/app/update {"install": true}`, the sidebar's button and Settings > General) updates the
  service. Before it answers, in this order:
  - **The gate:** every new turn waits (`held: "update"`), the person's own too, since the hand-over would cut it
    off; a message never goes into a turn under way, it becomes a turn of its own.
  - **The carry-ons:** one queued turn for each turn that runs and for each agent whose idle engine runs background
    commands, on the same task, answering the turn it carries on from ("Gathr's update to 0.1.N ended your turn. …"),
    all in one transaction (`gathr::carry`). When one can't be saved, nothing is stopped, the gate is lifted, and
    Update fails (500, with a reference).

  Then it stops what runs as Stop now does (F10): turns interrupted, background commands ended, no pause saved, so a
  pause or stop the person set before is untouched. Twenty seconds after Update was accepted at the latest
  (`gathr.app.hold`, `busy`, with what still ran and a reference), the service stops the ordinary way (engines
  closed gently, the database checkpointed and closed), keeps a copy of the database, names the new copy current on
  trial, and starts it. Each carry-on starts by itself on the copy that runs next, the new one or, after a rollback,
  the one before, with no Restart. While it waits, `GET /api/app` says what on (`running`). Nothing ready is refused
  (409, with a reference); `gathr.app.install` records each Update with how many carry-ons it saved.
- **A task under way carries on, and nobody is told it stopped.** A turn an update stopped ends `stopped` without
  settling its task, and a start leaves a `working` task that has a turn waiting on it: the task is never `stopped`,
  what its owner had reported stays, and its giver hears back once, when it ends. Stop now is unchanged: there the
  person stopped the work, and its carry-on waits for Restart.
- **The hand-over can't hang** (0.1.38 to 0.1.39 did, 2026-11-10): the stopped service waits at most 5 s for its
  runtime's blocking work, then leaves it to end with the process. On Windows, reading an engine's output is blocking
  work, and an engine killed at shutdown can leave children holding that pipe (a Codex engine did), so the wait never
  ended and the old service kept the folder locked. Each step is noted in `app/handover.json` (`ending background
  work`, `switching copies`, `starting the update`); the next start records it as `gathr.app.handover`, an error
  with a reference if it stopped before starting the update, with `left_running` when the 5 s ran out. A window that
  finds the folder held by a service stopped mid-update says so, with its process id, instead of "didn't answer".
  Killing an engine's whole process tree at shutdown was turned down here: it fixes only this one cause of blocking
  work, and 0031 keeps engines to the gentle close.
- **Trial:** a new copy is on trial from the switch until it finishes a turn or runs 10 minutes (`gathr.app.trial`,
  `by` `turn` or `time`). Until then the copy before it and the database as it was before the update are kept. Its
  first start records `gathr.app.switch`. The kept database is a SQLite snapshot (`VACUUM INTO`), not a file copy, so
  it has what a service that died without its checkpoint left in the WAL. A start marks its trial running before it
  writes `service.json`, so whoever waits for that file never finds a copy on trial that isn't marked yet.
- **Rollback:** a copy on trial that exits or doesn't write `service.json` within 60 s is ended and the copy before is
  started. One that started and then died is found by the next start (the window, `gathr open`): it never stopped
  cleanly, so that start goes back first. A clean stop on trial (Quit) isn't a death: the next start tries it again.
  The release is never tried again here. The database copy is put back only when the new copy moved the schema on,
  which the copy before can't open; otherwise what was written on the new copy stays. The service that comes back
  records it as a `gathr.app.update` `start` error with its reference.
- **What runs:** `GET /api/app` (`version`, `previous`, `state` `off`, `idle`, `checking`, `ready` or `updating`,
  `checked`, `failed` with its `ref`, `update` when a release is ready, `running`
  while Update waits), sent live as `{"type": "app"}`. `/health` names the version that runs and the build's own.

## The window

- **Tauri's updater** (`tauri-plugin-updater`, from Rust only, TLS through the system's own `native-tls`): a release
  build checks `latest.json` at start, every hour, and right after the service hands over (both come from one
  release); a debug build never checks. The update's minisign signature is checked against the public key in
  `tauri.conf.json` before anything installs. Events: `gathr.ui.update.check` (a refused update names its version and
  carries its reference) and `gathr.ui.update.install`.
- **When it restarts** (the owner's pick, 2026-11-09): at once if the window is hidden in the tray, else when it's next
  hidden; meanwhile the tray menu has Restart to update. A window hidden before starts hidden again. Its exit for an
  update never stops the service. The Update button doesn't reach the window (2026-11-12): everything the person sees
  and every behaviour comes from the service, and the window's own restart touches nothing that runs.
- **Installing:** Windows runs the new NSIS installer quietly (`/S /UPDATE /R`), which checks only for a running
  `gathr-app` and starts it again with its arguments. macOS replaces `Gathr.app` in place, and the window restarts.
- **After a hand-over:** when `service.json` names a new process that answers, the window signs in to it with a fresh
  one-time link (`gathr.ui.window.follow`). On the same port, which a service keeps when it's free, the page stays
  and only fetches the link for its new cookie, so drafts, staged answers and the open chat survive; its live socket
  reconnects and reloads what it shows (`in_place`). On another port the page's origin changes, so it loads anew.
- **A service that dies while the window is open:** after 3 s without an answer the window starts it again through
  `launch`, as a launch does, so a copy that died on trial rolls back at once (`gathr.ui.window.restart`). A start
  racing the hand-over's own finds the one that won. A failed restart goes to the crash log.
- **The key:** made with the Tauri CLI, password-protected, kept only as the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY`
  and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` and the owner's password manager. Lost, every installed Gathr is
  reinstalled by hand once.

## Releases

- **One per merge:** `.github/workflows/release.yml` runs once CI passes on a push to `main`, as `0.1.<run>` (written
  into `Cargo.toml` in CI only), and publishes `v0.1.<run>` with `gh`: the Windows NSIS installer (per user,
  unsigned), the Mac `.dmg` (Apple Silicon, ad-hoc signed), the window's update with `latest.json`, and each system's
  archive with `gathrd.json`. Every step is `scripts/release.mjs`, so a local build runs the same ones.
- **Before merge:** a PR that changes the release files builds them unpublished, both feeds built in as
  `127.0.0.1:47199`, so an update can be tried installed on either system.
- **Layout:** Windows installs `gathr-app.exe`, `gathrd.exe` and `ui/` side by side; macOS puts `gathrd` in
  `Contents/MacOS` beside the window and `ui/` in `Contents/Resources` (a folder in `MacOS` breaks the signature).

## Turned down

- **Tauri's updater for the service:** it updates the window, which has no reason to restart for a change in the
  service, and `gathrd` must be stopped first anyway.
- **Updating by itself between turns** (until 2026-11-12): the room's turns waited on whatever still ran, 30 minutes
  behind a background command that never ended (G194).
- **Letting the person's message start during an Update:** it was the rule while updates waited for quiet; an Update
  takes seconds and its hand-over would cut that turn off, so it waits and runs on the new copy.
- **One click that updates the window too** (planned 2026-11-12, then dropped the same day): the window said its
  update ready in a file in the data folder and Update left it a file to install by. Checked against the code three
  times, the plan kept failing on the hand-off between the two programs: an ask taken while the service was between
  copies, lost when the window restarted, never cancelled after a rollback. The window updates itself as it did.
- **Starting the new copy only from the window:** nobody may be at the keyboard, and the window may not be running.
  The stopping service starts it itself, after its lock is free, and the window finds whichever runs.
- **Signing the service's archive:** its SHA-256 comes over HTTPS from the same release as the archive; a signature
  key would be kept in the same GitHub account.
- **A full scratch start as the check before switching:** the switch is that check, with rollback, at the cost of one
  hold for a release that fails.
- **Rolling back on any later failed start:** only a copy on trial goes back; a later failure is reported, so data
  written since is never replaced by the older copy.
- **Always putting the database copy back on a rollback:** it would drop what the person wrote on the new copy (up to
  10 minutes) even when the copy before can read the database as it is.
- **Restarting the window at once while it's shown:** a chat in view reloads and a draft may be lost.
- **Paid OS code signing:** SmartScreen and Gatekeeper warn once, at the first install; updates come through the
  updater, so the warning doesn't return.
- **MSI on Windows:** NSIS installs per user with no administrator prompt.
- **`rustls` for the window's downloads:** 28 more crates, `ring` among them, against 15 for the system's own TLS.
- **A watchdog process:** every start already runs through one place (`launch`), the open window starts a dead
  service again through it, and with no window open nobody needs Gathr until it's next opened.

## Not verified

- A real release from GitHub (the first is F7's merge): the feed is proven through a stand-in on 127.0.0.1.
- Passing by time (10 minutes with no turn) isn't run in a test: the tests pass a trial by a turn.
