# 0034. The agent folder: where agents' work lives, and who cleans it up

Status: accepted (owner, 2026-11-09: "One shared project folder, then the agent folders all in the
same place is very ideal"). Date: 2026-11-09. Amends 0028's worktree place and 0029's folder guard.

## Choice

- **One folder for all agents' work,** set in Settings > General as the agent folder:
  `GET /api/settings` answers `agent_folder` (a full path), `PATCH /api/settings {"agent_folder"}`
  saves it. Unset, it's the home folder's `Gathr` (`~/Gathr`). A path that isn't full, that Gathr's
  data folder holds or is in, or that can't be made or written is refused with a reference
  (`gathr.settings.change`). A change applies to new work: what was made stays where it is.
- **Layout:** `<agent folder>/<Project>/Shared` for the team's kept files (plans, reviews, evidence),
  and `<agent folder>/<Project>/<Agent>` for each agent: a worktree per chat
  (`<Agent>/<chat>`, the chat's last six id characters, as in its branch) and its temporary files
  (`<Agent>/temp/task-<n>`, or `temp/chat-<chat>` for a turn on no task). A chat with no project
  keeps its work in `<agent folder>/Chats`, as a project named Chats would, in a plain folder.
- **Names** are the project's and agent's, made safe for both disks: `<>:"/\|?*` and control
  characters become `-`, a device name Windows keeps (`CON`, `NUL`, `COM1`…) gets `_` after its
  stem, leading dots and trailing dots or spaces go, 40 characters at most. Unique ignoring case
  among projects (with `Chats` kept) and among agents (with `Shared` kept), and never a folder
  already there: `Orin 2`. Each is saved the first time it's needed (`folder_name`), so a rename
  never moves work.
- **Each folder Gathr makes is recorded before it's made** (`agent_folders`: path, kind, chat,
  agent, task, project), and an engine's start event names its `cwd`, `gathr.folder.agent`,
  `gathr.folder.shared` and `gathr.folder.scratch`. A chat-agent's worktree is read from its row,
  never worked out again. A worktree from before (`<data>/worktrees/<chat>/<agent>`) or a plain
  chat's folder from before (`<data>/chats/<chat>`) stays where it is for an agent with a session
  in that chat: a session resumes only in the folder it was made in.
- **What agents may write without asking:** the folder their engine runs in, their own folder and
  their project's Shared. Gathr's hook (0029) allows writes in all three and still asks anywhere
  else; Claude Code gets `--add-dir=<own>` and `--add-dir=<Shared>`; Codex gets both as its
  sandbox's writable roots (`config.sandbox_workspace_write.writable_roots` on `thread/start` and
  `thread/resume`). Gathr's data folder (logins, database, engine copies) is never one of them, and
  the setting refuses a folder that holds it.
- **Temporary files per task:** each engine gets `TEMP`, `TMP` and `TMPDIR` pointed at its task's
  `temp` folder. An engine's environment is set when it starts, so a turn on another task closes
  the engine the gentle way first and starts it again with that task's folder; a turn on no task
  (the person's message) keeps the engine as it is.
- **Gathr cleans up, never the agents:** a task's temporary files go once the task is done or
  cancelled; a chat's worktrees, folders and temporary files once it's archived or deleted. Branches
  stay. A sweep runs with the service's looks (every 2 minutes, 0010), and at once on archive and
  delete. It never removes a folder a running engine works or keeps its temporary files in, or one a
  wait check runs in, and while a chat is only archived, never a worktree with changes not
  committed. Each such folder is skipped with an error event (`gathr.cleanup.skip`: `in_use`,
  `changes`, or `left` when the disk refuses), recorded once per reason with the reference its
  removal later shares (`gathr.cleanup.remove`), and tried again at the next sweep. Restoring an
  archived chat keeps what's still there; a removed worktree is made again from its branch at the
  agent's next turn.

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
  behind), and an agent deleting folders is a risk of its own. Gathr knows when a task or chat ends,
  and records what it made, so it removes only that.
- Recording each folder before it's made (Vale's point) means a folder Gathr didn't make is never
  removed, and a removal that has to wait can find the folder again after a restart.

## Turned down

- **A list of extra folders per project** (the earlier move-in plan): every project needs setting up,
  and plans and evidence end up wherever each list points.
- **Agents cleaning up themselves:** unreliable, and it hands agents deletion outside their task.
- **Removing a worktree when its task's PR merges:** a worktree belongs to the chat and agent, not
  the task; the agent's next task in the chat carries on in it, on the same branch. Archive and
  delete cover it.
- **One temporary folder per engine, not per task:** no restarts, but a task's files would stay
  until the engine closed or the chat went, mixed with the next task's.
- **Removing an archived chat's worktree with changes not committed:** archiving is undoable,
  deleting work isn't. Delete removes it, as before.
- **The data folder as the default for tests and dev:** `GATHR_AGENT_FOLDER` names the default in
  dev and tests only; `gathr dev` and the tests put it beside the data folder (`<data>-agents`), so a
  check never writes in the person's own `~/Gathr`.

## Limits

- **A process Gathr didn't start** (a dev server an agent left running) is seen only when the disk
  refuses the removal: on Windows it does, so the folder is skipped (`left`) and tried again; on
  macOS a folder in use is removed anyway.
- **A turn on another task restarts the engine,** which ends its background commands, as moving to
  another chat already does.
- **Turns sent to no agent** have no agent folder: they run in `<data>/chats/<chat>` as before, with
  no temporary folder of their own.
- **Shell commands still write anywhere on Windows** in Claude Code (0029's limits); the hook covers
  its file tools. Codex's sandbox enforces its writable roots; a check here with `command/exec` on
  0.166.0 wrote outside them too, so that wasn't confirmed on this PC.
- **A name chosen under one agent folder** is reused under a new one, even if a folder of that name
  is already there.

## Checked

- `claude --help` (2.1.322) lists `--add-dir <directories...>`; 2.1.318 took `--add-dir=<dir>` twice
  without complaint, signed out. Its effect on a signed-in turn isn't checked here.
- `codex app-server` 0.166.0, signed out: `thread/start` with
  `config: {"sandbox_workspace_write": {"writable_roots": [...]}}` (or the dotted key) answered its
  sandbox as `workspaceWrite` with those `writableRoots`.
