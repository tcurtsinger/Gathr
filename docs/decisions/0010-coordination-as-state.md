# 0010. Coordination is state in the service

Status: proposed. Date: 2026-11-06.

## Choice

The service owns all coordination between agents and the person as records with named states and fixed transitions: tasks, turns, waits, pauses, plans and their checks, hand-offs, side jobs and usage holds. An agent changes one only by calling a tool on the service's MCP server. The service checks who may make that transition, makes it, and writes the event (0011) in the same transaction.

Nothing is read out of what an agent says, and nothing is inferred from timing. "Nothing is running", "waiting on the reviewer since 14:02" and "this task is done" are facts the service holds, not claims an agent makes.

## Why

Lessons from the previous app, which coordinated agents by reading their words and guessing timing:
- Nearly every room fix in a month was the app reading words or timing better: a hand-off said mid-turn, a teammate named as "fixing it" who was never asked, the lead's last word given while work still ran.
- An agent said "nothing of mine is running"; six seconds later the app started its queued task, and the lead told the person everything had stopped. The start path checked nothing.
- With no pause, the only way to stop queued work was to close tasks. A closed task still started a turn, and reissuing lost the task numbers.
- After a restart, stopped tasks broke their `after` chains, and nothing could restart them.
- A wait on a check never fired for two hours, and nothing said so.
- A member's `task left` ignored the wait it had open, so the task came straight back.
- Nobody checked the lead's plan before the person saw it or members built it.
- A second plan in the same conversation replaced the first plan's card.
- Side jobs opened as separate chats nobody replied in.

When the state lives in one place with explicit transitions, each of these becomes a rule the service enforces and a test can prove.

## The scheduler gate

Every turn that starts without the person typing passes one function in the service, `may_start(agent, scope, reason)`. It answers start or hold, with the reason it held:
- the scope (app, chat, agent) is paused;
- the agent's account is out of usage until a reset (usage hold);
- the agent already has a turn running anywhere (one turn per agent at a time, see Sessions);
- the task's `after` tasks aren't all done;
- the plan it belongs to is still being checked.

A held start goes on the scope's held list, which is durable, and starts in order when its reason clears. Sources (task hand-out, waits firing, routines, nudges, restart carry-on) also check before queueing, but the gate is the backstop: a path that forgets is still caught, and the work isn't lost.

The previous app listed more than a dozen start paths that each needed their own check; one gate replaces them.

## Tasks

A task is work owed by one agent, or by the person.

Fields: number (one sequence for agents' and the person's tasks), giver, owner, chat, the turn that gave it (`given_in`), `what` (one line, at most 200 characters, no paths), `why`, files, `after` (task numbers), parent (the task the giving turn worked on), kind (`work`, `review`, `person`, `person_request`, `plan_check`), its PR, for a review the commit reviewed, and for decisions the choices with the giver's pick.

Each turn records the task it works on. Together with `given_in`, that's what puts a member's work under the message that asked for it (the inline thread, F8b), rather than under whatever message came just before.

F5 builds `work` and `review` tasks between agents. F9 adds the person's (`person`, owner none) and a member's request for one (`person_request`), each with how it's answered (`done`, `yesno`, `choice`, `choices`, `text`), its choices with the asker's pick first, and files copied when it was given. Plan checks come when a feature needs them.

F9 folds the planned `decision` kind into `person`: a to-do for the person isn't a decision, and one kind keeps one list; the form says how it's answered. Turned down: a kind per form, which would split the person's list and the rules on it five ways.

States:

| State | Means |
|---|---|
| `waiting` | Its `after` tasks aren't all done. Never nudged. |
| `ready` | Can start; queued behind the gate. |
| `working` | A turn of its owner's is on it. |
| `held` | Its owner's turn ended well but something it started is still open: a background command, a side job, a wait, a task for the person, work handed on, project checks. The reason is stored. |
| `done` | Settled, with the result. |
| `blocked` | The owner can't go on without a named member or the person, who then owes a task. |
| `failed`, `stopped` | A turn failed or was stopped; the person or the giver restarts it or gives it to someone else. |
| `cancelled` | Nobody is to do it any more. |

Transitions and who makes them:

