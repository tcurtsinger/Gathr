# 0080: The team checks the lead's answer in the open

Status: accepted (owner, 2026-11-19: "after the lead's plan or answer every member who took no part checks it and
replies under that message (a silent thumbs-up if it holds), and the lead answers there; SerenityOS's lines on
disagreeing, quoting and plain talk; each message drawn like SerenityOS's card inside Gathr's threads. Gathr's
one-checker notes form for plans goes."). Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/plans/check.rs`, `crates/gathr/tests/plan_flow.rs`,
`crates/gathr/tests/topics.rs`, `crates/gathr/tests/roles.rs`, `ui/src/chat/checked.test.tsx`.

Builds on [0057](0057-agreeing-is-a-reaction.md) (agreeing is a thumbs-up), [0075](0075-a-team-chat-is-main-messages-each-with-its-thread.md)
and [0078](0078-said-to-you-is-main-one-bubble-per-turn.md) (where a turn sits). Ports SerenityOS's room checks
(`desktop/server/room-checks.ts`, added 2026-10-20, the owner's own; `roomSettle` and `checkNext` in `index.ts`).
Amends 0062 and 0066, below. Serves promise 2 of [the charter](../charter.md#the-promises), parts 2.3 and 2.5 of
[the truth map](../truth-map.md), and part 11.1 for its role lines.

## The choice

- **When.** Once per message of the person's (theirs, or the words they sent a plan back with): when the chat's lead
  has answered it, its newest turn on that ask ended well, and nothing of its ask runs, is queued or is owed (no task
  of the ask open, no question to the person open). A lead turn that wrote a plan version is such an answer. Not in
  a chat of one agent. The service looks after every write to the chat and at every look
  (`gathr::open_check::settle`, from `coord/quiet.rs`).
- **The answer** is the newest plan version the lead's turn wrote, else its last words. The lead's turns opened by a
  wake, a silent turn, stale tasks, the engine, a compaction, a check or a last word answer nothing (0078's rule).
- **Who.** Every member of the chat but the lead (the Reviewer too) who is free (no turn of its own queued or running
  there) and took no part (no turn of its own in the chat since the person wrote). With nobody to check, the round
  ends at once.
- **The check is a turn, not a task:** opened by Gathr's words of a new kind, `check`, answering the lead's turn. Its
  words, adapted from SerenityOS's `checkText`: check the lead's answer before the person relies on it, don't redo
  it, check what was built by running it; if it holds up, give the message a thumbs-up with `react` and write
  nothing; else say what in a few lines with evidence, quoting the line answered as `> Name: their words`; nothing
  to check, write nothing. A plan's form keeps what 0062's check brief looked for (a wrong fact or file, a missed
  step or caller, a part given to the wrong member, a proof that proves nothing, what the ask doesn't need) and
  carries the person's ask and the plan's body. `react` on the answer answers "That is your whole check: end your
  turn now and write nothing more."
- **The lead's last word.** Once every check turn has ended, a member who wrote words and gave no thumbs-up questioned
  the answer. With one or more, the lead is woken once, `last_word`, with each one's words as `Name: words` and,
  adapted from SerenityOS's settle text: this is your last word; correct the answer if they're right, or say plainly
  why it stands; a few lines, to the person. For a plan: write the next version with `plan` to change it. No second
  round: a version the last word writes is checked by nobody.
- **The plan gate.** A version reaches the person (Approve, Send back, Drop) when its round has ended and, if any
  member questioned it, the last word has ended; the newest undecided version of the ask's plan is the one sent. The
  10-minute cap stays: a version unsent 10 minutes after it was written reaches the person with "<names> didn't
  finish checking version n of the plan "<title>"", naming whoever's check or last word still runs, or "The team".
  The chats with a round under way or a version unsent are looked at, so the cap needs no write.
- **Where it shows.** A check turn and the last word sit in the thread of the answer (`topics.rs`, facts `check` and
  `last_word`), after an agent's own choice with `post` and after a version the turn wrote (a version is its own main
  message, 0078). History and the live `turn` event carry each turn's `opened` kind. The window draws a check's bubble
  with "Checked {Name}'s answer" and a check-double icon in a small grey line first, the quote and words after it
  as Markdown. A check with no words draws no bubble, only its thumbs-up on the answer. Gathr's words that opened a
  check or a last word draw nowhere.
- **The record.** One event per round, `gathr.check.round`, when it ends: `gathr.check.asked`, `agreed`, `objected`,
  `silent`, `last_word`, `gathr.plan.sent`, the answer's message and turn. `gathr diag words` counts `check` and
  `last_word` turns.
- **Role lines,** untried (the owner, 2026-11-19: the room's next run is the trial): L5 for everyone in a team (quote
  the line you answer as `> Name: their words`; speak only as yourself, in a few lines: a conversation, not a
  report), L6 for a member and a reviewer (argue your case once, in words, then follow the call), L7 for the lead
  (members who took no part check your answer and you have the last word; make the call when members disagree and
  give the reason; don't pass on what members said). L3 stays the agreeing line. `roles::UNTRIED` names them;
  0066's gate still holds L2 and L3 to the trial, and `every_role_line_is_kept_by_the_trial_or_named_untried` fails
  for a line that is neither. `words::ROLE` is 2.
- **The schema** (migration `0060_open_checks.sql`): `open_checks`, one row per message of the person's: the answer,
  its turn, when the round began and ended, and the last word. Its check turns are the `check` turns answering the
  answer's turn. The migration counts every message sent before it as had, so an upgrade checks nothing old.
- **With 0081** (agents hear the room, `@Name` wakes): a round's turn hears nothing twice. A check's words carry the
  answer and the person's message, and the last word's carry the members' checks, so the chat's lines a turn opens
  with leave those out (`room.rs`); and a check that names the lead with `@Name` leaves it to the last word
  (`handoffs::mentions`). Proof: `tests/plans/check.rs::a_rounds_turns_hear_nothing_twice` and
  `::a_check_that_names_the_lead_wakes_it_once_by_its_last_word`.

## What goes

The single checker Gathr picks, the check as a review task, the `plan_check` words, a plan's notes filed with
`finish_task`, the lead's `answers` (the `plan` tool takes none), Gathr's lines "X agrees with version n" and "X has k
findings", and the thumbs-up that finished a check task. A code review of a pull request is untouched.

## Why

- The owner: SerenityOS's agents "were checking each others plans" and argued when things were wrong; Gathr's one
  checker and its notes form talked to nobody. In the SerenityOS chat the owner pointed at, 37 of 159 member turns
  were a silent nod and 24 were objections quoting the line.
- A turn, not a task: a check settles nothing and is owed nothing, so it needs no report, hear-back or state.
- Thumbs-up wins over words: engines close a turn with a sentence even after agreeing (0066's T8), and that sentence
  mustn't wake the lead.

## Turned down

- **A second round** after the last word (SerenityOS has none either).
- **Checking every answer of the lead's,** not only the newest per message: each would cost a turn per member.
- **Keeping the one checker for plans beside it,** or the notes form.
- **A setting to turn it off.**
- **Gathr's own line for how the round went:** the thumbs-ups and the members' words say it.

## Different from the brief

- **No round for an answer that ended failed or stopped, or that asked the person a question** (an open person task
  of the ask): SerenityOS's `checkDue` skips a failed or stopped round and an answer that left a card waiting on the
  person.
- **A last word is skipped** when the person has written again since, or decided the plan (approved or dropped): it
  would answer nothing they wait on. SerenityOS drops its check when the person writes again.
- **Gathr's words that opened the lead's last word aren't drawn either:** the members' words are right above it in
  the thread, and the quote would repeat them.
- **The 10-minute line names who is still checking,** sorted by name, or "The team" before any check has begun.
- **Service tests of other parts run with nobody to check.** Every team answer now starts turns, which 36 existing
  service tests (compaction, sessions, pauses, questions and more) didn't expect. The test harness starts `gathrd` with
  `GATHR_OPEN_CHECK=off`, read only in a debug build as `GATHR_TRIAL_WITHOUT` is: rounds still happen, end at once
  and send plans, with no check turns. The open check's own service tests set it `on`; the library's tests call the
  round directly. A release build never reads it, and no person can turn the check off.
- **The migration writes rows** (one per old message of the person's, into the new table only) so an upgrade starts no
  round for an old message: no old row is rewritten.

## Said plainly

- **Every team answer costs a turn per free member,** small talk included (the words tell a member to write nothing
  then). The event counts it: `gathr diag query "SELECT count(*) AS rounds, sum(attrs ->> '$.\"gathr.check.asked\"')
  AS turns, sum(attrs ->> '$.\"gathr.check.objected\"') AS objected FROM event_log WHERE name = 'gathr.check.round'"
  --data <folder>`.
- **What a member hears of the chat** when its check opens is Builder B's part (0081); the check's words carry the
  person's message and the answer, so it works without it.
- **A check's words show in its bubble even after a thumbs-up,** though they wake nobody.
- **Old chats** keep their check tasks, findings chip and lines; the tasks' rules for an old check still in flight at
  the upgrade stay (its cancel on a new version or the person's word, no hear-back, no fix), with no new brief.
- **Not verified:** how real Claude Code and Codex follow the check's words (thumbs-up only, quoting, writing
  nothing), the three new role lines, and the cost per round on real engines; the window was looked at only in the
  pictures made for the PR.

## Amends

- [0062](0062-a-plan-is-checked-and-approve-hands-it-out.md): H4 and H4a (the check task, its checker, its notes,
  the lead's answers, Gathr's check lines) are replaced by this round; the 10 minutes and Approve's wait for the check
  stay.
- [0066](0066-role-text-tried-on-real-engines.md): three role lines ship without a trial, named in `roles::UNTRIED`;
  the gate holds the tried ones.
