# 0028. Chats for a team: routing, order, worktrees and PATH

Status: accepted (slice 2, F4). Date: 2026-11-08. Builds on [0013](0013-one-chat-model.md).

## Choice

The service side of a chat with several agents, through the API until F8's screens.

- **Members.** `POST /api/chats {"members": [{"agent", "role"}], "project"}` makes a chat with its agents; `PATCH /api/chats/<id> {"members"}` replaces them as a whole. Roles are `supervisor`, `reviewer` and `member`. The whole list is checked at once against 0013's rules: 1 to 5 agents, each once, one Supervisor once there are two or more, at most one Reviewer and never on its own. Each change is a `gathr.chat.members` event. A chat made without members stays a plain chat, as before. An agent in a chat isn't deleted (409), as an account with agents isn't.
- **Routing.** A message sent to an agent goes to it, and it must be one of the chat's. Otherwise the first `@Name` of a member in the text picks the agent (ignoring case; `@Al` isn't in `@Alex`). Otherwise the Supervisor, or the only agent. Agents will address each other through a tool (F5), never by parsed text.
- **Each message's agent.** Every message carries `agent`: who it was for, or who wrote it (its turn's agent).
- **Order.** A turn takes the chat's next `position` when it leaves the wait (starts, or fails before starting). Messages are shown in their turn's position, then in time. Replies from agents working at once never interleave; a turn not started yet shows last.
- **Project folder.** Set only when the chat is made (0013), saved as its git top folder. A path that isn't in a git repository is refused.
- **A worktree per agent.** The first time an agent's engine starts in a project chat, it gets `<data>/worktrees/<chat>/<agent>`, on a branch of its own (`gathr/<name>-<chat>`), from the remote's default branch (`origin/HEAD`) fetched then; with no remote, from the project's `HEAD`. A failed fetch starts it from what was fetched last and says so on the `gathr.worktree.add` event. The engine runs there. Nothing removes a worktree: its work is the agent's until someone takes it. Adapted from the owner's SerenityOS `worktrees.ts`.
- **The person's PATH (G23).** Each engine and each git command gets the environment a new sign-in would give: on Windows the machine's and the person's variables read again from the registry (`reg export`, which writes UTF-16), on macOS the login shell's PATH (`-ilc`). The service's own PATH entries they lack follow. The engine's start event says where it came from (`gathr.env.from`) and why not, if it fell back.

## Why

- Members as one list, checked whole, means no order of single changes can pass through a team that breaks the rules (removing the Supervisor before naming the next).
- The first name is how people address someone: "@Pia, ask @Vale" is for Pia. One addressee keeps one message one turn.
- Ordering by when a turn started is Meldwork's answer to several streams in one chat (slice 2 plan); ordering by time interleaves two agents' blocks.
- Worktrees made at the agent's first turn, not when it joins, keep a slow or failing fetch out of the request that makes the chat, and the failure lands on a turn with a reference.
- A process keeps the environment it started with. Gathr runs for days in the tray, so a tool installed since is missing from its agents' PATH; that's what happened to the room in SerenityOS (G23). Windows' own Explorer reads the registry again for the same reason.

## Turned down

- **Every `@Name` gets a turn.** One message would be stored once per agent or split across turns; and a name mentioned in passing would start work nobody asked for.
- **Refusing a message that names several agents.** Mentioning a teammate is normal.
- **Order by when a turn was sent.** A queued turn's reply would appear above replies already shown.
- **A worktree when the agent joins.** Network work inside the API request, and a worktree for agents that never run.
- **`reg query`.** Its output's code page differs by computer, so a folder with an accented name could come back wrong.
- **PowerShell to read the variables.** About 200 ms per engine start against about 20 ms for `reg export`.
- **The project's own toolchain files** (`.node-version`, `rust-toolchain.toml`). rustup already reads `rust-toolchain.toml` once `cargo` is found; node version managers are left for when friction shows them.
