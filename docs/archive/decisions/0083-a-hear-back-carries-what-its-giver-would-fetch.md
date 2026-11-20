# 0083: A hear-back carries what its giver would fetch

Status: accepted (owner, 2026-11-19: approved plan "Stage 4's rest: what a turn opens with", version 2, part 1).
Date: 2026-11-19.

Built: yes. Proof: `crates/gathr/tests/tasks/hear_backs.rs`, `crates/gathr/tests/words.rs`.

Amends [0051](0051-the-giver-hears-when-all-is-in.md) (a hear-back's words) and builds on
[0081](0081-agents-hear-the-room-and-name-each-other.md) (the chat's lines at a turn's opening). Serves promise 8 of
[the charter](../charter.md#the-promises), part 8.2 of [the truth map](../truth-map.md).

## Why

A hear-back's task line gave the task's outcome only: a merged PR's task read "done: #9 merged", without its PR, the
commit or file to look at, or what its owner said in its report. The giver then went to fetch them, and every
hear-back ended with "Read replies with read_chat, then continue." Since 0081, the chat's lines come with the turn,
and a line that came cut says itself that read_chat has it whole.

## The choice

- **A task's line** (`tasks/words.rs` `line`), in a hear-back and in Gathr's "A task is done:" line, is
  `- #<number> <owner>, <what>: <how>; <note> (PR #<pr>, look at <see>)`. The note is the owner's last report's,
  shown unless the reason is the one written from the report: the whole note, with a review's suggestions on the lines
  after it, or for a report of work left, "left: " and the note, with the fix Gathr gave after it. A merge or a hold
  writes its own reason instead, and one that only happens to begin with the note ("#19 merged", note "#19") still
  shows it. The PR is the task's, and the look-at is the report's commit or file. Each one shows only when the
  task has it.
- **No closing line** sends the giver to read_chat. A hear-back on the giver's own task still ends with "This turn is
  on your task #N: end it with finish_task."
- **The first lines stay the same**, so the window still hides both messages by their first line (`TOLD` in
  `ui/src/chat/blocks.ts`).
- These are the words of turns Gathr already records (the hear-back's words version, 4), so no event is added.

## Turned down

- **A list of the review's notes in the hear-back.** A review's notes reach its builder in the fix brief, and the
  lead hears the review's outcome. Repeating them would make each hear-back longer for every giver.
- **Dropping the PR when the outcome already names it** ("#9 merged"). The line would then name the PR only
  sometimes. One extra "PR #9" is cheaper than a reader checking which case applies.
