# Gathr

Gathr is an open-source desktop app for running a team of AI agents from different providers (Claude Code, Codex and more) on your own computer: chats, rooms where agents work together, tasks, plans and reviews, with logging and analytics built in, so you can see how the agents work and make it better.

Status: early. Product, design and architecture come first; see `docs/`.

## How we build

- **Plans before code.** Nothing is built until its plan is approved. Each decision gets a file in `docs/decisions/`: the choice, why, and what was turned down.
- **One feature, one PR,** its engine and its screens together, opened when the feature is done. From your own branch and your own git worktree (never the main checkout), base `main`, marked ready. Don't stack a PR on another PR's branch: merging that branch closes it.
- **Every change that can break has its check:** a test, or a command anyone can run that fails if the change breaks.
- **Tests where they earn it, not one per function.** Test behaviour that can go wrong: logic with branches, parsers and protocols (an engine driver against replayed raw frames), coordination state and its transitions, logins, security and data that could be lost, and every bug fixed (its test first). Don't test what can't really break: plain glue, getters, constants, what the type system already guarantees, or a layout a screenshot proves. One test per behaviour, through the real code where it's cheap, not mocks of our own code. A test that can't fail is deleted. Tests are written test first, at the seams the plan names: read [docs/testing.md](docs/testing.md) before the first test of a change.
- **Tests:** CI is the gate. The author runs the tests the change touches once before pushing; the reviewer reads and doesn't rerun suites.
- **Review once, when it's ready.** The code reviewer reviews every PR. The author answers each note; a second look only for a fix that changes behaviour. Minor notes are fixed in passing or dropped, never a round of their own. A docs-only PR merges without review rounds unless something in it is wrong.
- **The lead merges** once the notes are answered and CI is green, with a merge commit: `gh pr merge --merge --delete-branch --match-head-commit <full head commit SHA>`. Nobody turns on auto-merge. Check `main` is green before opening the next PR.
- **Clean up after yourself:** once your PR is merged or closed, remove your worktree and branch.
- **Keep this file current:** when a step here changes, or you learn one, change this file in the same PR.

## Build and check

- **Toolchain:** Rust through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the version, so `cargo` picks it up by itself. The UI in `ui/` uses Node 24 (`ui/.node-version`) and npm: run `npm ci` in `ui/` once, and again when `package-lock.json` changes.
- **The checks CI runs**, from the repo root (run them once before pushing):
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `bash scripts/check-file-size.sh` (source files up to 1,000 lines, lines up to 200 characters)
  - `node scripts/scrub-frames.check.mjs` (a recording's scrub keeps the person's own names out)
  - in `ui/`: `npx biome ci`, `npx tsc --noEmit`, `npx vitest run`, `npx vite build`. The tests run in a real browser: Edge on Windows (already installed), WebKit on macOS (`npx playwright install webkit` once). A failing test leaves its screenshot in `ui/.vitest/`.
- **CI** is `.github/workflows/ci.yml`, on Windows and macOS, for every PR and every push to `main`. `main` only takes PRs with CI green.
- **On a Mac without Rust:** each macOS run keeps its build for 7 days as the artifact `gathr-macos` (`gh run download <run> -n gathr-macos`). Start `gathrd` in the Mac's login session, not an SSH shell: over SSH the Keychain is locked, so every account reads as signed out. From SSH, run it as a launchd job in that session (`launchctl bootstrap gui/$(id -u) <plist>`, with `RunAtLoad`), and `launchctl bootout` it after. To run it installed, download the same run's `gathr-ui` artifact too, as a folder named `ui` beside the binaries, and open `gathr-app` (no Node needed). For the dev window: copy the same commit's `ui/` and `designs/tokens.css` (the UI imports it) beside the build, keeping their folders, run `npm ci` in it, then from that folder `./gathr dev --port <port> --data <folder> --window` in Terminal (it uses the `ui` folder it's started in, as the build came from another computer).
- **Commit messages on Windows:** write the message to a file and use `git commit -F <file>`; the same for scripts, write them to a file and run that. Inline multi-line strings get mangled by the shell.
- **PR screenshots:** attach them with `gh pr edit <n> --attach <file>`.
- **Flaky is broken.** A test that fails at random is fixed or deleted the day it's seen, never rerun until green. A failing test says which test it is, what it expected and what it got ([docs/testing.md](docs/testing.md)).

