# 0029. Approvals: what an agent may do without asking

Status: accepted (slice 2, the owner's pick of "Auto, with guards"). Date: 2026-11-08.

## Choice

- **A mode per agent:** `approvals` on `/api/agents`, one of `ask`, `edits`, `auto` (the default, for new agents and every agent before this) or `full`, set in Settings > Agents. A change closes the agent's engine gently before its next turn, like a model change (0026).
- **Each maps to Claude Code's own permission mode** (`--permission-mode`): `ask` is Manual (`default`: everything but reading and read-only commands asks), `edits` is `acceptEdits` (file edits and `mkdir`/`mv`/`cp` inside the folder it runs in), `auto` is `auto` (a classifier model checks each action), `full` is `bypassPermissions`. Codex maps when its driver lands: `workspace-write` with `on-request`, and `danger-full-access` with `never` for `full`.
- **Gathr's own asks, in every mode but `full`:** Claude Code calls Gathr's hook (an HTTP `PreToolUse` hook at `/hook`, given in `--settings`) before each `Write`, `Edit`, `NotebookEdit`, `Bash` and `PowerShell` call. It answers "ask" for a file write outside the engine's folder (its worktree in a project's chat, 0028) and for a command that reads as a `git … push` (`git -C <dir> push` too). Claude Code then sends the call to Gathr as an approval with the hook's reason, whatever the mode or a project's rules allow. The hook takes only the engine's own token (0005), and `onFailure: "block"`: a Gathr that can't answer lets neither through. Each ask is a `gathr.approvals.ask` event on the turn's trace.
- **The folder is compared as the disk resolves it:** every part of the path that exists is resolved with its links followed (symlinks, Windows junctions), both before and after `..` is worked out (Windows works it out by the text first, macOS after the link), and a link that leads nowhere yet asks. So a junction in the worktree to another folder asks, and a worktree reached through a link (macOS's `/var` is `/private/var`) doesn't.
- **Every push waits for the person in git itself:** each agent's worktree gets a `pre-push` hook, set for that worktree alone (`extensions.worktreeConfig` on the project, then `core.hooksPath` in the worktree's own config), so the project's checkout and other worktrees keep their hooks. It asks Gathr at `/hook/push` with the engine's token (`GATHR_AGENT_TOKEN`, `GATHR_PUSH_URL`) and the push waits for the person's answer as an approval ("Allow git push?"), whichever command, alias, script or `sh -c` ran it. In Full it's allowed at once, and so is a push right after the person allowed the command that runs it, so a plain `git push` asks once. With no turn of the agent's running in the chat, no token, or a service stopping, it's refused. The project's own hooks folder is kept as `gathr.hooks`, and Gathr's runs the project's hook of each name after its own part. Each push is a `gathr.approvals.push` event, its answer `gathr.approvals.push_answer`.
- **A wait's check asks once, when it's set** (0010): Gathr runs its command itself, outside Claude Code's permission checks and auto mode's classifier, every few minutes until it exits 0. So the hook also matches Gathr's `wait` tool (`mcp__gathr__wait`) and answers "ask" when the call has a `check`: the call reaches the person as an approval with the command in its input, in every mode but `full`, as SerenityOS's checks are asked through the agent's own command. Refused, the wait isn't left. Allowed, it's saved and runs from then on, after a restart too, without asking again. It runs in the agent's own folder (its worktree in a project's chat), never the service's; a wait with no folder is refused.
- **Codex wait checks are gated at the service boundary.** Codex has no Claude PreToolUse hook.
  Before a `wait` with `check` is saved, the active turn shows the command as a `wait check`
  approval in Ask, Edits and Auto; Full admits it immediately. Refusal or shutdown saves no wait.
  It shares the push approval's question/response path, but an allowed push command never admits
  a wait check. Claude retains its native hook, avoiding a second ask and its 60-second MCP limit.
  Codex's configured MCP timeout is one hour; if its caller times out, the question is no longer
  answerable and no wait is saved. Once approved and saved, normal wait persistence applies.
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
- **In auto mode, asking only for a check that reads as a push or a write outside the worktree:** for an agent's own commands Gathr reads only pushes; writes made by a command are left to auto mode's classifier, which a check never passes through. Reading a shell command for every way it can write (redirects, `tee`, `cp`, a script, `python -c`) has the same gaps as the push patterns below. One question when the wait is set costs little; a check that runs unread every few minutes for a day doesn't.
- **The `wait` tool asking the person itself, while its call waits:** Claude Code gives up on an MCP call after 60 s ("The operation timed out", seen live on 2.1.319), so a person who took longer could never allow it. Its own approval waits as long as the person takes (allowed after 150 s, live).
- **More patterns in the command check** for pushes (aliases, quoting, line continuations): a shell has too many ways to run git (an alias set by an earlier command, a script, `sh -c`, a Makefile). The `pre-push` hook sees every one; the command check stays only to ask before the command starts.
- **A `pre-push` hook in the project's shared hooks folder:** it would run in the person's own checkout too.

## Limits

- **The folder guard covers Gathr's file tools, not every shell command.** On Windows Claude Code runs shell commands without a sandbox, so a command can still write outside the worktree, and auto mode's classifier is the only check on it. Codex's sandbox enforces the workspace on Windows (its docs; not run, no driver yet). The push hook doesn't share this limit: it's git that asks.
- **Routes past the push hook:** `git push --no-verify` and `git -c core.hooksPath=…` skip it (the command check still asks when the command names them plainly; an alias hides them); so does a command that edits the worktree's git config or Gathr's hooks first (outside the worktree: a file tool asks, a shell command doesn't), and a push that isn't `git push` at all (`git send-pack`, `gh`, an API call, another git program). Auto mode's classifier is the check left on those.
- A push that waits longer than the command's own time limit (Claude Code's Bash has one, `BASH_DEFAULT_TIMEOUT_MS`; not measured here) is ended by Claude Code: the push doesn't happen, and its approval left in the chat answers nothing (it's gone when the chat opens again). The command check usually asks first, before the command starts, so this hits only a push it didn't see.
- Writes to Claude Code's own scratch folder outside the worktree also ask.
