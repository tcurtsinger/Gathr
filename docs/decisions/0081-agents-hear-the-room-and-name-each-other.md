# 0081: Agents hear the room, and name each other with @Name

Status: accepted (owner, 2026-11-19: port how SerenityOS makes the conversation, whole, keeping Gathr's threads:
"Members hear the room when they wake; @Name in plain words wakes that agent"). Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/room.rs`, `crates/gathr/tests/mentions.rs`, `crates/gathr/tests/sessions.rs`.

Amends [0028](0028-team-chats.md) (agents address each other through tools, "never by parsed text": an agent's words
are read for `@Name` now; the person's still aren't) and [0055](0055-a-members-task-starts-in-a-new-session.md) (a
member's task still starts a new conversation, but no longer knowing nothing of the room). Builds on the `message`
tool and its hand-off limit (plans/agent-messages.md), [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md)
and [0078](0078-said-to-you-is-main-one-bubble-per-turn.md) (where a turn sits). Its sibling, the open check of the
lead's answer, is 0080, built at the same time. Serves promise 6 of [the charter](../charter.md#the-promises),
parts 6.4 and 6.5 of [the truth map](../truth-map.md), and touches 8.2 and 8.8.

## Why

In SerenityOS, the owner's earlier app, the agents read the room and answered each other in it, objecting in the
room and quoting the line they objected to. In Gathr a member's
task started knowing nothing of the chat, and an agent's words reached nobody: only the `message` tool did, a tool an
agent has to think of. The owner chose to port SerenityOS's way whole (2026-11-19). The code it is ported from,
`roomCatchUp` and `forMember` (`desktop/server/prompt-state.ts`, added after the Bloks import) is the owner's;
`addressees` (`desktop/server/bloks.ts`) is Bloks', read to understand and written anew.

## The choice

### An agent hears the room when its turn opens

- **What it hears.** In a chat of two or more agents, whatever opens the turn (the person's message, a task's brief,
  a hear-back, a message, a wait, a wake-up), what Gathr sends the engine starts with the chat's lines, each as
  `Name: words`: `Person` for the person, `Gathr` for Gathr's lines said in no turn, the agent's name for its words.
  Never a tool row, Gathr's copy of a message (a brief, a hear-back, what a `message` carried), the message that
  opened the turn, or what that message carries. It travels with the ask, never saved, as 0055's opening does; the
  briefing stays the same for the engine's life, so the provider's cache keeps it.
- **How much.** A turn that starts a new conversation (a member's task under 0055, the agent's first turn in the
  chat, a conversation started over after a refused resume, or one a compaction summed up) hears the last 30 lines
  under "Recent lines in this chat:", its own among them. Any other turn hears what came since its last turn there,
  up to the last 30, under "New in this chat since your last turn:", without its own lines and without what was
  carried into its turns: its conversation has those.
- **Since when** is read from what Gathr records, with no migration: the newest turn of the agent in the chat that
  started (it has a place), by when it was made. A line said between that turn being queued and it starting is heard
  again; a line is never missed. Since [0084](0084-a-tasks-turn-opens-with-its-questions.md), when the engine opens
  again on a task's conversation, it is the newest such turn in that conversation, and the agent's own lines said
  in another conversation since are heard too.
- **Whole or cut.** The lead hears every line whole: checking the work is its job. A member hears whole the person's
  lines, Gathr's, the lead's, its own and a line that names it with `@Name`; another member's line is cut to its
  first sentence, 160 characters at most, ending "…", and one line says "(Lines not for you are cut to their first
  sentence: read_chat has them whole.)". A whole line over 1,900 characters is cut in the middle as `read_chat` cuts
  one, naming the message to read for all of it (`gathr::text::kept`, moved there from the tool).
- **A member's task.** 0055 stays. Its opening now reads "This is a new conversation for task #N. Your earlier turns
  in this chat aren't in it, only its recent lines below: read_chat and list_tasks have the rest.", then its last
  tasks, then the lines, then the brief.
- **The briefing** says it in the team's line: "The chat's other agents: Ann, Bo. Each turn opens with what the chat
  said since your last one; read_chat has the rest." A chat of one, or one with no list, keeps "Their replies may be
  missing from your context."

### @Name in an agent's words wakes that member

- **When a turn ends done** in a chat of two or more agents, each member its text messages name with `@Name` is woken
  once with "<sender>: <words>", the words of the sender's messages that name it. Ignoring case, longest name first
  (each place found is blanked, so `@Bo` is never found inside `@Bobby`), a whole word only: `@Al` isn't in
  `@Alex`, and `bob@ann.com` names nobody (`gathr::team::mentioned`). Naming nobody wakes nobody, an agent never
  wakes itself, and a name that is no member's is text. Words sent with `message` aren't read: they reached their
  member already. A turn that failed or was stopped wakes nobody: its words may be cut off.
- **Delivered as a `message` is** (`turns/message.rs`, one `deliver` for both): written into the member's running
  turn in that chat, joined to its turn queued there (a hear-back the same turn's end queued, say: never two turns
  for one thing), or a new turn on no task answering the sender's, which a pause holds. Gathr's copy carries the
  first message that names the member (`copy_of`), so the window shows the words once and the woken turn sits where
  the sender's does, in that message's thread (0075's `copy` fact), as a reply to a `message` does.
- **A hand-off.** Each wake is a hand-off record as a `message` is (`handoffs::record`), counted in the same chain,
  with the same pause at the limit: the chat pauses, "Paused after N hand-offs between agents. Resume to carry on."
  goes in the sender's turn, and nobody from that member on is woken.
- **One function where a turn ends** (`mentions::wake`, called from `turn_ended`). A check of the lead's answer (0080)
  hands its words to the lead in the lead's last word, so naming the lead there wakes nobody
  (`handoffs::mentions`). It runs apart from the agent's own task: delivery waits on the member's task, which may be
  waiting on this one.
- **`message` stays,** for reaching a member before the turn ends. Its description and the briefing say both: "To
  reach a member of a team chat, write @Name in your words: they get them when your turn ends. message reaches one
  now, while you work. Work to be done and heard back on is a task."

## Diagnostics

- `gathr.room.heard` on each turn that heard something: `gathr.turn.id`, `lines`, `chars`, `cut` (lines cut to a
  sentence) and `fresh`. What hearing the room costs is read with `gathr diag query` on `event_log`. A failed read is
  the same event as an error `sqlite` with `why`; the turn goes without the lines.
- `gathr.message.mention` on the sender's turn's trace for each member woken: `from`, `to`, `message`, `depth` and
  `delivered` (`in_turn`, `joined`, `queued`, `held`, `held_app`, `held_agent`, `held_update`, `failed`, or `paused`
  at the limit, with `gathr.handoff.limit`). A wake that failed is an error `sqlite` with `why`, nobody woken, found
  with `gathr diag trace` from the turn's reference.

## Turned down

- **The whole chat in every turn:** each turn would re-read every other member's reports and reviews, which fill a
  context fastest; the cut to a sentence and the 30-line cap are SerenityOS's answer to that.
- **A record of the last line each agent heard** (SerenityOS keeps one per conversation): a migration, for a few
  lines heard twice.
- **A new kind of Gathr's words for the lines, with kept samples (0056):** the lines aren't what opens a turn but
  what goes with every opening, as 0055's opening does.
- **Waking on a failed or stopped turn's words, or reading `message` texts for names:** a cut-off turn may name
  someone mid-thought, and a `message` already reached its member.
- **The blue @Name in the window for an agent's own words** (0072 draws it for a `message`): the words are drawn by
  the registry's markdown, unchanged (0030); marking a name inside them is a change of its own, not made here.
- **A role line asking agents to name each other:** the role lines are 0066's and need a trial; the briefing's talk
  line says how.

## Not in it

- Not tried on real engines: whether agents name each other unasked, and whether they talk better for hearing the
  room. 0066 calls for a new trial when the briefing around its lines changes, and this changes it: not run.
- A task's own record at its turn's opening (part 8.2): this is the chat's lines. Its questions and answers came with
  [0084](0084-a-tasks-turn-opens-with-its-questions.md); its reviews and reports are not in it.
