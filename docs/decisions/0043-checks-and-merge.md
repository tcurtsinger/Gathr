# 0043. Gathr runs the checks and does the merge

Status: accepted. Approved 2026-11-10; recorded 2026-11-12 against the built implementation.
Amends [0029](0029-approvals.md). The earlier draft called this work 0036; that number now belongs to
[Colours: surfaces in three steps](0036-surfaces-in-three-steps.md). Checks-and-merge references use 0043.

## Why

Projects need a merge gate even when they have no hosted CI. Instructions asking agents to merge only
reviewed, checked code do not enforce that rule. Gathr already knows the project, reviewer, commit and
task; it can run the checks, enforce the gate and settle the work when GitHub confirms the merge.

The person should not have to write check commands. The Supervisor chooses them from the project's
own runners, while the reviewer signs off changes to tests and check settings. Reusing caches keeps
an exact-commit checkout practical without admitting an agent's uncommitted files into a passing run.

## Choice

### 1. The Supervisor sets the project's checks

- `checks` gives a project chat's Supervisor its ordered commands, who set each and when, plus `found`:
  runners discovered from package.json, Cargo.toml, pyproject.toml and GitHub workflow `run:` steps.
  Discovery is a starting point, not an automatically accepted check list. New projects have no commands.
- `set_checks` replaces that list and can set Required checks (`ci`) and the merge method. Other agents
  suggest changes to the Supervisor; the service refuses their setting calls with a reference.
- A command the project's files name needs no new permission. A different command asks the person once
  for that project and command before it is accepted. This permission is separate from a wait check.
- Settings > Projects shows commands read-only with their source and set time. The person never types a
  check. The person can change Required checks and merge method there; their source is `person`, while
  the Supervisor's changes record its identity. Project name, folder and team are editable in General;
  changes to folder/team affect new chats. The sidebar's Project settings opens that project.
- Required checks is **CI**, **Gathr's checks** or **both**. A new project's default is CI if its checkout
  has a GitHub workflow (`.yml` or `.yaml`), otherwise Gathr's checks. Projects migrated before this choice
  default to Gathr's checks. CI-only projects need no local check commands. Gathr's own project: review
  and local tests are the gate, CI is information ([0045](0045-review-gates-gathrs-own-repo.md)).
- Merge method is **fast-forward** by default, **merge commit**, or **squash** through GitHub's API.

### 2. Run at the commit, reuse only the caches

- A review task naming a PR causes the PR looks to run the project's commands at head H, and again for
  a new head with no recorded, non-interrupted run. The API can start another run. This does not create a review task;
  automatic review assignment remains C9 below.
- The service fetches the commit into a detached worktree at `<Project>/checks` in the agent folder,
  resets it with `checkout --force`, then `clean -ffdx`. An agent's uncommitted files and an earlier
  run's leftover files cannot make the next commit pass.
- One run at a time per project; commands run in order in `sh` (Git Bash on Windows), at most one hour
  each, stopping on failure. This uses the normal short-program process handling, not an engine session.
- Caches are outside the checkout in `<Project>/caches`: Cargo's target, npm's cache and pip's cache.
  They are reused across runs; this is not a promise to preserve node_modules or a dependency directory
  per lockfile. Settings shows their size and Clear; caches unused for 14 days are swept.
- Each run records the project and SHA, state, each command's exit, duration and last 64 KB of log.
  Failure has a reference in Diagnostics. A result that cannot be saved is not a pass. A run interrupted
  by service shutdown becomes failed on restart and can run again.
- A task's checks view uses the PR head last seen, so an older pass is not presented as proof for a newer
  head. The latest run at H must pass and must have started after the check list was last changed.

### 3. The reviewer asks Gathr to merge

The reviewer uses `merge {"pr": N, "signed_off": true}` when sign-off is needed, rather than typing a
merge command. A refusal says why and gives a reference. Gathr requires:

1. The caller owns a completed review of that PR in this chat. The recorded commit R resolves in the
   project (a true 7–40-character hexadecimal prefix), and covers the current head H.
2. The configured checks passed on H. CI must have a nonempty rollup with no unfinished or failing
   result; the implementation accepts SUCCESS, NEUTRAL and SKIPPED. Gathr checks require a current
   passing local run; no commands means they cannot pass. `both` requires both sources.
3. The fetched PR base commit B is an ancestor of H. Updating the branch means running checks on the
   new H, even if its review carries forward.

A completed review survives a CI wait waking its task. It also covers a later H only when every added
first-parent commit is an ordinary two-parent merge: its first parent is the preceding accepted commit,
its other parent is on the fetched base, and its entire tree equals Git's clean `merge-tree --write-tree`
result. An author commit, conflict resolution, edited merge tree or unsupported merge shape needs review
again. An explicit unfinished finish, cancellation or restart withdraws approval. A held review's owner
can re-record done on the same task without changing its PR or dependencies (G165).

The reviewer must explicitly sign off a PR touching tests or check settings: test/spec paths, workflow
files, package/Cargo/pyproject settings and similar files are flagged. A local-check project bringing
its first passing baseline also needs that sign-off; there is no imaginary passing baseline on main.
Checks still run at H: reading a runner from main alone would not stop the PR changing the script it runs.

### 4. Pin the head and the base the implementation can pin

- Merges are serialised per repository. Gathr reads the open PR's head and target branch, fetches origin,
  checks B and H, then reads the PR again immediately before the push/API call. A changed head or target
  refuses. A head not fetched from origin is refused.
- **Fast-forward:** push H to the target without force.
- **Merge commit:** create M with H's tree and parents B and H using `git commit-tree`, then push M
  without force. A competing base update that makes either push non-fast-forward is refused; update
  the PR and run its checks again. Two PRs checked against one base cannot bypass this by forcing a push.
