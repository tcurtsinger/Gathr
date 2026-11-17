# 0067: A merge brings the project folder along, or says why not

Status: accepted (owner, 2026-11-16, room task 227: Gathr updates the person's own checkout whenever it merges a PR, and tells them why when it can't do it safely). Date: 2026-11-16.

Built: yes. Proof: `crates/gathr/tests/merge/checkout.rs`.

Builds on [0043](0043-checks-and-merge.md) and [0049](0049-gathr-lands-pull-requests.md). Serves promise 10 of [the charter](../charter.md#the-promises), part 10.9 of [the truth map](../truth-map.md), and promise 7: the friction rows S27 (the main checkout fell behind `main`, so sessions read an old `AGENTS.md`) and G169 (the merger couldn't update the owner's checkout from its own worktree). It replaces the AGENTS.md rule (P7) that had the lead fast-forward the checkout by hand after each merge.

The **project folder** is the folder a project is set to: the person's own checkout, where they may be working.

## The choice

- **Every merge Gathr makes ends with the project folder.** Once GitHub confirms the merge, by any method (fast-forward, merge commit, squash), `push_merge` fetches from `origin` again, in the project folder and under the repository's merge lock. The merge tool and landing share that code, so both do it. A squash's commit is made on GitHub, so the fetch before the merge doesn't have it.
- **Only a fast-forward, only when it's safe.** The folder is moved only when all of this holds: the PR's base is the branch `origin`'s HEAD names (asked of the remote, `git ls-remote --symref`, as landing does); no merge, rebase or cherry-pick is under way in the folder; the folder is on that branch; and that branch has no commits `origin` doesn't. Then `git merge --ff-only --no-autostash origin/<base>`. Changes to files the merge doesn't touch stay as they are.
- **Git refuses what would overwrite the person's work.** `--no-autostash` holds whatever the person's config says: with `merge.autostash` on, git stashes a changed file the merge touches, moves, and leaves conflict markers in it (Vale). With it off, git refuses, and Gathr names the changed files the merge touches, read with `git diff --name-only` rather than from git's words, which are in the person's language.
- **When it doesn't move the folder, the person is told once, in the chat.** One line Gathr says in no turn (0051's `store::say`, published live as a message), with why and the merge's reference: `Merged #5, but didn't update the project folder: it's on mine, not main. Update it yourself with git pull --ff-only once it's safe. (ref 0123abcd)`. A merge into a branch that isn't the default gets no next step: there is nothing to update. Each merge happens once, so its line is said once.
- **The merge stays merged.** A fetch or git that fails after the merge doesn't undo it or change the merge's answer: it's a folder not updated, said as above.
- **Recorded.** `gathr.merge.checkout` on the merge's trace, with `pr`, `base`, and `commit` when it moved, or an error with why: `fetch`, `default`, `not_default`, `under_way`, `branch`, `local_commits`, `refused`.

## Why

- The person reads the project folder, and so do new sessions: a folder behind `main` showed an old `AGENTS.md` (S27). Nobody in a chat can update it from their own worktree (G169).
- A fast-forward only ever adds what `origin` already has. Anything else (a merge of local commits, a rebase, a stash) changes the person's work, and is theirs to choose.

## Turned down

- **A stash, or `merge.autostash`, to update a folder with changes the merge touches.** It leaves conflict markers in the person's file when the stash doesn't apply cleanly.
- **`git pull`.** It merges or rebases local commits by the person's config; a fast-forward of the fetched branch is exactly the part that's safe.
- **Updating whichever branch the folder is on, or the PR's base whatever it is.** Only the default branch is what the person reads as the project; a PR into another branch leaves the folder.
- **Telling the agent that merged, in the tool's answer.** The folder is the person's; the agent can do nothing about it.
- **A new notice for it.** The chat already has Gathr's own lines.
