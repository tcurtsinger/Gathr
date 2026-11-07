# Gathr

Gathr is an open-source desktop app for running a team of AI agents from different providers (Claude Code, Codex and more) on your own computer: chats, rooms where agents work together, tasks, plans and reviews, with logging and analytics built in, so you can see how the agents work and make it better.

Status: early. Product, design and architecture come first; see `docs/`.

## How we build

- **Plans before code.** Nothing is built until its plan is approved. Each decision gets a file in `docs/decisions/`: the choice, why, and what was turned down.
- **One feature, one PR,** its engine and its screens together, opened when the feature is done. From your own branch and your own git worktree (never the main checkout), base `main`, marked ready. Don't stack a PR on another PR's branch: merging that branch closes it.
- **Every change has its check:** a test, or a command anyone can run that fails if the change breaks.
- **Tests:** CI is the gate. The author runs the tests the change touches once before pushing; the reviewer reads and doesn't rerun suites.
- **Review once, when it's ready.** The code reviewer reviews every PR. The author answers each note; a second look only for a fix that changes behaviour. Minor notes are fixed in passing or dropped, never a round of their own. A docs-only PR merges without review rounds unless something in it is wrong.
- **The lead merges** once the notes are answered and CI is green, with a merge commit: `gh pr merge --merge --delete-branch --match-head-commit <head commit>`. Nobody turns on auto-merge. Check `main` is green before opening the next PR.
- **Clean up after yourself:** once your PR is merged or closed, remove your worktree and branch.
- **Keep this file current:** when a step here changes, or you learn one, change this file in the same PR.

## Build and check

- **Toolchain:** Rust through [rustup](https://rustup.rs); `rust-toolchain.toml` pins the version, so `cargo` picks it up by itself.
- **The checks CI runs**, from the repo root (run them once before pushing):
  - `cargo fmt --all --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `cargo test --workspace --locked`
  - `bash scripts/check-file-size.sh` (source files up to 1,000 lines, lines up to 200 characters)
- **CI** is `.github/workflows/ci.yml`, on Windows and macOS, for every PR and every push to `main`. `main` only takes PRs with CI green.
- **Commit messages on Windows:** write the message to a file and use `git commit -F <file>`; the same for scripts, write them to a file and run that. Inline multi-line strings get mangled by the shell.
- **PR screenshots:** attach them with `gh pr edit <n> --attach <file>`.

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
- Dev ports and data folders: the lead assigns them when the first runnable piece lands. Never use another member's.
- After a merge, bring `X:\code\gathr` up to date: `git fetch`, then `git merge --ff-only origin/main`. Check `git status` first and never discard the owner's changes: if something would be overwritten, stop and ask.

### Learning from SerenityOS

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
