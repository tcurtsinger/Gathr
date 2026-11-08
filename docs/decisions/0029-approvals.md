# 0029. Approvals: what an agent may do without asking

Status: accepted (slice 2, the owner's pick of "Auto, with guards"). Date: 2026-11-08.

## Choice

- **A mode per agent:** `approvals` on `/api/agents`, one of `ask`, `edits`, `auto` (the default, for new agents and every agent before this) or `full`, set in Settings > Agents. A change closes the agent's engine gently before its next turn, like a model change (0026).
- **Each maps to Claude Code's own permission mode** (`--permission-mode`): `ask` is Manual (`default`: everything but reading and read-only commands asks), `edits` is `acceptEdits` (file edits and `mkdir`/`mv`/`cp` inside the folder it runs in), `auto` is `auto` (a classifier model checks each action), `full` is `bypassPermissions`. Codex maps when its driver lands: `workspace-write` with `on-request`, and `danger-full-access` with `never` for `full`.
- **Gathr's own asks, in every mode but `full`:** Claude Code calls Gathr's hook (an HTTP `PreToolUse` hook at `/hook`, given in `--settings`) before each `Write`, `Edit`, `NotebookEdit`, `Bash` and `PowerShell` call. It answers "ask" for a file write outside the engine's folder (its worktree in a project's chat, 0028) and for any `git … push`, however it's written (`git -C <dir> push` too). Claude Code then sends the call to Gathr as an approval with the hook's reason, whatever the mode or a project's rules allow. The hook takes only the engine's own token (0005), and `onFailure: "block"`: a Gathr that can't answer never lets a push through. Each ask is a `gathr.approvals.ask` event on the turn's trace.
- **What auto mode blocks** isn't a question: Claude Code refuses it, tells the agent why, and sends `system/permission_denied`. Gathr records it as `gathr.approvals.blocked` with its reason (`[Data Exfiltration]`). After 3 blocks in a row or 20 in a session Claude Code falls back to asking (its docs), which reaches Gathr as an approval like any other.
- **An engine in another mode than asked** (auto mode isn't offered for every model: Claude Code starts in Manual instead) is a `gathr.approvals.mode` error on the turn's trace, with the mode given and the one running.
- **A project's own Claude rules never widen this.** Gathr reads a project's `.claude/settings*.json` (`--setting-sources project,local`, needed for its `CLAUDE.md`), but Claude Code ignores a project's allow rules until the folder is trusted, and Gathr never marks a folder trusted. Deny and ask rules beat allow rules from any file (Claude Code's docs). Not checked: whether the hook's "ask" also beats an allow rule in a folder someone trusted by hand.
- Approvals still go to the person. Whether a team chat's lead answers some of them comes with the Tasks list.

## Why

- With every command asking (F4), a member's turn stopped at its first build or commit, so the lead couldn't hand work out and get it back.
- Auto is the mode the engines' own apps offer for normal work; Codex's default for a repository is the same idea. What leaves the agent's copy (a push, a write into another checkout) is where a mistake reaches someone else, so that still comes to a person.
- Checked live on Windows (Claude Code 2.1.319, the test login, Haiku) before building: an ask rule and a hook's "ask" both reach Gathr as approvals in auto mode; a classifier block doesn't, it comes as `permission_denied`; dropping `project` from the setting sources loses `CLAUDE.md`; a project's allow rules are ignored in an untrusted folder.
- A hook, not ask rules: rules can't say "outside this folder", and `Bash(git push *)` misses `git -C <dir> push` (Claude Code's docs say so). The hook is one place with one test.

## Turned down

- **Edits for everyone** (SerenityOS's `edits`): every `git` and `cargo` still asks, the F4 problem.
- **Full for everyone:** the engines' docs keep it for containers and VMs.
- **Ask rules in `--settings` for the guards:** can't express a folder boundary, and miss pushes written another way.
- **Dropping the project setting sources** to keep a project's rules out: Claude Code then doesn't read the project's `CLAUDE.md`, and the trust gate already keeps its allow rules out.

## Limits

- On Windows Claude Code runs shell commands without a sandbox, so a command (not a file tool) can still write outside the worktree, and auto mode's classifier is the only check on it. Codex's sandbox enforces the workspace on Windows (its docs; not run, no driver yet).
- Writes to Claude Code's own scratch folder outside the worktree also ask.
