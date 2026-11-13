# Gathr

Gathr is an open-source desktop app for working with your own AI agents, from different providers (Claude Code, Codex and more), on your own computer: an assistant for everyday asks, your own specialists, and a team with a lead when the work is a project, with tasks, plans and reviews, and logging built in, so you can see how the agents work and make it better.

Status: early. What Gathr is for, the loop work follows and its promises are in [docs/charter.md](docs/charter.md): every plan names the promise it serves. How it's built is in `docs/`.

**Rules with a tag are stopgaps.** A rule here that ends with a tag such as `(P2)` is a process rule: people and agents follow it by hand because Gathr doesn't keep that [promise](docs/charter.md#the-promises) by itself yet. The PR that makes Gathr keep the promise deletes the rule. A new process rule needs a tag, or it isn't added. Rules with no tag are facts about this repo and where it's built, and they stay. `node scripts/docs.check.mjs` counts the tags.

## How we build

- **Plans before code.** Nothing is built until its plan is approved. Each decision gets a file in `docs/decisions/`: the choice, why, and what was turned down, under a `Status:` line and a `Built:` line ([the list](docs/decisions/README.md) says what each means). (P2)
- **The lead writes the plans; the reviewer checks them** ([0010](docs/decisions/0010-coordination-as-state.md#plans-and-plan-checks), by hand until Gathr does it). Members research or draw a part when the lead asks, as input. Before a plan reaches the owner, the reviewer reads it against the code: a wrong fact, a wrong file, a missed caller, a missing step, a part given to the wrong member, a check that proves nothing. The owner sees the plan with the check's outcome. (P2)
- **One feature, one PR,** its engine and its screens together, opened when the feature is done. From your own branch and your own git worktree (never the main checkout), base `main`, marked ready. Don't stack a PR on another PR's branch: merging that branch closes it.
- **Every change that can break has its check:** a test, or a command anyone can run that fails if the change breaks.
- **Tests where they earn it, not one per function.** Test behaviour that can go wrong: logic with branches, parsers and protocols (an engine driver against replayed raw frames), coordination state and its transitions, logins, security and data that could be lost, and every bug fixed (its test first). Don't test what can't really break: plain glue, getters, constants, what the type system already guarantees, or a layout a screenshot proves. One test per behaviour, through the real code where it's cheap, not mocks of our own code. A test that can't fail is deleted. Tests are written test first, at the seams the plan names: read [docs/testing.md](docs/testing.md) before the first test of a change.
- **Tests:** review and the author's local tests are the gate ([0045](docs/decisions/0045-review-gates-gathrs-own-repo.md)); CI runs as information. Before pushing, the author runs the quick checks (format, lint, file size, and the UI's lint and types for a UI change) and the test files the change touches; the whole suite only for a change to shared code (a migration, task or turn states, the engine drivers, `gathr::run`). The reviewer reads and doesn't rerun suites.
- **Review once, when it's ready.** The code reviewer reviews every PR. The author answers each note; a second look only for a fix that changes behaviour. Minor notes are fixed in passing or dropped, never a round of their own. A docs-only PR merges without review rounds unless something in it is wrong. (P10)
- **The reviewer merges** once their review (or second look) is clean, the notes are answered and the author ran the tests the change touches, on the head they reviewed, with a merge commit: in Gathr with the `merge` tool ([0046](docs/decisions/0046-github-tools.md)), by hand with `gh pr merge --merge --delete-branch --match-head-commit <full head commit SHA>`. Nobody turns on auto-merge. A PR must be up to date with `main` before it merges, but authors don't update their branch to keep up with `main`: a PR is opened and reviewed on the head it has. Once its review is clean and the notes are answered, it joins the landing order, in the order reviews came clean unless the lead says otherwise; the reviewer updates only the PR first in that order (`update_pr` in Gathr, `gh pr update-branch <n>` by hand; a head that only brings in `main` needs no new review) and merges it, then the next. A real conflict goes back to its author, and the next PR takes its place. A docs-only PR is merged by its author. After a merge, the merger brings the owner's main checkout up to date (below). When a PR's CI is already running, the merger waits for it to finish and reads any failure first. A red CI job is read, and a real failure is fixed in a follow-up, but it doesn't hold a merge. (P10)
- **Clean up after yourself:** once your PR is merged or closed, remove your worktree and branch. (P7)
- **Keep this file current:** when a step here changes, or you learn one, change this file in the same PR. The docs this file links are part of it: a step that changes is fixed in the doc that holds it.

## The owner's room

Gathr is built by a room of agents on the owner's PC: Orin (lead), Juno (product design), Cass (systems), Pia (frontend) and Vale (code review). This section is how that room works.

### Where you are

- You work inside Gathr itself, installed on the owner's PC. SerenityOS, the owner's earlier app, runs beside it until Gathr replaces it. Never install, quit or restart SerenityOS, and never stop processes by name (`Stop-Process -Name`, `taskkill /IM`). Stop only what you started, by its process id.
- Your own logins only. Never touch the `~/.claude`, `~/.codex` or `~/.serenityos` logins, and never copy a `.credentials.json` or `auth.json`.
- Ask before installing or downloading anything (its name, source and size), and clean up after trials. No sound without warning. Never read or write the owner's real clipboard.
- Running many agents at once is normal here; don't flag it as load or a risk.

### Where things go

- **Everything the room makes lives in the Agent folder, `X:\Gathr`, in Gathr's own layout ([0034](docs/decisions/0034-agent-folder.md)).** The project's `X:\Gathr\Gathr\Shared` holds what the team keeps: `plans\` (the plan, and `STATE.md`: where the work stands, read it first), `reviews\`, `evidence\`, `designs\`, `FRICTION.md`, and `archive\` (older documents, each listed in its `INDEX.md`). Your own `X:\Gathr\Gathr\<your name>` holds your worktrees, builds, test data, probes and scratch. Nothing of yours goes in the owner's main checkout `X:\code\gathr`, not even in an ignored folder. The repo is public, so plans that mention the owner, reviews, evidence and screenshots of real use stay in Shared. Screenshots the owner should see are sent to them. (P7)
- **Clean up your own folder** when a task ends: delete builds, test data and probes nobody needs, and stop every process you started by its process id. Keep only scripts you reuse. (P7)
- Each member works in its own worktree: `git fetch`, then `git worktree add X:\Gathr\Gathr\<your name>\<topic> -b room/<topic> origin/main`. (P7)
- Dev ports and data folders: each member's dev service runs on its own port and data folder: Orin 47100, Cass 47101, Pia 47102, Juno 47103, Vale 47104, each with `X:\Gathr\Gathr\<name>\data` (and its agent folder `X:\Gathr\Gathr\<name>\data-agents`); with `--window` the UI takes the port plus 10 (47110 to 47114). Never use another member's, and never run without `--data`. (P7)
- The installed app too: open `gathr-app --data X:\Gathr\Gathr\<name>\data-<topic>`, never `gathr-app` alone or by double-click. With no `--data` it runs on the owner's real Gathr folder (`%APPDATA%\Gathr`, `~/Library/Application Support/Gathr`). A release build's service downloads its own Claude Code (about 345 MB) into the data folder on first start: reuse a folder that already has it, and delete the folder when you're done with it (a test folder also holds the window's webview data, in `webview`). Closing the window doesn't end it: use Quit in the tray, or stop the service over the API and then the window by its process id. (P7)
- A hand-made `Gathr.app` for a Mac check: its `CFBundleExecutable` is `gathr-app` itself, never a script that `exec`s it (macOS then can't match the process to the app, and the menu bar icon never appears). It takes the test folder from `GATHR_DATA` in `Info.plist`'s `LSEnvironment` (debug builds only, as CI's are; a release build ignores it); `--data` wins over it. Run `lsregister -f` on the bundle after changing its `Info.plist`.
- After a merge, bring `X:\code\gathr` up to date: `git fetch`, then `git merge --ff-only origin/main`. Check `git status` first and never discard the owner's changes: if something would be overwritten, stop and ask. (P7)

### Learning from SerenityOS

- **Check SerenityOS first.** Before researching how to do something, look at how SerenityOS already does it: it has usually met the problem. Then verify it still holds (docs, a live run) before building on it.
- Read it freely: the code at `X:\code\serenityos\desktop` (its comments hold weeks of lessons: engine quirks, login safety, Windows processes), the running app, `X:\code\serenityos\PRODUCT.md`, `planning\LATER.md` and `FRICTION.md`, and `X:\Gathr\Gathr\Shared\from-serenityos.md`.
- Most of it is the owner's own work: everything written after the Bloks import (commit `5f6947a3`, 2026-10-04). Reference it, adapt it and port it to Rust freely.
- The rest came with Bloks and is under its licence (FSL): read it to understand, then write your own. Copying or translating it would put Gathr under that licence too.
- How to tell: `git log --diff-filter=A --format=%h -- <file>` shows the commit that added a file (after `5f6947a3` means it's the owner's). Inside an older file, `git blame` shows each line: a commit after `5f6947a3` by the owner is theirs; the import commit or an earlier one is Bloks'.
- Usage data (what the owner and agents used there, 30 days to 2026-11-06): `X:\Gathr\Gathr\Shared\archive\2026-11\plans\usage-30d.md`. Newer numbers come from the owner.
- The Portal isn't part of Gathr.

### Working with the owner

- Decisions for the owner are tasks for the owner, with options, your pick first and why. Never inside a plan. (P4)
- The lead is the owner's one point of contact. A member's request for the owner goes to the lead, who checks it against the whole project, answers it when it can, and otherwise asks in its own words, with its pick, merging requests that belong together; never forwarded as written. The lead passes the answer back. (P4)
- Design questions get options or mockups first. While the owner is still deciding, end with the question, not an action, and never ask "should I start building?": they say when. (P2)
- A check the owner does by hand says how to make every test file, what to type and where to click. (P3)
- **No assumptions.** A fact you state or build on (how a tool, API, engine or library behaves, what a file says, what a number is) is checked first: in the code, the current docs, the source, or by running it. What you remember from training can be out of date: Claude Code reading `AGENTS.md` changed a month before we assumed it didn't. What can't be checked is said plainly as not verified, never written as fact. (P2)
- Ask before you guess: when you're about to build on something you haven't checked and can't check in a few minutes, or the same thing failed twice, ask the teammate who knows that area. (P2)
- Reviews: every PR goes to Vale, who merges it as above. The owner doesn't need to install anything until there's an app to run. (P10)

### Friction

At the end of your answer, as "Friction: <what, where>": anything in Gathr or SerenityOS that got in your way (a lost message, a wrong card, a missing command). The lead logs it in `X:\Gathr\Gathr\Shared\FRICTION.md`. It's logged, not sent to SerenityOS to be fixed, unless the owner asks. Each one is also a requirement: write down what Gathr must do instead, and which [promise](docs/charter.md#the-promises) it breaks. (P11)

Friction is how Gathr learns what not to repeat, so it's read all the time, not only logged:
- Two sources: ours, and SerenityOS's own (`X:\code\serenityos\planning\FRICTION.md`), which the agents maintaining SerenityOS keep adding to.
- `X:\Gathr\Gathr\Shared\plans\friction-ledger.md` holds every "Gathr must" from both. A row that belongs to a promise sits under it and under its part of [the truth map](docs/truth-map.md), with its state (Open, Written, Partly, Built or Kept) and where that state comes from; rows about how Gathr is built, and rows with nothing to build, have a section each. The lead brings it up to date from both files at least daily, and runs the check beside it (`friction-ledger-check.cjs`).
- Every feature plan starts by reading the ledger for entries that touch it; the plan names the ones it covers, and its PR proves them. (P11)

## Build and check

- **Toolchain:** Rust through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the version, so `cargo` picks it up by itself. The UI in `ui/` uses Node 24 (`ui/.node-version`) and npm: run `npm ci` in `ui/` once, and again when `package-lock.json` changes.
- **The checks CI runs**, from the repo root (before pushing, the quick ones and the tests your change touches, as in How we build):
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo nextest run --workspace --locked` (once: `cargo install cargo-nextest --locked`). It runs every test binary at once: the whole suite in about 50 s on the owner's PC, against 5 minutes for `cargo test --workspace --locked`, which runs the same tests one binary after another and still works. One test file: `cargo nextest run -p gathr --test <name>`. A failing test that started a service prints the service's last 50 events, and with `GATHR_TEST_KEEP=<folder>` set keeps a copy of its data folder there; in CI, a failed Rust job keeps them as the artifact `rust-failures-<Windows|macOS>`. A request that times out names itself, how long it waited and how many bytes came back.
  - `bash scripts/check-file-size.sh` (source files up to 1,000 lines, lines up to 200 characters, no NUL byte in any tracked text file, and `AGENTS.md` itself up to 32,768 bytes: reference goes in the docs it links). Stage new source files first: this check reads `git ls-files`.
  - `node scripts/scrub-frames.check.mjs` (a recording's scrub keeps the person's own names out)
  - `node scripts/docs.check.mjs` (every decision has a `Status:` and a `Built:` line and is in [the decisions list](docs/decisions/README.md), every link in a Markdown file leads somewhere, every doc is linked from this file or the README, every rule tag here names a promise, every part of [the truth map](docs/truth-map.md) that says built names a file and a test that exist, and every part has a [scenario](docs/scenarios.md); `--write` rewrites the list)
  - `node scripts/registry.check.mjs` (the files in `ui/src/registry` are as published, [0030](docs/decisions/0030-ui-foundation.md))
  - in `ui/`: `npx biome ci`, `npx tsc --noEmit`, `npx vitest run`, `npx vite build`. The tests run in a real browser: Edge on Windows (already installed), WebKit on macOS (`npx playwright install webkit` once). A failing test leaves its screenshot in `ui/.vitest/`; in CI, a failed UI job keeps that folder as the artifact `ui-failures-<Windows|macOS>`.
- **UI tests get the app's styles** from `ui/src/test.setup.ts`, which imports `ui/src/theme.css` as the app entry does. Without it, unstyled tooltips covered menu items in WebKit (archive.test.tsx failed at random on macOS); a pointer failure must be checked against the saved screenshot and click log. Its font arrives by swap, so the same file loads it before each file's tests: a swap mid-test moved text a test measured.
- **CI** is `.github/workflows/ci.yml`, on Windows and macOS, for every PR and every push to `main`. It's information, not a gate: `main` requires no check (0045). A PR that changes only Markdown files (`*.md`) runs the file size and docs checks alone, under the same four job names, so it reports in about a minute; anything else in it, and every push to `main`, runs everything.
- **Releases** (F7): `.github/workflows/release.yml` publishes a GitHub Release `v0.1.<run>` once CI passes on a push to `main`: the Windows installer (NSIS, per user, unsigned, so SmartScreen warns on a first install), the Mac `.dmg` (Apple Silicon, ad-hoc signed: a first open needs System Settings > Privacy & Security > Open Anyway), the window's update with `latest.json`, and the service's `gathrd-<version>-<target>.tar.gz` with `gathrd.json` (0031). Updates are signed with the updater's key, kept only as the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` and `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (its public key is in `crates/gathr-app/tauri.conf.json`). The version is written into `Cargo.toml` in CI only. A PR that changes `release.yml`, `scripts/release.mjs` or `crates/gathr-app` builds the same files unpublished, as the artifact `release-0.1.<run>`, its window pointed at a feed on `127.0.0.1:47199`. Each step is `node scripts/release.mjs` (`stamp`, `sidecar`, `collect`, `feeds`), so a local build runs the same ones; the Tauri CLI is `ui/node_modules/.bin/tauri build --config crates/gathr-app/tauri.release.conf.json`, from the repo root.
- **On a Mac without Rust:** each macOS run keeps its build for 7 days as the artifact `gathr-macos` (`gh run download <run> -n gathr-macos`). Start `gathrd` in the Mac's login session, not an SSH shell: over SSH the Keychain is locked, so every account reads as signed out. From SSH, run it as a launchd job in that session (`launchctl bootstrap gui/$(id -u) <plist>`, with `RunAtLoad`), and `launchctl bootout` it after. To run it installed, download the same run's `gathr-ui` artifact too, as a folder named `ui` beside the binaries, and open `gathr-app` (no Node needed). For the dev window: copy the same commit's `ui/` and `designs/tokens.css` (the UI imports it) beside the build, keeping their folders, run `npm ci` in it, then from that folder `./gathr dev --port <port> --data <folder> --window` in Terminal (it uses the `ui` folder it's started in, as the build came from another computer).
- **Commit messages on Windows:** write the message to a file and use `git commit -F <file>`; the same for scripts, write them to a file and run that. Inline multi-line strings get mangled by the shell.
- **Long commands run in the background.** Start anything that may take over 2 minutes (the whole suite, a release build) in the background where the engine can: it tells you when the command ends. Never wait on one with a sleep loop, and never hold a turn on one in the foreground: Claude Code holds the turn for 10 minutes before it moves the command to the background itself. Send a suite's output to a file once and read the file; don't run the suite again to see its output another way. This matters most for a chat's Supervisor, as every hear-back in the chat waits behind its turn. To wait on CI or a PR, use the `wait` tool, not a loop.
- **PR screenshots:** attach them with `gh pr edit <n> --attach <file>`.
- **Release CLI check:** `node scripts/release.check.mjs` checks staging and the service archive.
  Release builds also run `scripts/release-smoke.mjs` on disposable CI runners: it checks both
  binary versions and runs the packaged CLI's diagnostics against seeded temporary data. Never run
  its NSIS installation on the person's PC; use the PR release artifact's smoke report instead.
