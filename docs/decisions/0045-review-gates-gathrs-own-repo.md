# 0045: Gathr's own repo: review and local tests are the gate, not GitHub CI

Owner, 2026-11-12 (room tasks 167-169).

## The choice

- `main` requires no GitHub check. A PR merges once its review (or second look) is clean, its notes are answered,
  and its author ran the checks and tests the change touches (AGENTS.md, How we build). The reviewer reads and
  doesn't rerun suites, as before.
- GitHub CI keeps running on every PR and every push to `main`, as information: a red job is read and, if it is a real
  failure, fixed in a follow-up, but it doesn't hold a merge.
- Releases still wait for green CI on `main` (F7; owner, room task 175): it's the one run on clean machines before an
  install updates, so a red `main` is fixed before the next release.
- When a PR's CI is already running, the merger waits for it to finish and reads any failure before merging.
- Everything else in the merge rule stays: the landing order, up to date with `main` before merging, the reviewed
  head pinned (`--match-head-commit`), a merge commit, no auto-merge.
- Until Gathr's `merge` tool has a review-only setting for Required checks, the reviewer merges Gathr's own PRs with
  `gh pr merge --merge --delete-branch --match-head-commit <reviewed head>`.

## Why

On 2026-11-12 GitHub's macOS runners stopped taking jobs: every macOS job queued and was cancelled after about 15
minutes for capacity, with no incident posted. Three PRs with clean reviews and passing Windows checks (#157, #156,
#154) and a release waited on them, and the whole room went idle. CI on two systems plus the author's local tests plus
review was more gate than this app needs; review and local tests find the issues.

## Turned down

- Keep the two Windows checks required, drop macOS: still holds a merge on a hosted runner's queue.
- Stop GitHub CI entirely: CI on a clean Windows and macOS machine still catches what a local run misses, and
  releases are built there.
- Wait for GitHub: no date, and the room stalls meanwhile.

## Changes

- 0043: "Gathr's own project uses CI" is replaced by this decision.
- AGENTS.md: How we build (Tests, The reviewer merges) and Build and check (CI).
- `main`'s branch protection: required status checks removed (done 2026-11-12).
