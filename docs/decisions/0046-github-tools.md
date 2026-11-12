# 0046. Gathr reaches GitHub for agents, with its own sign-in

Status: accepted (owner, 2026-11-12: option A of `Shared\plans\codex-github.md`, "Fix now"). Date:
2026-11-12. G185, G147, G179. 0044 is the step-results decision and 0045 is Gathr's own review gate,
so this is 0046.

## Choice

- **Three tools on Gathr's tool server, made host-side** with the `gh` the service already runs for
  its PR looks and `merge` (`coord::github`), with the computer's own GitHub sign-in. No GitHub
  credential goes into an engine, its environment, its arguments or anything it reads:
  - **`pr {"pr"}`:** the PR's state, head, base, mergeable and review, and each check on its head.
  - **`ci_failures {"pr"}`:** for each failed check that ran on GitHub Actions, its job's log (`gh run
    view --job <id> --log-failed`) up to its last `##[error]` line, its last 40 lines, each cut to 300
    characters and hidden with `gathr::redact`; then its first 10 annotations (the check run's
    `annotations`). A check that isn't an Actions job gives its link instead.
  - **`update_pr {"pr", "head"}`, the PR's reviewer only** (a review task of it in the chat): GitHub's
    Update branch (`pulls/<n>/update-branch` with `expected_head_sha`), refused when the head isn't the
    one given, when the PR isn't open, or when it isn't into the default branch (retargeted). It
    waits up to 20 s for the new head, and `gathr.github.update` records the head requested and
    returned.
- **Merging stays `merge`** (0043 C4), with every gate it has. These tools merge nothing.
- **The repository is the chat's project's own folder** (its `origin`), never a repository, host,
  folder or command an agent names. Arguments are fixed arrays, with no generic `gh` or `gh api`
  passthrough, no `gh auth token`, no output files and no extensions. `GH_DEBUG` is dropped from
  every `gh` Gathr runs, since it prints the HTTP traffic, headers and all, into what an error shows.
- **A failure is told in words an agent can act on, with a reference:** "GitHub sign-in isn't
  available to Gathr" (no login, 401, bad credentials) versus "this GitHub account can't reach the
  project's repository" (404, 403, unknown repository). Neither starts a login.
- **Diagnosable (0019):** each call is a `gathr.tool.call` on the turn's trace, `pr` and
  `ci_failures` a `gathr.github.read` (operation, PR, head), and `update_pr` a `gathr.github.update`.
  None holds what GitHub said (0011).
- **The briefing says it** in a project chat: use `pr`, `ci_failures` and `update_pr`, not your own
  `gh`, and merge only with `merge`.

## Why

A Codex agent's sandbox can't read the computer's credential store, where `gh` keeps its sign-in, so its
own `gh` answers 401 (Juno's measurements in the plan). The reviewer then couldn't read why CI failed,
update the PR first in line or merge without the lead. Gathr's service already runs `gh` host-side for
looks and merging; three typed operations on that path restore the reviewer's work for every engine,
with the sign-in kept out of the sandbox and one place to diagnose a failure.

## Turned down

- **GitHub's official MCP server** in each engine: it needs a GitHub token in the engine's process
  (the credential the sandbox is meant to keep out). Its tool set is far wider than review needs
  (writes, admin, arbitrary API calls), and its calls would skip Gathr's merge gate and records.
  Juno adds the details to the plan.
- **A GitHub login inside the sandbox** (the plan's option B): any command there can read it, and its
  scope, expiry and renewal become Gathr's to manage.
- **A generic `gh` or `gh api` proxy:** credentialed authority for any command an agent writes.
- **Full access, or the sandbox off, for every Codex agent;** asking the lead to run each command.

## Live check

Read-only, on this repository's PR #131 (its Rust jobs failed on a NUL byte in README.md), through the
real `gh` and its sign-in on the owner's computer: `pr` listed its five checks, and `ci_failures`
returned both failed jobs, each ending at its error line, with their annotations. The checkout step's
`AUTHORIZATION` header in the log came back redacted. Nothing was changed.

## Not verified

- **`update_pr` against GitHub itself:** it changes a PR, so it runs only against the `gh` stand-in until
  a disposable test repository exists (the owner's call).
- **A Codex engine calling these tools inside its sandbox on Windows and macOS.** They're Gathr's MCP
  tools, which Codex reaches as it does `merge`, but no Codex turn called them live.
- **Author-side GitHub work** (opening a PR, commenting) and `git fetch`/push from the sandbox: not in
  this slice. G185 isn't covered until those are inventoried.
