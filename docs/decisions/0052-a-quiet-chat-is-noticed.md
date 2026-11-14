# 0052: A quiet chat is noticed, and its dead time has a number

Status: accepted (owner, 2026-11-13, with Stage 2's spec; the 5 minutes and "only when new" are the owner's answers). Date: 2026-11-13.

Built: yes. Proof: `crates/gathr/tests/quiet.rs`, `crates/gathr/tests/quiet_room.rs`, `crates/gathr/tests/waiting.rs`, `crates/gathr/tests/codex_recovery.rs`, `ui/src/chat/Waiting.test.tsx`.

Builds on the answer of [0050](0050-what-a-chat-waits-on.md). Serves promise 6 of [the charter](../charter.md#the-promises), the room explains itself (part 6.3 of [the truth map](../truth-map.md)), and promise 10's "nobody waits on nothing" (part 10.4).

## The choice

- **One rule for the chat, read from the one answer.** A chat is stuck when all of these hold: it has open
  work, which is an agent's task that is neither done nor cancelled (blocked, failed and stopped work counts);
  it isn't archived; and in 0050's answer no line moves by itself and no line waits on you. Nothing is kept or
  decided line by line. A chat that waits on you says so, and isn't stuck.
- **Two things read differently in the answer from now on.** A running turn moves by itself until it has made
  no progress for ten minutes, twenty while a tool is open. Then it is silent: a line nobody is on, with since
  when. A task whose owner is paused, by the agent's pause, the chat's or the app's, waits on you.
- **A stretch** is one spell of being stuck: from the first check that finds the chat stuck to the first that
  doesn't, or to your next message in the chat.
- **A count** is the stuck time that leads to a telling. It starts when you write in the chat, and again when
  a stretch you were told about ends. It adds up across the stretches in between.
- **Only what is new is said again.** Each thing that isn't moving has a key, made from its line's fields and
  never from its words: its kind, its task, what it waits for, its agent and since when. The same thing keeps
  its key from check to check, and trouble keeps it when its giver hears of it. A task that is restarted and
  fails again has a new key, by its time. Gathr keeps the keys of what you were last told of, and of what the
  lead was last woken for. They outlast a count: your message doesn't make an old thing new.
- **First Gathr wakes the lead,** with a turn on no task. It goes through the queue as any turn does, so a
  pause holds it. It says what isn't moving and for how long, what is new first, that nothing waits on the
  person, and what the lead can do: give, restart or cancel work, or ask the person. It goes only when there
  is news for the lead that wasn't in its last wake. News is something nobody is on, other than trouble its
  giver has already heard and other than another agent's silent turn; or trouble its giver hasn't heard, with
  no hear-back coming. It doesn't go while the lead's last wake is still queued or running, or while the
  lead's own turn is the silent one. A chat with no lead has no such step.
- **Then Gathr tells you,** once the chat has been stuck 5 minutes in all in this count, and only when
  something that isn't moving wasn't in the last telling. It is a line in the chat, said by Gathr in no turn,
  that names what isn't moving and for how long, what is new first: "Not moving for 6 min: #4 (Cass) and 1
  more. Write here to move it on." The chat then needs you, in the chat list and its dot
  ([0040](0040-chat-state-in-the-list.md)), until that stretch ends. Stuck time adds up across stretches, so
  work that is restarted and fails again and again still reaches you.
- **The clock counts only time Gathr saw.** At each check of a stuck chat, the time since the check before is
  added. A gap longer than two looks adds nothing: the service was down, or the computer asleep. While the
  lead's wake runs and makes progress, the clock is stopped. Once you are told, no more time is added to the
  count. None is added either while everything that isn't moving is something you were told of: the count
  then holds no time, and a new thing gets its own 5 minutes from the check that first sees it.
- **When it is checked:** after every write that changes a chat's turns, tasks or waits, with a pause, a resume
  and your own message among them; and at every look the service makes (every 2 minutes), for each chat with
  open work or an open stretch.
- **One row for the chat outlasts a restart** (`quiet`, migration 0039): since when the stretch runs, the stuck
  time counted in it, the stuck time of the count, the lead's wake, whether you were told, and the keys of what
  you were last told of and of what the lead was last woken for. After a restart nobody is woken or told a
  second time.
- **Dead time is an event.** Each stretch, when it ends, is recorded as `gathr.quiet.stretch`: its length (the
  stuck time counted in it), since when, what wasn't moving at the last check, whether the lead was woken and
  you were told, and whether it ended because something moved or because you wrote. A stretch still open is the
  chat's row in `quiet`. These are what the `diag stalls` of [0019](0019-troubleshooting.md) will read; until
  it is built, `gathr diag query` reads both.
- **A silent turn is shown, and its lead is told once.** The line at the foot of the chat reads "Working: Cass
  silent 12m" and opens the list, where the turn sits under Not moving. The turn's lead gets one turn for it:
  who has made no progress and for how long, that the turn still runs, and that a message to that agent reaches
  it (`gathr.turn.silent_told`, beside `gathr.turn.stalled` on the turn's reference). The turn is never
  stopped. A lead's own silent turn tells nobody: the rule above covers it.
- **Stale tasks stay as they are.** A task nothing moved for days is marked, and cancelled days later. That is
  clearing up, not noticing, and it can follow this notice by days.

## Why

- In the room's first three days the owner noticed seven stalls before Gathr did. Five were a PR that looked
  ready and sat unmerged, which Gathr now lands ([0049](0049-gathr-lands-pull-requests.md)). For the rest, and
  for the next kind nobody has met yet, Gathr needs a rule that names no cause.
- 0050's answer already says of every line who can end it. A chat where nothing moves by itself and nothing
  waits on you is a chat nobody will move. No second source is needed to know it.
- A turn silent for ten minutes was an event that no screen showed and no agent was told.
- Dead time, the time with work owed and nothing running, was an estimate made from outside Gathr, and only a
  lower bound. Promise 10 asks for it measured.
- The lead comes first because it can usually move the work without you. You come second, at 5 minutes, because
  a lead that can't move it shouldn't keep you waiting unseen.

## Turned down

- **Noticing part by part, as SerenityOS does.** There each stuck part is found by itself, its owner is nudged
  once, and then the person is told. That is a second state to keep in step with tasks, turns and waits, and
  the owner of a stuck part can't always move it. The answer says who can end each line, and the lead can act on
  all of it.
- **A setting for the time.** 5 minutes is the owner's answer, and nobody has needed another. A value that
  doesn't change gets no control. `GATHR_QUIET_TELL_MS` shortens it in dev and tests only.
- **A clock handed into the service,** so tests could move time. The rule is one function that takes the time
  and what the running service knows as arguments, so its tests pass both in. The service calls it with the
  real clock, and no other code learns of a second clock.
- **Reading the flag Gathr raises once a turn** to know a turn is silent. It is raised once and never lowered,
  so a turn that made progress again would read as silent to its end. Silent is the time of the turn's last
  progress, said by its agent's queue while the turn is past the limit and taken back when it moves.
- **A wake in every stretch.** A lead that answers its wake by writing to a member ends the stretch, since a
  turn then runs. If that turn changes nothing, the chat is stuck again, in a new stretch, and the lead would
  be woken again, without end, while the 5 minutes never add up. A wake only for news that wasn't in the last
  one stops that.
- **Once in a count,** as this was first built: one wake and one telling in each count. Every message you
  wrote started a new count, so a chat with one task stuck for days woke its lead and told you again, about
  the same list, after every conversation.
- **Waking only at the second check that finds the chat stuck,** to be safe against state saved in two steps.
  It would delay every wake by up to a look. The one place found that saved "not moving" and what follows it
  in two steps, a PR left to its reviewer, now saves both in one.

## Different from the spec

Stage 2's spec for this part, and what was built instead. Each is the smaller shape, or the one that is right
against the code.

1. **When it is checked.** The spec named three moments: a turn ends, tasks change, and every look. The first
   two are one: Gathr already hears every write that changes a chat's turns, tasks or waits. That now also
   hears a pause, a resume and a message of yours into a running turn, none of which it heard before. The look
   is the other.
2. **What silent is read from.** The spec said the flag Gathr raises today. That flag is set once and never
   cleared, so it is kept only for "told once". Silent is the turn's last progress, as under Turned down.
3. **A pause.** As the spec says, a task whose owner is paused waits on you. This changes what 0050's answer
   said for such a task. The spec's scenario "a chat with one agent paused" is read as: that agent has a task
   or a turn in the chat. An agent paused with nothing in the chat doesn't keep the chat from being noticed.
   The spec doesn't say which it meant.
4. **The lead is woken only for what is new,** not once in a stretch: when there is news that wasn't in its
   last wake, and that wake is over. The owner chose this ("only when new"). Once in a stretch would repeat
   without end, as under Turned down.
5. **You are told only of what is new.** The spec told you once in each count, and each message you wrote
   started a new one, so you were told again about the same list. The owner chose this instead: you are told
   at 5 minutes of stuck time, and only when something that isn't moving wasn't in the last telling. Told, you
   aren't told again in that stretch, and no time is added. The next count starts when that stretch ends.
6. **Who isn't woken.** The lead isn't woken for another agent's silent turn: it already has that turn's own
   telling. It isn't woken when its own turn is the silent one: the wake would only queue behind it. Trouble
   its giver hasn't heard, with no hear-back coming, does wake it: a task whose turn was cut off when the
   service stopped is left stopped that way.
7. **When the clock stops.** The spec stopped it while the lead's wake runs. It stops only while that turn
   runs and makes progress: not while it is queued behind the lead's other work, and not while it hangs. A
   wake that waits on you (a sign-in, an approval, a pause) ends the stretch, because the chat then waits on
   you.
8. **Time the service was down.** One rule: a gap longer than two looks adds nothing. It covers the service
   being down and the computer being asleep. It also drops one real interval when a look takes over four
   minutes.
9. **A queued CI job** gets no rule of its own. A wait on CI already moves by itself, however long the job has
   queued. No age is kept for each job.
10. **A stretch's length** is the stuck time counted in it, not the time on the wall from its start to its end.
    Added: what ended it, so "the person stepped in" can be told from "it moved".
11. **A turn that waits for an approval isn't silent.** It waits on you, and the answer says so. This changes
    `gathr.turn.stalled`, which was recorded for such turns too. Answering the approval counts as progress.
    The live message `stalled`, which nothing in the window read, is gone.
12. **The line at the foot** keeps the order it was built with. Ages read "12m" as everywhere in the window,
    where the spec wrote "12 min". The silent line takes the place of Working, so that it can open the list.
13. **A PR left to its reviewer** (0049) is saved together with its reviewer's wake, in one step. In two, a
    check between them saw a PR nobody was on and would wake the lead for nothing.
14. **Not built, and not asked for:** a limit on a wait that never comes true, and noticing one idle task in a
    chat where other work moves.

## Changes

- [0050](0050-what-a-chat-waits-on.md): its answer gains a silent turn and a paused owner's task, and it is
  built whole.
- [0040](0040-chat-state-in-the-list.md): a chat told it isn't moving needs you, a fourth cause.
- [0049](0049-gathr-lands-pull-requests.md): a PR left to its reviewer and the reviewer's wake are one write.
- The live message `stalled` is gone. `waiting` is also sent on a pause, a resume, your message, and a turn
  going silent or moving again.
- [The scenarios](../scenarios.md): 6c and 6d are reworded and 6h is new. "How long a quiet room is left" is
  settled: 5 minutes.
