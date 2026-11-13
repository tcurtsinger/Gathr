# 0049: Gathr lands a pull request

Status: accepted (owner, 2026-11-13: "Gathr merges"). Date: 2026-11-13.

Built: yes. Proof: `crates/gathr/tests/merge/lands.rs`, `crates/gathr/tests/tasks/landings.rs`.

Serves promise 10 of [the charter](../charter.md#the-promises), nobody waits on nothing (part 10.4 of [the truth map](../truth-map.md)), and step 10 of the charter's loop: Gathr lands the work.

## The choice

- **A clean review is the word to land.** The reviewer reports its review done on the commit it reviewed, and that
  is all it does. Gathr does the waiting and the merge. Whether the work is finished stays the reviewer's
  judgement, made when it approves.
- **A PR lands when all of this holds.** The newest review of the PR in the chat that isn't cancelled is done on a
  commit that covers the PR's head (the same commit, or only clean merges of the base since, as in
  [0043](0043-checks-and-merge.md)). The PR is open, its head is on `origin`, its base is the default branch, and it
  is up to date with it. What the project requires passed on the head (Gathr's checks, GitHub's CI, both, or none).
  No CI run is still going on the head, whatever the project requires. Where the PR changes tests or check
  settings, or brings the project its first passing checks, its reviewer signed that off.
- **Sign-off is said with the review:** `finish_task {"how": "done", "commit", "signed_off": true}`. It is kept
  with the review and stands as long as that review does: a review of another commit needs it said again. The
  same done review reported again with it is taken. The `merge` tool accepts it too.
- **The merge is the merge tool's own code,** with Gathr in the reviewer's place: the PR is read again just before
  the push, a push git rejects merges nothing, and the PR's tasks settle only once GitHub shows it merged. A push
  GitHub doesn't confirm is recorded and never made again.
- **When Gathr looks:** when a review is reported done (in that call, so the answer says what Gathr did or saw),
  when a run of Gathr's checks ends, after each merge, and at each of the service's looks at tasks' PRs (every 2
  minutes). One repository at a time, under the merge tool's lock, the PR whose review came clean first first.
- **A PR behind its base is brought up to date by Gathr,** as `update_pr` does, once for that head. Only the first
  in line: each merge puts the others behind again, and each update runs their checks again. If GitHub accepts
  the update and the head hasn't moved 5 minutes on, the reviewer is told.
- **Whatever else stops a PR wakes its reviewer, once for that head,** on no task, with what Gathr saw: the update
  failed (a conflict), a required check failed, was cancelled or never ran, CI failed where the project doesn't
  require it, sign-off is missing, the base isn't the default branch, or the merge was refused. Its review
  stands. When it is the reviewer's own report that had Gathr look, the answer to the report says it and no turn
  follows.
- **Nothing is done or said twice for one head.** Gathr keeps, for each PR at each head, that it asked for the
  update, told the reviewer, or pushed a merge GitHub didn't confirm: across looks, two looks at once, and a
  restart.
- **The `merge` tool stays,** for a PR Gathr left to its reviewer.

Settled while building, each the smaller choice:

- A PR whose review doesn't reach its head (its author committed since) wakes nobody, whatever else is wrong
  with it. It needs a review again, as it did before, and each commit of a fix under way would otherwise wake
  the reviewer.
- A fork's PR is never landed, by what GitHub says of it (`isCrossRepository`), though its head may be in the
  project because someone fetched it. Its reviewer is told once.
- A CI job GitHub cancelled isn't a failure where CI isn't required ([0045](0045-review-gates-gathrs-own-repo.md)
  came from such jobs). Where CI is required, anything but passed or skipped holds the PR.
- After Gathr's own update, CI gets 2 minutes to start on the new head before "no checks" reads as none.
- A pause of the app or the chat holds landing, as it holds everything Gathr starts by itself.
- A reviewer's report waits up to 15 seconds for another look at the project's PRs to end, then reads the
  review and the pause again before it looks. Past that it answers that the PR is next.
- A cancel or a restart of a review withdraws its sign-off with its approval.
- The default branch is asked of `origin` itself (`git ls-remote --symref`), not of GitHub.

## Why

In the team's first three days in Gathr, five of the seven stalls the owner noticed before Gathr did were a PR
that looked ready and sat unmerged: reviewed, green, and waiting for the reviewer's next turn to notice. All 16
merges went through the reviewer's `merge` call, each after a wait on CI and a turn to be woken by it. The
conditions were already Gathr's to check; only the noticing was an agent's.

## Turned down

- **Merge on green with no review word.** A done review is the reviewer's judgement that the work is finished;
  without it Gathr would decide that.
- **Keep the reviewer's `merge` call and add a reminder.** Still a turn per PR, and still a PR that waits for one.
- **GitHub's auto-merge.** It needs required checks on the branch, which 0045 took away, and it knows nothing of
  Gathr's own checks, the review record or sign-off.
- **Bring every PR that is behind up to date.** Each merge would rerun every other PR's checks.
- **A switch to turn landing off for a project.** Nothing asks for it yet. A reviewer who doesn't want a PR landed
  doesn't report its review done.

## Not in it

Landing for work that isn't a GitHub pull request: a repository with no GitHub, or work that isn't code. Reviews,
the merge and the waits all read a GitHub PR today.

## Changes

- [0045](0045-review-gates-gathrs-own-repo.md): "no auto-merge" now means GitHub's; Gathr does the waiting for a
  running CI. A failure is still read by the reviewer before a merge, and still holds none where CI isn't
  required.
- [0043](0043-checks-and-merge.md): sign-off may be said with the review as well as with `merge`.
- AGENTS.md: "The reviewer merges" is rewritten: the reviewer's done review lands the PR.