- **Squash:** call GitHub's merge API with `sha = H`. This pins **H, not B**; GitHub squashes onto the base
  it has at that moment. Do not describe this path as atomic pinning of both commits.
- **As built, B is fetched from the PR's `baseRefName`.** It is normally the default branch, but the tool
  does not independently require that target to equal the repository's default branch. Retargeting
  during a merge is checked; an initial non-default target is not forbidden by this gate.
- A pushed merge settles tasks only after GitHub reports that PR merged at H into the same target.
  Confirmation is bounded; if GitHub has not confirmed, or settlement fails, the answer has a reference
  and the later PR looks reconcile it. Do not close a PR by hand and call that confirmation.
- A confirmed merge settles its tasks immediately. The giver hears back when appropriate; the reviewer
  that just merged it does not get an extra turn to be told what it already knows.
- Merged **or closed** PRs queue worktree/branch cleanup. The existing sweep removes only recorded clean
  worktrees and their local branch, never dirty or in-use work, newer commits on a reused branch, other
  branches, unrecorded checkouts or a fork's same-named branch. Skips have references and retry later.

### 5. Settings tools and the current agent boundaries

- Agents can read General, agents and projects through `settings`, and change General/agent settings
  when the person asks, with the same validation as the API and an event naming who changed them.
  Projects/checks/merge policy use the Supervisor's tools above; the person's Settings controls remain.
- `answer_approval` and `answer_person` can answer for the person when asked, **never the caller's own
  approval, its own question, or a question filed for it**. An agent cannot move anyone into or out of
  Full approvals, which changes which pushes ask. The persisted write enforces that boundary.
- The shipped pre-push hook asks about a default-branch push, force/non-fast-forward push, deletion or
  a push outside that worktree's origin; ordinary feature-branch pushes go through. Command checks
  also catch force/delete/hook-bypass forms. Full retains its existing bypass semantics. These are
  agent guards, not restrictions on the person's own git use.
- The hook is installed for the agent's worktree. Extending it to every repository the engine can
  create or clone, and explicitly guarding typed GitHub merges, is **C5, not built yet**. The `merge`
  tool is the required workflow; the current hooks do not make it the only technically possible path.
- Known single read-only wait commands (`gh pr view`, `gh pr checks`, restricted `git log`, `git rev-parse`)
  can run without asking, parsed without shell expansion and with repository-controlled git output
  settings overridden. `git status` is not on that allowlist because git can run configured filters.
  In Auto, other wait checks are judged by the engine's own Auto mechanism; Ask/Edits use the existing
  person-approval path. This is not permission to change which pushes ask.
- `wait {"pr": N, "up_to_date": true}` is a built-in wait for the PR to contain its base's commits,
  or to merge/close. It does not require an agent's polling shell loop or claim a new review is needed.

## Implementation status

| Part | Status |
|---|---|
| C2, C2b, C2c | Built: project settings; Supervisor checks, CI choice and merge method; attribution. |
| C3 | Built: exact-commit runner, project caches, stored results, API and task display. |
| C4 | Built: reviewer merge gate, sign-off, methods, confirmation and immediate settlement; G165 review carry-forward. |
| C5a | Built: settings and answer-for-person tools with the self-answer and Full boundaries above. |
| C6 | Built: Settings > Projects, sidebar entry, cache controls and task checks. |
| C8 | Built: inherited local git variables stripped in tests/short programs, tracked NUL-byte check, wait-shell wording. |
| C10 | Built: queued, retryable cleanup after PR merge/close. |
| C11, C12 | Built: restricted read-only wait checks and built-in up-to-date wait. |
| C5b | **Not built:** move the service key to Windows credentials/macOS Keychain; `service.json` still carries it. |
| C5 | **Not built:** every-repository push hooks and typed-merge guards; strengthen target/base guarantees above. |
| C9 | **Not built:** automatic review task when a work task's PR opens, plus the complete PR/check/review/next-step task view. Existing check results are only part of that view. |

## Turned down

- **Instructions as the only merge gate.** They can describe the policy but cannot enforce it.
- **Requiring hosted Actions, paid branch protection or a self-hosted Actions runner for every project.**
  Local checks should work without that service or setup; hosted CI remains a project choice.
- **The person typing checks in Settings.** The Supervisor sets them from the code and discovered runners;
  the reviewer signs off changes, rather than handing the person a command configuration job.
- **Checking an agent's working copy.** Uncommitted files could produce a pass the commit cannot reproduce.
- **Throwing away all caches per commit.** Keep the checkout clean while reusing external build/download caches.
- **Locally squashing/rebasing and pretending GitHub merged the PR.** Rewritten commits do not retain H;
  squash uses the API, with its explicit base-pinning limit.
- **Ancestry alone as proof of review, or reviewing every clean main update again.** A merge can hide an
  edited resolution; compare its whole tree, and carry a completed review only across proven base merges.

## Checks and diagnostics

The implementation's seam tests cover exact-commit checks despite dirty agent work, failed/missing/stale
checks, sign-off and first baseline, competing bases, head/target changes, canonical review carry-forward,
CI wakes, settlement, safe cleanup, Supervisor-only settings and self-answer/Full restrictions.
See `tests/checks.rs`, `check_runs.rs`, `merge.rs`, `pr_cleanup.rs`, `settings_tools.rs` and `coordination.rs`.
Events include `gathr.checks.*`, `gathr.merge.done`, `gathr.project.change`, settings/answer events and
`gathr.cleanup.*`. Failures carry references usable through `gathr diag` ([0019](0019-troubleshooting.md)).
The API details are in [checks and merging](../api-checks.md), [projects](../api.md) and
[approvals and tools](../api-approvals.md).
