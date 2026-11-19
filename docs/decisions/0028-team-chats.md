# 0028. Chats for a team: routing, order, worktrees and PATH

Status: accepted (slice 2, F4). Date: 2026-11-08. Builds on [0013](0013-one-chat-model.md).

Built: yes. Proof: `crates/gathr/tests/team.rs`, `crates/gathr/tests/projects.rs`.

Out of date below: a worktree now lives in the agent folder ([0034](0034-agent-folder.md)); the screens exist; the two columns its order was drawn in are one timeline ([0037](0037-one-timeline.md)); an agent in a chat is deleted, not refused ([0069](0069-deleting-a-project-or-an-agent.md)).

## Choice

The service side of a chat with several agents, through the API until F8's screens.

- **Members.** `POST /api/chats {"members": [{"agent", "role"}], "project"}` makes a chat with its agents; `PATCH /api/chats/<id> {"members"}` replaces them as a whole. Roles are `supervisor`, `reviewer` and `member`. The whole list is checked at once against 0013's rules: 1 to 5 agents, each once, one Supervisor once there are two or more, at most one Reviewer and never on its own. Each change is a `gathr.chat.members` event. A chat made without members stays a plain chat, as before. An agent in a chat isn't deleted (409), as an account with agents isn't.
- **Routing.** A message sent to an agent goes to it. Otherwise the Supervisor, or the only agent. The person's text is never read for names (C24, 2026-11-17): a name counts only when picked from the window's `@` list, which sends that agent. There is no Everyone: one message is one turn for one agent (the owner's call, 2026-11-08). Agents address each other through Gathr's tools (F5), never by parsed text: the `message` tool's `to` still matches a member's name as before.
- **Picking an agent (F8b).** In a project chat, the agent must be one of the chat's. In a chat with no project, picking an agent from outside adds it, up to five: the chat's agent so far becomes its Supervisor and the new one a member, so unnamed messages keep going where they went. A chat that has run on one agent keeps no list until a second joins. Each join is a `gathr.chat.members` event and goes out live as `{"type": "members"}`, as a `PATCH` does; the Supervisor is changed with `PATCH`.
- **A name counts only when picked (C24, 2026-11-17).** The message box has no picker. Its hint names who gets a message that names nobody ("Message Mira": the Supervisor, else the chat's agent so far, else the first agent), and typing `@` at a word's start lists who can be named: the chat's agents, and in a chat with no project every other agent too, under "Bring in". A name picked from the list (Enter, Tab or a click, or the panel's Message) is a range of the text the box keeps, with its agent, through typing before and after it, pasting and a draft kept while another chat is open; an edit inside it makes it plain text. Only picked names are blue and count. The window always sends an agent's id: the first picked agent in the text, else the hint's agent; one picked from outside a chat with no project joins it. By Send, "To Ann" or "Bo joins" says which. A typed `@Ann` with nothing picked is plain text and goes where an unnamed message goes. The window sends every picked name with the message too (`picks`: where each `@Name` stands, in UTF-16 units, and whose it is, the owner's call of 2026-11-18), and the service keeps them on the message, so the sent message draws exactly those names blue ([0072](0072-the-chat-reads-like-a-team-chat.md)). Picks route nothing: a second picked name gets no turn and brings no agent in.
- **Projects (F8b).** A project is a saved record: a name (unique ignoring case), a git folder checked as above, and a team checked by the same rules. `POST /api/chats {"project"}` copies its folder and team into the new chat, and nothing else: no messages, tasks, sessions or worktrees (the new-chat leak). After that the chat's own folder and members count; changing either never changes the project, and a project's later team only reaches chats made after.
- **Assignment links (F8b).** Each turn Gathr starts carries `answers`, the turn it answers: a task's turn and a hear-back answer the turn that gave the task, a wake-up the turn that set the wait. These links remain recorded when 0015's split team conversation replaces the inline presentation.
- **Each message's agent.** Every message carries `agent`: who it was for, or who wrote it (its turn's agent).
- **What each turn is about (2026-11-18, [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md)).** Each
  turn, each task and each line said in no turn carries `topic`: the main message it sits under, worked out by the
  service from these links and three facts it now records where they happen (a result, a landing's turn, what a
  line is about). The links above are still what it reads.
- **Order.** A turn takes the chat's next `position` when it leaves the wait (starts, or fails before starting). Messages are shown in their turn's position, then in time. Replies from agents working at once never interleave; a turn not started yet shows last.
- **Project folder.** Set only when the chat is made (0013), from its project, saved as its git top folder. A path that isn't in a git repository is refused.
- **A worktree per agent.** The first time an agent's engine starts in a project chat, it gets `<data>/worktrees/<chat>/<agent>`, on a branch of its own (`gathr/<name>-<chat>-<agent id>`: two names can read the same in a branch), from the remote's default branch (`origin/HEAD`) fetched then; with no remote, from the project's `HEAD`. A failed fetch starts it from what was fetched last and says so on the `gathr.worktree.add` event. The engine runs there. A change to the chat's roles closes an agent's engine gently before its next turn, so its briefing names the new ones. A worktree is removed when its chat is archived (its branch stays, and a restored chat checks it out again) or deleted, and once its PR has merged or closed ([0034](0034-agent-folder.md), [0043](0043-checks-and-merge.md)); until then its work is the agent's. Adapted from the owner's SerenityOS `worktrees.ts`.
- **The person's PATH (G23).** Each engine and each git command gets the environment a new sign-in would give: on Windows the machine's and the person's variables read again from the registry (`reg export`, which writes UTF-16), on macOS the login shell's PATH (`-ilc`). The service's own PATH entries they lack follow. The engine's start event says where it came from (`gathr.env.from`) and why not, if it fell back.

## Why

- Members as one list, checked whole, means no order of single changes can pass through a team that breaks the rules (removing the Supervisor before naming the next).
- A name is picked, as `@` and `/` work in other apps' message boxes (the owner, 2026-11-17: "For my @ to register I need to select an agent from the list. If I just type a normal @Ann without selecting anything then it just registers as a normal message"). The first picked name is how the person addresses someone: one addressee keeps one message one turn.
- Ordering by when a turn started is Meldwork's answer to several streams in one chat (slice 2 plan); ordering by time interleaves two agents' blocks.
- Worktrees made at the agent's first turn, not when it joins, keep a slow or failing fetch out of the request that makes the chat, and the failure lands on a turn with a reference.
- A process keeps the environment it started with. Gathr runs for days in the tray, so a tool installed since is missing from its agents' PATH; that's what happened to the room in SerenityOS (G23). Windows' own Explorer reads the registry again for the same reason.

- A project is what the person sets up once; copying it into each chat keeps every rule above (worktrees, roles, routing) on the chat, where it already lives.
- The agent a plain chat already runs on is the one the person has been talking to, so it keeps the unnamed messages when a second agent joins.

- No picker (the owner, 2026-11-16): a picker and a typed `@Ann` could disagree, and the picker won. With names picked from the list, what the box shows (blue, "To Ann") is what is sent.

## Turned down

- **A recipient picker by Send.** It sent its agent with every message, so an `@Name` in the text did nothing (C24).
- **Reading names from the text** (this decision until C24). An email address named an agent (`bob@ann.com` went to Ann), a name mentioned in passing addressed the message, and the window had to guess what the service would read. A typed name now stays text.
- **Everyone.** Every member answering the same message, with a Supervisor wrap-up: several turns for one ask, and the person can't tell who is doing the work. The owner removed it.
- **A chat that follows its project's team live.** A change to the project would close and re-brief the engines of every running chat in it.
- **Asking for a Supervisor when a plain chat's second agent joins.** It stops the person mid-send; the agent already there is the right default, and `PATCH` changes it.
- **Every `@Name` gets a turn.** One message would be stored once per agent or split across turns; and a name mentioned in passing would start work nobody asked for.
- **Refusing a message that names several agents.** Mentioning a teammate is normal.
- **Order by when a turn was sent.** A queued turn's reply would appear above replies already shown.
- **A worktree when the agent joins.** Network work inside the API request, and a worktree for agents that never run.
- **`reg query`.** Its output's code page differs by computer, so a folder with an accented name could come back wrong.
- **PowerShell to read the variables.** About 200 ms per engine start against about 20 ms for `reg export`.
- **The project's own toolchain files** (`.node-version`, `rust-toolchain.toml`). rustup already reads `rust-toolchain.toml` once `cargo` is found; node version managers are left for when friction shows them.

## Amended: a reply's target (2026-11-18, 0076)

`POST /api/chats/{id}/messages` also takes `reply_to` (a text message of the chat; Gathr's copy of a message stands
for the message it carries) or `reply_task` (a task of the chat), never both, for a reply typed in a thread. The
service checks it before anything is saved and refuses anything else with why and a reference. The message keeps
it (`messages.reply_to`, `reply_task`, migration 0056); its turn serves the ask of what it answers, opens with the
`reply` kind's line before the person's words, and sits in that thread. Routing is unchanged: the window sends the
agent it shows by Send, the author of what is answered unless the person picks another.
