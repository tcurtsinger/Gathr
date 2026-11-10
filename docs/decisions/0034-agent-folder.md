# 0034. The agent folder: where agents' work lives, and who cleans it up

Status: accepted (owner, 2026-11-09: "One shared project folder, then the agent folders all in the
same place is very ideal"). Date: 2026-11-09. Amends 0028's worktree place and 0029's folder guard.

## Choice

- **One folder for all agents' work,** set in Settings > General as the agent folder:
  `GET /api/settings` answers `agent_folder` (a full path), `PATCH /api/settings {"agent_folder"}`
  saves it. Unset, it's the home folder's `Gathr` (`~/Gathr`) for the system's own data folder only;
  any other data folder (`--data`: a test, a check, `gathr dev`, a window opened on a test folder)
  keeps its agents beside it, `<data>-agents`, so every test folder keeps to itself. A path that isn't full, that Gathr's
  data folder holds or is in, or that can't be made or written is refused with a reference
  (`gathr.settings.change`). A change applies to new work: what was made stays where it is.
- **Layout:** `<agent folder>/<Project>/Shared` for the team's kept files (plans, reviews, evidence),
  and `<agent folder>/<Project>/<Agent>` for each agent: a worktree per chat
  (`<Agent>/<chat>`, the chat's last six id characters, as in its branch) and its engine's
  temporary files in that chat (`<Agent>/temp/chat-<chat>`). A chat with no project
  keeps its work in `<agent folder>/Chats`, as a project named Chats would, in a plain folder.
- **Names** are the project's and agent's, made safe for both disks: `<>:"/\|?*` and control
  characters become `-`, a device name Windows keeps (`CON`, `NUL`, `COM1`…) gets `_` after its
  stem, leading dots and trailing dots or spaces go, 40 characters at most. Unique ignoring case
  among projects (with `Chats` kept) and among agents (with `Shared` kept), and never a folder
  already there: `Orin 2`. Each is saved the first time it's needed (`folder_name`), so a rename
  never moves work.
- **Each folder Gathr makes is recorded before it's made** (`agent_folders`: path, kind, chat,
  agent, project), and an engine's start event names its `cwd`, `gathr.folder.agent`,
  `gathr.folder.shared` and `gathr.folder.scratch`. A chat-agent's worktree is read from its row,
  never worked out again. A worktree from before (`<data>/worktrees/<chat>/<agent>`) or a plain
  chat's folder from before (`<data>/chats/<chat>`) stays where it is for an agent with a session
  in that chat: a session resumes only in the folder it was made in.
- **What agents may write without asking:** the folder their engine runs in, their own folder and
  their project's Shared. Gathr's hook (0029) allows writes in all three and still asks anywhere
  else; Claude Code gets `--add-dir=<own>` and `--add-dir=<Shared>`; Codex gets both as its
  sandbox's writable roots (`config.sandbox_workspace_write.writable_roots` on `thread/start` and
  `thread/resume`). Gathr's data folder (logins, database, engine copies) is never one of them, and
  the setting refuses a folder that holds it. Whatever those folders say, the hook asks before a
  write that lands in the data folder, links followed, so a link made later in Shared or the
  agent's own folder never opens it. The one exception is an engine's own folder from before the
  agent folder, recognised by where it is, never by being in the data folder:
  `<data>/worktrees/<chat>/<agent>` or `<data>/chats/<chat>` for that chat and agent; it stays
  writable, and nothing else is ever added beside it. Each turn reads the agent folder's real path
  first: one that holds the data folder or leads into it (a link made after it was saved) is
  refused before anything is made or recorded, and the turn fails with `gathr.folder.refused` and
  its reference; Settings still shows it, and saving it again is refused. A Shared or own folder
  that leads into the data folder isn't passed to the engine (`--add-dir`, Codex's writable roots),
  a turn whose worktree or temp folder would land there fails the same way, and a sweep never
  removes a folder that now leads there (`gathr.cleanup.skip`, `in_data`).