- **Git in tests:** run it through `common::git_command()` (or `common::git`), which drops git's repository variables (`gathr::platform::env::LOCAL_GIT_VARS`), so a test run from a git hook or `git bisect run` never touches the real repository (S37). The service drops them from every child's environment the same way (`env::fresh`).
- **Flaky is broken.** A test that fails at random is fixed or deleted the day it's seen, never rerun until green. A failing test says which test it is, what it expected and what it got ([docs/testing.md](docs/testing.md)).
- **Schema changes** are a new numbered file in `crates/gathr/migrations/`, never an edit to a released one. Migrations run with foreign keys off, so one may rebuild a table others point at, and each is checked for broken references before it commits. Released means part of a build published to people; until the first, a migration on `main` may be edited, and the PR says so. The search index (0015) points at `messages` by rowid: a migration that rebuilds `messages` keeps every rowid (`INSERT INTO messages_new (rowid, …) SELECT rowid, …`) and refills `message_search`, and never runs FTS5's `'rebuild'` on it (that would index tool rows too); `tests/search.rs` fails if they drift apart.

## Run, use and check it

Each part has its own doc. Read the one your task touches before you start, and when a step in it changes, change that doc in the same PR.

- [docs/truth-map.md](docs/truth-map.md): Each promise, part by part: Written, Partly, Built or Kept, with the code and the test that proves it. Before planning work on a promise. A PR that builds or changes a part changes its row.
- [docs/scenarios.md](docs/scenarios.md): Each promise as scenarios in plain steps (given, when, then), and the two runs that test the whole product. Before writing a promise's tests: its scenarios become the tests, and fail first.
- [docs/architecture.md](docs/architecture.md): What is built, part by part, with the decision behind each. Before changing how a part works.
- [docs/running.md](docs/running.md): Run Gathr. Before starting a dev service, seeding test data, running without a login, running it installed, or recording an engine.
- [docs/window.md](docs/window.md): Use the window. Before using or checking a screen in the window, or the window's own updates.
- [docs/api.md](docs/api.md): The API. Before calling the service: its address and sign-in, chats, team chats, projects, the live socket, search, context, open at login and the window's events.
- [docs/api-engines.md](docs/api-engines.md): The API: engines, accounts and agents. Before touching accounts, sign-in, models, agents, an engine's copy or Gathr's own updates.
- [docs/api-approvals.md](docs/api-approvals.md): The API: approvals and Gathr's tools. Before touching approvals, pushes, Gathr's tools or agents' messages to each other.
- [docs/api-tasks.md](docs/api-tasks.md): The API: tasks, pauses, archive and the agent folder. Before touching tasks, waits, the person's tasks, pause and stop, archive, stale tasks, the agent folder or PR cleanup.
- [docs/api-checks.md](docs/api-checks.md): The API: checks runs and merging. Before touching a project's checks runs or the merge tool.
- **Diagnose with `gathr diag`** (`target/debug/gathr` once built, or `cargo run -q --bin gathr --`), always with `--data <folder>`. JSON by default, `--text` to read:
  - `diag health`: the service now, the database, the last start-up check and the crash log. It answers even when the service or the database won't start.
  - `diag trace <ref>`: everything behind an error's reference, in order.
  - `diag failures --since 24h`: failures grouped by cause, with references.
  - `diag query "<sql>"`: read-only SQL, up to 1,000 rows. Use the `event_log` view: it shows `attrs` as JSON, a readable `time` and each row's `ref`.
  - `diag turn <ref> --raw`: the turns behind a reference, with every engine frame in and out of each, redacted.
  - `diag bundle`: health, a day's failures, recent events and engine frames (no bodies) in one file under `<data>/bundles`.
  - `diag health` also shows each account's sign-in state, the messages held on the chats' account, and each chat that works or needs the person.

