# 0086: A turn opens with its PR's head, and failed CI with its last lines

Status: accepted (owner, 2026-11-19: approved plan "Stage 4's rest: what a turn opens with", version 2, part 2).
Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/merge/opening.rs`, `crates/gathr/tests/github.rs`.

Builds on [0046](0046-github-tools.md) (`pr` and `ci_failures`, Gathr's own `gh`),
[0049](0049-gathr-lands-pull-requests.md) (a PR Gathr can't land wakes its reviewer) and
[0081](0081-agents-hear-the-room-and-name-each-other.md) (what a turn opens with). Serves promise 8 of
[the charter](../charter.md#the-promises), part 8.2 of [the truth map](../truth-map.md).

## Why

In the baseline, 70% of turns on a task read git state in their first three calls, and a wake on failed CI said only
"Your wait ended: … 1 failed", leaving the agent to call `ci_failures`. Gathr already reads both for the tools.

## The choice

- **Failed CI.** A CI wait met with a check that failed (not one only cancelled) keeps the commit that failed
  (`waits.failed_at`). Its wake turn keeps each failed part's commit, the repository the wait named and the folder its
  looks ran in (`turns.failed_ci`), for a wait on a PR, on a commit or of several parts. So does the reviewer's wake
  for a PR Gathr can't land because of red CI, required or not (`landing.rs` `red_ci`); other stops read nothing. When
  the turn starts it opens with each failed job's last lines and annotations, read for that commit in that
  repository, never for a head pushed since (`github::failed_at`, sharing `each_failed` with `ci_failures`).
- **A turn on a task's PR**, a member's in its task's conversation or the lead's in its own, opens with the PR's
  branch, head and every changed file (`github::changes`, from the same view as `pr`, with `files`).
- **Told once per head.** Each engine's task keeps, in memory, the head each conversation was told of each PR. While
  Gathr's own looks still hold that head (`tasks.pr_head`), no call is made; a head GitHub shows that was told already
  isn't said again. A new conversation, or one a compaction summed up, is told again. So is every conversation after
  the service restarts, as the memory goes with it: one repeat, which costs a GitHub read and a few lines.
- **Bounded and diagnosable.** All of it waits at most 20 seconds before the turn starts without it. Any read that
  fails or runs over leaves the turn without these lines, never with part of them, and records `gathr.github.read`
  with `why` and how long it took (`ms`); a read that worked records the same event as ok.

## Turned down

- **Keeping the told head in `task_sessions`.** It would survive a restart, but the lead's conversation has no row
  there, and a restart costs one repeat. Not worth a column and its writes at each Init.
- **Reading failures for the PR's head when the turn starts.** A push while the wake was queued would show the new
  head's checks, often still running, instead of the ones that failed.
- **A log read for a cancelled check alone.** The wake already says it was cancelled; there's no failure to read.
- **Best-effort lines when a job's log can't be read**, as `ci_failures` gives. An opening that says part of what
  failed reads as all of it; the agent can still call `ci_failures`.