- **One name for each folder:** Gathr resolves the agent folder as the disk names it (links
  followed, macOS's `/var` as `/private/var`, without Windows' `\\?\` prefix) before it makes
  anything in it, so the engine, its sandbox and Gathr's own checks all see the same paths.
- **Temporary files per engine:** one folder per agent per chat, `<Agent>/temp/chat-<chat>`, set as
  the engine's `TEMP`, `TMP` and `TMPDIR` when it starts. It lives as long as that engine: every
  task and message the engine runs in the chat shares it, and nothing restarts an engine to change
  it.
- **Gathr cleans up, never the agents; Archive removes only what Restore brings back:** an
  engine's temporary files go once it has closed (idle close, a move to another chat, account or
  model, archive, delete; at shutdown, at the next start's first sweep). Archiving removes a chat's
  worktrees: their branches stay, and a restored chat's next turn checks each out again where it
  was, with what was committed on it. A chat's folder outside a project has no branch to come back
  from, so it stays until the chat is deleted; deleting removes everything. A sweep runs with the
  service's looks (every 2 minutes, 0010), and at once on archive and delete. It never removes a
  folder a running engine works or keeps its temporary files in, or one a wait check runs in, and
  while a chat is only archived, never a worktree with changes not committed (files git ignores
  aren't changes: they go with it). Each such folder is skipped with an error event
  (`gathr.cleanup.skip`: `in_use`, `changes`, `in_data`, or `left` when the disk refuses),
  recorded once per reason with the reference its removal later shares (`gathr.cleanup.remove`),
  and tried again at the next sweep. Restore waits for a removal already under way, and each removal reads again,
  right before it starts, whether its chat is still archived: a chat restored while a sweep looks
  at it keeps what's there.

## Why

- The owner's own room keeps plans, reviews and evidence in a folder next to the repo, and each
  agent's trees and test data in folders of its own. One root with a folder per project makes that
  the default for every Gathr user, instead of each project listing its extra folders.
- Worktrees in the data folder put agents' work beside logins and the database, and every write to
  the temp folder asked (0029's limits). Moving both under the agent's own folder makes the safe
  folders and the work the same place.
- Names that read like the project and agent let the person find the work in a file manager; saving
  them keeps sessions and worktrees where they were after a rename.
- Agents don't clean up after themselves reliably (SerenityOS's helpers left worktrees and test data
  behind), and an agent deleting folders is a risk of its own. Gathr knows when an engine closes or
  a chat ends, and records what it made, so it removes only that.
- Temporary files that live as long as the engine need no restart: a restart only to change `TEMP`
  would cost a start on every task switch, lose the provider's prompt cache and end the engine's
  background commands. The engine closes on its own after 10 idle minutes, so its files don't
  outlive the work by long.
- Recording each folder before it's made (Vale's point) means a folder Gathr didn't make is never
  removed, and a removal that has to wait can find the folder again after a restart.

## Turned down

- **A list of extra folders per project** (the earlier move-in plan): every project needs setting up,
  and plans and evidence end up wherever each list points.
- **Agents cleaning up themselves:** unreliable, and it hands agents deletion outside their task.
- **Removing a worktree when its task's PR merges:** a worktree belongs to the chat and agent, not
  the task; the agent's next task in the chat carries on in it, on the same branch. Archive and
  delete cover it.
- **A temporary folder per task:** an engine's environment is fixed when it starts, so each task
  switch would close and start the engine again, losing its prompt cache and background commands.
- **Removing an archived chat's worktree with changes not committed:** archiving is undoable,
  deleting work isn't. Delete removes it, as before.
- **Removing a chat's folder outside a project on archive:** nothing could bring it back on
  Restore, and it holds what the person asked for.
- **Keeping every worktree until Delete:** a worktree with nothing uncommitted comes back whole
  from its branch, and archived chats' worktrees (with their builds) would fill the disk.
- **Restore waiting for a whole sweep:** a sweep waits on `git status` for each worktree, which
  can be slow; Restore waits only for one folder's removal.
- **`~/Gathr` as the default for every data folder:** a window opened with `--data <test folder>` and
  no agent folder set would put that folder's agents in the person's real one.
- **A `GATHR_*` variable naming the default for tests and dev:** an installed Gathr never reads
  those, so a test folder opened in the installed window would still miss it. The data folder
  alone decides, everywhere.

## Limits

- **A process Gathr didn't start** (a dev server an agent left running) is seen only when the disk
  refuses the removal: on Windows it does, so the folder is skipped (`left`) and tried again; on
  macOS a folder in use is removed anyway.
- **Temporary files are shared by the engine's tasks in a chat:** what one task leaves there is
  still there for the next until the engine closes.
- **Turns sent to no agent** have no agent folder: they run in `<data>/chats/<chat>` as before, with
  no temporary folder of their own.
- **Shell commands still write anywhere on Windows** in Claude Code (0029's limits); the hook covers
  its file tools. Codex's sandbox enforces the workspace: Juno's live M2 on Gathr's Codex 0.167.0
  saw a `workspace-write` turn blocked writing outside its worktree. An earlier `command/exec` probe
  here wrote outside its roots, but it ran the person's own Codex 0.166.0 install, not Gathr's copy,
  and `command/exec` isn't a turn, so its result doesn't apply to turns. The person's own Codex is
  never run for checks again.
- **A name chosen under one agent folder** is reused under a new one, even if a folder of that name
  is already there.

## Checked

- `claude --help` (2.1.322) lists `--add-dir <directories...>`; 2.1.318 took `--add-dir=<dir>` twice
  without complaint, signed out. Its effect on a signed-in turn isn't checked here.
- `codex app-server` 0.166.0 (the person's own install, run once with a scratch `CODEX_HOME`; not
  to be repeated), signed out: `thread/start` with
  `config: {"sandbox_workspace_write": {"writable_roots": [...]}}` (or the dotted key) answered its
  sandbox as `workspaceWrite` with those `writableRoots`.