## Done means used, with proof

- Run it and use what changed the way a person will, with real-size data: hundreds of messages, a dozen agents, long conversations.
- Hand back proof in the PR:
  - **Screenshots:** before and after, at the sizes it's used at, in light and dark, next to the design when there is one.
  - **Timings** for anything that should feel fast: opening a screen, switching, a reply starting to stream.
  - **What you couldn't run,** said plainly.
- Passing tests and a clean build are needed, but they aren't done. (P5)

## How the code is written

- **YAGNI.** The smallest code that gives the expected result. Before writing, climb the ladder and stop at the first rung that holds: does it need to exist; is it already in this codebase; does the standard library do it; does the platform (CSS, SQLite, the OS) do it; does a dependency we already have do it; does a known, trusted library made for the job do it (the owner OKs each new one); only then, the minimum new code. No abstraction with one user, no config for a value that never changes, nothing "for later".
- **Shared components, one of each.** One button, one scroll bar, one input, one menu, one dialog, one message, one empty state: each lives once in the shared UI code and the design system, and every screen uses it. A second version of something that exists is a bug in review, even if it looks the same. Same in the service: one way to do a thing (one error type per crate, one HTTP client, one way to spawn a process). The one exception: short programs (git, gh, a wait's check) start through `process-wrap` in `gathr::run`, so everything they start ends with them; engines start as they do and close the gentle way ([0031](docs/decisions/0031-short-programs-end-together.md)).
- **Registry files stay as published** ([0030](docs/decisions/0030-ui-foundation.md)). `ui/src/registry` holds the assistant-ui and shadcn registry files a merged screen uses, unchanged, each with its URL and sha256 in `ui/registry.json`. Gathr's code composes or wraps them from outside the folder. A change that must go inside is an adaptation: record its new hash and why in the same entry. Biome and the line limits skip the folder; the registry check holds it instead. Add a file only when a screen uses it.
- **Small files, readable lines.** Keep a file to what one part of the code does: usually a few hundred lines. CI fails a source file over 1,000 lines or any line over 200 characters (`scripts/check-file-size.sh`), so nothing grows into a monolith and nothing packed or minified is committed. Split a file where it has a seam, by what each part does. Small functions with one job.
- **Comments only where they earn it:** why something is done, a non-obvious constraint, a lesson ("never kill here: the engine may be renewing its login"). Never what the next line plainly does, never a comment on every function or field. Names carry the meaning.
- **Best practices of each language:** `rustfmt` and `clippy -D warnings` for Rust; for the UI, the linter and formatter that land with it. Errors handled where something can recover, passed up everywhere else. No dead code, no commented-out code.
- **Reviewers check all of this**, not only behaviour: a duplicate component, a file near the limit, a comment that restates the code, an abstraction nobody needs.

## How Gathr looks and reads

- **The design system wins over taste.** Design questions get two or three options, or mockups, before anything is built. (P2)
- **Copy follows [DESIGN.md: Copy](DESIGN.md#copy), including the owner's wording.** Use [JetBrains: Writing Short and Clear](https://plugins.jetbrains.com/docs/intellij/writing-short.html) for brevity, [Paste: Word list](https://paste-dsys.com/foundations/content/word-list/) for terms, and [Paste: Alert composition notes](https://paste-dsys.com/components/alert/#composition-notes) for notifications. Gathr terms take precedence. This applies to UI strings, accessible labels, Help, errors and Gathr-authored chat messages. Preserve identifiers, references, recovery steps and tool arguments. Agent instructions must remain clear enough to act on.
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

## Stack

Each choice has its decision in [docs/decisions/](docs/decisions/README.md), which lists them all with what is built.

- A Rust service that owns the data, the engines, scheduling and the API.
- SQLite for storage, with an event record of everything agents do.
- Each provider's own agent harness behind one interface.
- One MCP tool server inside the service.
- A Tauri desktop app over a web UI that a browser could also use.
- Windows and macOS, both checked in CI; what differs lives in one `platform` module of the service (`docs/decisions/0016-macos.md`).

## Design preview checks

The design previews, their checks and when to run each are in [designs/README.md](designs/README.md). Run them after changing a preview or a shared pattern.