## Run, use and check it

- **The crate:** `crates/gathr` holds the shared library and two binaries: `gathrd`, the service, and `gathr`, the command line. Service code is in `src/bin/gathrd/`, command-line code in `src/bin/gathr/`, and what both use in `src/`.
- **Start it with test data:** `cargo dev --port <port> --data <folder> --seed` builds both, adds the made-up chats (500 messages, 50 tool calls in a turn, a 20,000-word answer) and starts the service. Ctrl+C stops it and everything it started. `cargo dev` and `gathr seed` refuse to run without `--data`, so made-up data never reaches the installed Gathr's own folder.
- **With the window:** add `--window` to `cargo dev`: it also starts the UI's dev server on the port plus 10 and the desktop window on it, and Ctrl+C stops them too. A browser can open the same address (`http://127.0.0.1:<port + 10>/`). The dev server adds the service's token to every `/api` request, so the page never holds it. Its window's title says `Gathr (dev <port>)`.
- **Without a login:** `examples/replay_engine.rs` stands in for Claude Code. Build it (`cargo build --example replay_engine`), start `cargo dev` with `GATHR_REPLAY` set to a recording in `crates/gathr/tests/frames` and `GATHR_CLAUDE_PATH` set to `target/debug/examples/replay_engine(.exe)`, add an account (`POST /api/accounts`), and put a made-up `.credentials.json` with an `accessToken` and `refreshToken` in its `<data>/accounts/<id>/claude`. Every send then plays that recording. A `{"dir": "sleep", "line": "<ms>"}` row in a recording pauses that long, for a stream at a real pace when timing.
- **Run it installed, without cargo or a dev server:** put `gathr-app`, `gathrd` and the built UI (`npx vite build` in `ui/`, then `ui/dist` copied as a folder named `ui`) in one folder, and open `gathr-app`. With no arguments it uses the system's app data folder (`--data <folder>` for a check), finds the service there or starts `gathrd` beside it, and signs in through a one-time link. `gathrd` serves the UI from the `ui` folder beside it (`--ui <folder>` for another). Closing the window hides it in the tray (the menu bar on macOS) and the service keeps running; opening Gathr again on the same data folder shows that window, starting the service again if it stopped. Quit in the tray stops both. Each data folder has its own window, so a check on a test folder never brings up the owner's. A service started this way never sees a `GATHR_*` variable, and one started with no `--port` never keeps a port from 47100 to 47114.
- **A browser on the service itself:** `gathr open --data <folder>` starts the service if it isn't running and prints a link that signs a browser in once (good for 60 s). On Windows, a service it starts inherits its output: send that output to a file, not a pipe you wait on.
- **Test data alone:** `gathr seed --data <folder>`, with the service stopped. Each run adds another set, so repeated runs make a large folder for timings.
- **Diagnose with `gathr diag`** (`target/debug/gathr` once built, or `cargo run -q --bin gathr --`), always with `--data <folder>`. JSON by default, `--text` to read:
  - `diag health`: the service now, the database, the last start-up check and the crash log. It answers even when the service or the database won't start.
  - `diag trace <ref>`: everything behind an error's reference, in order.
  - `diag failures --since 24h`: failures grouped by cause, with references.
  - `diag query "<sql>"`: read-only SQL, up to 1,000 rows. Use the `event_log` view: it shows `attrs` as JSON, a readable `time` and each row's `ref`.
  - `diag turn <ref> --raw`: the turns behind a reference, with every engine frame in and out of each, redacted.
  - `diag bundle`: health, a day's failures, recent events and engine frames (no bodies) in one file under `<data>/bundles`.
  - `diag health` also shows each account's sign-in state, and the messages held on the chats' account.