| From | To | By |
|---|---|---|
| (new) | `waiting` or `ready` | the giver (`task --for`), the Supervisor for a plan's hand-out |
| `waiting` | `ready` | the service, when every `after` task is `done` |
| `waiting` | `blocked` | the service, when an `after` task fails, stops or is cancelled; the giver is told |
| `ready` | `working` | the service, through the gate |
| `working` | `done`, `held`, `blocked`, `ready` | the owner's turn ending: what it said with `finish_task` (`done`, `left`, `blocked`), applied once the turn has ended; with nothing said, `done` if nothing is open, else `held` |
| `held` | `done` or `ready` | the service, when what it held on ends |
| `working` | `failed`, `stopped` | the service, from the turn's outcome |
| `failed`, `stopped`, `blocked` | `ready` | the person, or the giver (`task restart`) |
| any open | `cancelled` | the person, or the giver |

Rules:
- A turn that ends well with something still open (a wait, a task given in its turns that it hasn't heard back on, a work task's PR) holds the task on it, and the reason says on what and whether its owner called it done: "done, waiting on CI on #54" reads apart from stalled work (G75). A wait firing, or the hear-back from the tasks it gave, wakes the owner on the task; its PR merging settles it as `done` without a turn, and its PR closing cancels it (G67).
- A review is `done` only with the commit it reviewed, and is done when its owner says so: its PR is what it reviewed, not work it still owes. A newer head on that PR marks it out of date until the PR merges or closes (G73).
- A turn on a task that fails or stops before it runs (no engine, no worktree) settles the task the same way, so its dependants move and it can be restarted.
- Every turn Gathr starts (a task's, a wake-up, a hear-back) is saved, queued with its ask, in the transaction of the change that calls for it, so a stop right after that commit loses nothing: the next start takes it over. A turn's status is saved in the same transaction as what it moves (its task, its giver's hear-back, waits on its agent), so no stop falls between a turn's end and the round it closes.
- `blocked` may name an existing task instead of filing another (G52): the task goes `ready` again once that one is `done`.
- `left` with nothing open starts another turn on the task, with what was left; twice in a row by turns that ran no tool is `blocked`.
- Giving a task answers with the chat's other open tasks and the project's open PRs, so a fix already under way shows (G74).
- A task that hasn't started survives a restart unchanged. A task that was working when the app stopped becomes `stopped` with a carry-on note, and its dependants stay `waiting`, not broken.
- Cancelling is not holding. To keep work from starting, pause (below).
- `left` twice in a row by turns that changed nothing and left nothing running is `blocked`, not a loop.
- `left` with an open wait stays `held` on the wait; the task isn't handed straight back.
- The giver hears back once on all the tasks it gave in one turn: when that turn has ended (it's done handing out work) and every one of them is settled. A turn cut off by a restart counts as ended. A task restarted after the hear-back brings another when it settles again.
- **Only the Supervisor gives the person tasks** ([0013](0013-one-chat-model.md)); in a one-agent chat, that agent. A member who needs the person's input asks the Supervisor: a `person_request` task to the Supervisor with the question, the options, the member's pick and why. The Supervisor checks it (is it really the person's to decide, is it already answered, is the pick sound), then files it for the person, folds it into one it's already asking, or answers it itself. The person's answer goes to the Supervisor, who passes it back to the member. A member never files one: Gathr's one tool for asking the person (`ask_person`) turns a member's ask into a `person_request` for the Supervisor, so no prompt has to teach the rule (turned down: refusing the member and making it ask again). The person answers several at once (the pager's Send); each answer is taken or refused on its own, so one that's wrong stays open with its reference while the rest go, and each asker hears back once on what that Send answered.
- A decision for the person is a `person` task with its choices, the Supervisor's pick first, and why. A design pick is one too, with its pages as files, so every pick waiting on the person is on their Tasks list (G70).
- A plan card that's built, sent back or replaced stops counting as waiting on the person.

## Turns

A turn is one run of an agent's engine on one conversation's session.

States: `starting`, `streaming`, `ended`, with an outcome: `ok`, `failed`, `stopped` (the person's Stop or Stop now), `interrupted` (the app or engine went away mid-turn).

- Stop always uses the engine's own interrupt, never a process kill: a kill can land while the engine is renewing its login, which spends the login (lesson from the previous app). Each engine's driver provides the interrupt ([0001](0001-engine-interface.md)).
- A turn records the background processes it started, by process id, so "still running" and Stop now can name them.

## Waits

An agent waits instead of looping. Kinds:
- `agent`: until another agent's turn has ended and nothing it left running still runs;
- `ci`: until a PR's or commit's checks have all finished (the service looks itself, through `gh`). A PR is followed to its newest commit and the wake-up says which (G49); a wait on a commit an open PR has moved past ends, saying so (G69);
- `check`: a command the service runs every few minutes in the agent's folder (Git Bash on Windows, `sh` on macOS), exit 0 when it has happened;
- `pr`: a built-in check for a PR merged or closed, so nobody writes one by hand;
- `all`: several of the above, firing once when all are met.

States: `open`, `fired` (its wake-up turn is queued, which is durable), `expired` (a day passed; the agent is woken anyway and told), or `cancelled`. The parts of an `all` wait are each `met` until the last, when the whole fires once.

Rules:
- A wait outlasts a restart.
- Any exit other than 0 is "not yet" (G69: an exit 2 read as broken). A look that can't run at all (the command or `gh` doesn't start or finish, GitHub refuses) is a failure; three in a row show on the wait, in `gathr diag health` and as a `gathr.wait.failing` event with its reference. The previous app's wait failed silently for two hours.
- Every call to GitHub is tried once more before it counts as failed (G5).
- An agent lists its own waits with their ids and ends any of them (G54). A wait on an agent with nothing running or queued in the chat is refused, saying so.
- A wait fires in the session it was left in.
- While its scope is paused, a wait doesn't fire; it fires on resume.

## Pause

Scopes: the whole app, one chat, one agent ([0013](0013-one-chat-model.md)). Each pause is a durable record: scope, since, by (person or Supervisor), and whether "nothing is running" has been said.

States per scope: `running`, `pausing` (paused, but turns or commands still run), `paused` (paused and quiet), back to `running` on resume.

- While paused, running turns finish; nothing new starts on its own (the gate holds it). On resume, held work starts in the order it was held.
- A message the person types into a paused scope starts only the agent it's for. What that turn gives out (tasks, @names, the check on its answer) is held until resume.
- `pausing` becomes `paused` when the service's own state shows no turn, check, side job or tracked background command in the scope. The service then says once: "Everything is paused. Nothing is running." Agents never say it.
- While `pausing`, the service lists what still runs and whose.
- Stop now (the person's alone): interrupts every turn in the scope, ends the background commands those turns started by the engine's own id (never by name), and holds a carry-on per agent naming what was ended. The scope stays paused as `stopped`; Restart is its resume.
- F10 builds the person's pauses. The Supervisor's chat pause, the hand-off limit's pause and holding notifications come with the features that need them.
- Who may pause: the whole app, the person only; a chat, the person or its Supervisor; an agent, the person. The Supervisor may resume only its own pause, or the person's once the person has written in the chat since.
- Notifications and sounds are held during an app-wide pause, except sign-in and security problems; resume shows one summary.

## Plans and plan checks

A plan is a record with its own file and versions. States: `drafting`, `checking`, `revising`, `waiting_on_person`, then `built`, `sent_back`, `replaced` or `dropped`.

- When the Supervisor finishes a plan in a chat with other agents, they check it before the person sees it. Checkers: the chat's Reviewer if one is set, else every free member. They read it against the code: wrong files, a missed caller, a missing step, a part given to the wrong member, a check that proves nothing. An agree is one line; an objection is a few lines with evidence.
- The Supervisor answers objections with a new version or a reason it stands. Only those who objected re-check a later version, and only their points.
- If the check stalls (a checker's turn fails, or 10 minutes pass), the plan reaches the person anyway, saying who didn't finish.
- On the person's card: who checked it and what changed because of it.
- Outside plan mode, a Supervisor's turn that gives two or more tasks, or names a plan file, is a plan: its tasks wait in `waiting` until the check is done.
- Each plan keeps its own record and file until it's built or dropped. A second plan never replaces the first.
- In plan mode the Supervisor may start read-only side jobs and write its own notes; changes to the project wait for Build.
- A member who finds the plan wrong for its part says so (`blocked`, with evidence) before building on it.

## Hand-offs

Naming a member (@Name) passes the turn and owes nothing; giving work is a task. Both are transitions made by tool calls at the end of a turn, never parsed from mid-turn words.

- Each hand-off records its chain: who passed to whom, under which ask. A chain longer than the chat's hand-off limit pauses the chat (a `pause` record by the service) instead of running on.
- A member answering a task it owes settles it; naming the giver doesn't.

## Side jobs and asking before guessing

A side job (`ask`) is a record: asker, helper, question, the asker's turn it came from, state `asked`, `working`, `answered`, `passed` (the helper can't help) or `failed`.

- It shows inside the asker's turn card, where it happened: the question, the helper's work, the answer. The chat's side panel lists them. The helper's session never appears in the chat lists; "Open as chat" is there when the person wants to step in.
- Every agent's briefing names its teammates with role and seniority, and the rule: ask the teammate who knows that area when about to build on something unchecked that can't be checked in a few minutes, or when the same thing failed twice. Not for what one read of the code answers.
- Asks, passes and whether the answer was used are events, so diagnostics can show whether asking helps.

## Usage holds

When an engine's account is out of usage until a time it gives, the turn isn't failed: it's held on that account and starts 30 seconds after the reset, oldest first, one turn per conversation. A turn cut off partway carries on on the same engine, never another, since it could repeat what it already did. Holds outlast a restart.

Built in F10: Claude Code says so with a `rate_limit_event` whose status is `rejected` and whose `resetsAt` is the reset (Unix seconds). A turn that ran no tool is sent again at the reset; one that ran a tool ends `stopped` and a carry-on asks the agent to check what already happened. Not yet seen live: no account here ran out.

## Background commands (F10)

Claude Code reports the commands it runs in the background as a whole list (`background_tasks_changed`), ends one with its own `stop_task` control request, and starts a turn by itself, with no input, when one ends (checked live on 2.1.319). Gathr keeps each under the turn that started it: its task stays `held` while one runs, a pause stays `pausing`, the engine isn't closed as idle, Stop now ends them by id (never a process kill), and the engine's own turn gets its row on that task. They end with their engine, so a start forgets them and a task held only on one carries on.

Turned down: tracking them by process id. Claude Code's frames carry no pid, and its own stop ends the command's whole tree.

## Sessions

An agent runs one turn at a time, across all its conversations, the way a member of the previous app got its next turn only when free. This also keeps each agent to one engine process on its account's shared login folder ([0002](0002-login-folders.md)), so fewer processes can renew one login at once.

- A task gets a fresh engine session when its owner already has another task in progress; the earlier session is set aside, whole, and resumed by a wait it set or a task it holds. At most three are set aside per agent per chat. The previous app's members carried 200K tokens a request when every task shared one session.
- Switching sessions: when the agent's next turn is in another session than its open engine process, the driver closes that process the gentle way (end of input, then the engine's own exit, per [0001](0001-engine-interface.md)) and starts the next with that session's resume id. It never kills a process to switch. A process that hasn't exited after 30 seconds holds the switch, shows as a hold on the agent, and is recorded; it's never forced while it might be renewing its login.
- An idle process (no turn for 10 minutes) is closed the same way, so a held process doesn't keep the folder.
- Codex's app-server runs several threads in one process; the one-turn-at-a-time rule applies to it too, so both engines behave alike.

## Turned down

- **Reading coordination from agents' words.** It's how the previous app started, and most of its room bugs came from it.
- **A file per room on disk.** The previous app kept tasks and waits in JSON files. In SQLite, the state change and its event commit together, and diagnostics can query across chats.
- **A workflow engine (Temporal and the like).** It needs its own server and adds a dependency for what a handful of tables and one gate do.
- **Agents polling in loops for waits.** A loop looks like work, dies with the app, and hides what it waits on.
- **Closing a task to hold it.** Closing loses the task and its number; pause is the hold.
