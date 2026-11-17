# 0069: Deleting a project or an agent

Status: accepted (owner, 2026-11-17: the `remove` mock-up, cases 1 to 9, option A in each of its three choices, and the confirm's wording). Date: 2026-11-17.

Built: partly. Proof: `crates/gathr/tests/delete.rs`, `ui/src/deleting.test.ts`, `ui/src/deletes.test.tsx`, `ui/src/settings/Agents.test.tsx`, `ui/src/settings/Projects.test.tsx`. Not built: a removed agent's face with no mark and its card saying Removed (a later window change).

Amends [0034](0034-agent-folder.md) (what Gathr removes) and [0028](0028-team-chats.md) (an agent in a chat is no longer refused). Serves promise 7 of [the charter](../charter.md#the-promises), part 7 of [the truth map](../truth-map.md): nothing Gathr made is left behind that nobody can find, and nothing the person keeps is lost.

A project's **only agent** is the one agent of its team. A chat's only agent is the one agent of its members, or, in a chat with no members, the one agent its turns ran as. The **lead** is the Supervisor.

## The choice

- **A project is deleted with its chats, at once.** `DELETE /api/projects/<id>` deletes each of its chats, archived ones too, through a chat's own delete: archived first, its engines closed the gentle way, its rows, worktrees, hooks and task files removed, its branches kept. Then, in one transaction, the project's rows (team, checks, allowed commands, check runs and their steps, caches record) go and its own folders are recorded as due. No Undo, as for a chat.
- **What stays on disk: the person's checkout and Shared.** In `<agent folder>/<Project>`, Gathr removes each agent's folder (`<Agent>`, named by any agent's folder name, a removed one's too), its checks checkout (`checks`, unregistered from the project's git) and its caches (`caches`). It never removes `Shared`, the project folder itself, a path that is or holds the person's checkout, or one that leads into Gathr's data folder. A folder that can't go yet is named in the answer's `left` with its reference, and the sweep (0034) tries it again at each look until it's gone. Branches and open PRs stay.
- **An agent is deleted wherever it is.** `DELETE /api/agents/<id>`, in this order:
  1. Its work stops as Stop stops it: its queued turns are stopped, its running turn is interrupted, and its engine closes the gentle way. An engine that may be renewing its login is held, never killed (0010).
  2. In one transaction: its open tasks (every state but done and cancelled) are cancelled with why, `its agent was deleted`, with what follows them; its waits end; its pauses go; it leaves every chat and project team it isn't the only agent of; where it led, the next agent in the team's order leads (the rule Settings > Projects already has, now the service's); the agent is marked removed and lets go of its account.
  3. A project whose only agent it was is deleted as above, with its chats; a chat whose only agent it was is deleted, in a project or not.
- **A check run under way isn't ended.** Gathr has no way to stop a run once started, and none is built for this: the run's results go with the project, and its checkout, in use, is left and goes at a later sweep.
- **A removed agent keeps its row.** Its id and name stay, so its old messages still say who wrote them; it has no account; every list, picker and team leaves it out; a turn, task, seat, pause, restart or change for it is refused as for an agent that doesn't exist; its name is free for a new agent.
- **Nobody hears back for a removed agent.** A task it gave settles as before, but its hear-back starts no turn: the tasks count as heard and `gathr.task.hear_back` says `its giver was deleted`. The person's questions it asked are cancelled with its tasks. Handing its hear-backs to the new lead was turned down for now: a hear-back is found from the giver's own turn and speaks of "the task you gave", so moving it means new words and a new way to find it. `GET /api/agents` answers the removed ones apart, as `removed: [{id, name}]`, for the names of old messages.
- **The confirm's words are true.** `GET /api/projects/<id>/delete` and `GET /api/agents/<id>/delete` answer what the delete would do, in one shape: `{chats, tasks, projects, leads}`. `chats` and `projects` count what is deleted, `tasks` the agent's open tasks cancelled in chats that stay (0 for a project), `leads` the names of the agents who become lead. The delete answers the same fields, with `deleted: true` and `left`.
- **A second try finishes it.** A delete that fails half way answers with its reference and leaves what it hadn't reached. Sent again, it carries on: a project's chats already gone stay gone; a removed agent's delete deletes what it was still the only agent of. Folders left are the sweep's.
- **Recorded.** `gathr.project.delete` (the folders due) and `gathr.project.files` (ok, or `left` with why), `gathr.agent.change` with `change: remove` and its counts (an error, `delete` or `left`, with why when it didn't finish), each chat's own delete events, and the sweep's `gathr.cleanup.skip` and `gathr.cleanup.remove` for each folder, on the delete's trace or the folder's own reference.

## Why

- The owner's case: a chat deleted left its project with no chat and a folder gone, and its agent behind. Deleting a project or an agent has to end in one place, from one confirm that says what goes.
- One step that says what it ends is one confirm; archiving every chat first is four confirms for the same result.
- Shared holds the team's plans, reviews and evidence: nobody can make it again. Each agent's folder holds worktrees (their branches stay) and temporary files; the checks checkout and caches are git's and a cache's.
- A team and a chat always have 1 to 5 agents (0013). Taking out the only agent would make a state with none that every screen would have to handle, so what it was alone in goes with it.
- Keeping the row keeps the name for its old messages at the cost of one migration, which rebuilds `agents` so a removed agent lets go of its account and its name, and a filter on each list; copying the name into every message touches every message and every reader of them.
- Recording the project's folders as due reuses the sweep's guards (in use, leads into the data folder) and its retries across restarts, instead of a second remover.

## Turned down

- **Refusing a project delete until its chats are archived or deleted** (mock-up option B): four confirms, and the same result.
- **Removing Shared too:** it is the one part nobody can make again.
- **An agent's delete refused while it is in a chat or project** (as before, from 0028): the person had to take it out of each by hand, and its project's only agent couldn't be taken out at all.
- **Copying a deleted agent's name into its messages:** more to write and read for the same name.
- **A project or chat left with no agents** (mock-up option B for the only agent): a state no screen handles.
- **Undo:** a chat's delete has none, and a deleted project's folders are gone.

## Changes to other decisions

- [0034](0034-agent-folder.md): "Gathr only ever removes what it recorded" holds for a chat. A deleted project's agents' folders, checks checkout and caches go whole, Shared stays, and each is recorded before it is removed.
- [0028](0028-team-chats.md): "An agent in a chat isn't deleted (409)" goes. An account still isn't removed while an agent that isn't removed uses it.