- **The API** is on `127.0.0.1:<port>`. Every `/api` request needs `Authorization: Bearer <token>`, with the token from `<data>/service.json`, or the browser's cookie `gathr-<port>`: `POST /api/once` gives a code, and opening `/?once=<code>` swaps it for the cookie (HTTP-only, `SameSite=Strict`, new at each start of the service). Other paths serve the built UI. Errors answer `{"error": "… failed", "why", "ref"}`, and each is recorded before the answer goes out, so its `ref` traces at once.
  - Chats: `GET`/`POST /api/chats`, `GET /api/chats/<id>` (messages and turns), `POST /api/chats/<id>/messages {"text", "agent"}` (as that agent; with none, on the oldest account). An agent runs one turn at a time across its chats, with its own engine session in each chat, so a message for a busy agent answers `queued`. `POST /api/chats/<id>/stop` (the chat's turns, whichever agent they're for), `POST /api/chats/<id>/approvals/<request_id> {"allow"}`.
  - Claude Code: `GET /api/engines/claude` (where it was found, Gathr's copy). `POST /api/engines/claude/update` checks the release feed now. Settings has no path: `GATHR_CLAUDE_PATH` in the service's environment stands in for Gathr's copy, in dev and tests only (the replay engine, or a missing file for a forced failure).
  - Accounts: `GET /api/accounts`, `POST /api/accounts {"engine": "claude", "name"}`, `GET /api/accounts/<id>`, `PATCH /api/accounts/<id> {"name"}`, `DELETE /api/accounts/<id>` (signs it out, closes its engines gently and deletes its folder). Names are unique, ignoring case. Each account's login is in `<data>/accounts/<id>/claude`. A new data folder has no accounts; a message sent to no agent runs on the oldest one.
  - Models: `GET /api/accounts/<id>/models` lists the account's models (`id`, `resolved`, `latest`, `name`, `description`, `efforts`) as its Claude Code reports them; `latest` marks a family alias such as `opus`, which moves to each new release. The replay engine answers with `tests/frames/initialize.json`.
  - Agents: `GET`/`POST /api/agents {"name", "account", "model", "effort"}`, `GET`/`PATCH`/`DELETE /api/agents/<id>`. A `null` model or effort is the engine's default; one the account's engine doesn't list is refused. A new account clears the model and effort the same change doesn't set. A change applies from the agent's next turn. An account isn't removed while agents use it (409). Each agent shows `hold` (`why`, `pid`, `since_us`) while its engine hasn't exited 30 s after a gentle close; the live socket sends `{"type": "hold"}` when it starts and ends.
  - Engine times: an engine closes the gentle way to switch chats, accounts, model or effort, and after 10 idle minutes; it's never killed except at shutdown. `GATHR_HOLD_AFTER_MS` and `GATHR_IDLE_CLOSE_MS` in the service's environment shorten the 30 s hold and the 10 minutes, in dev and tests only.
  - Sign in: `POST /api/accounts/<id>/sign-in` runs `claude auth login` in the account's folder and answers its page's `url`; `POST …/sign-in/code {"code"}` passes a pasted code; `DELETE …/sign-in` cancels. `POST /api/accounts/<id>/sign-out` runs `claude auth logout` there. The replay engine answers both: `made-up-code#…` signs in, any other code fails. To keep a real run from opening your browser, set `BROWSER` to a script that only logs its argument.
  - Live: a WebSocket at `/api/live?token=<token>` sends every delta, message, tool, turn, approval and read, as JSON.
  - Diagnostics: `GET /api/chats/<id>/turns`, each turn with what was asked and its events in order.
  - The window's own events: `POST /api/ui/events {"name": "gathr.ui.…", "duration_ms", "error", "attrs"}`, recorded like the service's (UI errors, open and switch timings, first text shown).
- **A signed-in test account:** a new data folder has none. Add one in Settings (or `POST /api/accounts`) and sign it in there; its login lives in `<folder>/accounts/<id>/claude`. Never point Claude Code at `~/.claude`.
- **Engine recordings:** the tests replay real Claude Code frames from `crates/gathr/tests/frames` through `examples/replay_engine.rs`, which `cargo test` builds. To record one, run the turn on a dev service, export its frames with the query in `scripts/scrub-frames.mjs`, run that script, and read the result for anything personal before committing it.
- **Schema changes** are a new numbered file in `crates/gathr/migrations/`, never an edit to a released one. Released means part of a build published to people; until the first, a migration on `main` may be edited, and the PR says so.

## Done means used, with proof

- Run it and use what changed the way a person will, with real-size data: hundreds of messages, a dozen agents, long conversations.
- Hand back proof in the PR:
  - **Screenshots:** before and after, at the sizes it's used at, in light and dark, next to the design when there is one.
  - **Timings** for anything that should feel fast: opening a screen, switching, a reply starting to stream.
  - **What you couldn't run,** said plainly.
- Passing tests and a clean build are needed, but they aren't done.

## How the code is written

- **YAGNI.** The smallest code that gives the expected result. Before writing, climb the ladder and stop at the first rung that holds: does it need to exist; is it already in this codebase; does the standard library do it; does the platform (CSS, SQLite, the OS) do it; does a dependency we already have do it; only then, the minimum new code. No abstraction with one user, no config for a value that never changes, nothing "for later".
- **Shared components, one of each.** One button, one scroll bar, one input, one menu, one dialog, one message, one empty state: each lives once in the shared UI code and the design system, and every screen uses it. A second version of something that exists is a bug in review, even if it looks the same. Same in the service: one way to do a thing (one error type per crate, one HTTP client, one way to spawn a process).
- **Small files, readable lines.** Keep a file to what one part of the code does: usually a few hundred lines. CI fails a source file over 1,000 lines or any line over 200 characters (`scripts/check-file-size.sh`), so nothing grows into a monolith and nothing packed or minified is committed. Split a file where it has a seam, by what each part does. Small functions with one job.
- **Comments only where they earn it:** why something is done, a non-obvious constraint, a lesson ("never kill here: the engine may be renewing its login"). Never what the next line plainly does, never a comment on every function or field. Names carry the meaning.
- **Best practices of each language:** `rustfmt` and `clippy -D warnings` for Rust; for the UI, the linter and formatter that land with it. Errors handled where something can recover, passed up everywhere else. No dead code, no commented-out code.
- **Reviewers check all of this**, not only behaviour: a duplicate component, a file near the limit, a comment that restates the code, an abstraction nobody needs.

## How Gathr looks and reads

- **The design system wins over taste.** Design questions get two or three options, or mockups, before anything is built.
- **Copy is terse.** Buttons and menu items are the verb: Undo, not "Undo run"; Organize, not "Organize with Gathr". No helper line under a title or a row: a setting is its name plus a few words at most, with any detail behind a small ?. A confirm is a short question, Cancel and the verb, with no line explaining the consequences. Errors read "… failed". AI tends to write labels that read like tooltips; check the words in review, not just the code.
- **Every setting has a control in Settings.** Never "ask an agent" as the only way to change something.
- **One app, not one per screen.** A pattern that appears on more than one screen (messages, links, notices, buttons, empty states) lives in shared code and the design system, so every screen gets it. Something stays screen-specific only when that screen's job differs, and the design doc says why.
- **Agents are generic.** Never design a feature or its wording around one particular agent.

## Safety, sized to the risk

- **Guard only against real risks.** No kill switches, disabled features or disclaimers for their own sake. When a safety question comes up, check how Claude's and Codex's own apps handle it, and match that.
- **Diagnostics from the first attempt.** Anything that runs where nobody watches it (background work, installs, long jobs) writes what it did and where it stopped from day one.
- **Every feature is diagnosable** ([0019](docs/decisions/0019-troubleshooting.md)). Its events show in Diagnostics and `gathr diag`, every error it can show carries a reference, and its PR proves one forced failure found from that reference. Troubleshoot with `gathr diag` before reading files by hand.
- **No paid services required.** Gathr runs the providers' own agents on the user's own logins. Anything that costs money is an opt-in, off by default, and never required.
- **Logins are single-use secrets.** Each account's login lives in its own Gathr folder, never the person's own `~/.claude` or `~/.codex`. Agents on one account share its folder only through the renewal gate (`docs/decisions/0002-login-folders.md`). Never copy a login file, or force-kill an engine while it might be renewing its login.

## Rules for a public repo

- **Everything here is public.** No secrets, keys, tokens, `.env` files or login files, not even in tests or history. Never print a secret: check config and login files with counts or masked values.
- **No personal data:** no emails, addresses, IPs, device serials, employers, or screenshots of real conversations. Test data is made up. Commits use the owner's GitHub noreply address (git config already has it); check it before your first commit.
- **Reuse what we have the rights to.** Code the owner wrote can be referenced, adapted and ported freely. Code under a licence that doesn't allow reuse (see "Learning from SerenityOS" below) is for learning only: understand it, then write your own.
- **Dependencies:** well maintained, with a licence compatible with this repo's. Never add one for what a few lines can do.

## Stack (proposed until `docs/decisions/` says otherwise)

- A Rust service that owns the data, the engines, scheduling and the API.
- SQLite for storage, with an event record of everything agents do.
- Each provider's own agent harness behind one interface.
- One MCP tool server inside the service.
- A Tauri desktop app over a web UI that a browser could also use.
- Windows and macOS, both checked in CI; what differs lives in one `platform` module of the service (`docs/decisions/0016-macos.md`).

## The owner's room

Gathr is built by a room of agents on the owner's PC: Orin (lead), Juno (product design), Cass (systems), Pia (frontend) and Vale (code review). This section is how that room works.

### Where you are

- You work inside SerenityOS, the owner's current app, which Gathr replaces one day. Never install, quit or restart SerenityOS, and never stop processes by name (`Stop-Process -Name`, `taskkill /IM`). Stop only what you started, by its process id.
- Your own logins only. Never touch the `~/.claude`, `~/.codex` or `~/.serenityos` logins, and never copy a `.credentials.json` or `auth.json`.
- Ask before installing or downloading anything (its name, source and size), and clean up after trials. No sound without warning. Never read or write the owner's real clipboard.
- Running many agents at once is normal here; don't flag it as load or a risk.

### Where things go

- The repo is public. Plans that mention the owner, reviews, evidence, screenshots of real use, and friction go in `X:\code\gathr-work` (`plans\`, `reviews\`, `FRICTION.md`), never in the repo. Screenshots the owner should see are sent to them.
- Each member works in its own worktree: `git fetch`, then `git worktree add X:\code\gathr-work\trees\<your name>-<topic> -b room/<topic> origin/main`.
- Dev ports and data folders: each member's dev service runs on its own port and data folder: Orin 47100, Cass 47101, Pia 47102, Juno 47103, Vale 47104, each with `X:\code\gathr-work\data\<name>`; with `--window` the UI takes the port plus 10 (47110 to 47114). Never use another member's, and never run without `--data`.
- The installed app too: open `gathr-app --data X:\code\gathr-work\data\<name>-<topic>`, never `gathr-app` alone or by double-click. With no `--data` it runs on the owner's real Gathr folder (`%APPDATA%\Gathr`, `~/Library/Application Support/Gathr`). A release build's service downloads its own Claude Code (about 345 MB) into the data folder on first start: reuse a folder that already has it, and delete the folder when you're done with it (a test folder also holds the window's webview data, in `webview`). Closing the window doesn't end it: use Quit in the tray, or stop the service over the API and then the window by its process id.
- After a merge, bring `X:\code\gathr` up to date: `git fetch`, then `git merge --ff-only origin/main`. Check `git status` first and never discard the owner's changes: if something would be overwritten, stop and ask.

### Learning from SerenityOS

- **Check SerenityOS first.** Before researching how to do something, look at how SerenityOS already does it: it has usually met the problem. Then verify it still holds (docs, a live run) before building on it.
- Read it freely: the code at `X:\code\serenityos\desktop` (its comments hold weeks of lessons: engine quirks, login safety, Windows processes), the running app, `X:\code\serenityos\PRODUCT.md`, `planning\LATER.md` and `FRICTION.md`, and `X:\code\gathr-work\from-serenityos.md` once Nova writes it.
- Most of it is the owner's own work: everything written after the Bloks import (commit `5f6947a3`, 2026-10-04). Reference it, adapt it and port it to Rust freely.
- The rest came with Bloks and is under its licence (FSL): read it to understand, then write your own. Copying or translating it would put Gathr under that licence too.
- How to tell: `git log --diff-filter=A --format=%h -- <file>` shows the commit that added a file (after `5f6947a3` means it's the owner's). Inside an older file, `git blame` shows each line: a commit after `5f6947a3` by the owner is theirs; the import commit or an earlier one is Bloks'.
- Usage data (what the owner and agents actually use): ask Nova with `ask Nova`. Nova can also explain how something there works.
- The Portal isn't part of Gathr.

### Working with the owner

- Decisions for the owner are tasks for the owner, with options, your pick first and why. Never inside a plan.
- Design questions get options or mockups first. While the owner is still deciding, end with the question, not an action, and never ask "should I start building?": they say when.
- A check the owner does by hand says how to make every test file, what to type and where to click.
- **No assumptions.** A fact you state or build on (how a tool, API, engine or library behaves, what a file says, what a number is) is checked first: in the code, the current docs, the source, or by running it. What you remember from training can be out of date: Claude Code reading `AGENTS.md` changed a month before we assumed it didn't. What can't be checked is said plainly as not verified, never written as fact.
- Ask before you guess: when you're about to build on something you haven't checked and can't check in a few minutes, or the same thing failed twice, ask the teammate who knows that area.
- Reviews: every PR goes to Vale. The lead merges as above. The owner doesn't need to install anything until there's an app to run.

### Friction

At the end of your answer, as "Friction: <what, where>": anything in SerenityOS that got in your way (a lost message, a wrong card, a missing command). The lead logs it in `X:\code\gathr-work\FRICTION.md` and passes workshop bugs to Nova. Each one is also a requirement: write down what Gathr must do instead.

Friction is how Gathr learns what not to repeat, so it's read all the time, not only logged:
- Two sources: ours, and SerenityOS's own (`X:\code\serenityos\planning\FRICTION.md`), which the agents maintaining SerenityOS keep adding to.
- `X:\code\gathr-work\plans\friction-ledger.md` holds every "Gathr must" from both, each with where it's covered (a decision, AGENTS.md, a feature on the backlog) or "open". The lead brings it up to date from both files at least daily.
- Every feature plan starts by reading the ledger for entries that touch it; the plan names the ones it covers, and its PR proves them.

## Design preview checks

The selected desktop prototypes are listed in `designs/README.md`. Run `node designs/check.cjs` from the repo root. Open the four latest pages at 1440 and 1024 px in dark and light (`#light`), exercise New, Search, composer, panel tabs and agent roles. `DESIGN.md`, `designs/tokens.css`, `designs/components.css` and `designs/system.html` define shared patterns. Keep them aligned when a pattern changes. No engine, real terminal or Settings service is connected. Browser interaction and keyboard checks complement the static command.
