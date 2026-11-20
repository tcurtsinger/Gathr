# Gathr

Gathr is an open-source desktop app for working with your own AI agents, from different providers (Claude Code, Codex and more), on your own computer: a chat where you and your agents work like a team.

**Status, 2026-11-20: the chat and the team layer were started over, and the old ones are deleted.** A chat is a room: messages, replies folded under the message they answer, reactions, a board of the work, and questions to the person. Gathr starts no turn and writes no message by itself, except the team's check of the lead's plan or answer. Under it, kept as it was: the engine drivers, sign-in and logins, storage, updates and releases, approvals, search, diagnostics.

`docs/archive/` is how Gathr was built before that day: the old `AGENTS.md`, the charter and its promises, the truth map, the scenarios, the testing guide and every numbered decision. It is history. Code comments cite decisions by number, and they are there to read. **Nothing in it is a rule, and nobody keeps it up to date.** If a habit comes from there and isn't on this page, it is gone.

## How we build

- **Take out before adding.** No new guard, message from Gathr, kind of task, wait, record or setting unless the owner asked for it. When something gets in the way, first look for a thing to remove.
- **What the owner sees is designed first.** Two or three options or mock-ups, the owner picks, then it is built. While they are still deciding, end with the question, never with "should I start?".
- **Only what was asked.** One ask, one pull request, as small as it can be, from your own branch and worktree, base `main`. No backlog work on the side.
- **Into the owner's app fast.** The author runs the quick checks and the tests the change touches, reads the whole diff, and merges with a merge commit. Nobody waits for CI: it runs as information. Each merge to `main` publishes a release.
- **Few tests.** A test is for a mistake that loses data or locks someone out: a database change, sign-in and logins, reading an engine's frames, the states of a task and a turn. No test for a look, for wording or for glue, and none written first as a rule. A review asks for a test only in those areas. A test that fails at random is deleted, and the owner is told.
- **No paperwork per change.** No decision file, map row or scenario for a pull request. A choice the owner made between options is written once, in the pull request that builds it.
- **Friction is told to the owner in plain words.** It is not a requirement, and it does not become a feature by itself.
- **No assumptions.** A fact you state or build on (how a tool, an engine or a library behaves, what a file says, what a number is) is checked first: in the code, the current docs, or by running it. What can't be checked is said plainly as not verified.
- **Done means used.** Run it and use what changed the way a person will. Say plainly what you couldn't run.
- **A database change** is a new numbered file in `crates/gathr/migrations/`, never an edit to a released one. It is tried on a copy of the owner's data before they update, and its pull request says it carries one.
- **Keep this page short.** A rule is added here only at the owner's word.

## Safety

- **The owner uses this PC.** Gathr is installed on it and holds their real chats; SerenityOS, the owner's earlier app, runs beside it. Never install, quit or restart either. Never stop processes by name (`Stop-Process -Name`, `taskkill /IM`): stop only what you started, by its process id.
- **The owner's data** is `%APPDATA%\Gathr` (`~/Library/Application Support/Gathr` on a Mac). Never run a service, the CLI or `gathr-app` without `--data <your own folder>`: without it they run on the real data.
- **Logins are single-use secrets.** Each account's login lives in its own Gathr folder. Never touch `~/.claude`, `~/.codex` or `~/.serenityos`, never copy a `.credentials.json` or `auth.json`, and never force-kill an engine that may be renewing its login.
- **Ask before installing or downloading anything** (its name, source and size). The one exception: `npm ci` in your own worktree's `ui` folder needs no asking, since it installs only what the project's lockfile names. No sound without warning. Never read or write the owner's clipboard. No window opens on the owner's desktop without telling them first.
- **Nothing that costs money is required.** Gathr runs the providers' own agents on the person's own logins. A run on real engines spends the owner's tokens: only at their word.
- **Guard only against real risks.** No kill switches, disabled features or disclaimers for their own sake.

## A public repo

- No secrets, keys, tokens, `.env` files or login files, not even in tests or history. Never print a secret.
- No personal data: no emails, addresses, IPs, employers, or screenshots of real conversations. Test data is made up. Commits use the owner's GitHub noreply address.
- Plans that mention the owner, reviews, evidence and screenshots of real use stay out of the repo, in the Agent folder.
- **SerenityOS** (`X:\code\serenityos`) is the reference for how the chat should feel: agents talking, disagreeing and checking each other in plain words. Everything the owner wrote there (after the Bloks import, commit `5f6947a3`) can be read, adapted and ported. The rest is under Bloks' licence (FSL): read it to understand, then write your own.
- Dependencies: well maintained, with a compatible licence, and never for what a few lines can do. The owner OKs each new one.

## Where things go

- Everything agents make lives in the Agent folder, `X:\Gathr\Gathr`: what the team keeps in `Gathr\Shared`, each agent's own worktrees, builds and scratch in `Gathr\<its name>`. Nothing goes in the owner's main checkout `X:\code\gathr`.
- A worktree: `git fetch`, then `git worktree add X:\Gathr\Gathr\<your name>\<topic> -b room/<topic> origin/main`. Remove it and its branch when the pull request is merged or closed.
- Each agent's dev service has its own port and data folder: Orin 47100, Cass 47101, Pia 47102, Juno 47103, Vale 47104, Claude 47105, each with `X:\Gathr\Gathr\<name>\data`. With `--window` the UI takes the port plus 10. Never use another's.
- Clean up when a task ends: delete builds, test data and probes, and stop every process you started by its process id.

## How the code is written

- **The smallest code that gives the result.** Before writing: does it need to exist, is it already here, does the standard library, the platform or a dependency we have do it. No abstraction with one user, no config for a value that never changes, nothing "for later".
- **One of each.** One button, one input, one menu, one dialog; one error type per crate, one HTTP client, one way to start a process. A second version of something that exists is a bug.
- **Registry files stay as published:** `ui/src/registry` holds assistant-ui and shadcn files unchanged; Gathr's code wraps them from outside.
- **Small files, readable lines:** CI fails a source file over 1,000 lines or a line over 200 characters.
- **Comments only where they earn it:** why, a constraint, a lesson. Never what the next line plainly does.
- `rustfmt` and `clippy -D warnings` for Rust; Biome and `tsc` for the UI. No dead code.
- **Words in the window** are short and plain: [DESIGN.md: Copy](DESIGN.md#copy). No hover text. Every setting has a control in Settings. Never design a feature around one particular agent.

## Reference

Facts, not rules. Read the one your task touches.

- [docs/building.md](docs/building.md): toolchain, the checks and how to run them, CI, releases, a Mac without Rust, and `gathr diag`.
- [docs/running.md](docs/running.md): run a dev service, seed test data, run it installed.
- [docs/window.md](docs/window.md): use and check a screen in the window.
- [docs/architecture.md](docs/architecture.md): what is built, part by part.
- [docs/api.md](docs/api.md), [api-engines](docs/api-engines.md), [api-approvals](docs/api-approvals.md), [api-tasks](docs/api-tasks.md): the service's API.
- [DESIGN.md](DESIGN.md) and [designs/README.md](designs/README.md): the design system and its previews. Their chat parts describe the chat that is being replaced.
- [docs/archive/README.md](docs/archive/README.md): what was archived, and why.
