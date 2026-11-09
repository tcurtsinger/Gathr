# 0032. Gathr updates itself between turns

Status: accepted (slice 2's F7, approved by the owner 2026-11-07; the window's updater, its key and its restart approved
2026-11-09). Date: 2026-11-09. Follows [0025](0025-engine-copies.md)'s way of keeping engine copies.

## The service

- **Where it runs from:** `<data>/app/<version>/gathrd`, with the built UI beside it in `ui/`, never the install folder,
  so an installer or the window's update never touches the program that runs. `<data>/app/current.json` names the copy
  to start, the one before it, whether the current one is still on trial, and the releases refused here.
- **The first copy:** the window (and `gathr open`) copies the installed `gathrd` and its UI in when the folder has no
  usable copy, when the installed one is newer (the window updated), or when it's the same version built again (a
  check on a fresh build). A newer one starts on trial. The UI is beside `gathrd`, or in a macOS bundle's `Resources`.
- **The feed:** each GitHub Release carries `gathrd.json`: `{"version": "x.y.z", "platforms": {"<target>": {"file",
  "sha256", "size"}}}`, where target is `win32-x64`, `darwin-arm64` (as Claude Code's feed names them), and the file
  a `.tar.gz` holding `gathrd(.exe)` and `ui/`. Read from `releases/latest/download`, following GitHub's redirects
  over HTTPS only. `GATHR_APP_RELEASES` names another feed at run time, and `GATHR_APP_FEED` when building builds
  one in (a test release, so an installed copy updates as a real one does); a debug build has none unless one is named.
- **When:** at start, every 10 minutes, and on `POST /api/app/update`.
- **Checks before a copy is used,** each failure one `gathr.app.update` event with its reference (`feed`,
  `manifest`, `download`, `checksum`, `unpack`, `probe`, `disk`, `platform`): the archive's size and SHA-256, the
  system's own `tar` unpacks it, and its `gathrd --version` names the release's version.
- **Between turns:** the turns Gathr queues wait (`held: "update"`); the person's own message never does, it starts at
  once and the update waits for it. When no turn, background command or wait check runs, the service stops the
  ordinary way (engines closed gently, the database checkpointed and closed), keeps a copy of the database, names the
  new copy current on trial, and starts it. If something still runs after 30 minutes, the turns go on and the next
  look tries again (`gathr.app.hold`, `busy`).
- **Trial:** a new copy is on trial from the switch until it finishes a turn or runs 10 minutes (`gathr.app.trial`,
  `by` `turn` or `time`). Until then the copy before it and the database as it was before the update are kept. Its
  first start records `gathr.app.switch`.
- **Rollback:** a copy on trial that exits or doesn't write `service.json` within 60 s is ended and the copy before is
  started. One that started and then died is found by the next start (the window, `gathr open`): it never stopped
  cleanly, so that start goes back first. A clean stop on trial (Quit) isn't a death: the next start tries it again.
  The release is never tried again here. The database copy is put back only when the new copy moved the schema on,
  which the copy before can't open; otherwise what was written on the new copy stays. The service that comes back
  records it as a `gathr.app.update` `start` error with its reference.
- **What runs:** `GET /api/app` (`version`, `previous`, `state` `off`, `idle`, `checking` or `updating`, `checked`,
  `failed` with its `ref`, and `update` while it holds), sent live as `{"type": "app"}`. `/health` names the version
  that runs and the build's own.

## The window

Pia's part: Tauri's updater, signed with the updater's own key (GitHub secrets), installs at once when the window is
hidden in the tray, else when it's next hidden; only the window restarts. Its own section follows in the F7 PR.

## Turned down

- **Tauri's updater for the service:** it updates the window, which has no reason to restart for a change in the
  service, and `gathrd` must be stopped first anyway.
- **Holding the person's message during an update:** the owner's word reaches the agent at once, as with a pause (0010).
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
- **The window watching the new copy, or a watchdog process:** the next start already runs through one place
  (`launch`), and a death with nobody starting Gathr needs no rollback until someone does.

## Not verified

- A real release from GitHub (the first is F7's merge): the feed is proven through a stand-in on 127.0.0.1.
- Passing by time (10 minutes with no turn) isn't run in a test: the tests pass a trial by a turn.
